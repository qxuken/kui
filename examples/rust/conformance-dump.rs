//! Dumps the scene corpus's reference report (see `kui_core::conformance`).
//!
//!     cargo run -p kui-core --features conformance --example conformance-dump -- target/conformance.txt
//!
//! The other bindings rebuild the same scenes and diff their own report
//! against this file. It is generated, never checked in: the quad digests
//! cover real glyph geometry, so they hold only for the machine and fonts
//! that produced them.

fn main() {
    let text = kui_core::conformance::reference_report();
    match std::env::args().nth(1) {
        Some(path) => std::fs::write(&path, text).unwrap_or_else(|e| {
            eprintln!("conformance-dump: {path}: {e}");
            std::process::exit(1);
        }),
        None => print!("{text}"),
    }
}
