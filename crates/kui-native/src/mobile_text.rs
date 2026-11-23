//! The platform responders use UTF-16 offsets. KUI selections use line and
//! UTF-8 byte offset, and its document excludes the temporary marked text.
use super::{App, Core, Host, InputEvent};

#[derive(Debug)]
pub(super) struct NativeText {
    pub text: String,
    pub selection: (i32, i32),
    pub marked: (i32, i32),
    // Android's setComposingRegion marks an existing part of the document.
    pub original: Option<String>,
}

#[derive(Default, PartialEq)]
pub(super) struct Document {
    pub text: String,
    pub selection: (i32, i32),
}

impl Document {
    pub fn read(core: &Core, key: crate::Key) -> Self {
        let text = core.edit.text(key).unwrap_or_default();
        let (caret, selection) = core.edit.caret_and_selection(key).unwrap_or_default();
        let (lo, hi) = selection.unwrap_or((caret, caret));
        let anchor = if caret == hi { lo } else { hi };
        let offset = |i| text.get(..i).unwrap_or("").encode_utf16().count() as i32;
        Self {
            selection: (offset(anchor), offset(caret)),
            text,
        }
    }
}

fn byte(text: &str, offset: i32) -> usize {
    let mut units = 0;
    for (i, ch) in text.char_indices() {
        units += ch.len_utf16() as i32;
        if units > offset {
            return i;
        }
    }
    text.len()
}

fn select(core: &mut Core, key: crate::Key, text: &str, start: usize, end: usize) {
    let range = (start != end).then_some((start.min(end), start.max(end)));
    if core.edit.caret_and_selection(key) == Some((end, range)) {
        return;
    }
    let position = |i| {
        let prefix = &text[..i];
        (
            prefix.bytes().filter(|&b| b == b'\n').count(),
            prefix.rsplit('\n').next().unwrap().len(),
        )
    };
    core.edit.set_selection(key, position(start), position(end));
}

// Find a single replacement on scalar boundaries, including edits where the
// new and old UTF-8 encodings share leading bytes.
fn replacement(doc: &Document, new: &str) -> (usize, usize, usize) {
    let old = &doc.text;
    // Prefer the actual selection when equal adjacent characters make
    // several replacements yield the same string (important for undo).
    if old != new {
        let a = byte(old, doc.selection.0.min(doc.selection.1));
        let b = byte(old, doc.selection.0.max(doc.selection.1));
        if let Some(rest) = new.strip_prefix(&old[..a])
            && let Some(inserted) = rest.strip_suffix(&old[b..])
        {
            return (a, b, a + inserted.len());
        }
    }
    let start: usize = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum();
    let suffix: usize = old[start..]
        .chars()
        .rev()
        .zip(new[start..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum();
    (start, old.len() - suffix, new.len() - suffix)
}

impl<A: App> Host<A> {
    pub(super) fn native_text(&mut self, key: crate::Key, state: NativeText) {
        let text = &state.text;
        let selection = (byte(text, state.selection.0), byte(text, state.selection.1));
        let marked = (state.marked.0 >= 0 && state.marked.1 >= state.marked.0)
            .then(|| byte(text, state.marked.0)..byte(text, state.marked.1));
        if marked.is_some() && self.core.edit.is_secure(key) {
            return;
        }
        let original = state.original.as_deref().unwrap_or_default();
        let mut committed = text.clone();
        if let Some(range) = &marked {
            committed.replace_range(range.clone(), original);
        }
        let (start, end, new_end) = replacement(&self.document, &committed);
        // A custom sink owns its document. Secure fields likewise never lend
        // existing secrets to the IME. Their native buffer is session-local.
        let mirrors_field = self.core.edit.contains(key) && !self.core.edit.is_secure(key);
        if self.core.edit.preedit(key).is_some() {
            self.input(InputEvent::Preedit(String::new(), None));
        }
        if start != end || start != new_end {
            if mirrors_field {
                select(&mut self.core, key, &self.document.text, start, end);
            }
            if start == new_end {
                self.input(InputEvent::Key(
                    crate::EditKey::Backspace,
                    Default::default(),
                ));
            } else {
                self.input(InputEvent::Commit(committed[start..new_end].into()));
            }
        }
        // An application can move focus in response to a change message.
        if self.editor_key() != Some(key) {
            return;
        }
        let mut native_selection = state.selection;
        match marked {
            Some(range) if mirrors_field && !original.is_empty() => {
                let preview = &text[range.clone()];
                // Merely marking a word can leave the IME's selection
                // outside it. Keep that selection until a preview replaces it.
                if preview == original
                    && [selection.0, selection.1]
                        .iter()
                        .any(|i| *i < range.start || *i > range.end)
                {
                    select(&mut self.core, key, &committed, selection.0, selection.1);
                } else {
                    select(
                        &mut self.core,
                        key,
                        &committed,
                        range.start,
                        range.start + original.len(),
                    );
                    let within = |i: usize| i.clamp(range.start, range.end) - range.start;
                    self.core.preview_ime_replacement(
                        key,
                        preview,
                        Some((within(selection.0), within(selection.1))),
                    );
                    native_selection = (state.marked.0, state.marked.0);
                }
            }
            Some(range) => {
                if mirrors_field {
                    select(&mut self.core, key, &committed, range.start, range.start);
                }
                let within = |i: usize| i.clamp(range.start, range.end) - range.start;
                self.input(InputEvent::Preedit(
                    text[range.clone()].into(),
                    Some((within(selection.0), within(selection.1))),
                ));
                let (lo, hi) = state.marked;
                let project = |offset: i32| {
                    if offset > lo {
                        lo.max(offset - (hi - lo))
                    } else {
                        offset
                    }
                };
                native_selection = (project(state.selection.0), project(state.selection.1));
            }
            None if mirrors_field => {
                select(&mut self.core, key, &committed, selection.0, selection.1);
            }
            None => {}
        }
        // An app can filter a committed edit. The next frame then
        // synchronizes the canonical value back to the responder.
        self.document = Document {
            text: committed,
            selection: native_selection,
        };
        self.editor_stamp = None;
    }
}
