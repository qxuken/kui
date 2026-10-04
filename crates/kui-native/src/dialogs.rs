//! The file dialogs an app asks for: `Core::request_files`
//! queues the ask, and the runner shows the platform's own Open, Save or
//! folder panel through rfd, as a sheet on the window that asked where
//! the platform has sheets.
//!
//! The panel is rfd's *async* one, so the loop never blocks in a modal:
//! the future is made here, on the loop's thread (macOS insists), and
//! waited on by a thread of its own, which posts the answer back through
//! the event loop as `UserEvent::Files`. `user_event` hands it to the
//! window's core as `InputEvent::Files`, and the core hands whoever asked
//! `{kind:"files", paths, tag}`.
//!
//! Without the `dialogs` feature every ask is answered at once with no
//! paths — what a cancelled dialog answers — so an app that asks is never
//! left waiting for a panel that will not come.

use crate::DynShell;
use kui_core::InputEvent;
use winit::event_loop::ActiveEventLoop;

impl DynShell<'_> {
    pub(super) fn show_file_dialogs(&mut self, event_loop: &ActiveEventLoop, i: usize) {
        let Some(pane) = self.panes.get_mut(i) else {
            return;
        };
        for dialog in pane.core.take_file_requests() {
            #[cfg(feature = "dialogs")]
            if let Some(proxy) = self.proxy.clone() {
                show(dialog, &self.panes[i].window, proxy);
                continue;
            }
            let _ = dialog;
            self.dispatch(event_loop, i, InputEvent::Files(Vec::new()));
        }
    }
}

#[cfg(feature = "dialogs")]
fn show(
    dialog: kui_core::FileDialog,
    window: &winit::window::Window,
    proxy: winit::event_loop::EventLoopProxy<crate::access_bridge::UserEvent>,
) {
    use kui_core::FileDialogMode;
    use std::future::Future;
    use std::pin::Pin;

    let mut d = rfd::AsyncFileDialog::new().set_parent(window);
    if let Some(t) = &dialog.title {
        d = d.set_title(t);
    }
    for f in &dialog.filters {
        d = d.add_filter(&f.name, &f.extensions);
    }
    if let Some(dir) = &dialog.directory {
        d = d.set_directory(dir);
    }
    if let Some(name) = &dialog.file_name {
        d = d.set_file_name(name);
    }
    let path = |h: &rfd::FileHandle| h.path().to_string_lossy().into_owned();
    // An Open or folder panel answers only with what is there. Windows'
    // refuses a typed name that is not in a single-file Open, but rfd's
    // multi-select and folder panels replace the options that ask it to,
    // and a name typed there came back as a path (backlog RG43).
    let file = |h: &rfd::FileHandle| h.path().is_file();
    let folder = |h: &rfd::FileHandle| h.path().is_dir();
    // Each mode's future is made now, on this thread; only the waiting
    // moves off it.
    let answer: Pin<Box<dyn Future<Output = Vec<String>> + Send>> =
        match (dialog.mode, dialog.multiple) {
            (FileDialogMode::Open, false) => {
                let f = d.pick_file();
                Box::pin(async move { f.await.iter().filter(|h| file(h)).map(path).collect() })
            }
            (FileDialogMode::Open, true) => {
                let f = d.pick_files();
                Box::pin(async move {
                    f.await
                        .unwrap_or_default()
                        .iter()
                        .filter(|h| file(h))
                        .map(path)
                        .collect()
                })
            }
            (FileDialogMode::Save, _) => {
                let f = d.save_file();
                Box::pin(async move { f.await.iter().map(path).collect() })
            }
            (FileDialogMode::Folder, false) => {
                let f = d.pick_folder();
                Box::pin(async move { f.await.iter().filter(|h| folder(h)).map(path).collect() })
            }
            (FileDialogMode::Folder, true) => {
                let f = d.pick_folders();
                Box::pin(async move {
                    f.await
                        .unwrap_or_default()
                        .iter()
                        .filter(|h| folder(h))
                        .map(path)
                        .collect()
                })
            }
        };
    let window = window.id();
    std::thread::Builder::new()
        .name("kui-file-dialog".into())
        .spawn(move || {
            let paths = pollster::block_on(answer);
            let _ = proxy.send_event(crate::access_bridge::UserEvent::Files { window, paths });
        })
        .expect("a thread to wait on the dialog");
}
