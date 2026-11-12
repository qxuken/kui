use fontdb::{Database, FaceInfo, Source};
use kui_core::text::{FontCatalog, FontLoader};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

mod source;

#[cfg(target_os = "macos")]
const FAMILIES: [&[&str]; 3] = [
    &["Helvetica Neue", "Helvetica"],
    &["Times New Roman", "Times"],
    &["SF Mono", "Menlo", "Monaco"],
];
#[cfg(target_os = "windows")]
const FAMILIES: [&[&str]; 3] = [
    &["Segoe UI", "Arial"],
    &["Times New Roman"],
    &["Cascadia Mono", "Consolas", "Courier New"],
];
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const FAMILIES: [&[&str]; 3] = [
    &["DejaVu Sans", "Noto Sans", "Liberation Sans", "Ubuntu"],
    &["DejaVu Serif", "Noto Serif", "Liberation Serif"],
    &[
        "DejaVu Sans Mono",
        "Noto Sans Mono",
        "Liberation Mono",
        "Ubuntu Mono",
    ],
];

struct Scan {
    fonts: Arc<FontCatalog>,
    sources: HashMap<PathBuf, Arc<source::Source>>,
}

static CATALOG: Mutex<Option<Scan>> = Mutex::new(None);

pub(super) fn catalog() -> Arc<FontCatalog> {
    let mut cached = CATALOG.lock().unwrap_or_else(|e| e.into_inner());
    cached
        .get_or_insert_with(|| scan(&HashMap::new()))
        .fonts
        .clone()
}

fn scan(previous: &HashMap<PathBuf, Arc<source::Source>>) -> Scan {
    let locale = sys_locale::get_locale().unwrap_or_else(|| "en-US".into());
    let mut db = Database::new();
    db.load_system_fonts();
    db.set_monospace_family("Noto Sans Mono");
    db.set_sans_serif_family("Open Sans");
    db.set_serif_family("DejaVu Serif");
    prepare_catalog(locale, &db, previous)
}

