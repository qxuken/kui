//! Durable artifacts and snapshot comparisons; no Lua or UI ownership.
use kui_core::Value;
use serde::Serialize;
use serde_json::{Value as J, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    atomic(path, &bytes)
}
pub fn atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("output has no parent")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut f = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    f.write_all(bytes).map_err(|e| e.to_string())?;
    persist(f, path, std::thread::sleep)
}

fn persist(
    mut file: tempfile::NamedTempFile,
    path: &Path,
    mut wait: impl FnMut(Duration),
) -> Result<(), String> {
    let mut delay = Duration::from_millis(10);
    loop {
        match file.persist(path) {
            Ok(_) => return Ok(()),
            // A reader or scanner can briefly block a Windows rename.
            // Retry access denied, sharing violation and lock violation.
            Err(e)
                if cfg!(windows)
                    && matches!(e.error.raw_os_error(), Some(5 | 32 | 33))
                    && delay <= Duration::from_millis(320) =>
            {
                file = e.file;
                wait(delay);
                delay *= 2;
            }
            Err(e) => return Err(format!("{}: {e}", path.display())),
        }
    }
}

#[cfg(all(test, windows))]
#[test]
fn persist_retries_after_a_windows_reader_releases_the_destination() {
    use std::os::windows::fs::OpenOptionsExt;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("status.json");
    std::fs::write(&path, b"old").unwrap();
    let mut reader = Some(
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .unwrap(),
    );
    let mut file = tempfile::NamedTempFile::new_in(dir.path()).unwrap();
    file.write_all(b"new").unwrap();

    persist(file, &path, |_| {
        drop(reader.take().expect("the lock should require one retry"));
        assert_eq!(std::fs::read(&path).unwrap(), b"old");
    })
    .unwrap();

    assert!(reader.is_none(), "expected a retry");
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
}

