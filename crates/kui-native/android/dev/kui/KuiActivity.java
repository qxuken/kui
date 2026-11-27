package dev.kui;

import android.app.NativeActivity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.text.Editable;
import android.text.InputType;
import android.text.Selection;
import android.view.View;
import android.view.ViewGroup;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;

/** Android mirrors the focused KUI document and reports edits in UTF-16. */
public final class KuiActivity extends NativeActivity {
    private KuiSpelling createKuiSpelling() {
        return new KuiSpelling(getApplicationContext());
    }

    private void copyKuiText(String text) {
        ClipboardManager clipboard = getSystemService(ClipboardManager.class);
        if (clipboard != null) clipboard.setPrimaryClip(ClipData.newPlainText("", text));
    }

    private String pasteKuiText() {
        ClipboardManager clipboard = getSystemService(ClipboardManager.class);
        ClipData clip = clipboard == null ? null : clipboard.getPrimaryClip();
        if (clip == null || clip.getItemCount() == 0) return "";
        ClipData.Item item = clip.getItemAt(0);
        if (item.getText() != null) return item.getText().toString();
        return item.getUri() == null ? "" : item.getUri().toString();
    }

    private View kuiAccessView;
    private native void attachKuiAccess(long request, View host);

    // Enter through the Activity so JNI resolves AccessKit's packaged delegate
    // with this application's class loader on every supported Android version.
    private void createKuiAccess(long request) {
        attachKuiAccess(request, kuiAccessView);
    }

