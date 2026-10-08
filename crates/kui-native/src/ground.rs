//! The `Tinted` backdrop kui draws itself where the OS has no material to
//! put behind a window (backlog F126): the desktop's wallpaper, read from
//! where the desktop keeps it, decoded, scaled down to a few hundred
//! pixels and blurred once — on a thread of its own, never on a frame —
//! then drawn as the window's ground under everything the frame paints
//! (`Renderer::set_ground`), the part of it the window covers on screen.
//!
//! The GPU stretches the small picture across the window with linear
//! filtering, which is the rest of the blur: a 160-pixel wallpaper over a
//! 1000-pixel window is soft the way Mica is, and costs one texture and one
//! quad a frame. Where the window is on the screen is known on Windows and
//! X11 and the ground follows it as it moves; on Wayland it is not, and the
//! ground sits as if the window were centred on its monitor — at this blur
//! the difference reads as none.
//!
//! Decoded once per wallpaper: the result is kept against the file's path
//! and modification time, so every window, and a window reopened after its
//! device was lost, takes the same picture.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// A wallpaper ready to draw: RGBA, row by row from the top left, small
/// and already blurred.
#[derive(Debug)]
pub(crate) struct Wallpaper {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// The long side a wallpaper is scaled down to before it is blurred.
const SIDE: u32 = 160;
/// The blur's sigma at that size, in its pixels: about a twentieth of the
/// picture, which over a window is the softness of Mica.
const SIGMA: f32 = 6.0;

/// What a window's ground is while it loads and after.
pub(crate) type Slot = Arc<Mutex<Option<Result<Arc<Wallpaper>, String>>>>;

/// Where a window's wallpaper comes from: a file the event loop already
/// named, or one the thread has yet to find.
#[derive(Debug, PartialEq)]
pub(crate) enum Source {
    /// The desktop is asked on the thread: GNOME's `gsettings`, up to
    /// three processes, which the loop must not wait on before the window
    /// shows.
    Find,
    /// The file, found on the loop where finding it is one call.
    Load(PathBuf),
}

/// The wallpaper's file where naming it is a call and not a process —
/// Windows' `SystemParametersInfoW`, Plasma's config file — so a window
/// with none to draw is `Opaque` from its first frame rather than
/// `Tinted` until the thread says so (backlog RG154): `Some(None)` for
/// none (macOS, where kui never reads one, and a desktop with none set),
/// `Some(Some(path))` for the file, and `None` where only the thread can
/// tell (GNOME and every desktop asked through its keys).
pub(crate) fn known_path() -> Option<Option<PathBuf>> {
    platform::known_path().map(|p| p.filter(|p| p.is_file()))
}

/// Starts loading the desktop's wallpaper — finding it first, for
/// [`Source::Find`] — on a thread of its own and returns where the answer
/// lands; `wake` is called once it has, so the loop draws the frame that
/// shows it, or, with no wallpaper to read or one that will not decode,
/// the frame that says the window is opaque.
pub(crate) fn spawn(source: Source, wake: impl FnOnce() + Send + 'static) -> Slot {
    let slot: Slot = Arc::new(Mutex::new(None));
    let out = slot.clone();
    let spawned = std::thread::Builder::new()
        .name("kui-wallpaper".into())
        .spawn(move || {
            let path = match source {
                Source::Load(path) => Some(path),
                Source::Find => wallpaper_path(),
            };
            let got = path
                .ok_or_else(|| "the desktop names no wallpaper file kui can read".to_string())
                .and_then(|path| load(&path));
            *out.lock().unwrap_or_else(|e| e.into_inner()) = Some(got);
            wake();
        });
    if let Err(e) = spawned {
        *slot.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(Err(format!("cannot start the wallpaper thread: {e}")));
    }
    slot
}

/// A decoded wallpaper with the file it came from: path and modification
/// time.
type Cached = (PathBuf, Option<SystemTime>, Arc<Wallpaper>);

/// The last wallpaper decoded.
static CACHE: Mutex<Option<Cached>> = Mutex::new(None);

/// The wallpaper at `path`, from the cache when the file has not changed
/// since it was decoded, else decoded, scaled down and blurred.
pub(crate) fn load(path: &Path) -> Result<Arc<Wallpaper>, String> {
    let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    if let Some((p, t, w)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
        && p == path
        && *t == mtime
    {
        return Ok(w.clone());
    }
    let img = image::ImageReader::open(path)
        .and_then(|r| r.with_guessed_format())
        .map_err(|e| format!("{}: {e}", path.display()))?
        .decode()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let wallpaper = Arc::new(soften(img.to_rgba8()));
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) =
        Some((path.to_path_buf(), mtime, wallpaper.clone()));
    Ok(wallpaper)
}

/// Scales `img` down to [`SIDE`] on its long side and blurs it.
pub(crate) fn soften(img: image::RgbaImage) -> Wallpaper {
    let (w, h) = img.dimensions();
    let scale = SIDE as f32 / w.max(h).max(1) as f32;
    let (tw, th) = if scale < 1.0 {
        (
            ((w as f32 * scale).round() as u32).max(1),
            ((h as f32 * scale).round() as u32).max(1),
        )
    } else {
        (w.max(1), h.max(1))
    };
    let small = image::imageops::thumbnail(&img, tw, th);
    let soft = image::imageops::blur(&small, SIGMA);
    Wallpaper {
        width: soft.width(),
        height: soft.height(),
        rgba: soft.into_raw(),
    }
}

/// One window's ground: the wallpaper as it loads, and whether its
/// renderer has it and shows the right part of it.
pub(crate) struct Ground {
    slot: Slot,
    wallpaper: Option<Arc<Wallpaper>>,
    /// The renderer holds the picture.
    uploaded: bool,
    /// The window moved or changed size since the part shown was set.
    moved: bool,
}

impl Ground {
    pub(crate) fn new(slot: Slot) -> Self {
        Ground {
            slot,
            wallpaper: None,
            uploaded: false,
            moved: true,
        }
    }

