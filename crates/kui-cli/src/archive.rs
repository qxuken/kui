//! The archive formats `kui pack` writes and the runner reads (ADR 0068,
//! decisions 11, 13, 14 and 18).
//!
//! A packed app is a zip: on its own as `<name>.kui`, appended to a runner
//! for the Windows `exe` and the Linux `bin`, and as `app.zip` inside a
//! macOS `.app`. An appended zip has its offsets shifted by the runner's
//! length, so the fused file is a valid zip as a whole (`unzip -l app.exe`
//! lists it without an "extra bytes" warning) and the runner finds its
//! payload from its own tail. The reader also accepts a zip someone
//! `cat`-ed onto a binary without shifting, by working out where it really
//! starts. The Linux `bin` ships in a `.tar.gz`, and the runner release
//! comes as `.tar.gz` and `.zip`, so tar is here too.
//!
//! These are written by hand rather than with the `zip` and `tar` crates:
//! the subset kui needs is small (store, deflate, no zip64, no
//! encryption; ustar files and folders), and packs come out reproducible
//! because every timestamp is fixed.

// Some of what is here (a reader's conveniences, the parsing back) only
// the tests and `kui pack` itself use; it stays as the format's whole API.
#![cfg_attr(not(test), allow(dead_code))]

// `kui pack` and the runner's payload lookup are wired to this module in a
// later step; until then its API is only reached from the tests.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use flate2::Compression;

/// 1980-01-01 00:00:00 UTC, the earliest moment a zip can name: the
/// timestamp every entry gets unless the caller passes one, so the same
/// app packs to the same bytes.
pub const EPOCH_1980: u64 = 315_532_800;

const LOCAL_SIG: u32 = 0x0403_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const EOCD_SIG: u32 = 0x0605_4b50;
const EOCD_LEN: usize = 22;
const LOCAL_LEN: usize = 30;
const CENTRAL_LEN: usize = 46;
/// Unix in the "version made by" high byte, which tells `unzip` the
/// external attributes carry a Unix mode.
const HOST_UNIX: u16 = 3;
/// General purpose bit 11: the name is UTF-8.
const FLAG_UTF8: u16 = 1 << 11;
const S_IFREG: u32 = 0o100000;
const S_IFDIR: u32 = 0o040000;
const S_IFMT: u32 = 0o170000;
/// The MS-DOS directory attribute, for tools that ignore the Unix mode.
const DOS_DIR: u32 = 0x10;

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

/// Whether an archive member name is safe to unpack: relative, with no
/// `..` component and no drive letter. Backslashes count as separators
/// because on Windows they are.
pub fn is_safe_name(name: &str) -> bool {
    if name.is_empty() || name.starts_with('/') || name.starts_with('\\') || name.contains('\0') {
        return false;
    }
    let b = name.as_bytes();
    if b.len() >= 2 && b[1] == b':' {
        return false;
    }
    !name.split(['/', '\\']).any(|c| c == "..")
}