    @Override protected void onCreate(android.os.Bundle state) {
        super.onCreate(state);
        kuiAccessView = new AccessView(this);
        kuiAccessView.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_YES);
        addContentView(kuiAccessView, new ViewGroup.LayoutParams(-1, -1));
    }

    // AccessKit queries the previously installed delegate. View exposed its
    // getter in API 29, so remember the delegate on the API 28 host as well.
    private static final class AccessView extends View {
        private AccessibilityDelegate delegate;
        AccessView(Context context) { super(context); }
        @Override public AccessibilityDelegate getAccessibilityDelegate() { return delegate; }
        @Override public void setAccessibilityDelegate(AccessibilityDelegate value) {
            super.setAccessibilityDelegate(value);
            delegate = value;
        }
    }

    private long kuiEvents;
    private Editor editor;
    private long session;
    private int type, start, end;
    private String text = "";
    private native void updateText(long session, String text, int start, int end, int markStart, int markEnd,
                                   String original);

    public void setKuiEditor(long owner, int kind, String value, int anchor, int caret, boolean show, boolean refresh) {
        runOnUiThread(() -> {
            InputMethodManager ime = getSystemService(InputMethodManager.class);
            boolean newEditor = session != owner || type != kind;
            session = owner;
            type = kind;
            if (refresh) { text = value; start = anchor; end = caret; }
            if (kind == 0) {
                if (editor != null) {
                    editor.active = null;
                    ime.hideSoftInputFromWindow(editor.getWindowToken(), 0);
                    editor.clearFocus();
                }
                return;
            }
            if (editor == null) {
                editor = new Editor(this);
                editor.setFocusableInTouchMode(true);
                editor.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
                addContentView(editor, new ViewGroup.LayoutParams(1, 1));
            }
            editor.requestFocus();
            if (!newEditor && refresh && editor.active != null && android.os.Build.VERSION.SDK_INT >= 33) {
                // A hardware edit changes KUI's document while Gboard still
                // owns this connection. Publish the new surrounding text on
                // this connection, preserving the IME and its keyboard view.
                Editable buffer = editor.active.getEditable();
                BaseInputConnection.removeComposingSpans(buffer);
                editor.active.original = null;
                if (!value.contentEquals(buffer)) buffer.replace(0, buffer.length(), value);
                Selection.setSelection(buffer, anchor, caret);
                ime.invalidateInput(editor);
                ime.updateSelection(editor, anchor, caret, -1, -1);
            } else if (newEditor || refresh) {
                editor.active = null;
                ime.restartInput(editor);
            }
            if (show) ime.showSoftInput(editor, 0);
        });
    }

    private final class Editor extends View {
        Editor(Context context) { super(context); }
        @Override public boolean onCheckIsTextEditor() { return type != 0; }
        @Override public InputConnection onCreateInputConnection(EditorInfo info) {
            if (type == 0) return null;
            info.inputType = type;
            info.initialSelStart = start;
            info.initialSelEnd = end;
            info.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI | EditorInfo.IME_FLAG_NO_FULLSCREEN
                    | ((type & InputType.TYPE_TEXT_FLAG_MULTI_LINE) != 0 ?
                            EditorInfo.IME_ACTION_NONE : EditorInfo.IME_ACTION_DONE);
            Connection connection = new Connection(this);
            active = connection;
            return connection;
        }
        Connection active;
    }

    private final class Connection extends BaseInputConnection {
        final long owner = session;
        int batch;
        boolean dirty;
        String original;
        Connection(Editor view) {
            super(view, true);
            getEditable().replace(0, getEditable().length(), text);
            Selection.setSelection(getEditable(), start, end);
        }
        private boolean changed(boolean ok) {
            if (!ok || session != owner || editor.active != this) return false;
            dirty = true;
            if (batch == 0) publish();
            return true;
        }
        private void publish() {
            dirty = false;
            Editable buffer = getEditable();
            text = buffer.toString();
            start = Selection.getSelectionStart(buffer);
            end = Selection.getSelectionEnd(buffer);
            int lo = getComposingSpanStart(buffer), hi = getComposingSpanEnd(buffer);
            if (lo < 0 || hi <= lo) original = null;
            updateText(owner, text, start, end, lo, hi, original);
            getSystemService(InputMethodManager.class).updateSelection(editor, start, end, lo, hi);
        }
        @Override public boolean beginBatchEdit() { batch++; return true; }
        @Override public boolean endBatchEdit() {
            if (batch > 0 && --batch == 0 && dirty && owner == session && editor.active == this) publish();
            return batch > 0;
        }
        @Override public boolean setSelection(int a, int b) { return changed(super.setSelection(a,b)); }
        @Override public boolean setComposingRegion(int a, int b) {
            Editable buffer = getEditable();
            int oldLo = getComposingSpanStart(buffer), oldHi = getComposingSpanEnd(buffer);
            boolean ok = super.setComposingRegion(a, b);
            int lo = getComposingSpanStart(buffer), hi = getComposingSpanEnd(buffer);
            if (ok && (lo != oldLo || hi != oldHi)) {
                // Marking existing text only prepares a replacement. Keep
                // the original through later updates, even in an IME batch.
                original = lo >= 0 && hi > lo ? buffer.subSequence(lo, hi).toString() : null;
            }
            return changed(ok);
        }
        @Override public boolean setComposingText(CharSequence s,int c) { return changed(super.setComposingText(s,c)); }
        @Override public boolean commitText(CharSequence s,int c) {
            original = null;
            return changed(super.commitText(s,c));
        }
        @Override public boolean finishComposingText() {
            original = null;
            return changed(super.finishComposingText());
        }
        @Override public boolean deleteSurroundingText(int a,int b) { if (a == 1 && b == 0 && getEditable().length() == 0) {
                int key = android.view.KeyEvent.KEYCODE_DEL;
                sendKeyEvent(new android.view.KeyEvent(android.view.KeyEvent.ACTION_DOWN, key));
                return sendKeyEvent(new android.view.KeyEvent(android.view.KeyEvent.ACTION_UP, key));
            }
            return changed(super.deleteSurroundingText(a,b)); }
        @Override public boolean deleteSurroundingTextInCodePoints(int a,int b) {
            return changed(super.deleteSurroundingTextInCodePoints(a,b));
        }
        @Override public boolean sendKeyEvent(android.view.KeyEvent e) {
            return owner == session && editor.active == this && super.sendKeyEvent(e);
        }
    }
}