pub fn value(v: &Value) -> J {
    match v {
        Value::Null => J::Null,
        Value::Bool(v) => json!(v),
        Value::Int(v) => json!(v),
        Value::Float(v) => json!(v),
        Value::Str(v) => json!(v),
        Value::List(v) => J::Array(v.iter().map(value).collect()),
        Value::Map(v) => J::Object(v.iter().map(|(k, v)| (k.clone(), value(v))).collect()),
    }
}
pub fn blob(root: &Path, bytes: &[u8]) -> Result<String, String> {
    let hash = ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let name = format!("blobs/{hash}.gz");
    let path = root.join(&name);
    if !path.exists() {
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gzip.write_all(bytes).map_err(|e| e.to_string())?;
        atomic(&path, &gzip.finish().map_err(|e| e.to_string())?)?;
    }
    Ok(name)
}
pub fn read_blob(root: &Path, name: &str, limit: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let p = safe_join(root, name)?;
    let f = std::fs::File::open(p).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(f)
        .take(limit as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| e.to_string())?;
    if out.len() > limit {
        return Err("artifact blob exceeds its declared size".into());
    }
    Ok(out)
}
pub fn safe_join(root: &Path, name: &str) -> Result<PathBuf, String> {
    let path = Path::new(name);
    if path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("artifact paths must stay within their run".into());
    }
    Ok(root.join(path))
}
pub fn png(c: &kui_native::Capture) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut bytes, c.width, c.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        writer
            .write_image_data(&c.rgba)
            .map_err(|e| e.to_string())?;
    }
    Ok(bytes)
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
        .chars()
        .filter(|c| *c == '\n' || *c == '\r' || *c == '\t' || *c >= ' ')
        .collect()
}
pub fn junit(path: &Path, results: &super::Results) -> Result<(), String> {
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites><testsuite name=\"kui\" tests=\"{}\" failures=\"{}\" time=\"{}\">\n",
        results.cases.len(),
        results.cases.iter().filter(|c| !c.ok).count(),
        results.cases.iter().map(|c| c.seconds).sum::<f64>()
    );
    for c in &results.cases {
        out.push_str(&format!(
            "<testcase name=\"{}\" classname=\"{}\" file=\"{}\" line=\"{}\" time=\"{:.6}\">",
            escape(&c.name),
            escape(&c.file),
            escape(&c.file),
            c.line,
            c.seconds
        ));
        if !c.ok {
            out.push_str(&format!(
                "<failure message=\"test failed\">{}</failure>",
                escape(c.error.as_deref().unwrap_or("failed"))
            ));
        }
        out.push_str("</testcase>\n");
    }
    out.push_str("</testsuite></testsuites>\n");
    atomic(path, out.as_bytes())
}
pub fn snapshot_path(
    file: &Path,
    case: &str,
    name: &str,
    kind: &str,
    extension: &str,
) -> Result<PathBuf, String> {
    if name.is_empty() || name.contains(['/', '\\']) || matches!(name, "." | "..") {
        return Err("snapshot name must be a nonempty filename".into());
    }
    let key = case
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let hash = ring::digest::digest(&ring::digest::SHA256, case.as_bytes());
    let short = hash.as_ref()[..4]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Ok(file.parent().unwrap().join("__snapshots__").join(format!(
        "{}.{}-{short}.{name}.{kind}.{extension}",
        file.file_stem().unwrap().to_string_lossy(),
        key.chars().take(80).collect::<String>()
    )))
}
pub fn text_snapshot(path: &Path, actual: &str, update: bool) -> Result<(), String> {
    if update {
        return atomic(path, actual.as_bytes());
    }
    match std::fs::read_to_string(path) {
        Ok(expected) if expected == actual => Ok(()),
        Ok(expected) => {
            atomic(&path.with_extension("actual.txt"), actual.as_bytes())?;
            atomic(
                &path.with_extension("diff.txt"),
                line_diff(&expected, actual).as_bytes(),
            )?;
            Err(format!(
                "snapshot differs: {}\n{}",
                path.display(),
                line_diff(&expected, actual)
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            atomic(&path.with_extension("actual.txt"), actual.as_bytes())?;
            Err(format!(
                "missing snapshot {}; run kui test --update",
                path.display()
            ))
        }
        Err(e) => Err(e.to_string()),
    }
}
pub fn line_diff(expected: &str, actual: &str) -> String {
    let a = expected.lines().collect::<Vec<_>>();
    let b = actual.lines().collect::<Vec<_>>();
    let first = a
        .iter()
        .zip(&b)
        .position(|(a, b)| a != b)
        .unwrap_or(a.len().min(b.len()));
    let mut out = format!("@@ line {} @@\n", first + 1);
    for l in a.iter().skip(first).take(12) {
        out.push_str(&format!("- {l}\n"));
    }
    for l in b.iter().skip(first).take(12) {
        out.push_str(&format!("+ {l}\n"));
    }
    out
}
pub fn image_snapshot(
    path: &Path,
    actual: &kui_native::Capture,
    tolerance: u8,
    update: bool,
) -> Result<(), String> {
    let bytes = png(actual)?;
    if update {
        return atomic(path, &bytes);
    }
    let expected = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            atomic(&path.with_extension("actual.png"), &bytes)?;
            let mut diff = actual.clone();
            diff.rgba
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .for_each(|p| p.copy_from_slice(&[255, 0, 90, 255]));
            atomic(&path.with_extension("diff.png"), &png(&diff)?)?;
            return Err(format!("{}: {e}; run kui test --update", path.display()));
        }
    };
    let image = kui_native::decode_image(&expected)?;
    let mut diff = actual.clone();
    let mut count = 0;
    if image.width == actual.width && image.height == actual.height {
        for ((a, b), d) in actual
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .zip(image.rgba.as_chunks::<4>().0.iter())
            .zip(diff.rgba.as_chunks_mut::<4>().0.iter_mut())
        {
            if a.iter().zip(b).any(|(x, y)| x.abs_diff(*y) > tolerance) {
                count += 1;
                d.copy_from_slice(&[255, 0, 90, 255]);
            } else {
                d.copy_from_slice(&[a[0] / 3, a[1] / 3, a[2] / 3, 255]);
            }
        }
        if count == 0 {
            return Ok(());
        }
    } else {
        count = actual.width as usize * actual.height as usize;
        diff.rgba
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .for_each(|p| p.copy_from_slice(&[255, 0, 90, 255]));
    }
    atomic(&path.with_extension("actual.png"), &bytes)?;
    atomic(&path.with_extension("diff.png"), &png(&diff)?)?;
    Err(format!(
        "image snapshot differs: {} ({count} pixels; expected {}x{}, actual {}x{})",
        path.display(),
        image.width,
        image.height,
        actual.width,
        actual.height
    ))
}

/// Human-readable projection of a captured graph. The original graph stays
/// available in the artifact; references and opaque resources remain explicit.
pub fn app_state_view(frame: &J) -> J {
    let model = graph_view(&frame["state"]["model"]);
    let adapter = &frame["state"]["adapter"];
    if adapter.is_object() {
        json!({"model":model,"adapter":{"renderer":adapter["renderer"],"state":graph_view(&adapter["graph"])}})
    } else {
        model
    }
}