/// Lowercase hex SHA-256: the sums `kui pack` writes beside its artifacts
/// and the installers check (decision 15), and the `hash` in
/// `kui.pack.lua` (decision 11).
pub fn sha256_hex(bytes: &[u8]) -> String {
    let d = ring::digest::digest(&ring::digest::SHA256, bytes);
    let mut s = String::with_capacity(64);
    for b in d.as_ref() {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// A Unix time as an MS-DOS (time, date) pair, in UTC. Times before 1980
/// become 1980-01-01, the format's floor; seconds lose their last bit.
fn dos_datetime(unix: u64) -> (u16, u16) {
    let unix = unix.max(EPOCH_1980);
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    // Howard Hinnant's days-to-civil.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    let y = (y - 1980).clamp(0, 127) as u16;
    let time = ((secs / 3600) as u16) << 11
        | (((secs % 3600) / 60) as u16) << 5
        | ((secs % 60) / 2) as u16;
    let date = y << 9 | (m as u16) << 5 | d as u16;
    (time, date)
}

struct Pending {
    name: String,
    method: u16,
    crc: u32,
    size: u32,
    data: Vec<u8>,
    external: u32,
}

/// Builds a zip in memory. Entries keep the order they were added in.
pub struct ZipWriter {
    entries: Vec<Pending>,
    names: HashMap<String, ()>,
    time: u16,
    date: u16,
}

impl Default for ZipWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl ZipWriter {
    /// A writer that stamps every entry 1980-01-01.
    pub fn new() -> Self {
        Self::with_mtime(EPOCH_1980)
    }

    /// A writer that stamps every entry with `unix_secs` (UTC).
    pub fn with_mtime(unix_secs: u64) -> Self {
        let (time, date) = dos_datetime(unix_secs);
        ZipWriter {
            entries: Vec::new(),
            names: HashMap::new(),
            time,
            date,
        }
    }

    fn check_name(&mut self, name: &str) -> io::Result<()> {
        if !is_safe_name(name) || name.len() > u16::MAX as usize {
            return Err(invalid(format!("{name:?} is not a relative archive path")));
        }
        if self.names.insert(name.to_string(), ()).is_some() {
            return Err(invalid(format!("{name:?} is in the archive twice")));
        }
        if self.entries.len() >= u16::MAX as usize {
            return Err(invalid(
                "more than 65535 entries need zip64, which kui does not write",
            ));
        }
        Ok(())
    }

    /// Adds a file. `unix_mode` is its permission bits (0o644, 0o755); the
    /// file-type bits are added here. With `deflate` the data is
    /// compressed, and stored anyway when compressing does not shrink it.
    pub fn add_file(
        &mut self,
        name: &str,
        data: &[u8],
        unix_mode: u32,
        deflate: bool,
    ) -> io::Result<()> {
        if name.ends_with('/') {
            return Err(invalid(format!(
                "{name:?} names a folder; add it with add_dir"
            )));
        }
        let size = u32::try_from(data.len())
            .ok()
            .filter(|&s| s < u32::MAX)
            .ok_or_else(|| invalid(format!("{name} is 4 GiB or more, which needs zip64")))?;
        self.check_name(name)?;
        let crc = crc32fast::hash(data);
        let mut method = 0;
        let mut stored = None;
        if deflate && !data.is_empty() {
            let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), Compression::best());
            enc.write_all(data)?;
            let packed = enc.finish()?;
            if packed.len() < data.len() {
                method = 8;
                stored = Some(packed);
            }
        }
        let data = stored.unwrap_or_else(|| data.to_vec());
        let external = (S_IFREG | (unix_mode & 0o7777)) << 16;
        self.entries.push(Pending {
            name: name.to_string(),
            method,
            crc,
            size,
            data,
            external,
        });
        Ok(())
    }

    /// Adds a folder entry, mode 0755. A trailing `/` is added when missing.
    pub fn add_dir(&mut self, name: &str) -> io::Result<()> {
        let name = if name.ends_with('/') {
            name.to_string()
        } else {
            format!("{name}/")
        };
        self.check_name(&name)?;
        let external = (S_IFDIR | 0o755) << 16 | DOS_DIR;
        self.entries.push(Pending {
            name,
            method: 0,
            crc: 0,
            size: 0,
            data: Vec::new(),
            external,
        });
        Ok(())
    }

    /// The zip's bytes, laid out as though `base_offset` bytes come before
    /// it: every offset in the central directory and the end record is
    /// shifted by it. Pass the runner's length when appending to a runner,
    /// 0 for a zip of its own.
    pub fn finish(self, base_offset: u64, comment: &[u8]) -> io::Result<Vec<u8>> {
        let mut out = Vec::new();
        self.finish_into(&mut out, base_offset, comment)?;
        Ok(out)
    }

    /// [`finish`](Self::finish) into any writer; returns the bytes written.
    pub fn finish_into<W: Write>(
        self,
        w: &mut W,
        base_offset: u64,
        comment: &[u8],
    ) -> io::Result<u64> {
        if comment.len() > u16::MAX as usize {
            return Err(invalid("a zip comment is at most 65535 bytes"));
        }
        let too_big = || invalid("the archive would pass 4 GiB, which needs zip64");
        let fits = |v: u64| {
            u32::try_from(v)
                .ok()
                .filter(|&v| v < u32::MAX)
                .ok_or_else(too_big)
        };
        let mut central = Vec::new();
        let mut pos: u64 = 0;
        for e in &self.entries {
            let offset = fits(base_offset + pos)?;
            let csize = fits(e.data.len() as u64)?;
            let flags = if e.name.is_ascii() { 0 } else { FLAG_UTF8 };
            let needed: u16 = if e.method == 8 || e.name.ends_with('/') {
                20
            } else {
                10
            };
            let mut h = Vec::with_capacity(LOCAL_LEN + e.name.len());
            put32(&mut h, LOCAL_SIG);
            put16(&mut h, needed);
            put16(&mut h, flags);
            put16(&mut h, e.method);
            put16(&mut h, self.time);
            put16(&mut h, self.date);
            put32(&mut h, e.crc);
            put32(&mut h, csize);
            put32(&mut h, e.size);
            put16(&mut h, e.name.len() as u16);
            put16(&mut h, 0);
            h.extend_from_slice(e.name.as_bytes());
            w.write_all(&h)?;
            w.write_all(&e.data)?;
            pos += (h.len() + e.data.len()) as u64;

            put32(&mut central, CENTRAL_SIG);
            put16(&mut central, HOST_UNIX << 8 | 20);
            put16(&mut central, needed);
            put16(&mut central, flags);
            put16(&mut central, e.method);
            put16(&mut central, self.time);
            put16(&mut central, self.date);
            put32(&mut central, e.crc);
            put32(&mut central, csize);
            put32(&mut central, e.size);
            put16(&mut central, e.name.len() as u16);
            put16(&mut central, 0); // extra
            put16(&mut central, 0); // comment
            put16(&mut central, 0); // disk
            put16(&mut central, 0); // internal attributes
            put32(&mut central, e.external);
            put32(&mut central, offset);
            central.extend_from_slice(e.name.as_bytes());
        }
        let cd_offset = fits(base_offset + pos)?;
        let cd_size = fits(central.len() as u64)?;
        fits(base_offset + pos + central.len() as u64)?;
        w.write_all(&central)?;
        let mut end = Vec::with_capacity(EOCD_LEN + comment.len());
        let n = self.entries.len() as u16;
        put32(&mut end, EOCD_SIG);
        put16(&mut end, 0);
        put16(&mut end, 0);
        put16(&mut end, n);
        put16(&mut end, n);
        put32(&mut end, cd_size);
        put32(&mut end, cd_offset);
        put16(&mut end, comment.len() as u16);
        end.extend_from_slice(comment);
        w.write_all(&end)?;
        Ok(pos + central.len() as u64 + end.len() as u64)
    }
}

