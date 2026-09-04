use std::{env, fs, path::PathBuf};

fn main() {
    napi_build::setup();

    // napi-rs derives the TypeScript signature of every `#[napi]` item from
    // the Rust one and writes it out — but only when NAPI_TYPE_DEF_TMP_FOLDER
    // names a directory (`@napi-rs/cli` normally sets it; this repo has no
    // CLI, only cargo). Point it at a fixed spot under target/ so that every
    // build of this crate leaves the type defs beside the addon, and
    // `npm run gen` in packages/kui can render them into the generated half
    // of index.d.ts.
    //
    // `cargo::rustc-env` rather than the ambient environment for two reasons:
    // nobody has to remember to set it (so CI and a laptop cannot disagree
    // about whether the file is there), and it does not flip the crate's
    // fingerprint back and forth between a build that sets it and one that
    // does not. napi appends to the file and does not create the directory,
    // hence the mkdir here — build scripts run before the crate is compiled,
    // which is when the macro writes.
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .join("../../target/napi-type-defs");
    fs::create_dir_all(&dir).expect("failed to create the napi type-def directory");
    let dir = fs::canonicalize(&dir).unwrap_or(dir);
    println!(
        "cargo::rustc-env=NAPI_TYPE_DEF_TMP_FOLDER={}",
        dir.display()
    );
}
