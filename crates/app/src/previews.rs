//! The `image://preview/<n>/<at>` pictures: the copies the pack holds, served to the rows.

#[cxx_qt::bridge]
pub(crate) mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qqmlengine.h");
        type QQmlEngine = cxx_qt_lib::QQmlEngine;
        include!("rimstack-app/src/application.h");
        /// The engine deletes the provider itself once it owns it.
        #[rust_name = "add_preview_provider"]
        fn addPreviewProvider(engine: Pin<&mut QQmlEngine>);
    }
}

use rimstack_core::base::locks::Shared;
use rimstack_core::base::thumbnails::Pack;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, RwLock};

#[derive(Default)]
struct Shown {
    pack: Pack,
    by_path: HashMap<PathBuf, usize>,
}

static SHOWN: LazyLock<RwLock<Shown>> = LazyLock::new(|| RwLock::new(Shown::default()));
/// Raised whenever the pack shown is replaced: the URLs the rows hold then mean nothing anew.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The URL a row's picture has when the pack holds a copy of it, `image://preview/<n>/<at>`,
/// else empty: the row shows its picture the way it always did until the copy is there.
pub(crate) fn url_of(picture: &str) -> String {
    if picture.is_empty() {
        return String::new();
    }
    let shown = SHOWN.reading();
    match shown.by_path.get(std::path::Path::new(picture)) {
        Some(at) => format!(
            "image://preview/{}/{at}",
            GENERATION.load(Ordering::Relaxed)
        ),
        None => String::new(),
    }
}

/// The generation in the key is still the one the rows were given; else the answer is empty,
/// for a row still holding the URL of the pack replaced meanwhile.
fn for_key(key: &str) -> Option<usize> {
    let (head, at) = key.rsplit_once('/')?;
    // The URL Qt hands over is "/<n>/<at>" while the test asks as spelled: either way the
    // generation is the part beside the index.
    let n = head.rsplit_once('/').map_or(head, |(_, last)| last);
    match (n.parse::<u64>(), at.parse::<usize>()) {
        (Ok(n), Ok(at)) if n == GENERATION.load(Ordering::Relaxed) => Some(at),
        _ => None,
    }
}

/// Get the wide and high of the pack the key names, or (0, 0) if the key is stale/empty.
/// Exposed to C++ as `rimstack_preview_size`.
#[unsafe(no_mangle)]
pub extern "C" fn rimstack_preview_size(
    key_utf16: *const u16,
    key_len: usize,
    wide: *mut u32,
    high: *mut u32,
) -> bool {
    if key_utf16.is_null() || wide.is_null() || high.is_null() {
        return false;
    }
    let key_slice = unsafe { std::slice::from_raw_parts(key_utf16, key_len) };
    let key_str = match String::from_utf16(key_slice) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let Some(_at) = for_key(&key_str) else {
        unsafe {
            *wide = 0;
            *high = 0;
        }
        return true;
    };
    let shown = SHOWN.reading();
    unsafe {
        *wide = shown.pack.wide;
        *high = shown.pack.high;
    }
    true
}

/// Copy the pixel frame for the pack the key names into `into`, or return false.
/// Exposed to C++ as `rimstack_preview_copy`.
#[unsafe(no_mangle)]
pub extern "C" fn rimstack_preview_copy(
    key_utf16: *const u16,
    key_len: usize,
    into: *mut u8,
    room: usize,
) -> bool {
    if key_utf16.is_null() || into.is_null() {
        return false;
    }
    let key_slice = unsafe { std::slice::from_raw_parts(key_utf16, key_len) };
    let key_str = match String::from_utf16(key_slice) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let Some(at) = for_key(&key_str) else {
        return false;
    };
    let shown = SHOWN.reading();
    let Some(pixels) = shown.pack.pixels(at) else {
        return false;
    };
    if pixels.len() != room {
        return false;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), into, pixels.len());
    }
    true
}

/// Safe Rust helper: the pixels for a key (used by tests).
/// Test helpers: safe Rust accessors for the current pack.
#[cfg(test)]
pub(crate) fn preview_pixels(key: &str) -> Vec<u8> {
    let Some(at) = for_key(key) else {
        return Vec::new();
    };
    let shown = SHOWN.reading();
    shown.pack.pixels(at).unwrap_or(&[]).to_vec()
}

#[cfg(test)]
pub(crate) fn preview_size(key: &str) -> (u32, u32) {
    let Some(_at) = for_key(key) else {
        return (0, 0);
    };
    let shown = SHOWN.reading();
    (shown.pack.wide, shown.pack.high)
}

/// The pack the rows hold; `workspace::previews` refreshes it off the window's thread.
pub(crate) fn shown() -> Pack {
    SHOWN.reading().pack.clone()
}

pub(crate) fn place(pack: Pack) {
    GENERATION.fetch_add(1, Ordering::Relaxed);
    let mut shown = SHOWN.writing();
    shown.by_path = pack
        .originals()
        .enumerate()
        .map(|(at, original)| (original.path.clone(), at))
        .collect();
    shown.pack = pack;
}