fn put16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn le16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Where a reader's bytes live: all in memory, or in a file read on
/// demand so a 20 MB runner is not loaded to reach a 200 KB payload.
#[derive(Clone)]
enum Source {
    Bytes(Arc<[u8]>),
    File(Arc<Mutex<File>>),
}

impl Source {
    fn read_at(&self, len_total: u64, at: u64, n: usize) -> io::Result<Vec<u8>> {
        if at.checked_add(n as u64).is_none_or(|end| end > len_total) {
            return Err(invalid("the archive points past its end"));
        }
        match self {
            Source::Bytes(b) => Ok(b[at as usize..at as usize + n].to_vec()),
            Source::File(f) => {
                let mut f = f.lock().unwrap_or_else(|p| p.into_inner());
                f.seek(SeekFrom::Start(at))?;
                let mut buf = vec![0; n];
                f.read_exact(&mut buf)?;
                Ok(buf)
            }
        }
    }
}

struct Meta {
    name: String,
    method: u16,
    flags: u16,
    crc: u32,
    csize: u64,
    size: u64,
    /// The local header's absolute position in the source.
    local: u64,
    mode: u32,
    dir: bool,
}

/// Reads a zip, alone or at the end of something else.
pub struct ZipReader {
    src: Source,
    len: u64,
    entries: Vec<Meta>,
    index: HashMap<String, usize>,
    comment: Vec<u8>,
    start: u64,
}

/// One member of a [`ZipReader`].
pub struct Entry<'a> {
    zip: &'a ZipReader,
    meta: &'a Meta,
}

impl ZipReader {
    /// Reads the zip that ends `bytes`.
    pub fn open(bytes: Arc<[u8]>) -> io::Result<ZipReader> {
        let len = bytes.len() as u64;
        Self::from_source(Source::Bytes(bytes), len)?
            .ok_or_else(|| invalid("no zip end record: not a zip, or cut short"))
    }

    /// Reads the zip that ends the file at `path`, reading entries from the
    /// file as they are asked for.
    pub fn open_file(path: &Path) -> io::Result<ZipReader> {
        Self::find_appended(path)?
            .ok_or_else(|| invalid(format!("{} does not end in a zip", path.display())))
    }

    /// Whether the file at `path` ends in a zip — a packed runner, a `.kui`
    /// — reading only its tail and central directory. `None` for a file
    /// with no zip end record (a plain runner); an error when it has one
    /// whose directory does not hold together.
    pub fn find_appended(path: &Path) -> io::Result<Option<ZipReader>> {
        let f = File::open(path)?;
        let len = f.metadata()?.len();
        Self::from_source(Source::File(Arc::new(Mutex::new(f))), len)
    }

