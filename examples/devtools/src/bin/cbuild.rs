//! The C examples' build and their round, one program for every platform:
//! what `examples/c/build.sh` and `examples/c/build.ps1` were. Builds
//! kui_ffi and the C examples against it, after checking that
//! `include/kui.h` still describes the structs Rust actually lays out.
//!
//!     cargo run -p kui-devtools --bin cbuild              # build, print the round
//!     cargo run -p kui-devtools --bin cbuild -- --run     # and run the round it prints
//!     cargo run -p kui-devtools --bin cbuild -- --release
//!
//! Everything lands in `target/<profile>/`, beside the libkui_ffi the hosts
//! link and the `c_panel` a plugin is loaded by, so nothing is written into
//! the source tree:
//!
//!     counter          C as the host, kui as a plain library
//!     surface          the header walk, every prototype called once
//!     conformance      the C adapter over the scene corpus
//!     host             C as the host of a C *extension* (ADR 0014's C half)
//!     panel.so/.dll    the panel, which either host loads
//!     panel-host.dll   Windows only: the same panel, importing from the Rust host
//!     panel-noabi.*    the panel with kui_ext_abi deleted; must be refused
//!
//! **The ABI check.** The header is hand-written, so nothing in Rust makes
//! it match the repr(C) structs in `crates/kui-ffi/src/types.rs`: a field
//! added there but missing from — or misordered in — kui.h shifts every
//! field after it, silently, at runtime. The same goes for the enums the
//! API reads as indices into a list the core owns (`KUI_ROLE_*` and the
//! rest), and for the prototypes: an argument Rust gained and the header
//! did not (ABI 12's case) reads a register the caller never filled.
//! `cargo test -p kui-ffi abi_parity` regenerates a translation unit of
//! `_Static_assert`s from the Rust layout and those lists, plus a second
//! declaration of every entry point under its Rust signature; compiling it
//! against the header settles the two, since C refuses two declarations of
//! one function that disagree. Nothing links — everything is checked in the
//! front end. Worth its own run on Windows and not only on Linux: the layout
//! being asserted is the MSVC one, and it is the compiler here that decides
//! it.
//!
//! **What differs on Windows is Windows', not kui's**, and is worth reading:
//!
//! * The compiler must be MSVC-ABI, because that is the ABI the Rust
//!   `x86_64-pc-windows-msvc` target links with. `cl` or `clang-cl`,
//!   whichever is found; a MinGW `gcc` would build and then not link. `cl`
//!   is not on PATH by default, so the `cc` crate's registry lookup (the
//!   same one every `-sys` crate uses) finds the VS install and its
//!   environment.
//! * The hosts link `kui_ffi.dll` through its import library
//!   (`kui_ffi.dll.lib`) and need the DLL beside them at run time, since
//!   Windows has no rpath. Building into `target/<profile>/` is what puts
//!   it there.
//! * A plugin links against an import library rather than against nothing.
//!   A DLL may not have an unresolved import: it names the module each
//!   `kui_*` comes from in its own import table and takes that name from a
//!   `.lib`. Which `.lib` is the choice, and both are built — from one
//!   compile of panel.c, since the two differ only in what the link
//!   resolves against. `panel.dll` takes `kui_ffi.dll`'s, and so loads into
//!   any host shipping that DLL — the shape a plugin you hand to somebody
//!   wants, and the default for both hosts. `panel-host.dll` takes the
//!   *Rust host's*, which `crates/kui-ffi/build.rs` arranges by handing
//!   link.exe a `/DEF:` naming every `kui_*` so that link.exe writes
//!   `c_panel.lib`; that plugin loads into `c_panel.exe` and no other,
//!   because an import library names the module it imports from. On ELF
//!   and Mach-O neither choice exists or is needed: the plugin leaves them
//!   undefined and the host is linked `--export-dynamic`, so one `panel.so`
//!   is built and both hosts load it (Apple's linker has to be told to
//!   allow the undefined symbols; ELF leaves them alone).
//! * The plugin's own entry points need `__declspec(dllexport)`, which
//!   kui.h puts on their declarations as `KUI_EXT_EXPORT`, so panel.c is
//!   the same source on every platform and this tool needs no export list.
//!
//! **The round.** What the artifacts are for, as one list, so that `--run`
//! and the printout cannot disagree and CI runs exactly what a reader is
//! told to run. Each step is headless and self-asserting: it exits 0 having
//! checked something, or non-zero having said what. The last is the one
//! that has to fail, and a non-zero exit is not enough on its own since
//! every other way of failing to load has one too: the message is the
//! check.
//!
//! Exit codes: 0 everything built (and, with `--run`, the round passed), 1
//! at the first failure, 2 a bad flag.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use kui_devtools::manifest;

const USAGE: &str = "usage: cbuild [--release] [--run]";