    /// The window moved or was resized: the part of the wallpaper behind
    /// it moved with it.
    pub(crate) fn moved(&mut self) {
        self.moved = true;
    }

    /// The window's renderer was made again (a device lost and reopened):
    /// it has no picture yet.
    pub(crate) fn renderer_replaced(&mut self) {
        self.uploaded = false;
        self.moved = true;
    }

    /// Before a frame's view runs: takes the thread's answer once it has
    /// landed. `Err` once, with why, when there was no wallpaper to read
    /// or it would not decode — the window is opaque from then on, and
    /// the view of the frame this is called for already reads so. A lock
    /// while loading, a flag after.
    pub(crate) fn settle(&mut self) -> Result<(), String> {
        if self.wallpaper.is_none() {
            let got = self.slot.lock().unwrap_or_else(|e| e.into_inner()).take();
            match got {
                None => {}
                Some(Err(e)) => return Err(e),
                Some(Ok(w)) => self.wallpaper = Some(w),
            }
        }
        Ok(())
    }

    /// Before a frame is drawn: hands the renderer the picture once it
    /// has loaded and the part of it behind `window` when that changed.
    /// Cheap on a frame with nothing to do: two flags.
    pub(crate) fn prepare(
        &mut self,
        window: &winit::window::Window,
        renderer: &mut kui_wgpu::Renderer,
    ) {
        let Some(w) = &self.wallpaper else {
            return;
        };
        if !self.uploaded {
            renderer.set_ground(&w.rgba, w.width, w.height);
            self.uploaded = true;
            self.moved = true;
        }
        if self.moved {
            self.moved = false;
            let size = window.inner_size();
            let position = window
                .inner_position()
                .ok()
                .map(|p| (p.x as f64, p.y as f64));
            let monitor = window.current_monitor().map(|m| {
                let (p, s) = (m.position(), m.size());
                (p.x as f64, p.y as f64, s.width as f64, s.height as f64)
            });
            renderer.set_ground_uv(uv(
                (size.width as f64, size.height as f64),
                position,
                monitor,
                (w.width, w.height),
            ));
        }
    }
}

/// A rectangle in physical pixels on the desktop: `(x, y, w, h)`.
pub(crate) type ScreenRect = (f64, f64, f64, f64);

/// The part of a `(iw, ih)` picture a window shows, as `[u0, v0, u1, v1]`:
/// the picture filling `monitor` the way a desktop fills a screen (scaled
/// to cover it, centred, cropped), and `window` cut out of that. A window
/// whose position is not known (`None`, Wayland) is placed centred on the
/// monitor; with no monitor either, the window shows the picture's middle
/// at the monitor's size, which is the window's own.
pub(crate) fn uv(
    (ww, wh): (f64, f64),
    position: Option<(f64, f64)>,
    monitor: Option<ScreenRect>,
    (iw, ih): (u32, u32),
) -> [f32; 4] {
    let (mx, my, mw, mh) = monitor.unwrap_or((0.0, 0.0, ww.max(1.0), wh.max(1.0)));
    let (wx, wy) = position.unwrap_or((mx + (mw - ww) / 2.0, my + (mh - wh) / 2.0));
    let (iw, ih) = (iw.max(1) as f64, ih.max(1) as f64);
    let s = (mw / iw).max(mh / ih).max(f64::MIN_POSITIVE);
    let (dw, dh) = (iw * s, ih * s);
    let (ox, oy) = (mx + (mw - dw) / 2.0, my + (mh - dh) / 2.0);
    [
        ((wx - ox) / dw) as f32,
        ((wy - oy) / dh) as f32,
        ((wx + ww - ox) / dw) as f32,
        ((wy + wh - oy) / dh) as f32,
    ]
}

/// Where the desktop keeps its wallpaper, if this platform says.
pub(crate) fn wallpaper_path() -> Option<PathBuf> {
    platform::wallpaper_path().filter(|p| p.is_file())
}

#[cfg(target_os = "windows")]
mod platform {
    use std::path::PathBuf;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETDESKWALLPAPER, SystemParametersInfoW,
    };

