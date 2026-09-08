//! What this runner can ask the OS for itself: the accent colour, the
//! reduce-motion setting and the UI language — `env.system` minus the
//! appearance, which comes off the window (winit's `theme()`) because it
//! is per-window and already answered there.
//!
//! Every answer is optional in the strong sense: a platform with no way to
//! ask reports the same "cannot tell" a headless core does, and never a
//! plausible-looking guess. What each one can ask:
//!
//! | | macOS | Windows | other |
//! |---|---|---|---|
//! | accent | `NSColor.controlAccentColor` | `DwmGetColorizationColor` | — |
//! | motion | `NSWorkspace.accessibilityDisplayShouldReduceMotion` | `SPI_GETCLIENTAREAANIMATION` | — |
//! | locale | `NSLocale.preferredLanguages` | `GetUserDefaultLocaleName` | `LANG` |
//!
//! On the main thread, at startup and whenever the user has plainly been
//! somewhere else (an OS theme change, a window taking focus back). Not on
//! a thread of its own: the macOS half is AppKit, which would have to hop
//! back to the main thread anyway, and the whole query is a handful of
//! syscalls — **6.5 µs measured on an M3 Pro**, against a window that takes
//! milliseconds to open, so a thread would cost more to start than the work
//! it moved. The thread would be worth it for a query that
//! spawns a process or talks to a bus, which is what a Linux answer will
//! need when it arrives (`xdg-desktop-portal`'s `Settings`, or
//! `gsettings`); the seam for it is [`query`], and only its body moves.

use kui_core::{Color, Locale, MotionPref};

/// The three facts [`query`] answers. `Default` is "asked nobody", which
/// is also what a platform with no answer reports.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Queried {
    pub accent: Option<Color>,
    pub motion: MotionPref,
    pub locale: Option<Locale>,
}

/// Ask the OS. Cheap enough to call on a focus change; see the module doc
/// for what each platform can actually answer.
pub(crate) fn query() -> Queried {
    Queried {
        accent: accent(),
        motion: motion(),
        locale: locale(),
    }
}

// -- macOS ------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn accent() -> Option<Color> {
    use objc2_app_kit::{NSColor, NSColorSpace};
    // `controlAccentColor` is a *dynamic* colour — it has no components
    // until it is resolved — so it is converted to sRGB before being read.
    // The conversion is what makes the components exist; without it the
    // reads raise. A colour that cannot convert (it never happens for this
    // one, but the API says it can) is no answer rather than a wrong one.
    let srgb =
        NSColor::controlAccentColor().colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())?;
    let ch = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(Color::rgba8(
        ch(srgb.redComponent()),
        ch(srgb.greenComponent()),
        ch(srgb.blueComponent()),
        ch(srgb.alphaComponent()),
    ))
}

#[cfg(target_os = "macos")]
fn motion() -> MotionPref {
    // "Reduce motion" in System Settings › Accessibility › Display.
    if objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion() {
        MotionPref::Reduced
    } else {
        MotionPref::Full
    }
}

#[cfg(target_os = "macos")]
fn locale() -> Option<Locale> {
    // The user's language list, already in the tag form a view wants
    // ("en-GB", "pt-BR"); the first entry is the one the app would be
    // localized into. `LANG` is the fallback, because a GUI app launched
    // from Finder has none and one launched from a terminal has both.
    let preferred = objc2_foundation::NSLocale::preferredLanguages()
        .firstObject()
        .and_then(|tag| Locale::new(&tag.to_string()));
    preferred.or_else(posix_locale_from_env)
}

// -- Windows ----------------------------------------------------------------

#[cfg(target_os = "windows")]
fn accent() -> Option<Color> {
    use windows_sys::Win32::Graphics::Dwm::DwmGetColorizationColor;
    // The colorization colour DWM composites the titlebars with, which is
    // what a Win32 app has without reaching into WinRT's `UISettings`. It
    // is 0xAARRGGBB and the alpha is the glass blend, not the colour's, so
    // the accent is taken opaque.
    let (mut argb, mut opaque) = (0u32, 0);
    if unsafe { DwmGetColorizationColor(&raw mut argb, &raw mut opaque) } < 0 {
        return None;
    }
    Some(Color::rgb8(
        (argb >> 16) as u8,
        (argb >> 8) as u8,
        argb as u8,
    ))
}