fn main() -> ExitCode {
    let mut release = false;
    let mut run = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--release" => release = true,
            "--run" => run = true,
            "-h" | "--help" => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
            other => {
                eprintln!("unknown flag {other}\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    if let Err(e) = std::env::set_current_dir(manifest::root()) {
        eprintln!("cannot enter the workspace root: {e}");
        return ExitCode::from(2);
    }
    match build_and_round(release, run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("cbuild: {msg}");
            ExitCode::from(1)
        }
    }
}

/// The compiler, found once, and the flags it takes for one job.
struct Cc {
    /// Everything a compiler invocation starts from: the program and, on
    /// Windows, the VS environment the `cc` crate found for it.
    make: Box<dyn Fn() -> Command>,
    name: String,
    msvc: bool,
}

impl Cc {
    fn find() -> Result<Cc, String> {
        if cfg!(windows) {
            // `cl` through the registry lookup every `-sys` crate uses, so
            // VS Build Tools that are installed and not on PATH still count;
            // then either compiler already on PATH.
            if let Some(tool) = cc::windows_registry::find_tool("x86_64-pc-windows-msvc", "cl.exe")
            {
                let name = tool.path().display().to_string();
                return Ok(Cc {
                    make: Box::new(move || tool.to_command()),
                    name,
                    msvc: true,
                });
            }
            for c in ["clang-cl", "cl"] {
                if Command::new(c)
                    .arg("/?")
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .is_ok()
                {
                    let owned = c.to_string();
                    return Ok(Cc {
                        make: Box::new(move || Command::new(&owned)),
                        name: c.to_string(),
                        msvc: true,
                    });
                }
            }
            return Err(
                "no MSVC-ABI C compiler: install the VS Build Tools (cl) or LLVM \
                        (clang-cl). A MinGW gcc will not do - it links a different ABI \
                        than the Rust msvc target."
                    .into(),
            );
        }
        let name = std::env::var("CC").unwrap_or_else(|_| "cc".into());
        let owned = name.clone();
        Ok(Cc {
            make: Box::new(move || Command::new(&owned)),
            name,
            msvc: false,
        })
    }

    /// One invocation, judged by its exit.
    fn run(&self, args: &[String], what: &str) -> Result<(), String> {
        let status = (self.make)()
            .args(args)
            .status()
            .map_err(|e| format!("{}: {e}", self.name))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{what} failed"))
        }
    }
}

fn s(x: impl AsRef<str>) -> String {
    x.as_ref().to_string()
}

/// A `cargo` invocation with inherited output.
fn cargo(args: &[&str]) -> Result<(), String> {
    let status = Command::new("cargo")
        .args(args)
        .status()
        .map_err(|e| format!("cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`cargo {}` failed", args.join(" ")))
    }
}

/// One step of the round: a program, its arguments, what it checks, and —
/// for the mutant — the refusal its stderr has to carry.
struct Step {
    exe: String,
    args: Vec<String>,
    note: &'static str,
    must_fail: Option<&'static str>,
}

