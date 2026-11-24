"""Check the mobile feature from a standalone project with no Cargo patch."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory

checkout = Path(__file__).resolve().parents[5]
native = checkout / "crates" / "kui-native"
target_dir = Path(os.environ.get("CARGO_TARGET_DIR", checkout / "target")).resolve()

with TemporaryDirectory(prefix="kui-mobile-consumer-") as temp:
    project = Path(temp)
    (project / "src").mkdir()
    (project / "src" / "lib.rs").write_text("pub use kui_native::mobile;\n")
    (project / "Cargo.toml").write_text(f"""\
[package]
name = "kui-mobile-consumer"
version = "0.0.0"
edition = "2024"

[dependencies]
kui-native = {{ path = {json.dumps(native.as_posix(), ensure_ascii=False)}, default-features = false, features = ["mobile"] }}
""")
    # Use the same registry versions as CI. Cargo adds the new root package.
    shutil.copyfile(checkout / "Cargo.lock", project / "Cargo.lock")
    subprocess.run(
        ["cargo", "check", "--offline", "--target", sys.argv[1], "--target-dir", str(target_dir)],
        cwd=project,
        check=True,
    )