    /// One call, so the loop asks.
    pub(super) fn known_path() -> Option<Option<PathBuf>> {
        Some(wallpaper_path())
    }

    /// `SPI_GETDESKWALLPAPER`: the path Explorer draws from — often
    /// `TranscodedWallpaper`, a JPEG with no extension, which the decoder
    /// knows by its bytes.
    pub(super) fn wallpaper_path() -> Option<PathBuf> {
        let mut buf = [0u16; 1024];
        // SAFETY: the buffer is as long as the length handed over.
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETDESKWALLPAPER,
                buf.len() as u32,
                buf.as_mut_ptr().cast(),
                0,
            )
        };
        if ok == 0 {
            return None;
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let s = String::from_utf16_lossy(&buf[..len]);
        (!s.is_empty()).then(|| PathBuf::from(s))
    }
}

/// macOS draws `Tinted` with AppKit's own material, so kui never reads the
/// wallpaper there (and the system's are HEIC, which the decoder does not
/// read): `KUI_BACKDROP_EMULATE` finds nothing and the ground is opaque.
#[cfg(target_os = "macos")]
mod platform {
    pub(super) fn known_path() -> Option<Option<std::path::PathBuf>> {
        Some(None)
    }

    pub(super) fn wallpaper_path() -> Option<std::path::PathBuf> {
        None
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    use std::path::PathBuf;

    fn is_plasma() -> bool {
        std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .split(':')
            .any(|d| d.eq_ignore_ascii_case("KDE"))
    }

    /// Plasma's is a file read, so the loop asks; every other desktop's
    /// is `gsettings`, the thread's.
    pub(super) fn known_path() -> Option<Option<PathBuf>> {
        is_plasma().then(plasma_path)
    }

    /// Plasma's from its own config, GNOME's (and every desktop that keeps
    /// GNOME's keys) from `gsettings`, by the colour scheme in force.
    pub(super) fn wallpaper_path() -> Option<PathBuf> {
        if is_plasma() {
            return plasma_path();
        }
        let gsettings = |schema: &str, key: &str| -> Option<String> {
            let out = std::process::Command::new("gsettings")
                .args(["get", schema, key])
                .output()
                .ok()?;
            out.status
                .success()
                .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        let dark = gsettings("org.gnome.desktop.interface", "color-scheme")
            .is_some_and(|s| s.contains("dark"));
        let key = if dark {
            "picture-uri-dark"
        } else {
            "picture-uri"
        };
        let uri = gsettings("org.gnome.desktop.background", key)
            .filter(|s| !super::unquote(s).is_empty())
            .or_else(|| gsettings("org.gnome.desktop.background", "picture-uri"))?;
        super::resolve(super::unquote(&uri))
    }

    /// The first `org.kde.image` wallpaper in Plasma's applets config.
    fn plasma_path() -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&home).join(".config"));
        let ini =
            std::fs::read_to_string(config.join("plasma-org.kde.plasma.desktop-appletsrc")).ok()?;
        super::plasma_image(&ini).and_then(|v| super::resolve(&v))
    }
}

/// A `gsettings get` string without its GVariant quotes.
#[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
pub(crate) fn unquote(s: &str) -> &str {
    let s = s.trim();
    s.strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
        .or_else(|| s.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
        .unwrap_or(s)
}

/// The `Image=` of the first `org.kde.image` wallpaper in Plasma's
/// `plasma-org.kde.plasma.desktop-appletsrc`.
#[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
pub(crate) fn plasma_image(ini: &str) -> Option<String> {
    let mut inside = false;
    for line in ini.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line.contains("[Wallpaper][org.kde.image][General]");
            continue;
        }
        if inside && let Some(v) = line.strip_prefix("Image=") {
            return Some(v.trim().to_string());
        }
    }
    None
}

