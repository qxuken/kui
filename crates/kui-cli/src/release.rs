//! The runner's releases, as `kui upgrade`, `kui pack` and `kui runners
//! fetch` read them (ADR 0068, decisions 1, 3, 5 and 14): one asset per
//! platform on the GitHub release, `SHA256SUMS` beside them, and the same
//! layout under any mirror — `<mirror>/v<version>/<asset>`, with
//! `<mirror>/latest.txt` naming the newest version.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The platforms a release builds, as its assets name them.
pub const PLATFORMS: [&str; 4] = ["linux-x64", "linux-arm64", "darwin-arm64", "win32-x64"];

const GITHUB: &str = "https://github.com/qxuken/kui/releases/download";
const LATEST_API: &str = "https://api.github.com/repos/qxuken/kui/releases/latest";

/// This machine's platform, as the assets name it; `None` where the
/// release builds none (an Intel Mac, a musl Linux).
pub fn current_platform() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") if !cfg!(target_env = "musl") => Some("linux-x64"),
        ("linux", "aarch64") if !cfg!(target_env = "musl") => Some("linux-arm64"),
        ("macos", "aarch64") => Some("darwin-arm64"),
        ("windows", "x86_64") => Some("win32-x64"),
        _ => None,
    }
}

/// Whether `t` is a platform the release builds; the error names them.
pub fn check_platform(t: &str) -> Result<&'static str, String> {
    PLATFORMS
        .iter()
        .find(|p| **p == t)
        .copied()
        .ok_or_else(|| format!("no runner for `{t}`: the release builds {}", PLATFORMS.join(", ")))
}

/// The runner's asset for `version` on `platform`.
pub fn asset_name(version: &str, platform: &str) -> String {
    if platform.starts_with("win32") {
        format!("kui-{version}-{platform}.zip")
    } else {
        format!("kui-{version}-{platform}.tar.gz")
    }
}

/// The runner binary's name inside its asset.
pub fn binary_name(platform: &str) -> &'static str {
    if platform.starts_with("win32") { "kui.exe" } else { "kui" }
}

/// Where a version's assets are: the mirror's, else the GitHub release's.
pub fn base_url(mirror: Option<&str>, version: &str) -> String {
    match mirror {
        Some(m) => format!("{}/v{version}", m.trim_end_matches('/')),
        None => format!("{GITHUB}/v{version}"),
    }
}

/// The mirror a command was given, else `KUI_RUNNER_MIRROR`, else
/// `KUI_MIRROR` (what the install scripts read).
pub fn mirror(flag: Option<&str>) -> Option<String> {
    flag.map(str::to_owned)
        .or_else(|| std::env::var("KUI_RUNNER_MIRROR").ok())
        .or_else(|| std::env::var("KUI_MIRROR").ok())
        .filter(|m| !m.is_empty())
}

/// A whole response body, up to 512 MB.
pub fn download(url: &str) -> Result<Vec<u8>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(600)))
        .user_agent(format!("kui/{}", crate::compat::VERSION))
        .build()
        .into();
    let mut res = agent.get(url).call().map_err(|e| format!("GET {url}: {e}"))?;
    res.body_mut()
        .with_config()
        .limit(512 << 20)
        .read_to_vec()
        .map_err(|e| format!("GET {url}: {e}"))
}

/// The newest version: the mirror's `latest.txt`, else the GitHub
/// release's tag.
pub fn latest_version(mirror: Option<&str>) -> Result<String, String> {
    if let Some(m) = mirror {
        let url = format!("{}/latest.txt", m.trim_end_matches('/'));
        let text = String::from_utf8(download(&url)?).map_err(|_| format!("{url}: not text"))?;
        return Ok(text.trim().trim_start_matches('v').to_string());
    }
    let body = download(LATEST_API)?;
    let text = String::from_utf8_lossy(&body);
    let json = kui_core::json::parse(&text).map_err(|e| format!("{LATEST_API}: {e}"))?;
    json.get_str("tag_name")
        .map(|t| t.trim_start_matches('v').to_string())
        .ok_or_else(|| format!("{LATEST_API}: no tag_name"))
}

/// A version as asked: `latest` resolved, a leading `v` dropped.
pub fn resolve(version: &str, mirror: Option<&str>) -> Result<String, String> {
    if version == "latest" {
        latest_version(mirror)
    } else {
        Ok(version.trim_start_matches('v').to_string())
    }
}

/// `SHA256SUMS`, by asset name.
pub fn sums(base: &str) -> Result<HashMap<String, String>, String> {
    let url = format!("{base}/SHA256SUMS");
    let text = String::from_utf8(download(&url)?).map_err(|_| format!("{url}: not text"))?;
    Ok(parse_sums(&text))
}

pub fn parse_sums(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|l| {
            let (sum, name) = l.split_once(char::is_whitespace)?;
            Some((name.trim().trim_start_matches('*').to_string(), sum.trim().to_lowercase()))
        })
        .collect()
}

pub use crate::archive::sha256_hex;

