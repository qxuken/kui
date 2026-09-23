//! What the clipboard says about a secret (backlog F84): the markers a
//! password manager puts on a copied password, read beside the text a
//! paste brings back, and the same markers written on a secret the app
//! puts there itself.
//!
//! The convention is nspasteboard.org's, which 1Password, Bitwarden,
//! KeePassXC and the macOS clipboard managers follow: a pasteboard type
//! `org.nspasteboard.ConcealedType` says the text is a secret (show it to
//! no one, keep it nowhere) and `org.nspasteboard.TransientType` that it is
//! there for a moment (keep it in no history). Windows spells the same two
//! things as registered clipboard formats — `ExcludeClipboardContentFromMonitorProcessing`
//! present, and `CanIncludeInClipboardHistory` holding 0 — and KDE as the
//! MIME type `x-kde-passwordManagerHint` holding `secret`.
//!
//! | platform | read on paste                      | written by `set_secret`                         |
//! |----------|------------------------------------|-------------------------------------------------|
//! | macOS    | both, from the general pasteboard's `types` | the text, ConcealedType and TransientType, declared together |
//! | Windows  | both, from the two formats         | arboard's exclusion from monitoring, history and the cloud clipboard |
//! | Linux    | neither: arboard can write the KDE hint and not list the MIME types | arboard's `exclude_from_history`, the KDE hint |
//!
//! The text itself still goes through arboard on every platform but macOS's
//! secret write, which needs its three types declared in one call so a
//! clipboard manager polling the change count never sees the text without
//! its markers.

use kui_core::ClipboardMarks;

/// The markers on what the clipboard holds now. Read after the text, in
/// the same turn of the loop, so the two describe one copy unless another
/// app copied in the microseconds between.
pub(crate) fn marks() -> ClipboardMarks {
    platform::marks()
}

/// Puts `text` on the clipboard marked concealed and transient, the way a
/// password manager puts a password there.
pub(crate) fn set_secret(clipboard: Option<&mut arboard::Clipboard>, text: String) {
    platform::set_secret(clipboard, text);
}

#[cfg(target_os = "macos")]
mod platform {
    use kui_core::ClipboardMarks;
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    use objc2_foundation::{NSArray, NSString};

    const CONCEALED: &str = "org.nspasteboard.ConcealedType";
    const TRANSIENT: &str = "org.nspasteboard.TransientType";

    pub(super) fn marks() -> ClipboardMarks {
        marks_of(&NSPasteboard::generalPasteboard())
    }

    pub(super) fn set_secret(_clipboard: Option<&mut arboard::Clipboard>, text: String) {
        write_secret(&NSPasteboard::generalPasteboard(), &text);
    }

    /// The markers on `pb`'s current contents.
    pub(super) fn marks_of(pb: &NSPasteboard) -> ClipboardMarks {
        let Some(types) = pb.types() else {
            return ClipboardMarks::default();
        };
        let has = |name: &str| {
            let name = NSString::from_str(name);
            types.iter().any(|t| *t == *name)
        };
        ClipboardMarks {
            concealed: has(CONCEALED),
            transient: has(TRANSIENT),
        }
    }

    /// `text` on `pb` with both markers, the three types declared in one
    /// call so the change count never shows the text without them.
    pub(super) fn write_secret(pb: &NSPasteboard, text: &str) {
        let concealed = NSString::from_str(CONCEALED);
        let transient = NSString::from_str(TRANSIENT);
        // SAFETY: an extern static AppKit defines for the process's life.
        let string = unsafe { NSPasteboardTypeString };
        let types = NSArray::from_slice(&[string, &*concealed, &*transient]);
        // SAFETY: no owner — the data is set right here, not promised.
        unsafe { pb.declareTypes_owner(&types, None) };
        pb.setString_forType(&NSString::from_str(text), string);
        // The markers' data is not read by anyone; an empty string is what
        // the convention's own examples put there.
        let empty = NSString::from_str("");
        pb.setString_forType(&empty, &concealed);
        pb.setString_forType(&empty, &transient);
    }

