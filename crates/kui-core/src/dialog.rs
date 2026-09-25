//! File dialogs as an ask (backlog C51): the app asks for an Open, a Save
//! or a folder, the host shows the platform's own dialog, and the answer
//! comes back as one event — `{kind:"files", paths, tag}`, the payload
//! shape a drop zone's `drop` carries (ADR 0031), `paths` empty for a
//! dialog the user cancelled.
//!
//! The core cannot show a dialog, so it does what it does for a paste
//! (`Core::request_paste`): it queues the ask, a host drains it
//! (`Core::take_file_requests` — the runner does, a Node `Ctx` or a C host
//! driving its own window does it by hand), and the host's answer is an
//! input (`InputEvent::Files`). One ask at a time: a second while one is
//! unanswered is dropped, so a view that asks every frame until the answer
//! lands asks once, and the answer goes to whoever asked — the host, or
//! the extension whose fill asked.

use crate::key::Key;
use crate::tree::OriginId;
use crate::value::Value;

/// What a dialog picks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FileDialogMode {
    /// An existing file (or several, with `multiple`).
    #[default]
    Open,
    /// A path to write: the file need not exist, and the platform asks
    /// before one that does is replaced.
    Save,
    /// A folder (or several, with `multiple`).
    Folder,
}

impl FileDialogMode {
    pub const ALL: [FileDialogMode; 3] = [Self::Open, Self::Save, Self::Folder];

    /// The spelling the bindings use: `"open"`, `"save"`, `"folder"`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Save => "save",
            Self::Folder => "folder",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == s)
    }
}

/// One entry of a dialog's file-type menu: `Images` over `png`, `jpg`.
/// Extensions are without the dot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

/// An Open, Save or folder dialog, as an app asks for one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileDialog {
    pub mode: FileDialogMode,
    /// More than one file or folder may be picked (Open and Folder).
    pub multiple: bool,
    /// The dialog's title, where the platform shows one.
    pub title: Option<String>,
    /// The file types offered, the first one chosen; none is any file.
    pub filters: Vec<FileFilter>,
    /// The folder it opens in; the platform's choice when absent.
    pub directory: Option<String>,
    /// A Save dialog's suggested name.
    pub file_name: Option<String>,
    /// Handed back on the `files` event, as a node's tag is.
    pub tag: Value,
}

impl FileDialog {
    /// An Open dialog for one existing file.
    pub fn open() -> Self {
        Self::default()
    }

    /// A Save dialog.
    pub fn save() -> Self {
        Self {
            mode: FileDialogMode::Save,
            ..Self::default()
        }
    }

    /// A folder picker.
    pub fn folder() -> Self {
        Self {
            mode: FileDialogMode::Folder,
            ..Self::default()
        }
    }

    pub fn multiple(mut self) -> Self {
        self.multiple = true;
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Offers the file type `name`, matching `extensions` (without the
    /// dot). The first filter added is the one chosen when it opens.
    pub fn filter(mut self, name: impl Into<String>, extensions: &[&str]) -> Self {
        self.filters.push(FileFilter {
            name: name.into(),
            extensions: extensions.iter().map(|e| e.to_string()).collect(),
        });
        self
    }

    pub fn directory(mut self, dir: impl Into<String>) -> Self {
        self.directory = Some(dir.into());
        self
    }

    pub fn file_name(mut self, name: impl Into<String>) -> Self {
        self.file_name = Some(name.into());
        self
    }

    pub fn tag(mut self, tag: impl Into<Value>) -> Self {
        self.tag = tag.into();
        self
    }

    /// The ask as plain data — `{mode, multiple, title?, filters: [{name,
    /// extensions}], directory?, fileName?, tag}` — what a Node host's
    /// `takeFileRequests` hands out and a script's `request_files` takes.
    pub fn to_value(&self) -> Value {
        let mut entries = vec![
            ("mode", Value::str(self.mode.name())),
            ("multiple", Value::Bool(self.multiple)),
            (
                "filters",
                Value::list(self.filters.iter().map(|f| {
                    Value::map([
                        ("name", Value::str(&f.name)),
                        (
                            "extensions",
                            Value::list(f.extensions.iter().map(Value::str)),
                        ),
                    ])
                })),
            ),
            ("tag", self.tag.clone()),
        ];
        if let Some(t) = &self.title {
            entries.push(("title", Value::str(t)));
        }
        if let Some(d) = &self.directory {
            entries.push(("directory", Value::str(d)));
        }
        if let Some(n) = &self.file_name {
            entries.push(("file_name", Value::str(n)));
        }
        Value::map(entries)
    }

    /// Reads the plain-data shape back, `file_name` or `fileName` alike.
    /// `Err` names what is wrong: an unknown mode, a filter without a name
    /// or with extensions that are not strings.
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let mode = match s("mode") {
            None => FileDialogMode::Open,
            Some(m) => FileDialogMode::from_name(&m)
                .ok_or_else(|| format!("unknown dialog mode {m:?} (open, save or folder)"))?,
        };
        let mut filters = Vec::new();
        if let Some(list) = v.get("filters").and_then(Value::as_list) {
            for f in list {
                let name = f
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or("a filter needs a `name`")?
                    .to_string();
                let extensions = f
                    .get("extensions")
                    .and_then(Value::as_list)
                    .unwrap_or(&[])
                    .iter()
                    .map(|e| {
                        e.as_str()
                            .map(|e| e.trim_start_matches('.').to_string())
                            .ok_or("a filter's `extensions` are strings")
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                filters.push(FileFilter { name, extensions });
            }
        }
        Ok(FileDialog {
            mode,
            multiple: v.get("multiple").and_then(Value::as_bool).unwrap_or(false),
            title: s("title"),
            filters,
            directory: s("directory"),
            file_name: s("file_name").or_else(|| s("fileName")),
            tag: v.get("tag").cloned().unwrap_or(Value::Null),
        })
    }
}

/// Where the one ask stands.
#[derive(Clone, Debug, Default)]
pub(crate) enum FileAsk {
    #[default]
    None,
    /// Asked, not yet taken by a host.
    Queued(FileDialog, OriginId),
    /// Taken, not yet answered: what the answer carries back and to whom.
    Taken(Value, OriginId),
}

impl FileAsk {
    pub(crate) fn pending(&self) -> bool {
        !matches!(self, FileAsk::None)
    }

    /// The `files` event for the answer `paths`, and the ask spent; `None`
    /// when nothing was asked — a late or stray answer is dropped.
    pub(crate) fn answer(&mut self, paths: &[String]) -> Option<crate::input::UiEvent> {
        let (tag, origin) = match std::mem::take(self) {
            FileAsk::None => return None,
            FileAsk::Queued(d, origin) => (d.tag, origin),
            FileAsk::Taken(tag, origin) => (tag, origin),
        };
        Some(
            crate::input::UiEvent::on(
                origin,
                Key::ROOT,
                Value::map([
                    ("kind", Value::str("files")),
                    ("paths", Value::list(paths.iter().map(Value::str))),
                ]),
            )
            .tagged(Some(&tag)),
        )
    }
}