/// A wallpaper setting made a file: `file://` and percent escapes undone;
/// a Plasma wallpaper package (a directory) is its largest picture under
/// `contents/images`; a GNOME slideshow (`.xml`) is its first `<file>`.
#[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
pub(crate) fn resolve(value: &str) -> Option<PathBuf> {
    let path = PathBuf::from(percent_decode(
        value.strip_prefix("file://").unwrap_or(value),
    ));
    if path.is_dir() {
        let images = path.join("contents").join("images");
        let mut best: Option<(u64, PathBuf)> = None;
        for entry in std::fs::read_dir(images).ok()?.flatten() {
            let p = entry.path();
            let name = p.file_stem()?.to_string_lossy().to_string();
            let area = name
                .split_once('x')
                .and_then(|(w, h)| Some(w.parse::<u64>().ok()? * h.parse::<u64>().ok()?))
                .unwrap_or(0);
            if best.as_ref().is_none_or(|(a, _)| area > *a) {
                best = Some((area, p));
            }
        }
        return best.map(|(_, p)| p);
    }
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
    {
        let xml = std::fs::read_to_string(&path).ok()?;
        return slideshow_first(&xml).map(PathBuf::from);
    }
    Some(path)
}

/// The first `<file>` of a GNOME background slideshow.
#[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
pub(crate) fn slideshow_first(xml: &str) -> Option<String> {
    let start = xml.find("<file>")? + "<file>".len();
    let end = start + xml[start..].find("</file>")?;
    Some(xml[start..end].trim().to_string())
}

