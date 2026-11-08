use std::{env, fs, path::PathBuf};

const DESKTOP: &str = include_str!("desktop-only.txt");

fn main() -> std::io::Result<()> {
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        return Ok(());
    }
    println!("cargo:rerun-if-changed=desktop-only.txt");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../rust")
        .canonicalize()?;
    let lua = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("wasi");
    let mut modules = String::new();
    let mut arms = String::new();
    for kind in ["apps", "widgets", "features", "tutorial"] {
        let dir = root.join(kind);
        println!("cargo:rerun-if-changed={}", dir.display());
        let mut files = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
        files.sort_by_key(|entry| entry.file_name());
        for entry in files {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            let name = path.file_stem().unwrap().to_str().unwrap();
            let id = format!("{kind}/{name}");
            if DESKTOP.lines().any(|name| name == id) || lua != (id == "tutorial/11_extensions") {
                continue;
            }
            let module = format!("{kind}_{}", name.replace('-', "_"));
            modules.push_str(&format!("#[path = {path:?}] mod {module};\n"));
            arms.push_str(&format!("{id:?} => {module}::main(),\n"));
        }
    }
    if lua {
        arms.push_str("\"lua/tasks\" => lua_app::launch_tasks(),\n");
        arms.push_str("\"lua/presentations\" => lua_app::launch_presentation(),\n");
    }
    modules.push_str(&format!(
        "fn launch(id: &str) -> Result<(), Box<dyn std::error::Error>> {{ match id {{ {arms} _ => Err(format!(\"Unknown book example: {{id}}\").into()), }} }}",
    ));
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("examples.rs"),
        modules,
    )
}
