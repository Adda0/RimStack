use crate::base::error::Error;
use image::{Rgba, RgbaImage};
use std::path::Path;

/// A step of a colour or of opacity too small to see.
const SAME_WITHIN: u8 = 2;

pub struct Difference {
    pub picture: RgbaImage,
    pub differing: u64,
    pub total: u64,
}

pub fn of(earlier: &RgbaImage, later: &RgbaImage) -> Difference {
    let (wide, high) = later.dimensions();
    let stretched;
    let earlier = if earlier.dimensions() == (wide, high) {
        earlier
    } else {
        let nearest = image::imageops::FilterType::Nearest;
        stretched = image::imageops::resize(earlier, wide, high, nearest);
        &stretched
    };
    let mut differing = 0;
    let picture = RgbaImage::from_fn(wide, high, |x, y| {
        let (a, b) = (earlier.get_pixel(x, y), later.get_pixel(x, y));
        // Two clear pixels are the same whatever colour they hide.
        let both_clear = a[3] == 0 && b[3] == 0;
        let close =
            a.0.iter()
                .zip(b.0)
                .all(|(a, b)| a.abs_diff(b) <= SAME_WITHIN);
        if both_clear || close {
            return Rgba([0, 0, 0, 0]);
        }
        differing += 1;
        // A pixel the later picture leaves clear still has to show.
        match b[3] {
            0 => Rgba([255, 0, 255, 255]),
            _ => Rgba([b[0], b[1], b[2], 255]),
        }
    });
    Difference {
        picture,
        differing,
        total: u64::from(wide) * u64::from(high),
    }
}

pub fn write(earlier: &Path, later: &Path, to: &Path) -> Result<(u64, u64), Error> {
    let read = |path: &Path| -> Result<RgbaImage, Error> {
        let bytes = std::fs::read(path).map_err(|e| Error::file(path, e))?;
        let picture = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png);
        Ok(picture
            .map_err(|e| Error::format(format!("{}: {e}", path.display())))?
            .to_rgba8())
    };
    let found = of(&read(earlier)?, &read(later)?);
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::file(dir, e))?;
    }
    let saved = found.picture.save_with_format(to, image::ImageFormat::Png);
    saved.map_err(|e| Error::file(to, e))?;
    Ok((found.differing, found.total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_pixels_that_differ_are_kept() {
        let earlier = RgbaImage::from_pixel(4, 2, Rgba([10, 20, 30, 255]));
        let mut later = earlier.clone();
        later.put_pixel(1, 0, Rgba([200, 20, 30, 255]));
        later.put_pixel(2, 1, Rgba([11, 21, 31, 255]));
        let found = of(&earlier, &later);
        assert_eq!((found.differing, found.total), (1, 8));
        assert_eq!(found.picture.get_pixel(1, 0), &Rgba([200, 20, 30, 255]));
        assert_eq!(found.picture.get_pixel(2, 1), &Rgba([0, 0, 0, 0]));
    }

    #[test]
    fn clear_pixels_are_the_same_and_a_cleared_one_still_shows() {
        let earlier =
            RgbaImage::from_fn(2, 1, |x, _| Rgba([9, 9, 9, if x == 0 { 0 } else { 255 }]));
        let later = RgbaImage::from_pixel(2, 1, Rgba([200, 200, 200, 0]));
        let found = of(&earlier, &later);
        assert_eq!(found.differing, 1);
        assert_eq!(found.picture.get_pixel(0, 0)[3], 0);
        assert_eq!(found.picture.get_pixel(1, 0), &Rgba([255, 0, 255, 255]));
    }

    #[test]
    fn another_size_is_stretched_before_comparing() {
        let earlier = RgbaImage::from_pixel(1, 1, Rgba([5, 5, 5, 255]));
        let later = RgbaImage::from_pixel(2, 2, Rgba([5, 5, 5, 255]));
        assert_eq!(of(&earlier, &later).differing, 0);
    }

    #[test]
    fn two_files_give_a_third() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b, to) = (
            dir.path().join("a.png"),
            dir.path().join("b.png"),
            dir.path().join("kept/d.png"),
        );
        RgbaImage::from_pixel(2, 2, Rgba([1, 1, 1, 255]))
            .save(&a)
            .unwrap();
        RgbaImage::from_pixel(2, 2, Rgba([90, 1, 1, 255]))
            .save(&b)
            .unwrap();
        assert_eq!(write(&a, &b, &to), Ok((4, 4)));
        assert_eq!(
            image::open(&to).unwrap().to_rgba8().get_pixel(0, 0),
            &Rgba([90, 1, 1, 255])
        );
        assert!(write(&dir.path().join("none.png"), &b, &to).is_err());
    }
}
