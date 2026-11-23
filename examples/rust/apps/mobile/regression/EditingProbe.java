package dev.kui;

import android.app.Activity;
import android.view.View;
import android.view.inputmethod.InputConnection;
import java.lang.reflect.Field;
import java.util.concurrent.FutureTask;
import java.util.concurrent.TimeUnit;

/** Drive the connection Android actually created for this focused view. */
public final class EditingProbe {
    private static InputConnection previous;

    private static String surroundingText(InputConnection field) {
        CharSequence before = field.getTextBeforeCursor(16, 0);
        CharSequence selected = field.getSelectedText(0);
        CharSequence after = field.getTextAfterCursor(16, 0);
        if (before == null || after == null) return null;
        return before.toString() + (selected == null ? "" : selected) + after;
    }

    public static boolean step(Activity activity, int step) throws Exception {
        FutureTask<Boolean> task = new FutureTask<>(() -> {
            View view = activity.getCurrentFocus();
            if (step == 12) return view == null || !view.onCheckIsTextEditor();
            if (view == null || !view.onCheckIsTextEditor()) return false;
            Field active = view.getClass().getDeclaredField("active");
            active.setAccessible(true);
            InputConnection field = (InputConnection)active.get(view);
            if (field == null) return false;
            switch (step) {
            case 1:
                if (!"a🙂b".equals(surroundingText(field))) return false;
                return field.setSelection(1,3);
            case 2:
                if (!"🙂".contentEquals(field.getSelectedText(0))) return false;
                return field.commitText("é",1);
            case 3:
                if (!"aé".contentEquals(field.getTextBeforeCursor(16,0))) return false;
                return field.commitText("!",1);
            case 4:
                if (!"second".equals(surroundingText(field))) return false;
                previous = field;
                return field.setSelection(6,6) && field.setComposingText("か",1);
            case 5:
                if (field == previous || !"aé!b".equals(surroundingText(field))) return false;
                previous.commitText("漏",1);
                previous = null;
                return field.commitText("Z",1);
            case 6:
                if (!"second".equals(surroundingText(field))) return false;
                return field.setSelection(3, 3) && field.setComposingRegion(0, 3);
            case 7:
                return field.setComposingText("日本", 1);
            case 8:
                return "aé!Zb".equals(surroundingText(field));
            case 9:
                if (!"second".equals(surroundingText(field))) return false;
                field.beginBatchEdit();
                try {
                    return field.setComposingRegion(0, 3) && field.setComposingText("日本", 1);
                } finally {
                    field.endBatchEdit();
                }
            case 10:
                if (!"日本ond".equals(surroundingText(field))) return false;
                return field.commitText("日本", 1);
            case 11:
                if (!"aé!Zb".equals(surroundingText(field))) return false;
                return field.setComposingText("か", 1);
            case 13:
                return "aé!Zb".equals(surroundingText(field));
            default: throw new AssertionError(step);
            }
        });
        activity.runOnUiThread(task);
        return task.get(5, TimeUnit.SECONDS);
    }
}