#[cfg(target_os = "windows")]
fn motion() -> MotionPref {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };
    // "Show animations in Windows" — the setting a Win32 app is expected
    // to honour. The call answers whether animations are *on*, so a failed
    // call is unknown rather than a reduction nobody asked for.
    let mut animations = 0i32;
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            (&raw mut animations).cast(),
            0,
        )
    };
    if ok == 0 {
        MotionPref::Unknown
    } else if animations == 0 {
        MotionPref::Reduced
    } else {
        MotionPref::Full
    }
}

#[cfg(target_os = "windows")]
fn locale() -> Option<Locale> {
    use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
    // Already a BCP-47 tag ("en-US"), UTF-16, NUL-terminated; the returned
    // length counts the NUL.
    const LOCALE_NAME_MAX_LENGTH: usize = 85;
    let mut buf = [0u16; LOCALE_NAME_MAX_LENGTH];
    let len = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
    if len <= 1 {
        return None;
    }
    let tag = String::from_utf16_lossy(&buf[..len as usize - 1]);
    Locale::new(&tag)
}

// -- Everywhere else --------------------------------------------------------

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn accent() -> Option<Color> {
    // No answer without a desktop-portal or gsettings query; see the
    // module doc for where one would go.
    None
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn motion() -> MotionPref {
    MotionPref::Unknown
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn locale() -> Option<Locale> {
    posix_locale_from_env()
}

// -- POSIX's answer, which two of the three platforms fall back to ----------

/// `LC_ALL`, then `LC_MESSAGES`, then `LANG`: the first one set wins, which
/// is the order POSIX resolves them in.
#[cfg_attr(target_os = "windows", allow(dead_code))]
fn posix_locale_from_env() -> Option<Locale> {
    let raw = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))?;
    posix_locale(&raw)
}

/// A POSIX locale (`en_US.UTF-8`, `pt_BR@euro`, `C`) as the BCP-47 tag a
/// view wants: the encoding and modifier dropped, the underscore turned
/// into the hyphen a language tag separates with. `C` and `POSIX` are the
/// absence of a choice rather than a language, so they answer `None` — a
/// view that formats a date for "C" would be formatting it for nobody.
fn posix_locale(raw: &str) -> Option<Locale> {
    let tag = raw.split(['.', '@']).next()?.replace('_', "-");
    if tag.is_empty() || tag == "C" || tag == "POSIX" {
        return None;
    }
    Locale::new(&tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tag a view reads, out of the spelling POSIX hands over — and
    /// nothing out of the two values that name no language.
    #[test]
    fn a_posix_locale_becomes_a_language_tag() {
        let tag = |s: &str| posix_locale(s).map(|l| l.as_str().to_string());
        assert_eq!(tag("en_US.UTF-8").as_deref(), Some("en-US"));
        assert_eq!(tag("pt_BR@euro").as_deref(), Some("pt-BR"));
        assert_eq!(tag("en").as_deref(), Some("en"));
        assert_eq!(tag("zh_Hant_HK.UTF-8").as_deref(), Some("zh-Hant-HK"));
        assert_eq!(tag("C"), None);
        assert_eq!(tag("POSIX"), None);
        assert_eq!(tag(""), None);
        assert_eq!(tag(".UTF-8"), None, "an encoding is not a language");
    }

    /// Whatever the platform answers, it answers *something* — the query
    /// cannot raise, and on a platform with no way to ask, "cannot tell"
    /// is the whole of it. On macOS and Windows the two settings always
    /// have an answer, so the reading is never all-unknown there.
    #[test]
    fn the_query_answers_without_raising() {
        let q = query();
        if cfg!(any(target_os = "macos", target_os = "windows")) {
            assert_ne!(q.motion, MotionPref::Unknown, "the OS knows this one");
            assert!(q.accent.is_some(), "and this one");
        } else {
            assert_eq!(
                q,
                Queried {
                    accent: None,
                    motion: MotionPref::Unknown,
                    locale: q.locale,
                }
            );
        }
    }
}
