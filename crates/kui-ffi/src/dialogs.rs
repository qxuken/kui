//! File dialogs as an ask (backlog C51): a view or a host asks for the
//! platform's Open, Save or folder dialog, and the answer is a `files`
//! event. Under `kui_run` the runner shows it; a host driving its own
//! window drains the ask with `kui_take_file_request` and answers with
//! `kui_input_files`.

use super::*;

/// `KUI_FILE_DIALOG_OPEN`: an existing file, or several.
pub const KUI_FILE_DIALOG_OPEN: u32 = 0;
/// `KUI_FILE_DIALOG_SAVE`: a path to write.
pub const KUI_FILE_DIALOG_SAVE: u32 = 1;
/// `KUI_FILE_DIALOG_FOLDER`: a folder, or several.
pub const KUI_FILE_DIALOG_FOLDER: u32 = 2;

/// [in] One entry of a dialog's file-type menu.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiFileFilter {
    pub name: KuiStr,
    /// `extension_count` extensions, without the dot.
    pub extensions: *const KuiStr,
    pub extension_count: usize,
}

/// [in] The dialog `kui_request_files` asks for. Zeroed, it is an Open
/// dialog for one file of any type.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiFileDialog {
    /// `KUI_FILE_DIALOG_*`.
    pub mode: u32,
    /// Nonzero: more than one file or folder may be picked.
    pub multiple: u32,
    /// Empty for the platform's own.
    pub title: KuiStr,
    pub filters: *const KuiFileFilter,
    pub filter_count: usize,
    /// The folder it opens in; empty for the platform's choice.
    pub directory: KuiStr,
    /// A save dialog's suggested name; empty for none.
    pub file_name: KuiStr,
}

fn opt(s: KuiStr) -> Option<String> {
    let s = kstr(s);
    (!s.is_empty()).then(|| s.into_owned())
}

fn dialog_of(d: &KuiFileDialog) -> Result<kui_core::FileDialog, String> {
    use kui_core::FileDialogMode;
    let mode = match d.mode {
        KUI_FILE_DIALOG_OPEN => FileDialogMode::Open,
        KUI_FILE_DIALOG_SAVE => FileDialogMode::Save,
        KUI_FILE_DIALOG_FOLDER => FileDialogMode::Folder,
        m => {
            return Err(format!(
                "KuiFileDialog.mode must be a KUI_FILE_DIALOG_*, not {m}"
            ));
        }
    };
    let filters = if d.filters.is_null() || d.filter_count == 0 {
        &[][..]
    } else {
        // SAFETY: the host promises `filter_count` filters at `filters`.
        unsafe { std::slice::from_raw_parts(d.filters, d.filter_count) }
    };
    let filters = filters
        .iter()
        .map(|f| {
            let exts = if f.extensions.is_null() || f.extension_count == 0 {
                &[][..]
            } else {
                // SAFETY: as above, for the filter's own array.
                unsafe { std::slice::from_raw_parts(f.extensions, f.extension_count) }
            };
            kui_core::FileFilter {
                name: kstr(f.name).into_owned(),
                extensions: exts
                    .iter()
                    .map(|e| kstr(*e).trim_start_matches('.').to_string())
                    .collect(),
            }
        })
        .collect();
    Ok(kui_core::FileDialog {
        mode,
        multiple: d.multiple != 0,
        title: opt(d.title),
        filters,
        directory: opt(d.directory),
        file_name: opt(d.file_name),
        tag: kui_core::Value::Null,
    })
}

/// Asks for the platform's Open, Save or folder dialog. The answer is a
/// `{kind:"files", paths, tag}` event — `paths` empty when the user
/// cancelled — to whoever asked. `dialog` NULL is an Open dialog for one
/// file; `tag` may be NULL and is consumed. False when one is already out
/// (one dialog at a time) or the dialog is malformed.
#[unsafe(no_mangle)]
pub extern "C" fn kui_request_files(
    ptr: *mut KuiCtx,
    dialog: *const KuiFileDialog,
    tag: *mut KuiValue,
) -> bool {
    guard(false, || {
        let tag = take_msg(tag).unwrap_or(kui_core::Value::Null);
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let mut d = match unsafe { dialog.as_ref() } {
            None => kui_core::FileDialog::open(),
            Some(d) => match dialog_of(d) {
                Ok(d) => d,
                Err(why) => {
                    eprintln!("kui: kui_request_files: {why}");
                    return false;
                }
            },
        };
        d.tag = tag;
        c.core().request_files(d)
    })
}

