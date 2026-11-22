//! Load a source .kui embedded in a native application.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use kui_core::{Core, Extensions, Fill, NodeSpec, OriginId, Ui, Value};
use kui_lua::LuaExtension;

use crate::{
    app::{self, Metadata, Paths},
    archive::{ZipReader, is_safe_name, sha256_hex},
    kit,
};

pub struct EmbeddedApp {
    drain: mlua::Function,
    kit: OriginId,
    worked: bool,
}

impl EmbeddedApp {
    /// The pack is part of the app bundle. Use the platform's private data
    /// and cache directories so replacing the pack preserves the app's data.
    pub fn load(
        bytes: &[u8],
        data: &Path,
        cache: &Path,
    ) -> Result<(Self, Extensions), Box<dyn Error>> {
        let zip = ZipReader::open(bytes.into())?;
        let lua = mlua::Lua::new();
        let manifest = zip
            .get("kui.pack.lua")
            .ok_or("missing kui.pack.lua")?
            .read()?;
        let manifest: mlua::Table = lua
            .load(&manifest)
            .set_name("@kui.pack.lua")
            .set_mode(mlua::chunk::ChunkMode::Text)
            .eval()?;
        let runner: String = manifest.get("runner")?;
        let current = env!("CARGO_PKG_VERSION");
        if runner != current {
            return Err(format!("pack requires kui {runner}, this runner is {current}").into());
        }
        if manifest.get::<Option<bool>>("bytecode")?.unwrap_or(false) {
            return Err("embedded apps require a source .kui pack".into());
        }
        let id: String = manifest.get("id")?;
        if !id.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        }) {
            return Err("invalid packed app identifier".into());
        }
        let name: String = manifest.get("name")?;
        let version: String = manifest.get("version")?;

        let root = unpack(&zip, &cache.join("packs"), &sha256_hex(bytes))?;
        let entry = entry(&lua, &root, &manifest)?;
        let source = fs::read_to_string(&entry)?;
        let paths = Paths::new(&root, data.join(&id), cache.join("data").join(&id));
        let app =
            LuaExtension::from_source_with(format!("@{}", entry.display()), &source, |lua| {
                app::install(
                    lua,
                    Metadata {
                        name: &name,
                        id: &id,
                        version: &version,
                        platform: std::env::consts::OS,
                    },
                    &paths,
                )
            })?;
        let drain = app::initialize(&app)?;
        let mut extensions = Extensions::new();
        let kit = extensions.add(kit::NAMESPACE, Box::new(kit::extension()?))?;
        extensions.push_as("app", Box::new(app))?;
        Ok((
            Self {
                drain,
                kit,
                worked: false,
            },
            extensions,
        ))
    }
}

impl kui_native::App for EmbeddedApp {
    fn before_frame(&mut self, _core: &mut Core) {
        self.worked = self.drain.call::<bool>(()).unwrap_or_else(|error| {
            eprintln!("{error}");
            false
        });
    }

    fn view(&mut self, ui: &mut Ui<'_>) {
        ui.configure_root(NodeSpec::column().fill().padding(ui.safe_area()));
        if !ui.core().tokens_declared(self.kit) {
            ui.component("kit/settings", &Value::Null);
        }
        if self.worked {
            ui.request_frame();
        }
    }
}

fn entry(lua: &mlua::Lua, root: &Path, manifest: &mlua::Table) -> Result<PathBuf, Box<dyn Error>> {
    let mut entry: Option<String> = manifest.get("entry")?;
    if entry.is_none() && root.join("kui.config.lua").is_file() {
        let config: mlua::Table = lua
            .load(fs::read(root.join("kui.config.lua"))?)
            .set_name("@kui.config.lua")
            .set_mode(mlua::chunk::ChunkMode::Text)
            .eval()?;
        entry = config.get("entry")?;
    }
    let entry = entry
        .or_else(|| {
            ["main.lua", "app.lua"]
                .into_iter()
                .find(|name| root.join(name).is_file())
                .map(str::to_owned)
        })
        .ok_or("pack has no Lua entry")?;
    if !is_safe_name(&entry) || entry.contains('\\') || !entry.ends_with(".lua") {
        return Err(format!("unsupported packed app entry {entry}").into());
    }
    Ok(root.join(entry))
}

fn unpack(zip: &ZipReader, cache: &Path, hash: &str) -> Result<PathBuf, Box<dyn Error>> {
    let root = cache.join(hash);
    if root.is_dir() {
        return Ok(root);
    }
    fs::create_dir_all(cache)?;
    let staging = tempfile::tempdir_in(cache)?;
    for member in zip.entries() {
        let Some(name) = member.name().strip_prefix("app/") else {
            continue;
        };
        if name.is_empty() && member.is_dir() {
            continue;
        }
        if member.is_special() || !is_safe_name(name) || name.contains('\\') {
            return Err(format!("invalid packed app path {name}").into());
        }
        let path = staging.path().join(name);
        if member.is_dir() {
            fs::create_dir_all(&path)?;
        } else {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, member.read()?)?;
        }
    }
    // A concurrent opener can publish the same complete archive first.
    if let Err(error) = fs::rename(staging.path(), &root)
        && !root.is_dir()
    {
        return Err(error.into());
    }
    Ok(root)
}
