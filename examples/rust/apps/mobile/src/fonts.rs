use std::{path::PathBuf, sync::Arc};

use fontdb::{Database, FaceInfo};
use kui_native::{Session, text::FontCatalog};

pub(super) fn session(latin: &[u8]) -> Session {
    let mut db = Database::new();
    if let Some((path, matches)) = japanese_face() {
        if db.load_font_file(&path).is_ok() {
            let selected = db.faces().find(|face| matches(face)).map(|face| face.id);
            let others: Vec<_> = db
                .faces()
                .map(|face| face.id)
                .filter(|id| Some(*id) != selected)
                .collect();
            for id in others {
                db.remove_face(id);
            }
            if let Some(id) = selected {
                // The mapped file belongs to the OS and is immutable while the app runs.
                unsafe { db.make_shared_face_data(id) };
            }
        }
    }
    db.load_font_data(latin.to_vec());
    Session::with_font_catalog(Arc::new(FontCatalog::from_database(
        "en-US".into(),
        db,
        [&["Noto Sans"], &[], &[]],
        None,
    )))
}

#[cfg(target_os = "ios")]
fn japanese_face() -> Option<(PathBuf, impl Fn(&FaceInfo) -> bool)> {
    use std::ffi::{CStr, c_char};
    use std::os::unix::ffi::OsStrExt;
    unsafe extern "C" {
        fn kui_japanese_face(
            path: *mut c_char,
            path_size: usize,
            name: *mut c_char,
            name_size: usize,
        ) -> bool;
    }
    let mut path = [0; 1024];
    let mut name = [0; 256];
    // Success means both caller-owned buffers contain terminated strings.
    unsafe {
        if !kui_japanese_face(path.as_mut_ptr(), path.len(), name.as_mut_ptr(), name.len()) {
            return None;
        }
        let path = std::ffi::OsStr::from_bytes(CStr::from_ptr(path.as_ptr()).to_bytes());
        let name = CStr::from_ptr(name.as_ptr()).to_str().ok()?.to_owned();
        Some((path.into(), move |face: &FaceInfo| {
            face.post_script_name == name
        }))
    }
}

#[cfg(target_os = "android")]
fn japanese_face() -> Option<(PathBuf, impl Fn(&FaceInfo) -> bool)> {
    use std::ffi::{CStr, c_char};
    use std::os::unix::ffi::OsStrExt;
    unsafe extern "C" {
        fn kui_match_japanese(path: *mut c_char, capacity: usize, index: *mut u32) -> i32;
    }
    let mut path = [0; 4096];
    let mut index = 0;
    // A positive result guarantees a terminated path and a valid collection index.
    let status = unsafe { kui_match_japanese(path.as_mut_ptr(), path.len(), &mut index) };
    let (path, index) = match status {
        1 => {
            let bytes = unsafe { CStr::from_ptr(path.as_ptr()) }.to_bytes();
            (PathBuf::from(std::ffi::OsStr::from_bytes(bytes)), index)
        }
        -1 => legacy_japanese_face()?,
        _ => return None,
    };
    Some((path, move |face: &FaceInfo| face.index == index))
}

#[cfg(target_os = "android")]
fn legacy_japanese_face() -> Option<(PathBuf, u32)> {
    let config = std::fs::read_to_string("/system/etc/fonts.xml").ok()?;
    let doc = roxmltree::Document::parse(&config).ok()?;
    let font = doc
        .descendants()
        .filter(|family| {
            family.has_tag_name("family")
                && family
                    .attribute("lang")
                    .unwrap_or_default()
                    .split_whitespace()
                    .any(|lang| lang.split('-').next() == Some("ja"))
        })
        .flat_map(|family| family.children())
        .find(|font| {
            font.has_tag_name("font")
                && font.attribute("weight").unwrap_or("400") == "400"
                && font.attribute("style").unwrap_or("normal") == "normal"
        })?;
    let index: u32 = font.attribute("index").unwrap_or("0").parse().ok()?;
    let path = PathBuf::from("/system/fonts").join(font.text()?.trim());
    Some((path, index))
}