pub fn graph_view(graph: &J) -> J {
    fn visit(v: &J, objects: &[J], seen: &mut std::collections::HashSet<u64>) -> J {
        match v["kind"].as_str().unwrap_or("") {
            "nil" => J::Null,
            "boolean" => v["value"].clone(),
            "integer" | "float" => v["value"]
                .as_str()
                .and_then(|s| serde_json::from_str::<J>(s).ok())
                .unwrap_or_else(|| v["value"].clone()),
            "string" => {
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(v["base64"].as_str().unwrap_or(""))
                    .unwrap_or_default();
                match String::from_utf8(bytes) {
                    Ok(s) => json!(s),
                    Err(_) => json!({"$bytes":v["base64"]}),
                }
            }
            "ref" => {
                let id = v["id"].as_u64().unwrap_or(0);
                if !seen.insert(id) {
                    return json!({"$ref":id});
                }
                let Some(o) = id.checked_sub(1).and_then(|i| objects.get(i as usize)) else {
                    return json!({"$invalid_ref":id});
                };
                if o["kind"] == "table" {
                    let entries = o["entries"].as_array().map(Vec::as_slice).unwrap_or(&[]);
                    if o["array"] == true {
                        return J::Array(
                            entries
                                .iter()
                                .map(|e| visit(&e["value"], objects, seen))
                                .collect(),
                        );
                    }
                    if entries.iter().any(|e| e["key"]["kind"] != "string") {
                        return json!({"$table":id,"entries":entries.iter().map(|e|json!({"key":visit(&e["key"],objects,seen),"value":visit(&e["value"],objects,seen)})).collect::<Vec<_>>()});
                    }
                    let mut map = serde_json::Map::new();
                    for e in entries {
                        let key = visit(&e["key"], objects, seen);
                        let key = key
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("[{key}]"));
                        map.insert(key, visit(&e["value"], objects, seen));
                    }
                    J::Object(map)
                } else if o["kind"] == "channel" {
                    let mut columns = serde_json::Map::new();
                    if let Some(c) = o["columns_data"].as_object() {
                        for (name, v) in c {
                            let values = v["values"].as_array().cloned().unwrap_or_default();
                            columns.insert(
                                name.clone(),
                                json!({"type":v["type"],"rows":values.len(),"values":values}),
                            );
                        }
                    }
                    if let Some(c) = o["columns"].as_object() {
                        for (name, v) in c {
                            columns.insert(name.clone(), visit(v, objects, seen));
                        }
                    }
                    json!({"$channel":id,"revision":o["revision"],"columns":columns,"bytes_base64":o["bytes_base64"]})
                } else {
                    json!({"$opaque":o["kind"],"id":id})
                }
            }
            _ => v.clone(),
        }
    }
    let objects = graph["objects"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    visit(&graph["root"], objects, &mut Default::default())
}
pub fn changes(before: &J, after: &J) -> J {
    fn walk(path: &str, a: &J, b: &J, out: &mut Vec<J>) {
        if a == b {
            return;
        }
        if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
            let keys = a
                .keys()
                .chain(b.keys())
                .collect::<std::collections::BTreeSet<_>>();
            for key in keys {
                walk(
                    &format!("{path}.{key}"),
                    a.get(key).unwrap_or(&J::Null),
                    b.get(key).unwrap_or(&J::Null),
                    out,
                );
            }
        } else {
            out.push(json!({"path":path,"before":a,"after":b}));
        }
    }
    let mut out = Vec::new();
    walk("$", before, after, &mut out);
    J::Array(out)
}

pub fn read_frame(path: &Path) -> Result<J, String> {
    let mut frame: J = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if frame["version"] != 1 {
        return Err("unsupported frame artifact version".into());
    }
    let root = path
        .parent()
        .and_then(Path::parent)
        .ok_or("invalid frame artifact path")?;
    for field in ["tree", "core", "state", "kit"] {
        if let Some(name) = frame[format!("{field}_blob")].as_str() {
            let value = serde_json::from_slice::<J>(&read_blob(root, name, 128 * 1024 * 1024)?)
                .map_err(|e| e.to_string())?;
            frame[field] = value;
        }
    }
    if let Some(name) = frame["model_blob"].as_str() {
        let graph: J = serde_json::from_slice(&read_blob(root, name, 128 * 1024 * 1024)?)
            .map_err(|e| e.to_string())?;
        frame["state"] = json!({"model":graph});
    }
    if let Some(name) = frame["adapter_blob"].as_str() {
        let graph: J = serde_json::from_slice(&read_blob(root, name, 128 * 1024 * 1024)?)
            .map_err(|e| e.to_string())?;
        frame["state"]["adapter"] = graph;
    }
    Ok(frame)
}

