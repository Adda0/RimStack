//! Pictures copied at the size a row shows them, kept together in one file.

use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Raised with any change to the file's layout; a file of another version reads as empty.
pub const VERSION: u32 = 1;
const MAGIC: &[u8; 4] = b"RSPV";

/// The file a copy was made from; a change of its size or time makes the copy stale.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Original {
    pub path: PathBuf,
    pub bytes: u64,
    /// Nanoseconds since 1970.
    pub modified: u64,
}

impl Original {
    pub fn of(path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        let since = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        Some(Self {
            path: path.to_path_buf(),
            bytes: meta.len(),
            modified: u64::try_from(since.as_nanos()).unwrap_or(u64::MAX),
        })
    }
}

/// A PNG at exactly `wide`×`high`: cut to that shape around its middle, as Qt's
/// `PreserveAspectCrop` cuts it, then scaled. Rows run top down; each pixel is a premultiplied
/// 0xAARRGGBB in this machine's byte order, QImage's `Format_ARGB32_Premultiplied`.
pub fn copy(png: &[u8], wide: u32, high: u32) -> Option<Vec<u8>> {
    if wide == 0 || high == 0 {
        return None;
    }
    let picture = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let picture = picture.to_rgba8();
    let (w, h) = (u64::from(picture.width()), u64::from(picture.height()));
    let (to_w, to_h) = (u64::from(wide), u64::from(high));
    let (cut_w, cut_h) = if w * to_h > h * to_w {
        (((h * to_w + to_h / 2) / to_h).max(1), h)
    } else {
        (w, ((w * to_h + to_w / 2) / to_w).max(1))
    };
    // Each cut side is at most the picture's own, which is a u32.
    let (x, y) = (((w - cut_w) / 2) as u32, ((h - cut_h) / 2) as u32);
    let cut = image::imageops::crop_imm(&picture, x, y, cut_w as u32, cut_h as u32).to_image();
    let filter = image::imageops::FilterType::Triangle;
    let scaled = image::imageops::resize(&cut, wide, high, filter);
    Some(scaled.pixels().flat_map(|p| premultiplied(p.0)).collect())
}

fn premultiplied([r, g, b, a]: [u8; 4]) -> [u8; 4] {
    let times = |c: u8| u32::from((u16::from(c) * u16::from(a) + 127) / 255);
    (u32::from(a) << 24 | times(r) << 16 | times(g) << 8 | times(b)).to_ne_bytes()
}

/// The copies of one size. The file holds a header (magic, version, size, count), an index of
/// originals each with the offset of its pixels, then the pixels; numbers are little-endian.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pack {
    pub wide: u32,
    pub high: u32,
    pub entries: Vec<(Original, usize)>,
    pub pixels: Vec<u8>,
}

impl Pack {
    fn frame(&self) -> usize {
        self.wide as usize * self.high as usize * 4
    }