/// `%20` and its kind undone; anything that is not an escape is kept.
#[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
pub(crate) fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(v) = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window filling the monitor shows the whole of a picture of the
    /// monitor's shape; half of it shows half.
    #[test]
    fn the_window_shows_the_part_of_the_wallpaper_behind_it() {
        let monitor = Some((0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(
            uv((1920.0, 1080.0), Some((0.0, 0.0)), monitor, (192, 108)),
            [0.0, 0.0, 1.0, 1.0]
        );
        let [u0, v0, u1, v1] = uv((960.0, 540.0), Some((960.0, 540.0)), monitor, (192, 108));
        assert_eq!([u0, v0, u1, v1], [0.5, 0.5, 1.0, 1.0]);
        // A second monitor to the right: the window's place is its own
        // monitor's.
        let right = Some((1920.0, 0.0, 1920.0, 1080.0));
        assert_eq!(
            uv((960.0, 1080.0), Some((1920.0, 0.0)), right, (192, 108)),
            [0.0, 0.0, 0.5, 1.0]
        );
    }

    /// A picture of another shape covers the monitor, centred and cropped,
    /// the way a desktop fills a screen.
    #[test]
    fn a_wallpaper_of_another_shape_covers_the_screen() {
        let monitor = Some((0.0, 0.0, 1000.0, 1000.0));
        // 2:1 over a square: scaled to the height, half the width cut off.
        let [u0, v0, u1, v1] = uv((1000.0, 1000.0), Some((0.0, 0.0)), monitor, (200, 100));
        assert_eq!([u0, v0, u1, v1], [0.25, 0.0, 0.75, 1.0]);
    }

    /// Where the window is not known (Wayland), it is taken as centred on
    /// its monitor; with no monitor either, as filling one of its size.
    #[test]
    fn a_window_with_no_position_is_taken_as_centred() {
        let monitor = Some((0.0, 0.0, 2000.0, 1000.0));
        assert_eq!(
            uv((1000.0, 500.0), None, monitor, (200, 100)),
            [0.25, 0.25, 0.75, 0.75]
        );
        assert_eq!(
            uv((400.0, 200.0), None, None, (200, 100)),
            [0.0, 0.0, 1.0, 1.0]
        );
    }

    /// Scaled to the long side and blurred: a picture with a bright
    /// block comes back small, the block's edge spread out.
    #[test]
    fn a_wallpaper_is_scaled_down_and_blurred() {
        let mut img = image::RgbaImage::from_pixel(1600, 900, image::Rgba([0, 0, 0, 255]));
        for y in 300..600 {
            for x in 600..1000 {
                img.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        let w = soften(img);
        assert_eq!((w.width, w.height), (SIDE, 90));
        assert_eq!(w.rgba.len(), (w.width * w.height * 4) as usize);
        let at = |x: u32, y: u32| w.rgba[((y * w.width + x) * 4) as usize];
        // The middle is light, the edge dark, and between them a ramp
        // rather than a step.
        assert!(at(80, 45) > 100, "{}", at(80, 45));
        assert!(at(5, 5) < 10);
        let ramp: Vec<u8> = (50..70).map(|x| at(x, 45)).collect();
        assert!(ramp.windows(2).all(|p| p[0] <= p[1]), "{ramp:?}");
        assert!(ramp[0] < ramp[ramp.len() - 1]);
    }

    /// The thread's answer is taken before a frame's view: nothing while
    /// it loads, `Err` once when there was no wallpaper (the window goes
    /// opaque), and the picture kept once it landed.
    #[test]
    fn the_answer_is_settled_before_the_view() {
        let slot: Slot = Arc::new(Mutex::new(None));
        let mut g = Ground::new(slot.clone());
        assert_eq!(g.settle(), Ok(()));
        assert!(g.wallpaper.is_none(), "still loading");
        *slot.lock().unwrap() = Some(Err("none".into()));
        assert_eq!(g.settle(), Err("none".into()));
        assert_eq!(g.settle(), Ok(()), "said once");

        let slot: Slot = Arc::new(Mutex::new(None));
        let mut g = Ground::new(slot.clone());
        let w = Arc::new(soften(image::RgbaImage::from_pixel(
            4,
            4,
            image::Rgba([1, 2, 3, 255]),
        )));
        *slot.lock().unwrap() = Some(Ok(w.clone()));
        assert_eq!(g.settle(), Ok(()));
        assert!(Arc::ptr_eq(g.wallpaper.as_ref().unwrap(), &w));
        assert_eq!(g.settle(), Ok(()));
    }

    /// Decoded once: a second load of the same unchanged file is the same
    /// picture, not another decode.
    #[test]
    fn a_wallpaper_is_decoded_once_per_file() {
        let dir = std::env::temp_dir().join(format!("kui-ground-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // No extension, as Windows' TranscodedWallpaper has none: the
        // decoder knows it by its bytes.
        let path = dir.join("TranscodedWallpaper");
        image::RgbaImage::from_pixel(64, 32, image::Rgba([10, 120, 200, 255]))
            .save_with_format(&path, image::ImageFormat::Png)
            .unwrap();
        let a = load(&path).unwrap();
        let b = load(&path).unwrap();
        assert!(Arc::ptr_eq(&a, &b), "the cache answered the second load");
        assert_eq!(
            (a.width, a.height),
            (64, 32),
            "small already: kept its size"
        );
        assert_eq!(&a.rgba[..3], &[10, 120, 200]);
        assert!(load(&dir.join("missing.jpg")).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The settings the Linux desktops keep, read: GNOME's quoted URI with
    /// escapes, Plasma's config, a slideshow's first picture.
    #[test]
    fn the_desktops_settings_are_read() {
        assert_eq!(
            unquote("'file:///usr/share/backgrounds/My%20Hills.jpg'\n"),
            "file:///usr/share/backgrounds/My%20Hills.jpg"
        );
        assert_eq!(
            percent_decode("/a/My%20Hills%2Cdark.jpg"),
            "/a/My Hills,dark.jpg"
        );
        assert_eq!(percent_decode("/a/100%"), "/a/100%");
        let ini = "[Containments][1][General]\nImage=nope\n\n\
                   [Containments][1][Wallpaper][org.kde.image][General]\n\
                   Image=file:///usr/share/wallpapers/Next/\nPreviewImage=x\n";
        assert_eq!(
            plasma_image(ini).as_deref(),
            Some("file:///usr/share/wallpapers/Next/")
        );
        assert_eq!(plasma_image("[General]\nImage=x\n"), None);
        let xml = "<background><static><duration>1</duration>\
                   <file> /usr/share/backgrounds/a.jpg </file></static></background>";
        assert_eq!(
            slideshow_first(xml).as_deref(),
            Some("/usr/share/backgrounds/a.jpg")
        );
        // A Plasma package: the largest picture it carries.
        let dir = std::env::temp_dir().join(format!("kui-pkg-{}", std::process::id()));
        let images = dir.join("contents").join("images");
        std::fs::create_dir_all(&images).unwrap();
        for name in ["1920x1080.png", "3840x2160.png", "1280x800.png"] {
            std::fs::write(images.join(name), b"").unwrap();
        }
        let got = resolve(&format!("file://{}", dir.display())).unwrap();
        assert_eq!(got.file_name().unwrap(), "3840x2160.png");
        std::fs::remove_dir_all(&dir).ok();
    }
}