fn build_and_round(release: bool, run: bool) -> Result<(), String> {
    let (profile, profile_dir) = if release {
        ("release", "release")
    } else {
        ("dev", "debug")
    };
    let bin = format!("target/{profile_dir}");
    let ex = format!("{bin}/examples");
    let cc = Cc::find()?;
    println!("compiler: {}", cc.name);

    // Shared flags. `KUI_PROFILE_DIR` is what host.c builds its default
    // plugin path out of, so a --release host looks beside itself rather
    // than in target/debug. On MSVC: `/D_CRT_SECURE_NO_WARNINGS` because
    // conformance.c reads the corpus with fopen/sscanf, which the CRT
    // deprecates and no other platform does; `/utf-8` because the sources
    // carry a few characters that say so (panel.c's " · %d left") and cl
    // reads a BOM-less file in the machine's ANSI code page unless told —
    // gcc and clang assume UTF-8 already, so this is what makes both
    // platforms compile the same bytes.
    let cflags: Vec<String> = if cc.msvc {
        [
            "/nologo",
            "/utf-8",
            "/D_CRT_SECURE_NO_WARNINGS",
            &format!("/DKUI_PROFILE_DIR={profile_dir}"),
            "/I",
            "crates/kui-ffi/include",
            "/std:c11",
            "/W3",
        ]
        .iter()
        .map(s)
        .collect()
    } else {
        [
            "-I",
            "crates/kui-ffi/include",
            "-std=c11",
            &format!("-DKUI_PROFILE_DIR={profile_dir}"),
            "-Wall",
            "-Wextra",
        ]
        .iter()
        .map(s)
        .collect()
    };

    // One build for both: the library the C hosts link, and the Rust host
    // that loads the same plugin they do (whose link, on Windows, is what
    // writes the import library panel-host.dll is linked against).
    cargo(&[
        "build",
        "--profile",
        profile,
        "-p",
        "kui-ffi",
        "--lib",
        "--example",
        "c_panel",
    ])?;
    let ffi_lib = format!("{bin}/kui_ffi.dll.lib");
    let host_lib = format!("{ex}/c_panel.lib");
    if cc.msvc && !Path::new(&host_lib).exists() {
        return Err(format!(
            "{host_lib} missing: the host exported no kui_*, so link.exe wrote no import \
             library. crates/kui-ffi/build.rs is what asks for them."
        ));
    }

    // --- kui.h against the Rust layout (see the header) --------------------
    let abi = "target/kui-abi-assert.c";
    let _ = std::fs::remove_file(abi);
    cargo(&["test", "-p", "kui-ffi", "--lib", "abi_parity"])?;
    let text = std::fs::read_to_string(abi)
        .map_err(|_| format!("{abi} missing: the test filter matched nothing"))?;
    // cl has its own spelling of a syntax-only run; clang-cl takes clang's.
    let syntax_only = if cc.msvc && !cc.name.contains("clang") {
        "/Zs"
    } else {
        "-fsyntax-only"
    };
    let mut args = cflags.clone();
    args.push(s(syntax_only));
    args.push(s(abi));
    cc.run(&args, "kui.h ABI check")?;
    // Anchored, so the #define in the prelude is not counted as a row.
    let count = |pred: &dyn Fn(&str) -> bool| text.lines().filter(|l| pred(l)).count();
    let is_proto = |l: &str| {
        l.ends_with(");")
            && l.starts_with(|c: char| c.is_ascii_alphabetic())
            && l.contains(" kui_")
            && !l.starts_with("KUI_")
    };
    println!(
        "kui.h matches Rust ({} fields, {} enum members, {} prototypes)",
        count(&|l| l.starts_with("KUI_FIELD")),
        count(&|l| l.starts_with("KUI_ENUM")),
        count(&is_proto),
    );

    // --- the hosts --------------------------------------------------------
    // One source into one executable, linking kui_ffi as a library. No rpath
    // spelling both unix linkers take, so the absolute path: the hosts are
    // beside the library either way, and this survives being run from
    // anywhere. On Windows the link is against the import library.
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let exe = |src: &str, out: &str| -> Result<(), String> {
        let out_path = if cc.msvc {
            format!("{bin}/{out}.exe")
        } else {
            format!("{bin}/{out}")
        };
        let mut args = cflags.clone();
        args.push(s(src));
        if cc.msvc {
            args.extend([
                format!("/Fe:{out_path}"),
                s("/Fo:target/"),
                s("/link"),
                ffi_lib.clone(),
            ]);
        } else {
            args.extend([
                s("-L"),
                bin.clone(),
                s("-lkui_ffi"),
                format!("-Wl,-rpath,{}", cwd.join(&bin).display()),
                s("-o"),
                out_path.clone(),
            ]);
        }
        cc.run(&args, &out_path)?;
        println!("built {out_path}");
        Ok(())
    };
    // The counter (the app), the header walk (the surface self-test) and
    // the corpus adapter: three programs over one common.h (ADR 0021); and
    // C on both sides — a C host that loads the same panel through
    // kui_ctx_add_extension / kui_run_with (ADR 0014's C half, ABI 10). It
    // links kui_ffi like counter.c does: a host is a host.
    exe("examples/c/apps/counter.c", "counter")?;
    exe("examples/c/tools/surface.c", "surface")?;
    exe("examples/c/tools/conformance.c", "conformance")?;
    exe("examples/c/features/slots/host.c", "host")?;

    // --- the panel --------------------------------------------------------
    // The other direction: C as an extension inside a host that already
    // owns the window (examples/c/features/slots/panel.rs).
    let plugin_ext = if cc.msvc { "dll" } else { "so" };
    let shared = |src: &str, out: &str, lib: &str| -> Result<(), String> {
        let out_path = format!("{bin}/{out}.{plugin_ext}");
        let mut args = cflags.clone();
        args.push(s(src));
        if cc.msvc {
            args.extend([
                s("/LD"),
                format!("/Fe:{out_path}"),
                s("/Fo:target/"),
                s("/link"),
                s(lib),
            ]);
        } else {
            // No -lkui_ffi and no rpath: the plugin leaves every kui_*
            // symbol undefined and resolves it from the host executable at
            // dlopen time, the way a Lua C module resolves lua_*.
            args.extend([s("-shared"), s("-fPIC")]);
            if cfg!(target_os = "macos") {
                args.push(s("-Wl,-undefined,dynamic_lookup"));
            }
            args.extend([s("-o"), out_path.clone()]);
        }
        cc.run(&args, &out_path)?;
        println!("built {out_path}");
        Ok(())
    };
    let panel_src = "examples/c/features/slots/panel.c";
    if cc.msvc {
        // Compiled once: the two DLLs are the same translation unit and
        // differ only in the import library the link resolves kui_* against.
        let mut args = cflags.clone();
        args.extend([s("/c"), s(panel_src), s("/Fo:target/panel.obj")]);
        cc.run(&args, "panel.obj")?;
        shared("target/panel.obj", "panel", &ffi_lib)?;
        shared("target/panel.obj", "panel-host", &host_lib)?;
    } else {
        shared(panel_src, "panel", "")?;
    }

    // The same plugin with its kui_ext_abi deleted: a plugin built against
    // a header from before ADR 0006 gave plugins a version, which is the
    // one the host must refuse and used to load unchecked (backlog S1).
    // Produced from panel.c by deleting the one line rather than kept as a
    // second source, so the mutant cannot drift from the example. The round
    // below loads it and requires the refusal; the check here is what makes
    // a filter that stopped matching fail at the mutation instead of at the
    // load.
    let noabi = "target/panel-noabi.c";
    let panel = std::fs::read_to_string(panel_src).map_err(|e| format!("{panel_src}: {e}"))?;
    let mutant: String = panel
        .lines()
        .filter(|l| !l.starts_with("uint32_t kui_ext_abi(void)"))
        .map(|l| format!("{l}\n"))
        .collect();
    if mutant.contains("kui_ext_abi") {
        return Err("panel-noabi.c still defines kui_ext_abi; the mutation missed".into());
    }
    std::fs::write(noabi, mutant).map_err(|e| format!("{noabi}: {e}"))?;
    shared(noabi, "panel-noabi", &host_lib)?;
    println!("  (kui_ext_abi deleted; must be refused)");

    // --- the round --------------------------------------------------------
    let exe_name = |name: &str| {
        if cc.msvc {
            format!("{name}.exe")
        } else {
            s(name)
        }
    };
    let mut round = vec![
        Step {
            exe: format!("{bin}/{}", exe_name("counter")),
            args: vec![s("--headless")],
            note: "C as the host: the counter's own drive",
            must_fail: None,
        },
        Step {
            exe: format!("{bin}/{}", exe_name("surface")),
            args: vec![],
            note: "the header walk: every prototype in kui.h called once",
            must_fail: None,
        },
        Step {
            exe: format!("{bin}/{}", exe_name("host")),
            args: vec![s("--headless")],
            note: "C on both sides: the slot filled, the click routed, the reply back",
            must_fail: None,
        },
        Step {
            exe: format!("{ex}/{}", exe_name("c_panel")),
            args: vec![s("--headless")],
            note: "the Rust host, loading the same plugin from the other side",
            must_fail: None,
        },
    ];
    if cc.msvc {
        round.push(Step {
            exe: format!("{ex}/{}", exe_name("c_panel")),
            args: vec![s("--headless"), format!("{bin}/panel-host.dll")],
            note: "the other plugin shape: imports the host executable, loads into it alone",
            must_fail: None,
        });
    }
    round.push(Step {
        exe: format!("{ex}/{}", exe_name("c_panel")),
        args: vec![s("--headless"), format!("{bin}/panel-noabi.{plugin_ext}")],
        note: "kui_ext_abi deleted: must be refused, not loaded",
        must_fail: Some("plugin declares no ABI; this build is"),
    });

    println!();
    if !run {
        println!("next:");
        for step in &round {
            println!("  ./{} {}", step.exe, step.args.join(" "));
            println!("      # {}", step.note);
        }
        println!("  # or pass --run to have this tool run them");
        println!();
        println!(
            "  ./{}/{}              # with a window",
            bin,
            exe_name("counter")
        );
        println!("  ./{}/{}     # with a window", ex, exe_name("c_panel"));
        return Ok(());
    }

    for step in &round {
        println!("+ ./{} {}", step.exe, step.args.join(" "));
        let out = Command::new(PathBuf::from(&step.exe))
            .args(&step.args)
            .output()
            .map_err(|e| format!("{}: {e}", step.exe))?;
        let stderr = String::from_utf8_lossy(&out.stderr);
        for l in String::from_utf8_lossy(&out.stdout)
            .lines()
            .chain(stderr.lines())
        {
            println!("  {l}");
        }
        match step.must_fail {
            None if out.status.success() => {}
            None => return Err(format!("{} failed: {}", step.exe, step.note)),
            Some(_) if out.status.success() => {
                return Err(format!("FAIL: {}: it was accepted", step.note));
            }
            Some(msg) if !stderr.contains(msg) => {
                return Err(format!("{}: refused, but not for that reason", step.note));
            }
            Some(_) => {}
        }
    }
    println!();
    println!(
        "the C round passed ({} checks, {profile_dir} profile)",
        round.len()
    );
    Ok(())
}