    /// Roughly what the pack is on disk: pixels, then the index beside them.
    pub fn bytes_len(&self) -> usize {
        self.pixels.len()
            + self.entries.len()
                * (28
                    + self
                        .entries
                        .iter()
                        .map(|(o, _)| o.path.to_string_lossy().len())
                        .sum::<usize>()
                        / self.entries.len().max(1))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn originals(&self) -> impl Iterator<Item = &Original> {
        self.entries.iter().map(|(original, _)| original)
    }

    /// The pixels of the `at`th original, as [`copy`] makes them.
    pub fn pixels(&self, at: usize) -> Option<&[u8]> {
        let (_, offset) = self.entries.get(at)?;
        self.pixels.get(*offset..offset + self.frame())
    }

    /// What has no copy of this size made from the file as it is now.
    pub fn stale<'a>(&self, wanted: &'a [Original], wide: u32, high: u32) -> Vec<&'a Original> {
        if (self.wide, self.high) != (wide, high) {
            return wanted.iter().collect();
        }
        let held: HashMap<&Original, ()> = self.originals().map(|o| (o, ())).collect();
        wanted.iter().filter(|o| !held.contains_key(o)).collect()
    }

    /// Only the wanted, in their order: the copies still fresh are kept, the rest made from
    /// their files, on every core. A file that is no PNG is left out.
    pub fn renewed(&self, wanted: &[Original], wide: u32, high: u32) -> Pack {
        let same_size = (self.wide, self.high) == (wide, high);
        let kept: HashMap<&Original, usize> = match same_size {
            true => self
                .originals()
                .enumerate()
                .map(|(at, o)| (o, at))
                .collect(),
            false => HashMap::new(),
        };
        let made: Vec<Option<Vec<u8>>> = wanted
            .par_iter()
            .map(|original| match kept.get(original) {
                Some(at) => self.pixels(*at).map(<[u8]>::to_vec),
                None => copy(&std::fs::read(&original.path).ok()?, wide, high),
            })
            .collect();
        let mut pack = Pack {
            wide,
            high,
            ..Pack::default()
        };
        for (original, pixels) in wanted.iter().zip(made) {
            if let Some(pixels) = pixels.filter(|p| p.len() == pack.frame()) {
                pack.entries.push((original.clone(), pack.pixels.len()));
                pack.pixels.extend_from_slice(&pixels);
            }
        }
        pack
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.pixels.len() + self.entries.len() * 128);
        bytes.extend_from_slice(MAGIC);
        for n in [VERSION, self.wide, self.high, self.entries.len() as u32] {
            bytes.extend_from_slice(&n.to_le_bytes());
        }
        for (original, offset) in &self.entries {
            let path = original.path.to_string_lossy();
            bytes.extend_from_slice(&(path.len() as u32).to_le_bytes());
            bytes.extend_from_slice(path.as_bytes());
            for n in [original.bytes, original.modified, *offset as u64] {
                bytes.extend_from_slice(&n.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&self.pixels);
        bytes
    }

    /// Anything amiss, a file cut short or of another version, reads as an empty pack.
    pub fn read(bytes: &[u8]) -> Pack {
        Self::parsed(bytes).unwrap_or_default()
    }

    fn parsed(bytes: &[u8]) -> Option<Pack> {
        let mut at = Reader(bytes);
        if at.take(4)? != MAGIC || at.u32()? != VERSION {
            return None;
        }
        let (wide, high, count) = (at.u32()?, at.u32()?, at.u32()? as usize);
        // Each entry takes at least 28 bytes: a count past them is no pack.
        if count > at.0.len() / 28 {
            return None;
        }
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let length = at.u32()? as usize;
            let path = PathBuf::from(std::str::from_utf8(at.take(length)?).ok()?);
            let (bytes, modified) = (at.u64()?, at.u64()?);
            let offset = usize::try_from(at.u64()?).ok()?;
            entries.push((
                Original {
                    path,
                    bytes,
                    modified,
                },
                offset,
            ));
        }
        let pack = Pack {
            wide,
            high,
            entries,
            pixels: at.0.to_vec(),
        };
        let frame = (wide as usize).checked_mul(high as usize)?.checked_mul(4)?;
        let fits = |offset: usize| {
            offset
                .checked_add(frame)
                .is_some_and(|end| end <= pack.pixels.len())
        };
        pack.entries
            .iter()
            .all(|(_, offset)| fits(*offset))
            .then_some(pack)
    }
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if n > self.0.len() {
            return None;
        }
        let (taken, rest) = self.0.split_at(n);
        self.0 = rest;
        Some(taken)
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(wide: u32, high: u32, colour: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let picture = image::RgbaImage::from_fn(wide, high, |x, y| image::Rgba(colour(x, y)));
        let mut bytes = std::io::Cursor::new(Vec::new());
        picture
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    fn argb(pixels: &[u8]) -> Vec<u32> {
        let (words, _) = pixels.as_chunks::<4>();
        words.iter().map(|p| u32::from_ne_bytes(*p)).collect()
    }

    const RED: [u8; 4] = [255, 0, 0, 255];
    const GREEN: [u8; 4] = [0, 255, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    fn bands(at: u32) -> [u8; 4] {
        match at {
            0..10 => RED,
            10..30 => GREEN,
            _ => BLUE,
        }
    }

    #[test]
    fn a_copy_is_exactly_the_size_asked_and_keeps_the_middle_as_qt_crops() {
        let wide = copy(&png(40, 10, |x, _| bands(x)), 4, 2).unwrap();
        assert_eq!(wide.len(), 4 * 2 * 4);
        assert_eq!(argb(&wide), vec![0xff00_ff00; 8]);
        let tall = copy(&png(10, 40, |_, y| bands(y)), 2, 4).unwrap();
        assert_eq!(argb(&tall), vec![0xff00_ff00; 8]);
        let one_pixel = copy(&png(1, 1, |_, _| BLUE), 96, 60).unwrap();
        assert_eq!(argb(&one_pixel), vec![0xff00_00ff; 96 * 60]);
    }

    #[test]
    fn a_copy_is_premultiplied() {
        let half = copy(&png(2, 2, |_, _| [255, 255, 255, 128]), 2, 2).unwrap();
        assert_eq!(argb(&half), vec![0x8080_8080; 4]);
        let clear = copy(&png(2, 2, |_, _| [200, 100, 50, 0]), 2, 2).unwrap();
        assert_eq!(argb(&clear), vec![0; 4]);
    }

    #[test]
    #[ignore = "a timing for measuring by hand, no number is a promise"]
    fn a_copy_made_and_read_have_the_costs_the_issue_reports() {
        // The table of issue #43 takes the median original, 636×400 at 261 KB.
        let png = png(636, 400, |x, y| {
            [(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255]
        });
        let started = std::time::Instant::now();
        let mut copies = 0;
        for _ in 0..50 {
            copies += copy(&png, 90, 56).unwrap().len();
        }
        let made = started.elapsed().as_secs_f64() * 1000.0 / 50.0;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.png"), &png).unwrap();
        let wanted = vec![Original::of(&dir.path().join("a.png")).unwrap()];
        let pack = Pack::default().renewed(&wanted, 90, 56);
        let bytes = pack.to_bytes();
        let read = std::time::Instant::now();
        let mut rounds = 0;
        for _ in 0..100 {
            rounds += Pack::read(&bytes).pixels(0).unwrap().len();
        }
        let took = read.elapsed().as_secs_f64() * 1e6 / 100.0;
        let _ = (copies, rounds);
        println!("copy_ms={made:.3} pack_read_us={took:.2}");
    }

    #[test]
    fn what_is_no_png_gives_no_copy() {
        assert_eq!(copy(b"\xff\xd8\xff\xe0 a JPEG", 4, 2), None);
        assert_eq!(copy(&png(4, 4, |_, _| RED), 0, 2), None);
    }

    fn originals(dir: &Path) -> Vec<Original> {
        let files = [("a.png", RED), ("b.png", GREEN), ("c.jpg", BLUE)];
        for (name, colour) in files {
            std::fs::write(dir.join(name), png(8, 4, |_, _| colour)).unwrap();
        }
        std::fs::write(dir.join("c.jpg"), b"not a png").unwrap();
        let names = files.iter().map(|(name, _)| dir.join(name));
        names.map(|path| Original::of(&path).unwrap()).collect()
    }

    #[test]
    fn a_pack_reads_back_what_was_written() {
        let dir = tempfile::tempdir().unwrap();
        let wanted = originals(dir.path());
        let pack = Pack::default().renewed(&wanted, 4, 2);
        assert_eq!(
            pack.originals().collect::<Vec<_>>(),
            [&wanted[0], &wanted[1]]
        );
        assert_eq!(argb(pack.pixels(1).unwrap()), vec![0xff00_ff00; 8]);
        assert_eq!(pack.pixels(2), None);
        let read = Pack::read(&pack.to_bytes());
        assert_eq!(read, pack);
        assert_eq!((read.wide, read.high), (4, 2));
    }

    #[test]
    fn a_changed_file_size_or_version_makes_copies_stale() {
        let dir = tempfile::tempdir().unwrap();
        let mut wanted = originals(dir.path());
        wanted.pop();
        let pack = Pack::default().renewed(&wanted, 4, 2);
        assert!(pack.stale(&wanted, 4, 2).is_empty());
        let mut touched = wanted.clone();
        touched[0].modified += 1;
        touched[1].bytes += 1;
        assert_eq!(pack.stale(&touched, 4, 2), [&touched[0], &touched[1]]);
        assert_eq!(pack.stale(&wanted, 6, 3).len(), 2);
        let mut older = pack.to_bytes();
        older[4..8].copy_from_slice(&(VERSION + 1).to_le_bytes());
        assert_eq!(Pack::read(&older).stale(&wanted, 4, 2).len(), 2);
        let resized = pack.renewed(&wanted, 6, 3);
        assert_eq!(
            (resized.len(), resized.pixels(0).unwrap().len()),
            (2, 6 * 3 * 4)
        );
    }

    #[test]
    fn a_pack_cut_short_or_spoilt_reads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = Pack::default()
            .renewed(&originals(dir.path()), 4, 2)
            .to_bytes();
        for end in 0..bytes.len() {
            assert!(Pack::read(&bytes[..end]).is_empty(), "cut at {end}");
        }
        let mut huge_count = bytes.clone();
        huge_count[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Pack::read(&huge_count).is_empty());
        let mut huge_size = bytes.clone();
        huge_size[8..16].copy_from_slice(&[0xff; 8]);
        assert!(Pack::read(&huge_size).is_empty());
        assert!(Pack::read(b"RSPV\x01\0\0\0 garbage").is_empty());
    }
}
