//! The same saved run can be read by the inspector, a person, or an agent.
use super::artifact;
use serde_json::{Value as J, json};
use std::path::{Path, PathBuf};
pub fn json(path: &Path) -> J {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(J::Null)
}
pub fn events(path: &Path) -> Vec<J> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|s| serde_json::from_str::<J>(s).ok())
        .map(|mut e| {
            if e["data"]["version"] == 1 {
                e["data"] = artifact::graph_view(&e["data"]);
            }
            e
        })
        .collect()
}
pub fn diagnostics(events: &[J], results: &J) -> Vec<J> {
    let mut out = Vec::new();
    for case in results["cases"].as_array().into_iter().flatten() {
        if case["ok"] == false {
            let source = events
                .iter()
                .rev()
                .find(|e| {
                    e["case"] == case["name"]
                        && ((e["kind"] == "assertion" && e["data"]["ok"] == false)
                            || e["kind"] == "step_error")
                        && e["data"]["source"].is_object()
                })
                .map(|e| e["data"]["source"].clone())
                .unwrap_or_else(|| json!({"file":case["file"],"line":case["line"]}));
            out.push(json!({"kind":"test_failure","severity":"error","case":case["name"],"message":case["error"],"source":source}));
        }
    }
    for event in events {
        let data = &event["data"];
        let kind = event["kind"].as_str().unwrap_or("");
        let severity = if matches!(kind, "error" | "step_error")
            || (kind == "log" && data["level"] == "error")
        {
            "error"
        } else if kind == "warning" || (kind == "log" && data["level"] == "warn") {
            "warning"
        } else {
            continue;
        };
        let message = data["message"]
            .as_str()
            .or_else(|| data["text"].as_str())
            .or_else(|| data["error"].as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| data.to_string());
        if kind == "step_error"
            && results["cases"].as_array().into_iter().flatten().any(|c| {
                c["ok"] == false
                    && c["name"] == event["case"]
                    && c["error"].as_str().is_some_and(|e| e.contains(&message))
            })
        {
            continue;
        }
        out.push(json!({"kind":kind,"severity":severity,"message":message,"case":event["case"],"step":event["step"],"time":event["time"],"actor":event["actor"],"source":data["source"]}));
    }
    out
}
pub fn file_results(output: &Path, dir: &Path, index: usize) -> (J, J) {
    let manifest = json(&output.join("run.json"));
    let overall = json(&output.join("results.json"));
    let mut collection = json(&dir.join("collection.json"));
    let source = collection["file"]
        .as_str()
        .or_else(|| manifest["files"][index].as_str())
        .unwrap_or("")
        .to_string();
    if !collection.is_object() {
        collection = json!({"file":source,"cases":[]});
    }
    let mut result = json(&dir.join("results.json"));
    if !result.is_object() {
        result = json!({"cases":[]});
    }
    if !result["cases"].is_array() {
        result["cases"] = json!([]);
    }
    for c in overall["cases"].as_array().into_iter().flatten() {
        if c["file"] == source {
            let cases = result["cases"].as_array_mut().unwrap();
            if let Some(saved) = cases.iter_mut().find(|x| x["name"] == c["name"]) {
                *saved = c.clone();
            } else {
                cases.push(c.clone());
            }
        }
    }
    if collection["cases"].as_array().is_none_or(|c| c.is_empty()) {
        collection["cases"] = result["cases"].clone();
    }
    (collection, result)
}
pub fn read(output: &Path) -> Result<J, String> {
    let root = crate::paths::canonical(output).map_err(|e| e.to_string())?;
    let output = root.as_path();
    let run = json(&output.join("run.json"));
    if run["version"] != 1 {
        return Err("unsupported or missing test-run artifact".into());
    }
    let results = json(&output.join("results.json"));
    let mut cases = results["cases"].as_array().cloned().unwrap_or_default();
    let mut files = Vec::new();
    for (index, dir) in artifact::file_dirs(output)?.into_iter().enumerate() {
        let (collection, results) = file_results(output, &dir, index);
        let events = events(&dir.join("events.jsonl"));
        files.push(json!({"artifact":dir.strip_prefix(output).unwrap_or(&dir),"file":collection["file"],"cases":results["cases"],"diagnostics":diagnostics(&events,&results),"output_log":dir.join("output.log"),"events":dir.join("events.jsonl")}));
    }
    for f in &files {
        for case in f["cases"].as_array().into_iter().flatten() {
            if !cases
                .iter()
                .any(|c| c["name"] == case["name"] && c["file"] == case["file"])
            {
                cases.push(case.clone());
            }
        }
    }
    let failed = cases.iter().filter(|c| c["ok"] == false).count();
    let outcome = match run["status"].as_str().unwrap_or("unknown") {
        "complete" => {
            if failed > 0 {
                "failed"
            } else {
                "passed"
            }
        }
        value => value,
    };
    let complete = results["complete"]
        .as_bool()
        .unwrap_or(matches!(outcome, "passed" | "failed"));
    let errors = files
        .iter()
        .flat_map(|f| f["diagnostics"].as_array().into_iter().flatten())
        .filter(|v| v["severity"] == "error")
        .count();
    Ok(
        json!({"version":1,"run_id":run["id"],"status":outcome,"ok":outcome=="passed","complete":complete,"summary":{"passed":cases.len()-failed,"failed":failed,"total":cases.len(),"errors":errors},"cases":cases,"files":files,"artifact_root":artifact::run_root(output)?,"error":run["error"]}),
    )
}
pub fn command(args: &[String]) -> Result<i32, String> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "kui test report|output|events [ARTIFACT] [--json] [--file NAME] [--case NAME] [--step ID]\nreport exits nonzero for a failed/incomplete run. output includes full stdout and stderr. events can be filtered by file, case and causal step."
        );
        return Ok(0);
    }
    let operation = &args[0];
    let mut path = None;
    let mut machine = false;
    let mut file = None;
    let mut case = None;
    let mut step = None;
    let mut iter = args[1..].iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--json" => machine = true,
            "--file" => file = Some(iter.next().ok_or("--file needs a name")?.clone()),
            "--case" => case = Some(iter.next().ok_or("--case needs a name")?.clone()),
            "--step" => {
                step = Some(
                    iter.next()
                        .ok_or("--step needs an integer")?
                        .parse::<u64>()
                        .map_err(|_| "invalid step")?,
                )
            }
            s if s.starts_with('-') => return Err(format!("unknown test query option {s}")),
            _ if path.is_none() => path = Some(PathBuf::from(arg)),
            _ => return Err("expected one artifact directory".into()),
        }
    }
    let path = path.unwrap_or_else(|| PathBuf::from("test-results"));
    let report = read(&path)?;
    if operation == "report" {
        if machine {
            println!(
                "{}",
                serde_json::to_string(&report).map_err(|e| e.to_string())?
            );
        } else {
            println!(
                "{}: {} passed, {} failed · {} error records",
                report["status"].as_str().unwrap_or("unknown"),
                report["summary"]["passed"],
                report["summary"]["failed"],
                report["summary"]["errors"]
            );
            for c in report["cases"].as_array().into_iter().flatten() {
                println!(
                    "{} {} ({}:{})",
                    if c["ok"] == true { "PASS" } else { "FAIL" },
                    c["name"].as_str().unwrap_or("?"),
                    c["file"].as_str().unwrap_or("?"),
                    c["line"]
                );
                if let Some(e) = c["error"].as_str() {
                    println!("{e}");
                }
            }
            for f in report["files"].as_array().into_iter().flatten() {
                println!("Output: {}", f["output_log"].as_str().unwrap_or("?"));
            }
        }
        return Ok(if report["ok"] == true { 0 } else { 1 });
    }
    let mut out = Vec::new();
    for f in report["files"].as_array().into_iter().flatten() {
        let id = f["artifact"].as_str().unwrap_or("");
        if file.as_ref().is_some_and(|s| {
            id != s
                && !id.replace('\\', "/").ends_with(&format!("/{s}"))
                && f["file"].as_str() != Some(s)
        }) {
            continue;
        }
        if operation == "output" {
            let text =
                std::fs::read_to_string(f["output_log"].as_str().unwrap_or("")).unwrap_or_default();
            if machine {
                out.push(json!({"file":f["file"],"artifact":id,"text":text}));
            } else {
                println!("=== {} ===\n{text}", f["file"].as_str().unwrap_or(id));
            }
        } else {
            for mut e in events(Path::new(f["events"].as_str().unwrap_or(""))) {
                if case.as_ref().is_some_and(|c| e["case"].as_str() != Some(c))
                    || step.is_some_and(|s| e["step"].as_u64() != Some(s))
                {
                    continue;
                }
                e["file"] = f["file"].clone();
                if machine {
                    out.push(e);
                } else {
                    println!("{}", e);
                }
            }
        }
    }
    if machine {
        println!(
            "{}",
            serde_json::to_string(&json!({"version":1,"kind":operation,"items":out}))
                .map_err(|e| e.to_string())?
        );
    }
    Ok(0)
}