    /// Against a private pasteboard, so the user's clipboard is never
    /// touched: what `set_secret` writes is what `marks` reads, the text
    /// intact beside the markers, and a plain string reads unmarked.
    #[cfg(test)]
    #[test]
    fn a_secret_written_is_read_back_marked() {
        let pb = NSPasteboard::pasteboardWithUniqueName();
        write_secret(&pb, "hunter2");
        assert_eq!(marks_of(&pb), ClipboardMarks::SECRET);
        // SAFETY: as above.
        let string = unsafe { NSPasteboardTypeString };
        assert_eq!(
            pb.stringForType(string).map(|s| s.to_string()).as_deref(),
            Some("hunter2")
        );
        pb.clearContents();
        pb.setString_forType(&NSString::from_str("a paragraph"), string);
        assert_eq!(marks_of(&pb), ClipboardMarks::default());
        // The objc2 bindings leave `releaseGlobally` out; it takes and
        // returns nothing. SAFETY: a plain message to a live pasteboard.
        let _: () = unsafe { objc2::msg_send![&*pb, releaseGlobally] };
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use kui_core::ClipboardMarks;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
        RegisterClipboardFormatW,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    fn format(name: &str) -> u32 {
        let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
        // SAFETY: a NUL-terminated UTF-16 string that outlives the call.
        unsafe { RegisterClipboardFormatW(wide.as_ptr()) }
    }

    pub(super) fn marks() -> ClipboardMarks {
        let monitor = format("ExcludeClipboardContentFromMonitorProcessing");
        let history = format("CanIncludeInClipboardHistory");
        // SAFETY: plain queries of the clipboard's format list.
        let concealed = monitor != 0 && unsafe { IsClipboardFormatAvailable(monitor) } != 0;
        let mut transient = false;
        // The history format says "may be kept" as a DWORD, so its
        // presence is not the answer: 0 is "keep it in no history". Its
        // value needs the clipboard open, which another app can be
        // holding — then the read gives up and the paste is unmarked on
        // this half, as it would be anywhere the marker is not read.
        if history != 0
            && unsafe { IsClipboardFormatAvailable(history) } != 0
            && unsafe { OpenClipboard(0) } != 0
        {
            // SAFETY: the clipboard is open; the handle is the system's
            // and only read between the lock and the unlock.
            unsafe {
                let h = GetClipboardData(history);
                if h != 0 {
                    let mem = h as *mut core::ffi::c_void;
                    let p = GlobalLock(mem);
                    if !p.is_null() {
                        if GlobalSize(mem) >= 4 {
                            transient = p.cast::<u32>().read_unaligned() == 0;
                        }
                        GlobalUnlock(mem);
                    }
                }
                CloseClipboard();
            }
        }
        ClipboardMarks {
            concealed,
            transient,
        }
    }

    pub(super) fn set_secret(clipboard: Option<&mut arboard::Clipboard>, text: String) {
        use arboard::SetExtWindows;
        let Some(cb) = clipboard else { return };
        let _ = cb
            .set()
            .exclude_from_monitoring()
            .exclude_from_history()
            .exclude_from_cloud()
            .text(text);
    }
}

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))
))]
mod platform {
    use kui_core::ClipboardMarks;

    /// Not read here yet: arboard writes KDE's hint and has no call that
    /// lists what the selection offers.
    pub(super) fn marks() -> ClipboardMarks {
        ClipboardMarks::default()
    }

    pub(super) fn set_secret(clipboard: Option<&mut arboard::Clipboard>, text: String) {
        use arboard::SetExtLinux;
        let Some(cb) = clipboard else { return };
        let _ = cb.set().exclude_from_history().text(text);
    }
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    all(
        unix,
        not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))
    )
)))]
mod platform {
    use kui_core::ClipboardMarks;

    pub(super) fn marks() -> ClipboardMarks {
        ClipboardMarks::default()
    }

    pub(super) fn set_secret(clipboard: Option<&mut arboard::Clipboard>, text: String) {
        if let Some(cb) = clipboard {
            let _ = cb.set_text(text);
        }
    }
}
