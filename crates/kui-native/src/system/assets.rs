use kui_core::{SvgFileUpdate, SvgStamp, dtcg};
use std::path::Path;

/// Reads a local DTCG document and its relative references.
pub fn load_dtcg(
    path: impl AsRef<Path>,
    opts: &dtcg::DtcgOptions,
) -> Result<dtcg::Dtcg, dtcg::DtcgError> {
    let path = path.as_ref();
    let read = |path: &Path| std::fs::read_to_string(path).map_err(|e| e.to_string());
    let src = read(path).map_err(|e| dtcg::DtcgError {
        message: format!("{}: {e}", path.display()),
    })?;
    dtcg::from_str_with(
        &src,
        path.parent(),
        &path.display().to_string(),
        opts,
        &mut { read },
    )
}

pub(super) fn svg_file(path: &str, previous: Option<SvgStamp>) -> SvgFileUpdate {
    let stamp = std::fs::metadata(path).ok().map(|meta| {
        let modified = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |time| time.as_nanos());
        (modified, meta.len())
    });
    if stamp.is_some() && stamp == previous {
        SvgFileUpdate::Unchanged
    } else {
        SvgFileUpdate::Changed {
            stamp,
            data: std::fs::read(path).map_err(|e| e.to_string()),
        }
    }
}

#[cfg(feature = "gui")]
pub(crate) fn capture_path(extension: &str) -> String {
    let dir = std::env::var_os("KUI_CAPTURE_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|home| std::path::PathBuf::from(home).join("Desktop"))
                .filter(|dir| dir.is_dir())
        })
        .unwrap_or_else(std::env::temp_dir);
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    dir.join(format!("kui-{}.{extension}", utc_stamp(secs)))
        .to_string_lossy()
        .into_owned()
}

#[cfg(feature = "gui")]
/// `secs` since the epoch as `YYYY-MM-DD-HHMMSS`, UTC (Howard Hinnant's
/// civil-from-days).
fn utc_stamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}-{:02}{:02}{:02}",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}