/// Snapshot comparisons carry their inputs into the run, so a CI artifact
/// remains inspectable without the source checkout or its baseline files.
pub fn snapshot_event(
    run: &crate::context::RunContext,
    name: &str,
    kind: &str,
    baseline: &Path,
    result: &Result<(), String>,
    actual: Option<&kui_native::Capture>,
) {
    let mut data = json!({"name":name,"kind":kind,"ok":result.is_ok(),"error":result.as_ref().err(),"baseline":baseline});
    if let Some(journal) = run.journal.lock().unwrap().as_ref()
        && let Some(root) = journal.parent()
    {
        let ext = baseline.extension().unwrap_or_default().to_string_lossy();
        for (variant, path) in [
            ("expected", baseline.to_path_buf()),
            ("actual", baseline.with_extension(format!("actual.{ext}"))),
            ("diff", baseline.with_extension(format!("diff.{ext}"))),
        ] {
            let bytes = if result.is_ok() && variant != "expected" {
                if let Some(actual) = actual {
                    let mut pixels = actual.clone();
                    if variant == "diff" {
                        for pixel in pixels.rgba.as_chunks_mut::<4>().0 {
                            *pixel = [pixel[0] / 3, pixel[1] / 3, pixel[2] / 3, 255];
                        }
                    }
                    png(&pixels)
                } else if variant == "diff" {
                    Ok(Vec::new())
                } else {
                    std::fs::read(baseline).map_err(|e| e.to_string())
                }
            } else {
                std::fs::read(&path).map_err(|e| e.to_string())
            };
            if let Ok(bytes) = bytes {
                let digest = ring::digest::digest(&ring::digest::SHA256, &bytes)
                    .as_ref()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>();
                let target = format!("snapshots/{digest}.{ext}");
                if atomic(&root.join(&target), &bytes).is_ok() {
                    data[variant] = json!(target);
                }
            }
        }
    }
    run.record("snapshot", None, data);
}

pub fn image(root: &Path, bytes: &[u8]) -> Result<String, String> {
    let hash = ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let name = format!("images/{hash}.png");
    if !root.join(&name).is_file() {
        atomic(&root.join(&name), bytes)?;
    }
    Ok(name)
}

/// A stable run directory; the top-level manifest only selects the latest.
pub fn run_root(output: &Path) -> Result<PathBuf, String> {
    let manifest = std::fs::read(output.join("run.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<J>(&b).ok());
    match manifest.as_ref().and_then(|v| v["artifact_root"].as_str()) {
        Some(name) => safe_join(output, name),
        None => Ok(output.to_path_buf()),
    }
}
pub fn file_dirs(output: &Path) -> Result<Vec<PathBuf>, String> {
    let root = run_root(output)?;
    let mut dirs = std::fs::read_dir(root)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .starts_with("file-")
        })
        .collect::<Vec<_>>();
    dirs.sort();
    Ok(dirs)
}

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn state_view_keeps_adapter_data_and_every_channel_row_and_byte() {
        let graph = json!({"version":1,"root":{"kind":"ref","id":1},"objects":[{"kind":"channel","id":1,"revision":3,"columns_data":{"count":{"type":"integer","values":(0..50).collect::<Vec<_>>()}},"bytes_base64":"a".repeat(200)}]});
        let frame = json!({"state":{"model":graph,"adapter":{"renderer":"saved","graph":graph}}});
        let view = app_state_view(&frame);
        assert_eq!(view["model"]["columns"]["count"]["values"][49], 49);
        assert_eq!(
            view["adapter"]["state"]["columns"]["count"]["values"]
                .as_array()
                .unwrap()
                .len(),
            50
        );
        assert_eq!(
            view["adapter"]["state"]["bytes_base64"]
                .as_str()
                .unwrap()
                .len(),
            200
        );
    }
    #[test]
    fn passing_tolerant_comparison_retains_actual_pixels_and_diff() {
        let dir = tempfile::tempdir().unwrap();
        let baseline = dir.path().join("baseline.png");
        let expected = kui_native::Capture {
            width: 1,
            height: 1,
            scale: 1.0,
            rgba: vec![0, 0, 0, 255],
        };
        atomic(&baseline, &png(&expected).unwrap()).unwrap();
        let actual = kui_native::Capture {
            rgba: vec![1, 0, 0, 255],
            ..expected
        };
        let result = image_snapshot(&baseline, &actual, 2, false);
        assert!(result.is_ok());
        let run = crate::context::RunContext::test(
            Vec::new(),
            "snapshot".into(),
            crate::modules::AppInfo::default(),
            dir.path().join("temporary"),
        );
        *run.journal.lock().unwrap() = Some(dir.path().join("events.jsonl"));
        snapshot_event(
            &run,
            "accepted",
            "expect_shot",
            &baseline,
            &result,
            Some(&actual),
        );
        let event: J = serde_json::from_str(
            std::fs::read_to_string(dir.path().join("events.jsonl"))
                .unwrap()
                .trim(),
        )
        .unwrap();
        for field in ["expected", "actual", "diff"] {
            assert!(
                dir.path()
                    .join(event["data"][field].as_str().unwrap())
                    .is_file()
            );
        }
        let pixels = kui_native::decode_image(
            &std::fs::read(dir.path().join(event["data"]["actual"].as_str().unwrap())).unwrap(),
        )
        .unwrap();
        assert_eq!(pixels.rgba, actual.rgba);
    }
}
