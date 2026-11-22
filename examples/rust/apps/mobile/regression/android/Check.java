package dev.kui.probe;

import android.app.Activity;
import android.app.Instrumentation;
import android.app.UiAutomation;
import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.concurrent.TimeoutException;

public class Check extends Instrumentation {
    private boolean talkback;
    private UiAutomation automation;

    @Override
    public void onCreate(Bundle args) {
        talkback = args != null && "true".equals(args.getString("talkback"));
        super.onCreate(args);
        start();
    }

    private AccessibilityNodeInfo find(AccessibilityNodeInfo node, String label) {
        if (node == null) return null;
        if (label.equals(String.valueOf(node.getContentDescription()))) return node;
        for (int i = 0; i < node.getChildCount(); i++) {
            AccessibilityNodeInfo found = find(node.getChild(i), label);
            if (found != null) return found;
        }
        return null;
    }

    private AccessibilityNodeInfo waitFor(String label, boolean needsFocus) {
        long deadline = SystemClock.uptimeMillis() + 8000;
        do {
            AccessibilityNodeInfo node = find(automation.getRootInActiveWindow(), label);
            if (node != null && (!needsFocus || node.isFocused())) return node;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Timed out waiting for " + label + (needsFocus ? " to gain focus" : ""));
    }

    private void checkGesture(String gesture, String label) {
        Bundle status = new Bundle();
        status.putString("gesture", gesture);
        try {
            automation.executeAndWaitForEvent(() -> sendStatus(0, status), event -> {
                if (event.getEventType() != AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED) return false;
                AccessibilityNodeInfo source = event.getSource();
                return source != null && label.equals(String.valueOf(source.getContentDescription()));
            }, 8000).recycle();
        } catch (TimeoutException error) {
            throw new AssertionError("TalkBack did not focus " + label + " after " + gesture, error);
        }
    }

    private void checkTalkBack() {
        Rect bounds = new Rect();
        waitFor("Increment", false).getBoundsInScreen(bounds);
        if (bounds.isEmpty()) throw new AssertionError("Increment has no bounds");
        checkGesture("touch " + bounds.centerX() + " " + bounds.centerY(), "Increment");
        automation.getRootInActiveWindow().getBoundsInScreen(bounds);
        checkGesture("swipe " + bounds.width() / 3 + " " + bounds.centerY() + " "
                + bounds.width() * 2 / 3, "Note");
    }

    @Override
    public void onStart() {
        Bundle result = new Bundle();
        try {
            automation = talkback
                    ? getUiAutomation(UiAutomation.FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES)
                    : getUiAutomation();
            if (talkback) {
                checkTalkBack();
                result.putString("result", "PASS TalkBack touch focused Increment and swipe focused Note");
                finish(Activity.RESULT_OK, result);
                return;
            }
            waitFor("Button taps: 0", false);
            if (!waitFor("Increment", false).performAction(AccessibilityNodeInfo.ACTION_CLICK)) {
                throw new AssertionError("Increment declined the click");
            }
            waitFor("Button taps: 1", false);
            if (!waitFor("Note", false).performAction(AccessibilityNodeInfo.ACTION_FOCUS)) {
                throw new AssertionError("Note declined input focus");
            }
            waitFor("Note", true);
            result.putString("result", "PASS virtual button click, updated count and editor focus");
            finish(Activity.RESULT_OK, result);
        } catch (AssertionError | RuntimeException e) {
            result.putString("error", e.toString());
            finish(Activity.RESULT_CANCELED, result);
        }
    }
}
