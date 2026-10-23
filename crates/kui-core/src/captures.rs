//! What an app asks a driver to keep of its window (ADR 0055): one frame
//! as a picture, or every frame for a while as an animation, written to a
//! file. The core only queues the ask, as data ([`CaptureCommand`]); a
//! driver with a GPU serves it after a frame is drawn (the runner's
//! `kui_native::Captures`) and reports what it wrote as a `capture`
//! event, the message the verb named with the path in it.

use crate::value::Value;

/// One ask, in the order the app made them; drained by
/// `Core::take_capture_commands`.
#[derive(Clone, Debug, PartialEq)]
pub enum CaptureCommand {
    /// The next frame drawn, written to `path` as one picture.
    Still {
        path: String,
        message: Option<Value>,
    },
    /// Every frame drawn from the next one on, written to `path` as an
    /// animation when a [`CaptureCommand::Stop`] ends it.
    Record {
        path: String,
        message: Option<Value>,
    },
    /// Ends the recording with the next frame drawn.
    Stop,
    /// The next frame drawn, put on the clipboard as a picture: a
    /// [`crate::menu::MenuAction::SetClipboardImage`] the driver applies.
    Clipboard { message: Option<Value> },
}

/// What a path's extension says a capture is written as: the one rule
/// every binding shares, so a path means the same file in each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureFormat {
    /// `.png` (a recording's `.apng` too): lossless, an animated PNG for a
    /// recording.
    Png,
    /// `.jpg` / `.jpeg`: a still only, quality 90, alpha dropped.
    Jpeg,
    /// `.webp`: a still only, lossless.
    WebP,
    /// `.gif`: 256 colours, a still or an animation.
    Gif,
}

impl CaptureFormat {
    /// The format a still at `path` is written as, or why there is none.
    pub fn of_still(path: &str) -> Result<CaptureFormat, String> {
        match extension(path).as_deref() {
            Some("png") => Ok(CaptureFormat::Png),
            Some("jpg" | "jpeg") => Ok(CaptureFormat::Jpeg),
            Some("webp") => Ok(CaptureFormat::WebP),
            Some("gif") => Ok(CaptureFormat::Gif),
            _ => Err(
                "the extension names no picture format: a capture is .png, .jpg, .webp or .gif"
                    .into(),
            ),
        }
    }

    /// The format a recording at `path` is written as, or why there is
    /// none: an animated PNG or GIF.
    pub fn of_recording(path: &str) -> Result<CaptureFormat, String> {
        match extension(path).as_deref() {
            Some("png" | "apng") => Ok(CaptureFormat::Png),
            Some("gif") => Ok(CaptureFormat::Gif),
            _ => Err(
                "the extension names no animation format: a recording is .png, .apng or .gif"
                    .into(),
            ),
        }
    }
}

fn extension(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extension_is_the_format() {
        assert_eq!(CaptureFormat::of_still("a/b.PNG"), Ok(CaptureFormat::Png));
        assert_eq!(CaptureFormat::of_still("b.jpeg"), Ok(CaptureFormat::Jpeg));
        assert_eq!(CaptureFormat::of_still("b.webp"), Ok(CaptureFormat::WebP));
        assert!(CaptureFormat::of_still("b.apng").is_err());
        assert!(CaptureFormat::of_still("noext").is_err());
        assert_eq!(
            CaptureFormat::of_recording("r.apng"),
            Ok(CaptureFormat::Png)
        );
        assert_eq!(CaptureFormat::of_recording("r.gif"), Ok(CaptureFormat::Gif));
        let why = CaptureFormat::of_recording("r.jpg").unwrap_err();
        assert!(why.contains(".apng"), "{why}");
    }
}
