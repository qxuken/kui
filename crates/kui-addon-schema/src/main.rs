//! `kui-addon-schema gen <schema.lua> <out>`: writes an add-on's typed
//! wrappers for every binding (ADR 0066); `--check` writes nothing and
//! fails when a file is missing or stale.

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let check = args.iter().any(|a| a == "--check");
    let pos: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let (Some(cmd), Some(schema), Some(out)) = (pos.first(), pos.get(1), pos.get(2)) else {
        eprintln!("usage: kui-addon-schema gen <schema.lua> <out> [--check]");
        return ExitCode::from(2);
    };
    if cmd.as_str() != "gen" {
        eprintln!("kui-addon-schema: unknown command `{cmd}` (gen)");
        return ExitCode::from(2);
    }
    let schema = match kui_addon_schema::Schema::from_path(Path::new(schema.as_str())) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("kui-addon-schema: {e}");
            return ExitCode::FAILURE;
        }
    };
    let out = Path::new(out.as_str());
    let mut stale = Vec::new();
    for f in kui_addon_schema::generate(&schema) {
        let path = out.join(&f.path);
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        if current == f.text {
            continue;
        }
        if check {
            stale.push(f.path);
            continue;
        }
        if let Some(dir) = path.parent()
            && let Err(e) = std::fs::create_dir_all(dir)
        {
            eprintln!("kui-addon-schema: {}: {e}", dir.display());
            return ExitCode::FAILURE;
        }
        if let Err(e) = std::fs::write(&path, f.text) {
            eprintln!("kui-addon-schema: {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        println!("wrote {}", path.display());
    }
    if !stale.is_empty() {
        eprintln!(
            "kui-addon-schema: stale: {} — run `cargo run -p kui-addon-schema -- gen` without --check",
            stale.join(", ")
        );
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