/// An asset, downloaded and checked against the release's sums.
pub fn fetch_verified(base: &str, asset: &str, sums: &HashMap<String, String>) -> Result<Vec<u8>, String> {
    let want = sums
        .get(asset)
        .ok_or_else(|| format!("{base}/SHA256SUMS lists no {asset}"))?;
    let bytes = download(&format!("{base}/{asset}"))?;
    let got = sha256_hex(&bytes);
    if &got != want {
        return Err(format!("{asset}: checksum {got} is not the release's {want}; not using it"));
    }
    Ok(bytes)
}

/// The runner binary inside an asset.
pub fn binary_from_asset(asset: &[u8], platform: &str) -> Result<Vec<u8>, String> {
    let name = binary_name(platform);
    if platform.starts_with("win32") {
        let zip = crate::archive::ZipReader::open(asset.to_vec().into())
            .map_err(|e| format!("the runner's zip: {e}"))?;
        let entry = zip.get(name).ok_or_else(|| format!("the runner's zip holds no {name}"))?;
        entry.read().map_err(|e| format!("the runner's zip: {e}"))
    } else {
        let files = crate::archive::read_tar_gz(asset).map_err(|e| format!("the runner's tarball: {e}"))?;
        files
            .into_iter()
            .find(|(n, _, _)| n.trim_start_matches("./") == name)
            .map(|(_, data, _)| data)
            .ok_or_else(|| format!("the runner's tarball holds no {name}"))
    }
}

/// The runner binary for `version` on `platform`: this runner itself when
/// they are its own, else from the cache, else downloaded, checked and
/// cached (`<kui cache>/runners/<version>/<platform>/kui[.exe]`). `local`
/// is `--runner <file>`, used as given.
pub fn runner(version: &str, platform: &str, mirror: Option<&str>, local: Option<&Path>) -> Result<Vec<u8>, String> {
    if let Some(file) = local {
        return std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()));
    }
    if version == crate::compat::VERSION && Some(platform) == current_platform() {
        let exe = std::env::current_exe().map_err(|e| format!("this runner's path: {e}"))?;
        return std::fs::read(&exe).map_err(|e| format!("{}: {e}", exe.display()));
    }
    let cached = cache_path(version, platform);
    if let Ok(bytes) = std::fs::read(&cached) {
        return Ok(bytes);
    }
    let base = base_url(mirror, version);
    let sums = sums(&base)?;
    let asset = fetch_verified(&base, &asset_name(version, platform), &sums)?;
    let bin = binary_from_asset(&asset, platform)?;
    if let Some(dir) = cached.parent() {
        let _ = std::fs::create_dir_all(dir);
        let tmp = cached.with_extension("part");
        if std::fs::write(&tmp, &bin).is_ok() {
            let _ = std::fs::rename(&tmp, &cached);
        }
    }
    Ok(bin)
}

fn cache_path(version: &str, platform: &str) -> PathBuf {
    crate::paths::kui_cache()
        .join("runners")
        .join(version)
        .join(platform)
        .join(binary_name(platform))
}

/// `kui runners fetch <version> --to <dir>`: every platform's asset with
/// the sums, in the release's layout, for a company's mirror; `latest.txt`
/// written when the version asked was `latest`.
pub fn fetch_all(version: &str, to: &Path, mirror: Option<&str>) -> Result<(), String> {
    let resolved = resolve(version, mirror)?;
    let base = base_url(mirror, &resolved);
    let dir = to.join(format!("v{resolved}"));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let sums_text = download(&format!("{base}/SHA256SUMS"))?;
    let sums = parse_sums(&String::from_utf8_lossy(&sums_text));
    for platform in PLATFORMS {
        let asset = asset_name(&resolved, platform);
        let bytes = fetch_verified(&base, &asset, &sums)?;
        std::fs::write(dir.join(&asset), &bytes).map_err(|e| format!("{asset}: {e}"))?;
        println!("{}  {asset}", sha256_hex(&bytes));
    }
    for extra in ["install.sh", "install.ps1"] {
        if let Ok(bytes) = download(&format!("{base}/{extra}")) {
            let _ = std::fs::write(dir.join(extra), bytes);
        }
    }
    std::fs::write(dir.join("SHA256SUMS"), &sums_text).map_err(|e| e.to_string())?;
    if version == "latest" {
        std::fs::write(to.join("latest.txt"), format!("{resolved}\n")).map_err(|e| e.to_string())?;
    }
    println!("wrote {} (sync it to the mirror as is)", dir.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_urls_follow_the_release() {
        assert_eq!(asset_name("0.1.0", "darwin-arm64"), "kui-0.1.0-darwin-arm64.tar.gz");
        assert_eq!(asset_name("0.1.0", "win32-x64"), "kui-0.1.0-win32-x64.zip");
        assert_eq!(base_url(Some("https://s3.example/kui/"), "0.1.0"), "https://s3.example/kui/v0.1.0");
        assert!(base_url(None, "0.1.0").ends_with("/releases/download/v0.1.0"));
        assert!(check_platform("darwin-x64").unwrap_err().contains("linux-x64"));
    }

    #[test]
    fn sums_parse_both_spellings() {
        let s = parse_sums("ABC  kui-1-linux-x64.tar.gz\ndef *kui-1-win32-x64.zip\n");
        assert_eq!(s["kui-1-linux-x64.tar.gz"], "abc");
        assert_eq!(s["kui-1-win32-x64.zip"], "def");
    }
}
