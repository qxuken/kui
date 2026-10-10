//! Every input under `seeds/<target>` and `regressions/<target>`, through
//! the function the fuzz target calls — on stable, as part of the
//! workspace's `cargo test`, so a crash the fuzzer once found stays fixed
//! without anyone running a fuzzer.
//!
//! `regressions/` holds what a fuzzer crashed on, minimized
//! (`scripts/fuzz.nu keep`), named after the backlog item that fixed it;
//! `seeds/` the starting inputs a run grows its corpus from.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

#[test]
fn every_kept_input_runs_clean() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut ran = 0;
    let mut failed = Vec::new();
    for (name, run) in kui_fuzz::TARGETS {
        for kind in ["seeds", "regressions"] {
            let dir = root.join(kind).join(name);
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut files: Vec<_> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_file() && p.extension().is_none_or(|x| x != "md"))
                .collect();
            files.sort();
            for file in files {
                let data = std::fs::read(&file).expect("a kept input");
                ran += 1;
                if catch_unwind(AssertUnwindSafe(|| run(&data))).is_err() {
                    failed.push(
                        file.strip_prefix(root)
                            .unwrap_or(&file)
                            .display()
                            .to_string(),
                    );
                }
            }
        }
    }
    assert!(
        ran > 0,
        "no seeds or regressions found under {}",
        root.display()
    );
    assert!(
        failed.is_empty(),
        "{} of {ran} kept inputs fail: {failed:#?}",
        failed.len()
    );
}