/// Whether a file dialog asked for is still unanswered.
#[unsafe(no_mangle)]
pub extern "C" fn kui_awaiting_files(ptr: *mut KuiCtx) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().awaiting_files())
    })
}

fn lend(out: *mut KuiStr, s: &str) {
    if let Some(o) = unsafe { out.as_mut() } {
        *o = KuiStr {
            ptr: s.as_ptr(),
            len: s.len(),
        };
    }
}

/// Drains the dialog asked for, for a host that shows it itself (under
/// `kui_run` the runner does): its `KUI_FILE_DIALOG_*` mode, whether it
/// picks several, its title, folder and suggested name (empty for none),
/// and how many filters it offers — read each with
/// `kui_file_request_filter`. Any out pointer may be NULL; the strings are
/// borrowed until the next call on this context. False when nothing is
/// asked. Answer with `kui_input_files`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_take_file_request(
    ptr: *mut KuiCtx,
    mode: *mut u32,
    multiple: *mut bool,
    title: *mut KuiStr,
    directory: *mut KuiStr,
    file_name: *mut KuiStr,
    filter_count: *mut usize,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(d) = c.core().take_file_requests().into_iter().next() else {
            return false;
        };
        let code = match d.mode {
            kui_core::FileDialogMode::Open => KUI_FILE_DIALOG_OPEN,
            kui_core::FileDialogMode::Save => KUI_FILE_DIALOG_SAVE,
            kui_core::FileDialogMode::Folder => KUI_FILE_DIALOG_FOLDER,
        };
        unsafe {
            if let Some(m) = mode.as_mut() {
                *m = code;
            }
            if let Some(m) = multiple.as_mut() {
                *m = d.multiple;
            }
            if let Some(n) = filter_count.as_mut() {
                *n = d.filters.len();
            }
        }
        let d = c.file_request.insert(d);
        lend(title, d.title.as_deref().unwrap_or(""));
        lend(directory, d.directory.as_deref().unwrap_or(""));
        lend(file_name, d.file_name.as_deref().unwrap_or(""));
        true
    })
}

/// Filter `i` of the dialog `kui_take_file_request` last handed out: its
/// name, and its extensions joined with `;` (`"png;jpg"`, no dots).
/// Borrowed until the next call; false for a filter that is not there.
#[unsafe(no_mangle)]
pub extern "C" fn kui_file_request_filter(
    ptr: *mut KuiCtx,
    i: usize,
    name: *mut KuiStr,
    extensions: *mut KuiStr,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(f) = c.file_request.as_ref().and_then(|d| d.filters.get(i)) else {
            return false;
        };
        c.file_filter_text = f.extensions.join(";");
        let f = &c.file_request.as_ref().expect("checked above").filters[i];
        lend(name, &f.name);
        lend(extensions, &c.file_filter_text);
        true
    })
}

/// A file dialog's answer: the `count` paths picked, none for a cancelled
/// dialog. Whoever asked hears `{kind:"files", paths, tag}`; with nothing
/// asked it is dropped.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_files(ptr: *mut KuiCtx, paths: *const KuiStr, count: usize) {
    guard((), || {
        let paths = if paths.is_null() || count == 0 {
            Vec::new()
        } else {
            // SAFETY: the host promises `count` strings at `paths`.
            unsafe { std::slice::from_raw_parts(paths, count) }
                .iter()
                .map(|s| kstr(*s).into_owned())
                .collect()
        };
        push_input(ptr, kui_core::InputEvent::Files(paths));
    });
}