    fn from_source(src: Source, len: u64) -> io::Result<Option<ZipReader>> {
        if len < EOCD_LEN as u64 {
            return Ok(None);
        }
        let tail_len = len.min((u16::MAX as usize + EOCD_LEN) as u64) as usize;
        let tail_at = len - tail_len as u64;
        let tail = src.read_at(len, tail_at, tail_len)?;
        // The end record is the last one whose comment runs exactly to the
        // end of the file; insisting on that keeps a stray signature in a
        // binary's data from passing for a zip.
        let mut found = None;
        for i in (0..=tail_len - EOCD_LEN).rev() {
            if le32(&tail, i) == EOCD_SIG && i + EOCD_LEN + le16(&tail, i + 20) as usize == tail_len
            {
                found = Some(i);
                break;
            }
        }
        let Some(i) = found else { return Ok(None) };
        let e = &tail[i..];
        let eocd_pos = tail_at + i as u64;
        let (disk, cd_disk) = (le16(e, 4), le16(e, 6));
        let (n_here, n) = (le16(e, 8), le16(e, 10));
        let cd_size = le32(e, 12) as u64;
        let cd_offset = le32(e, 16) as u64;
        if n == u16::MAX || cd_size == u32::MAX as u64 || cd_offset == u32::MAX as u64 {
            return Err(invalid("a zip64 archive, which kui does not read"));
        }
        if disk != 0 || cd_disk != 0 || n_here != n {
            return Err(invalid("a multi-part zip, which kui does not read"));
        }
        let comment = e[EOCD_LEN..].to_vec();
        // Offsets are absolute when the writer knew what came first (our
        // fused files) and relative to the zip's own start when it was
        // `cat`-ed on; in both cases the central directory sits right
        // before the end record, which says where the zip really starts.
        let cd_pos = eocd_pos
            .checked_sub(cd_size)
            .ok_or_else(|| invalid("the zip directory is larger than the file"))?;
        let shift = cd_pos
            .checked_sub(cd_offset)
            .ok_or_else(|| invalid("the zip directory points before the file"))?;
        let cd = src.read_at(len, cd_pos, cd_size as usize)?;
        let mut entries = Vec::with_capacity(n as usize);
        let mut index = HashMap::with_capacity(n as usize);
        let mut p = 0usize;
        for _ in 0..n {
            if p + CENTRAL_LEN > cd.len() || le32(&cd, p) != CENTRAL_SIG {
                return Err(invalid("the zip directory is damaged"));
            }
            let made_by = le16(&cd, p + 4) >> 8;
            let flags = le16(&cd, p + 8);
            let method = le16(&cd, p + 10);
            let crc = le32(&cd, p + 16);
            let csize = le32(&cd, p + 20) as u64;
            let size = le32(&cd, p + 24) as u64;
            let (nl, xl, cl) = (
                le16(&cd, p + 28) as usize,
                le16(&cd, p + 30) as usize,
                le16(&cd, p + 32) as usize,
            );
            let external = le32(&cd, p + 38);
            let local = le32(&cd, p + 42) as u64;
            let end = p + CENTRAL_LEN + nl + xl + cl;
            if end > cd.len() {
                return Err(invalid("the zip directory is damaged"));
            }
            let raw = &cd[p + CENTRAL_LEN..p + CENTRAL_LEN + nl];
            let name = String::from_utf8(raw.to_vec())
                .unwrap_or_else(|_| String::from_utf8_lossy(raw).into_owned());
            let unix = if made_by == HOST_UNIX {
                external >> 16
            } else {
                0
            };
            let dir = name.ends_with('/') || unix & S_IFMT == S_IFDIR || external & DOS_DIR != 0;
            let mode = match unix & 0o7777 {
                0 if dir => 0o755,
                0 => 0o644,
                m => m,
            };
            if csize == u32::MAX as u64 || size == u32::MAX as u64 || local == u32::MAX as u64 {
                return Err(invalid(format!(
                    "{name} needs zip64, which kui does not read"
                )));
            }
            index.entry(name.clone()).or_insert(entries.len());
            entries.push(Meta {
                name,
                method,
                flags,
                crc,
                csize,
                size,
                local: local + shift,
                mode,
                dir,
            });
            p = end;
        }
        Ok(Some(ZipReader {
            src,
            len,
            entries,
            index,
            comment,
            start: shift,
        }))
    }

    /// The member names, in directory order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|m| m.name.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The member named exactly `name` (folders end in `/`).
    pub fn get(&self, name: &str) -> Option<Entry<'_>> {
        self.index.get(name).map(|&i| Entry {
            zip: self,
            meta: &self.entries[i],
        })
    }

    /// Every member, in directory order.
    pub fn entries(&self) -> impl Iterator<Item = Entry<'_>> {
        self.entries
            .iter()
            .map(move |meta| Entry { zip: self, meta })
    }

    pub fn comment(&self) -> &[u8] {
        &self.comment
    }

    /// How many bytes come before the zip's own offsets begin: 0 for a
    /// zip whose offsets are absolute (a `.kui`, or a runner packed by
    /// `kui pack`), the prefix's length for one `cat`-ed onto a binary.
    pub fn shift(&self) -> u64 {
        self.start
    }
}

impl Entry<'_> {
    pub fn name(&self) -> &str {
        &self.meta.name
    }

    /// The permission bits (0o644 when the zip carries no Unix mode).
    pub fn mode(&self) -> u32 {
        self.meta.mode
    }

    pub fn is_dir(&self) -> bool {
        self.meta.dir
    }

    /// The uncompressed size the directory claims.
    pub fn size(&self) -> u64 {
        self.meta.size
    }

    /// The member's bytes, inflated and checked against its CRC.
    pub fn read(&self) -> io::Result<Vec<u8>> {
        let m = self.meta;
        if m.flags & 1 != 0 {
            return Err(invalid(format!("{} is encrypted", m.name)));
        }
        let zip = self.zip;
        let h = zip.src.read_at(zip.len, m.local, LOCAL_LEN)?;
        if le32(&h, 0) != LOCAL_SIG {
            return Err(invalid(format!(
                "{}: no local header where the directory points",
                m.name
            )));
        }
        let skip = (LOCAL_LEN + le16(&h, 26) as usize + le16(&h, 28) as usize) as u64;
        let raw = zip.src.read_at(zip.len, m.local + skip, m.csize as usize)?;
        let data = match m.method {
            0 => raw,
            8 => {
                // Cap the allocation by what the directory claims, and read
                // one byte past it to catch a member that inflates larger.
                let mut out =
                    Vec::with_capacity((m.size as usize).min(raw.len().saturating_mul(16)));
                flate2::read::DeflateDecoder::new(&raw[..])
                    .take(m.size + 1)
                    .read_to_end(&mut out)?;
                out
            }
            other => {
                return Err(invalid(format!(
                    "{}: compression method {other} is not store or deflate",
                    m.name
                )));
            }
        };
        if data.len() as u64 != m.size {
            return Err(invalid(format!(
                "{}: {} bytes where the directory says {}",
                m.name,
                data.len(),
                m.size
            )));
        }
        if crc32fast::hash(&data) != m.crc {
            return Err(invalid(format!("{}: CRC mismatch", m.name)));
        }
        Ok(data)
    }
}