fn prepare_catalog(
    locale: String,
    db: &Database,
    previous: &HashMap<PathBuf, Arc<source::Source>>,
) -> Scan {
    use fontdb::{Family, Query};

    let faces: Vec<_> = db
        .faces()
        .filter(|face| {
            db.with_face_data(face.id, FontCatalog::can_measure)
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    let mut paths = HashMap::<PathBuf, Vec<u32>>::new();
    for face in &faces {
        if let Source::File(path) = &face.source {
            let indices = paths.entry(path.clone()).or_default();
            if !indices.contains(&face.index) {
                indices.push(face.index);
            }
        }
    }
    let sources: HashMap<_, _> = paths
        .into_iter()
        .map(|(path, indices)| {
            let revision = File::open(&path)
                .ok()
                .and_then(|file| Revision::read(&file));
            let source = previous
                .get(&path)
                .filter(|source| {
                    revision.is_some() && source.revision() == revision.as_ref() && !source.failed()
                })
                .cloned()
                .unwrap_or_else(|| Arc::new(source::Source::new(path.clone(), indices, revision)));
            (path, source)
        })
        .collect();

    // Keep a usable default resident even if another font is removed
    // before its first use. Remaining files are prepared on demand.
    let preferred = FAMILIES[0].iter().find_map(|name| {
        db.query(&Query {
            families: &[Family::Name(name)],
            ..Default::default()
        })
    });
    let usable = preferred
        .into_iter()
        .chain(faces.iter().map(|face| face.id))
        .any(|id| {
            db.face(id).is_some_and(|face| match &face.source {
                Source::File(path) => sources.get(path).is_some_and(|source| source.prepare()),
                _ => true,
            })
        });
    let mut prepared = Database::new();
    for family in [
        fontdb::Family::SansSerif,
        fontdb::Family::Serif,
        fontdb::Family::Monospace,
    ] {
        let name = db.family_name(&family);
        match family {
            fontdb::Family::SansSerif => prepared.set_sans_serif_family(name),
            fontdb::Family::Serif => prepared.set_serif_family(name),
            _ => prepared.set_monospace_family(name),
        }
    }
    for mut face in faces {
        if let Source::File(path) = &face.source {
            if !usable {
                continue;
            }
            face.source = Source::SharedFile(path.clone(), sources[path].clone());
        }
        prepared.push_face_info(face);
    }
    let fonts = Arc::new(FontCatalog::from_platform_database(
        locale, prepared, FAMILIES,
    ));
    // A requested family can overtake these small default-family warmups.
    for family in [fontdb::Family::Serif, fontdb::Family::Monospace] {
        let names = if family == Family::Serif {
            FAMILIES[1]
        } else {
            FAMILIES[2]
        };
        let id = names.iter().find_map(|name| {
            db.query(&Query {
                families: &[Family::Name(name)],
                ..Default::default()
            })
        });
        if let Some(face) = id.and_then(|id| db.face(id))
            && let Source::File(path) = &face.source
            && let Some(source) = sources.get(path)
        {
            source.request(false);
        }
    }
    Scan { fonts, sources }
}

type FontBytes = Arc<dyn AsRef<[u8]> + Send + Sync>;

#[derive(Clone, PartialEq, Eq)]
struct Revision {
    len: u64,
    modified: std::time::SystemTime,
    #[cfg(unix)]
    inode: (u64, u64, i64, i64),
    #[cfg(not(unix))]
    created: Option<std::time::SystemTime>,
}

impl Revision {
    fn read(file: &File) -> Option<Self> {
        let meta = file.metadata().ok()?;
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Some(Self {
            len: meta.len(),
            modified: meta.modified().ok()?,
            #[cfg(unix)]
            inode: (meta.dev(), meta.ino(), meta.ctime(), meta.ctime_nsec()),
            #[cfg(not(unix))]
            created: meta.created().ok(),
        })
    }
}

#[derive(Clone)]
struct Mapping {
    revision: Option<Revision>,
    bytes: FontBytes,
    measurable: Vec<u32>,
}

fn map(
    path: &Path,
    indices: &[u32],
    previous: Option<&Mapping>,
    snapshots: &mut Snapshots,
) -> Option<Mapping> {
    let mut file = File::open(path).ok()?;
    let revision = Revision::read(&file);
    if let Some(previous) = previous
        && revision.is_some()
        && previous.revision == revision
    {
        return Some(previous.clone());
    }
    let immutable = on_readonly_snapshot(&file);
    let snapshot = if !immutable && revision.as_ref().is_some_and(|rev| rev.len <= 512 * 1024) {
        None
    } else {
        snapshots.capture(&mut file, immutable).ok()
    };
    let (bytes, measurable) = if let Some(snapshot) = snapshot {
        let check = map_snapshot(&snapshot)?;
        let measurable = measurable_indices(&check, indices)?;
        // Keep validated pages out of the resident set until shaping uses them.
        drop(check);
        (Arc::new(map_snapshot(&snapshot)?) as FontBytes, measurable)
    } else {
        file.rewind().ok()?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).ok()?;
        let measurable = measurable_indices(&bytes, indices)?;
        (snapshots.store(bytes), measurable)
    };
    if revision.is_some() && Revision::read(&file) != revision {
        return None;
    }
    Some(Mapping {
        revision,
        bytes,
        measurable,
    })
}

fn measurable_indices(bytes: &[u8], indices: &[u32]) -> Option<Vec<u32>> {
    let indices: Vec<_> = indices
        .iter()
        .copied()
        .filter(|&index| FontCatalog::can_measure(bytes, index))
        .collect();
    (!indices.is_empty()).then_some(indices)
}

#[derive(Default)]
struct Snapshots {
    #[cfg(target_os = "macos")]
    directory: Option<(tempfile::TempDir, File)>,
    #[cfg(unix)]
    small: Option<SnapshotPack>,
}

impl Snapshots {
    fn capture(&mut self, file: &mut File, immutable: bool) -> io::Result<File> {
        if immutable {
            return file.try_clone();
        }
        #[cfg(target_os = "macos")]
        {
            if let Ok(snapshot) = self.clone_file(file) {
                return Ok(snapshot);
            }
        }
        let mut snapshot = tempfile::tempfile()?;
        io::copy(file, &mut snapshot)?;
        Ok(snapshot)
    }

    fn store(&mut self, bytes: Vec<u8>) -> FontBytes {
        #[cfg(unix)]
        if let Ok(mapping) = self.pack(&bytes) {
            return Arc::new(mapping);
        }
        Arc::new(bytes)
    }

    #[cfg(unix)]
    fn pack(&mut self, bytes: &[u8]) -> io::Result<memmap2::Mmap> {
        if self.small.is_none() {
            self.small = Some(SnapshotPack {
                file: tempfile::tempfile()?,
                len: 0,
            });
        }
        let pack = self.small.as_mut().unwrap();
        let mapped = pack.append(bytes);
        // The on-demand worker lives for the process. Release completed
        // packs so obsolete ranges can go away with their last mapping.
        if mapped.is_err() || pack.len >= 8 * 1024 * 1024 {
            self.small = None;
        }
        mapped
    }

    #[cfg(target_os = "macos")]
    fn clone_file(&mut self, file: &File) -> io::Result<File> {
        use std::os::fd::{AsRawFd, FromRawFd};
        if self.directory.is_none() {
            let dir = tempfile::tempdir()?;
            let fd = File::open(dir.path())?;
            self.directory = Some((dir, fd));
        }
        let dir = self.directory.as_ref().unwrap().1.as_raw_fd();
        let name = c"font".as_ptr();
        // SAFETY: The descriptors and destination name are valid. This
        // worker exclusively owns the directory and removes each clone.
        unsafe {
            if libc::fclonefileat(file.as_raw_fd(), dir, name, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            let fd = libc::openat(dir, name, libc::O_RDONLY | libc::O_CLOEXEC);
            let opened = if fd >= 0 {
                Ok(File::from_raw_fd(fd))
            } else {
                Err(io::Error::last_os_error())
            };
            if libc::unlinkat(dir, name, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            opened
        }
    }
}

/// Many small fonts can share a private file. Earlier byte ranges are
/// immutable even while the worker appends more fonts.
#[cfg(unix)]
struct SnapshotPack {
    file: File,
    len: u64,
}

#[cfg(unix)]
impl SnapshotPack {
    fn append(&mut self, bytes: &[u8]) -> io::Result<memmap2::Mmap> {
        use std::io::Write;
        self.file.write_all(bytes)?;
        // SAFETY: This file is unlinked and only this worker can write to
        // it. The writer only appends, so the published range stays fixed.
        let mapped = unsafe {
            memmap2::MmapOptions::new()
                .offset(self.len)
                .len(bytes.len())
                .map(&self.file)?
        };
        self.len += bytes.len() as u64;
        Ok(mapped)
    }
}

#[cfg(not(target_os = "macos"))]
fn on_readonly_snapshot(_file: &File) -> bool {
    false
}

#[cfg(target_os = "macos")]
fn on_readonly_snapshot(file: &File) -> bool {
    use std::os::fd::AsRawFd;
    let mut fs = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: fstatfs initializes the output on success and the fd is live.
    if unsafe { libc::fstatfs(file.as_raw_fd(), fs.as_mut_ptr()) } != 0 {
        return false;
    }
    // SAFETY: the successful call above initialized fs.
    let flags = unsafe { fs.assume_init() }.f_flags;
    let immutable = (libc::MNT_SNAPSHOT | libc::MNT_RDONLY) as u32;
    flags & immutable == immutable
}

fn map_snapshot(file: &File) -> Option<memmap2::Mmap> {
    // SAFETY: The backing is either a read-only filesystem snapshot or a
    // private, unlinked file we never write again after copying.
    let bytes = unsafe { memmap2::MmapOptions::new().map(file).ok()? };
    #[cfg(unix)]
    let _ = bytes.advise(memmap2::Advice::Random);
    Some(bytes)
}

fn prepare(
    db: &Database,
    previous: &HashMap<PathBuf, Mapping>,
) -> (Vec<FaceInfo>, HashMap<PathBuf, Mapping>) {
    // Assign one mapping per path, then reuse the scan's metadata and face
    // order. In particular, every face of a collection shares its mapping.
    let mut indices = HashMap::<&Path, usize>::new();
    let mut paths = Vec::<(&Path, Vec<u32>)>::new();
    for face in db.faces() {
        if let Source::File(path) = &face.source {
            let group = *indices.entry(path).or_insert_with(|| {
                paths.push((path, Vec::new()));
                paths.len() - 1
            });
            let face_indices = &mut paths[group].1;
            if !face_indices.contains(&face.index) {
                face_indices.push(face.index);
            }
        }
    }
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(paths.len().div_ceil(64));
    let mut maps = vec![None; paths.len()];
    let workers = workers.max(1);
    let mut chunks = (0..workers).map(|_| Vec::new()).collect::<Vec<_>>();
    for (index, job) in paths.iter().zip(&mut maps).enumerate() {
        chunks[index % workers].push(job);
    }
    std::thread::scope(|scope| {
        for chunk in chunks {
            scope.spawn(move || {
                let mut snapshots = Snapshots::default();
                for ((path, indices), entry) in chunk {
                    *entry = map(path, indices, previous.get(*path), &mut snapshots);
                }
            });
        }
    });
    let faces = db
        .faces()
        .filter_map(|face| {
            let mut face = face.clone();
            if let Source::File(path) = &face.source {
                let mapping = maps[indices[path.as_path()]].as_ref()?;
                if !mapping.measurable.contains(&face.index) {
                    return None;
                }
                face.source = Source::SharedFile(path.clone(), mapping.bytes.clone());
            }
            Some(face)
        })
        .collect();
    let mappings = paths
        .into_iter()
        .zip(maps)
        .filter_map(|((path, _), map)| Some((path.to_owned(), map?)))
        .collect();
    (faces, mappings)
}

pub(super) struct Loader;

impl FontLoader for Loader {
    fn file(&self, path: &Path) -> Vec<FaceInfo> {
        let Ok(bytes) = std::fs::read(path) else {
            return Vec::new();
        };
        let mut db = Database::new();
        db.load_font_source(Source::SharedFile(PathBuf::from(path), Arc::new(bytes)));
        db.faces().cloned().collect()
    }

    fn directory(&self, path: &Path) -> Vec<FaceInfo> {
        let mut db = Database::new();
        db.load_fonts_dir(path);
        prepare(&db, &HashMap::new()).0
    }

    fn reload(&self) -> Arc<FontCatalog> {
        let mut cached = CATALOG.lock().unwrap_or_else(|e| e.into_inner());
        let fresh = match cached.as_ref() {
            Some(previous) => scan(&previous.sources),
            None => scan(&HashMap::new()),
        };
        let fonts = fresh.fonts.clone();
        *cached = Some(fresh);
        fonts
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    #[cfg(unix)]
    #[test]
    fn completed_snapshot_packs_keep_old_mappings_and_release_the_writer() {
        let mut snapshots = super::Snapshots::default();
        let bytes = vec![42; 512 * 1024];
        let first = snapshots.pack(&bytes).unwrap();
        for _ in 1..16 {
            assert_eq!(&*snapshots.pack(&bytes).unwrap(), &bytes);
        }
        assert!(snapshots.small.is_none());
        assert_eq!(&*first, &bytes);
        assert_eq!(&*snapshots.pack(b"next pack").unwrap(), b"next pack");
        assert!(snapshots.small.is_some());
    }

    #[test]
    fn startup_prepares_the_default_face_and_keeps_other_weights_deferred() {
        let dir = tempfile::tempdir().unwrap();
        let regular = dir.path().join("regular.ttf");
        let bold = dir.path().join("bold.ttf");
        std::fs::write(
            &regular,
            include_bytes!("../../../kui-cli/assets/fonts/NotoSans-Regular.ttf"),
        )
        .unwrap();
        std::fs::write(
            &bold,
            include_bytes!("../../../kui-cli/assets/fonts/NotoSans-Bold.ttf"),
        )
        .unwrap();
        let mut db = fontdb::Database::new();
        db.load_font_file(&regular).unwrap();
        db.load_font_file(&bold).unwrap();
        let scan = super::prepare_catalog("en-US".into(), &db, &Default::default());
        let session = kui_core::Session::with_font_catalog(scan.fonts);
        let mut core = kui_core::Core::new_in(&session);
        assert!(scan.sources[&regular].prepared());
        assert!(
            !scan.sources[&bold].prepared(),
            "startup copied an unrequested font file"
        );
        assert_eq!(core.system_font_families(), ["Noto Sans"]);

        let mut ui = core.frame(kui_core::Size::new(400.0, 200.0), 1.0);
        ui.text("The first regular label", kui_core::TextStyle::new(18.0));
        ui.finish();
        assert!(
            !scan.sources[&bold].prepared(),
            "matching a regular label loaded an unrelated weight"
        );

        let font = core.add_system_font("Noto Sans").unwrap();
        assert!(
            !scan.sources[&bold].prepared(),
            "registering a regular face loaded an unrequested weight"
        );
        let style = kui_core::TextStyle::new(18.0).font(font).bold();
        let measured = core.measure_text("Bold is ready on its first use", &style, None);
        assert!(scan.sources[&bold].prepared());
        std::fs::OpenOptions::new()
            .write(true)
            .open(&bold)
            .unwrap()
            .set_len(0)
            .unwrap();
        assert_eq!(
            measured,
            core.measure_text("Bold is ready on its first use", &style, None)
        );
    }

    #[test]
    fn unchanged_rescans_reuse_prepared_and_pending_sources() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(include_bytes!(
            "../../../kui-cli/assets/fonts/NotoSans-Regular.ttf"
        ))
        .unwrap();
        let mut db = fontdb::Database::new();
        db.load_font_file(file.path()).unwrap();
        let first = super::prepare_catalog("en-US".into(), &db, &Default::default());
        let next = super::prepare_catalog("en-US".into(), &db, &first.sources);
        assert!(std::sync::Arc::ptr_eq(
            &first.sources[file.path()],
            &next.sources[file.path()]
        ));
        let session = kui_core::Session::with_font_catalog(first.fonts);
        let mut core = kui_core::Core::new_in(&session);
        assert_eq!(core.apply_font_catalog(next.fonts), 0);
    }

    #[test]
    fn registered_file_survives_an_in_place_truncate() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(include_bytes!(
            "../../../kui-cli/assets/fonts/NotoSans-Regular.ttf"
        ))
        .unwrap();
        let mut core = kui_core::Core::new();
        let font = super::super::load_font_file(&mut core, file.path()).unwrap();
        file.as_file().set_len(0).unwrap();
        let style = kui_core::TextStyle::new(18.0).font(font);
        let measured = core.measure_text("Font still shapes", &style, None);
        let fallback =
            core.measure_text("Font still shapes", &kui_core::TextStyle::new(18.0), None);
        assert_ne!(measured, fallback);
    }
}
