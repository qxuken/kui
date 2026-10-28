//! Files on the clipboard (backlog F247, from Carnet's "Copy file"):
//! `set_clipboard_files` queues the paths for the host, as the other
//! clipboard writes are, and refuses a list with nothing in it.

use kui_core::{Core, MenuAction};

#[test]
fn files_are_queued_for_the_host_and_an_empty_list_is_refused() {
    let mut core = Core::new();
    assert!(core.set_clipboard_files(["C:\\notes\\a.pdf", "", "/tmp/b c.png"]));
    assert_eq!(
        core.take_menu_actions(),
        [MenuAction::SetClipboardFiles {
            paths: vec!["C:\\notes\\a.pdf".into(), "/tmp/b c.png".into()],
        }],
        "the empty one dropped, the rest as named"
    );
    assert!(!core.set_clipboard_files(Vec::<String>::new()));
    assert!(!core.set_clipboard_files([""]));
    assert!(core.take_menu_actions().is_empty(), "nothing queued");
}
