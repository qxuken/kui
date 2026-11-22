#!/bin/sh
# Called from run.sh after the instrumentation APK has been installed.
set -eu
adb="$ANDROID_HOME/platform-tools/adb"
trace="$1/talkback.log"
case $("$adb" get-serialno) in
    emulator-*) ;;
    *) echo "The TalkBack gesture regression needs an Android emulator" >&2; exit 1 ;;
esac

services=$("$adb" shell settings get secure enabled_accessibility_services)
enabled=$("$adb" shell settings get secure accessibility_enabled)
exploration=$("$adb" shell settings get secure touch_exploration_enabled)
restore() {
    if [ "$2" = null ]; then
        "$adb" shell settings delete secure "$1" >/dev/null
    else
        "$adb" shell settings put secure "$1" "$2"
    fi
}
cleanup() {
    restore enabled_accessibility_services "$services"
    restore accessibility_enabled "$enabled"
    restore touch_exploration_enabled "$exploration"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

"$adb" shell settings put secure enabled_accessibility_services com.google.android.marvin.talkback/.TalkBackService
sleep 1
"$adb" shell am start -S -W -n dev.kui.mobile.spike/dev.kui.KuiActivity --ez dev.kui.mobile.spike.ephemeral true
# Wait for startup insets before reading the first target's screen bounds.
sleep 1
"$adb" shell am instrument -w -r -e talkback true dev.kui.probe/.Check > "$trace" &
observer=$!

await_gesture() {
    for attempt in $(seq 1 25); do
        coordinates=$(sed -n "s/^INSTRUMENTATION_STATUS: gesture=$1 //p" "$trace")
        if [ -n "$coordinates" ]; then
            printf '%s\n' "$coordinates"
            return
        fi
        if ! kill -0 "$observer" 2>/dev/null; then break; fi
        sleep 0.2
    done
    return 1
}

# Use the emulator's touchscreen device, which enters TalkBack's input
# pipeline in the same way as a finger. ADB input tap goes to the app.
if touch=$(await_gesture touch); then
    read -r x y <<EOF
$touch
EOF
    "$adb" emu event mouse "$x" "$y" 0 1
    sleep 0.55
    "$adb" emu event mouse "$x" "$y" 0 0
    # Keep the next gesture outside TalkBack's double-tap window.
    sleep 0.5
fi
if swipe=$(await_gesture swipe); then
    read -r start y end <<EOF
$swipe
EOF
    "$adb" emu event mouse "$start" "$y" 0 1
    for step in 1 2 3 4; do
        "$adb" emu event mouse "$((start + (end - start) * step / 4))" "$y" 0 1
    done
    "$adb" emu event mouse "$end" "$y" 0 0
fi
wait "$observer"
cat "$trace"
grep -q 'INSTRUMENTATION_CODE: -1' "$trace"