// ---- tar + gzip ---------------------------------------------------------

const BLOCK: usize = 512;

fn octal(field: &mut [u8], v: u64) -> io::Result<()> {
    let digits = field.len() - 1;
    let s = format!("{v:0digits$o}");
    if s.len() > digits {
        return Err(invalid(format!("{v} does not fit a tar header field")));
    }
    field[..digits].copy_from_slice(s.as_bytes());
    field[digits] = 0;
    Ok(())
}

fn parse_octal(field: &[u8]) -> io::Result<u64> {
    if field.first().is_some_and(|b| b & 0x80 != 0) {
        return Err(invalid("a base-256 tar number, which kui does not read"));
    }
    let s: String = field
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as char)
        .collect();
    let s = s.trim();
    if s.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(s, 8).map_err(|_| invalid(format!("bad tar number {s:?}")))
}

/// Splits a long name into ustar's prefix (≤155) and name (≤100) at a `/`.
fn split_ustar(name: &str) -> Option<(&str, &str)> {
    if name.len() <= 100 {
        return Some(("", name));
    }
    let b = name.as_bytes();
    (0..b.len()).filter(|&i| b[i] == b'/').find_map(|i| {
        let (pre, rest) = (&name[..i], &name[i + 1..]);
        (pre.len() <= 155 && rest.len() <= 100 && !rest.is_empty()).then_some((pre, rest))
    })
}

/// A gzipped ustar archive of `entries` (name, data, permission bits). A
/// name ending in `/` is a folder. Owner is 0:0 and every mtime is 1980-01-01,
/// so the same input gives the same bytes.
pub fn write_tar_gz(entries: &[(&str, &[u8], u32)]) -> io::Result<Vec<u8>> {
    let mut tar = Vec::new();
    for &(name, data, mode) in entries {
        if !is_safe_name(name) {
            return Err(invalid(format!("{name:?} is not a relative archive path")));
        }
        let dir = name.ends_with('/');
        let (prefix, short) = split_ustar(name)
            .ok_or_else(|| invalid(format!("{name} is too long for a ustar header")))?;
        let mut h = [0u8; BLOCK];
        h[..short.len()].copy_from_slice(short.as_bytes());
        octal(&mut h[100..108], (mode & 0o7777) as u64)?;
        octal(&mut h[108..116], 0)?;
        octal(&mut h[116..124], 0)?;
        octal(&mut h[124..136], if dir { 0 } else { data.len() as u64 })?;
        octal(&mut h[136..148], EPOCH_1980)?;
        h[156] = if dir { b'5' } else { b'0' };
        h[257..263].copy_from_slice(b"ustar\0");
        h[263..265].copy_from_slice(b"00");
        octal(&mut h[329..337], 0)?;
        octal(&mut h[337..345], 0)?;
        h[345..345 + prefix.len()].copy_from_slice(prefix.as_bytes());
        h[148..156].fill(b' ');
        let sum: u32 = h.iter().map(|&b| b as u32).sum();
        let s = format!("{sum:06o}\0 ");
        h[148..156].copy_from_slice(s.as_bytes());
        tar.extend_from_slice(&h);
        if !dir {
            tar.extend_from_slice(data);
            tar.resize(tar.len().div_ceil(BLOCK) * BLOCK, 0);
        }
    }
    tar.resize(tar.len() + 2 * BLOCK, 0);
    let mut gz = flate2::GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::default());
    gz.write_all(&tar)?;
    gz.finish()
}

