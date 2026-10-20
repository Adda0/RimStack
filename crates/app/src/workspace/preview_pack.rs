//! Where the copies' pack is kept and made from `jobs`; `crate::previews` serves it to QML.

use crate::App;
use rimstack_core::base::thumbnails::{Original, Pack};
use std::path::PathBuf;

/// `cache/previews.pack` in the app's own folder.
fn pack_file(app: &App) -> PathBuf {
    app.data_dir().join("cache").join("previews.pack")
}

/// Every PNG preview of the mods installed; a JPEG one keeps the row's plain reading.
pub fn wanted(app: &App) -> Vec<Original> {
    let loaded = app.library();
    loaded
        .as_ref()
        .as_ref()
        .into_iter()
        .flat_map(|(library, _)| library.installed.iter())
        .filter_map(|installed| installed.preview.as_ref())
        .filter(|path| {
            path.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("png"))
        })
        .filter_map(|path| Original::of(path))
        .collect()
}

/// Makes the copies the pack shown misses, on every core, writes the file anew and has the
/// rows given it; `None` when it already holds every wanted copy of this size. The first run
/// builds; later runs with the file intact need nothing made again, only read back.
pub fn renew(app: &App, wanted: &[Original], wide: u32, high: u32) -> Option<()> {
    if wide == 0 || high == 0 || wanted.is_empty() {
        return None;
    }
    let shown = crate::previews::shown();
    let missing = shown.stale(wanted, wide, high);
    if missing.is_empty() {
        return None;
    }
    let started = std::time::Instant::now();
    // A file made from the wanted as they are now needs nothing made again, only to be shown.
    let from_disk = Pack::read(&std::fs::read(pack_file(app)).unwrap_or_default());
    let pack = match from_disk.stale(wanted, wide, high).is_empty() {
        true => from_disk,
        false => {
            let pack = shown.renewed(wanted, wide, high);
            // An original that could not be read stays out, so it is tried again next time.
            if pack.len() == wanted.len() {
                let file = pack_file(app);
                let wrote = rimstack_core::base::atomic_file::write(&file, pack.to_bytes());
                if let Err(problem) = wrote {
                    rimstack_core::problem!(
                        "the preview pack is not written to {}: {problem}",
                        file.display()
                    );
                }
            }
            pack
        }
    };
    rimstack_core::note!(
        "previews_made={} kept={} pack_bytes={} ms={}",
        missing.len(),
        wanted.len() - missing.len(),
        pack.bytes_len(),
        started.elapsed().as_millis()
    );
    crate::previews::place(pack);
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(colour: [u8; 4], w: u32, h: u32) -> Vec<u8> {
        let picture = image::RgbaImage::from_fn(w, h, |_, _| image::Rgba(colour));
        let mut bytes = std::io::Cursor::new(Vec::new());
        picture
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    #[test]
    fn a_url_answers_the_frame_and_a_stale_or_wrong_key_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.png");
        std::fs::write(&file, png([255, 0, 0, 255], 8, 4)).unwrap();
        let wanted = vec![Original::of(&file).unwrap()];
        crate::previews::place(Pack::default().renewed(&wanted, 4, 2));
        assert_eq!(crate::previews::shown().stale(&wanted, 4, 2).len(), 0);
        assert_eq!(crate::previews::url_of(""), "");
        assert_eq!(crate::previews::url_of("/no/such/file.png"), "");
        let key = crate::previews::url_of(&file.to_string_lossy());
        assert!(
            !key.is_empty(),
            "URL should be non-empty for a valid preview"
        );
        let (w, h) = crate::previews::preview_size(&key);
        assert_eq!((w, h), (4, 2));
        let pixels = crate::previews::preview_pixels(&key);
        assert_eq!(pixels.len(), 4 * 2 * 4);
        let (words, _) = pixels.as_chunks::<4>();
        assert!(
            words.iter().all(|p| *p == 0xffff_0000u32.to_ne_bytes()),
            "{pixels:?}"
        );
        crate::previews::place(Pack::default());
        assert!(crate::previews::preview_pixels(&key).is_empty());
        assert_eq!(crate::previews::url_of(&file.to_string_lossy()), "");
    }
}
