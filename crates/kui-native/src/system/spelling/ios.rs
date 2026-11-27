//! UITextChecker reports NSString ranges in UTF-16. The shared worker caches UTF-8 ranges.
use kui_core::spelling::SpellingChoices;
use std::{ffi::c_void, ops::Range, ptr::NonNull};

unsafe extern "C" {
    fn kui_ios_spell_open() -> *mut c_void;
    fn kui_ios_spell_close(dictionary: *mut c_void);
    fn kui_ios_spell_check(
        dictionary: *mut c_void,
        text: *const u8,
        len: usize,
        context: *mut c_void,
        emit: extern "C" fn(*mut c_void, usize, usize),
    );
    fn kui_ios_spell_choices(
        dictionary: *mut c_void,
        text: *const u8,
        len: usize,
        context: *mut c_void,
        emit: extern "C" fn(*mut c_void, *const u8, usize),
    ) -> bool;
    fn kui_ios_spell_learn(text: *const u8, len: usize) -> bool;
}

pub(super) struct Dictionary(NonNull<c_void>);

impl Dictionary {
    pub(super) fn open() -> Option<Self> {
        NonNull::new(unsafe { kui_ios_spell_open() }).map(Self)
    }
}

impl Drop for Dictionary {
    fn drop(&mut self) {
        unsafe { kui_ios_spell_close(self.0.as_ptr()) }
    }
}

impl super::Dictionary for Dictionary {
    fn check(&self, text: &str) -> Vec<Range<usize>> {
        extern "C" fn range(context: *mut c_void, start: usize, end: usize) {
            // The bridge calls back synchronously, in ascending order.
            unsafe { &mut *context.cast::<Vec<Range<usize>>>() }.push(start..end);
        }
        let mut ranges: Vec<Range<usize>> = Vec::new();
        unsafe {
            kui_ios_spell_check(
                self.0.as_ptr(),
                text.as_ptr(),
                text.len(),
                (&raw mut ranges).cast(),
                range,
            );
        }
        // Walk the string once even if there are many misspelled words.
        let mut chars = text.chars();
        let mut utf16 = 0;
        let mut byte = 0;
        let mut offset = |target| {
            while utf16 < target {
                let ch = chars.next()?;
                utf16 += ch.len_utf16();
                byte += ch.len_utf8();
            }
            (utf16 == target).then_some(byte)
        };
        ranges.retain_mut(|r| {
            let (Some(start), Some(end)) = (offset(r.start), offset(r.end)) else {
                return false;
            };
            *r = start..end;
            true
        });
        ranges
    }

    fn choices(&self, word: &str) -> SpellingChoices {
        extern "C" fn suggestion(context: *mut c_void, bytes: *const u8, len: usize) {
            let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
            if let Ok(value) = str::from_utf8(bytes) {
                unsafe { &mut *context.cast::<Vec<String>>() }.push(value.to_owned());
            }
        }
        let mut suggestions: Vec<String> = Vec::new();
        let can_learn = unsafe {
            kui_ios_spell_choices(
                self.0.as_ptr(),
                word.as_ptr(),
                word.len(),
                (&raw mut suggestions).cast(),
                suggestion,
            )
        };
        SpellingChoices {
            suggestions,
            can_learn,
        }
    }

    fn learn(&mut self, word: &str) -> bool {
        unsafe { kui_ios_spell_learn(word.as_ptr(), word.len()) }
    }
}