/// The regular files and folders of a gzipped tar, as (name, data, mode);
/// folders end in `/` with no data. Long names come from the ustar prefix,
/// a pax `path` record or a GNU long-name entry. Links and devices are an
/// error rather than skipped, so an archive is never unpacked half-right;
/// so is a name that is absolute or climbs out with `..`.
pub fn read_tar_gz(bytes: &[u8]) -> io::Result<Vec<(String, Vec<u8>, u32)>> {
    let mut tar = Vec::new();
    flate2::read::MultiGzDecoder::new(bytes).read_to_end(&mut tar)?;
    let mut out = Vec::new();
    let mut p = 0usize;
    let mut long_name: Option<String> = None;
    loop {
        if p + BLOCK > tar.len() {
            return Err(invalid("the tar ends without its end blocks"));
        }
        let h = &tar[p..p + BLOCK];
        if h.iter().all(|&b| b == 0) {
            break;
        }
        let want = parse_octal(&h[148..156])?;
        let sum: u64 = h
            .iter()
            .enumerate()
            .map(|(i, &b)| {
                if (148..156).contains(&i) {
                    32
                } else {
                    b as u64
                }
            })
            .sum();
        if sum != want {
            return Err(invalid("a tar header's checksum does not match"));
        }
        let size = parse_octal(&h[124..136])? as usize;
        let data_at = p + BLOCK;
        let data_end = data_at
            .checked_add(size)
            .filter(|&e| e <= tar.len())
            .ok_or_else(|| invalid("the tar is cut short"))?;
        let data = &tar[data_at..data_end];
        p = data_at + size.div_ceil(BLOCK) * BLOCK;
        let cstr = |f: &[u8]| {
            String::from_utf8_lossy(&f[..f.iter().position(|&b| b == 0).unwrap_or(f.len())])
                .into_owned()
        };
        let kind = h[156];
        match kind {
            b'x' => {
                long_name = pax_path(data).or(long_name);
                continue;
            }
            b'g' => continue,
            b'L' => {
                long_name = Some(cstr(data));
                continue;
            }
            _ => {}
        }
        let name = match long_name.take() {
            Some(n) => n,
            None => {
                let short = cstr(&h[..100]);
                // POSIX ustar only: GNU's "ustar  " keeps times where the prefix goes.
                let prefix = if &h[257..263] == b"ustar\0" {
                    cstr(&h[345..500])
                } else {
                    String::new()
                };
                if prefix.is_empty() {
                    short
                } else {
                    format!("{prefix}/{short}")
                }
            }
        };
        let name = name.strip_prefix("./").map(str::to_string).unwrap_or(name);
        if name.is_empty() || name == "." {
            continue;
        }
        if !is_safe_name(&name) {
            return Err(invalid(format!(
                "tar entry {name:?} would land outside the folder"
            )));
        }
        let mode = (parse_octal(&h[100..108])? & 0o7777) as u32;
        match kind {
            b'0' | 0 | b'7' => out.push((name, data.to_vec(), mode)),
            b'5' => {
                let name = if name.ends_with('/') {
                    name
                } else {
                    format!("{name}/")
                };
                out.push((name, Vec::new(), mode));
            }
            b'1' | b'2' => {
                return Err(invalid(format!(
                    "tar entry {name} is a link, which kui does not unpack"
                )));
            }
            other => {
                return Err(invalid(format!(
                    "tar entry {name} has type {:?}, which kui does not unpack",
                    other as char
                )));
            }
        }
    }
    Ok(out)
}

