package dev.kui.editingime;

import android.inputmethodservice.InputMethodService;

/** Lets the probe drive the real served connection without a second editor client. */
public final class EditingIme extends InputMethodService {
    @Override public boolean onEvaluateInputViewShown() {
        return false;
    }
}