/// The `path` record of a pax extended header, if it has one.
fn pax_path(data: &[u8]) -> Option<String> {
    let mut rest = data;
    let mut path = None;
    while !rest.is_empty() {
        let sp = rest.iter().position(|&b| b == b' ')?;
        let len: usize = std::str::from_utf8(&rest[..sp]).ok()?.parse().ok()?;
        if len <= sp || len > rest.len() {
            return None;
        }
        let rec = &rest[sp + 1..len];
        let rec = rec.strip_suffix(b"\n").unwrap_or(rec);
        if let Some(v) = rec.strip_prefix(b"path=") {
            path = Some(String::from_utf8_lossy(v).into_owned());
        }
        rest = &rest[len..];
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kui-archive-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    fn sample() -> ZipWriter {
        let mut z = ZipWriter::new();
        z.add_dir("app").unwrap();
        z.add_file(
            "app/main.lua",
            "return { view = function() end }\n".repeat(50).as_bytes(),
            0o644,
            true,
        )
        .unwrap();
        z.add_file("app/empty.txt", b"", 0o644, true).unwrap();
        z.add_file("app/stored.bin", &[1, 2, 3, 4, 5], 0o644, false)
            .unwrap();
        z.add_file("app/bin/tool", b"#!/bin/sh\necho hi\n", 0o755, true)
            .unwrap();
        z.add_file(
            "app/ünïcødé/файл.lua",
            "-- привет\n".as_bytes(),
            0o644,
            true,
        )
        .unwrap();
        z.add_dir("app/assets/").unwrap();
        z
    }

    fn check_sample(r: &ZipReader) {
        let names: Vec<_> = r.names().collect();
        assert_eq!(
            names,
            [
                "app/",
                "app/main.lua",
                "app/empty.txt",
                "app/stored.bin",
                "app/bin/tool",
                "app/ünïcødé/файл.lua",
                "app/assets/"
            ]
        );
        assert!(r.get("app/").unwrap().is_dir());
        assert!(r.get("app/assets/").unwrap().is_dir());
        assert_eq!(
            r.get("app/main.lua").unwrap().read().unwrap(),
            "return { view = function() end }\n".repeat(50).as_bytes()
        );
        assert_eq!(r.get("app/empty.txt").unwrap().read().unwrap(), b"");
        assert_eq!(
            r.get("app/stored.bin").unwrap().read().unwrap(),
            [1, 2, 3, 4, 5]
        );
        assert_eq!(r.get("app/bin/tool").unwrap().mode(), 0o755);
        assert_eq!(r.get("app/main.lua").unwrap().mode(), 0o644);
        assert!(!r.get("app/main.lua").unwrap().is_dir());
        assert_eq!(
            r.get("app/ünïcødé/файл.lua").unwrap().read().unwrap(),
            "-- привет\n".as_bytes()
        );
        assert!(r.get("missing").is_none());
    }

    #[test]
    fn zip_round_trip() {
        let bytes = sample().finish(0, b"kui pack").unwrap();
        let r = ZipReader::open(bytes.into()).unwrap();
        check_sample(&r);
        assert_eq!(r.comment(), b"kui pack");
        assert_eq!(r.shift(), 0);
    }

    #[test]
    fn zip_is_reproducible_and_deflates() {
        let a = sample().finish(0, b"").unwrap();
        let b = sample().finish(0, b"").unwrap();
        assert_eq!(a, b);
        // The repeated main.lua deflates; the 5-byte file is stored.
        let r = ZipReader::open(a.into()).unwrap();
        assert_eq!(r.entries[1].method, 8);
        assert_eq!(r.entries[3].method, 0);
        // Incompressible data asked to deflate is stored anyway.
        let mut z = ZipWriter::new();
        let noise: Vec<u8> = (0..4096u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect();
        z.add_file("noise", &noise, 0o644, true).unwrap();
        let r = ZipReader::open(z.finish(0, b"").unwrap().into()).unwrap();
        assert_eq!(r.get("noise").unwrap().read().unwrap(), noise);
    }

    #[test]
    fn dos_time() {
        assert_eq!(dos_datetime(0), (0, 1 << 5 | 1));
        // 2026-10-25 13:45:30 UTC
        let (t, d) = dos_datetime(1_792_935_930);
        assert_eq!((d >> 9) + 1980, 2026);
        assert_eq!((d >> 5) & 15, 10);
        assert_eq!(d & 31, 25);
        assert_eq!((t >> 11, (t >> 5) & 63, (t & 31) * 2), (13, 45, 30));
    }

    #[test]
    fn writer_rejects_bad_names() {
        let mut z = ZipWriter::new();
        assert!(z.add_file("/abs", b"", 0o644, false).is_err());
        assert!(z.add_file("a/../b", b"", 0o644, false).is_err());
        assert!(z.add_file("C:x", b"", 0o644, false).is_err());
        z.add_file("a", b"", 0o644, false).unwrap();
        assert!(z.add_file("a", b"", 0o644, false).is_err());
    }

    #[test]
    fn appended_with_shifted_offsets() {
        let prefix: Vec<u8> = (0..10_000u32).map(|i| (i * 7) as u8).collect();
        let mut fused = prefix.clone();
        fused.extend(sample().finish(prefix.len() as u64, b"").unwrap());
        let r = ZipReader::open(fused.into()).unwrap();
        check_sample(&r);
        assert_eq!(r.shift(), 0);
    }

    #[test]
    fn appended_without_shifting() {
        let prefix = vec![0x90u8; 777];
        let mut cat = prefix.clone();
        cat.extend(sample().finish(0, b"").unwrap());
        let r = ZipReader::open(cat.into()).unwrap();
        check_sample(&r);
        assert_eq!(r.shift(), 777);
    }

    #[test]
    fn find_appended_on_files() {
        let plain = scratch("plain.bin");
        let body: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&plain, &body).unwrap();
        assert!(ZipReader::find_appended(&plain).unwrap().is_none());
        let tiny = scratch("tiny.bin");
        std::fs::write(&tiny, b"MZ").unwrap();
        assert!(ZipReader::find_appended(&tiny).unwrap().is_none());

        let fused = scratch("fused.exe");
        let mut bytes = body.clone();
        bytes.extend(sample().finish(body.len() as u64, b"").unwrap());
        std::fs::write(&fused, &bytes).unwrap();
        let r = ZipReader::find_appended(&fused).unwrap().unwrap();
        check_sample(&r);
    }

    #[test]
    fn damaged_zips_error_without_panicking() {
        let bytes = sample().finish(0, b"c").unwrap();
        // Every truncation either errors or (when only the end is missing)
        // finds no zip; none panics.
        for cut in 0..bytes.len() {
            let r = ZipReader::open(bytes[..cut].to_vec().into());
            assert!(r.is_err(), "cut at {cut} opened");
        }
        // Flipping bytes may open or not, but reading never panics.
        for i in 0..bytes.len() {
            let mut b = bytes.clone();
            b[i] ^= 0xa5;
            if let Ok(r) = ZipReader::open(b.into()) {
                for e in r.entries() {
                    let _ = e.read();
                }
            }
        }
    }

    #[test]
    fn unzip_accepts_a_fused_file() {
        let unzip = Path::new("/usr/bin/unzip");
        if !unzip.exists() {
            eprintln!("skipped: no /usr/bin/unzip");
            return;
        }
        let fused = scratch("fused-unzip.exe");
        let runner = vec![0xccu8; 65_536];
        let mut bytes = runner.clone();
        bytes.extend(sample().finish(runner.len() as u64, b"").unwrap());
        std::fs::write(&fused, &bytes).unwrap();
        for flag in ["-l", "-t"] {
            let out = Command::new(unzip).arg(flag).arg(&fused).output().unwrap();
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(out.status.success(), "unzip {flag}: {text}");
            assert!(!text.contains("extra bytes"), "unzip {flag}: {text}");
            assert!(text.contains("app/bin/tool"), "unzip {flag}: {text}");
        }
        // Extracting restores the executable bit.
        let dest = scratch("unzipped");
        let _ = std::fs::remove_dir_all(&dest);
        let out = Command::new(unzip)
            .arg("-q")
            .arg(&fused)
            .arg("-d")
            .arg(&dest)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dest.join("app/bin/tool"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o755);
        }
        assert_eq!(
            std::fs::read(dest.join("app/stored.bin")).unwrap(),
            [1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn tar_round_trip() {
        let long = format!("{}/{}", "d".repeat(120), "f".repeat(90));
        let entries: Vec<(&str, &[u8], u32)> = vec![
            ("kui/", b"", 0o755),
            ("kui/kui", b"\x7fELF runner", 0o755),
            ("kui/README.md", b"read me\n", 0o644),
            ("kui/empty", b"", 0o644),
            ("kui/ünï.txt", "ü".as_bytes(), 0o644),
            (&long, b"long", 0o600),
        ];
        let gz = write_tar_gz(&entries).unwrap();
        assert_eq!(gz, write_tar_gz(&entries).unwrap());
        let back = read_tar_gz(&gz).unwrap();
        assert_eq!(back.len(), entries.len());
        for ((n, d, m), (bn, bd, bm)) in entries.iter().zip(&back) {
            assert_eq!((*n, *d, *m), (bn.as_str(), bd.as_slice(), *bm));
        }
        assert!(write_tar_gz(&[("../x", b"", 0o644)]).is_err());
        assert!(write_tar_gz(&[(&"x".repeat(300), b"", 0o644)]).is_err());
    }

    #[test]
    fn tar_reader_rejects_escapes_and_damage() {
        // Hand-write a header naming ../evil with a valid checksum.
        let mut h = [0u8; BLOCK];
        h[..7].copy_from_slice(b"../evil");
        octal(&mut h[100..108], 0o644).unwrap();
        octal(&mut h[124..136], 0).unwrap();
        h[156] = b'0';
        h[148..156].fill(b' ');
        let sum: u32 = h.iter().map(|&b| b as u32).sum();
        h[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
        let mut tar = h.to_vec();
        tar.resize(tar.len() + 2 * BLOCK, 0);
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), Compression::default());
        gz.write_all(&tar).unwrap();
        let err = read_tar_gz(&gz.finish().unwrap()).unwrap_err();
        assert!(err.to_string().contains("outside"), "{err}");

        let good = write_tar_gz(&[("a", b"hello", 0o644)]).unwrap();
        for cut in 0..good.len() {
            assert!(read_tar_gz(&good[..cut]).is_err(), "cut at {cut}");
        }
        assert!(read_tar_gz(b"not gzip").is_err());
    }

    #[test]
    fn system_tar_agrees() {
        let tar = Path::new("/usr/bin/tar");
        if !tar.exists() {
            eprintln!("skipped: no /usr/bin/tar");
            return;
        }
        let ours = scratch("ours.tar.gz");
        std::fs::write(
            &ours,
            write_tar_gz(&[("pkg/", b"", 0o755), ("pkg/run", b"x", 0o755)]).unwrap(),
        )
        .unwrap();
        let out = Command::new(tar).arg("-tzf").arg(&ours).output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let listed = String::from_utf8_lossy(&out.stdout);
        assert_eq!(listed.lines().collect::<Vec<_>>(), ["pkg/", "pkg/run"]);

        let src = scratch("tar-src");
        let _ = std::fs::remove_dir_all(&src);
        std::fs::create_dir_all(src.join("pkg/sub")).unwrap();
        std::fs::write(src.join("pkg/sub/file.txt"), b"from tar").unwrap();
        let deep = format!("pkg/{}/{}", "n".repeat(110), "m".repeat(60));
        std::fs::create_dir_all(src.join(&deep).parent().unwrap()).unwrap();
        std::fs::write(src.join(&deep), b"deep").unwrap();
        let theirs = scratch("theirs.tar.gz");
        let out = Command::new(tar)
            .env("COPYFILE_DISABLE", "1")
            .arg("-czf")
            .arg(&theirs)
            .arg("-C")
            .arg(&src)
            .arg("pkg")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let back = read_tar_gz(&std::fs::read(&theirs).unwrap()).unwrap();
        let find = |n: &str| back.iter().find(|e| e.0 == n).map(|e| e.1.clone());
        assert_eq!(find("pkg/sub/file.txt").unwrap(), b"from tar");
        assert_eq!(find(&deep).unwrap(), b"deep");
        assert!(back.iter().any(|e| e.0 == "pkg/sub/"));
    }

    #[test]
    fn sha256() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
