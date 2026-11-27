#!/bin/sh
# The probe drives Android's served connection directly. A normal IME can
# finish composing text it did not start as soon as it receives the selection.
set -eu
cd "$(dirname "$0")"
: "${ANDROID_HOME:?Set ANDROID_HOME to the Android SDK}"
tools="$ANDROID_HOME/build-tools/35.0.0"
platform="$ANDROID_HOME/platforms/android-35/android.jar"
adb="$ANDROID_HOME/platform-tools/adb"
app=dev.kui.mobile.spike
fixture=dev.kui.editingime
method="$fixture/.EditingIme"
runs=${KUI_EDITING_REPEATS:-1}
case "$runs" in
    *[!0-9]*) echo "KUI_EDITING_REPEATS must be a positive integer" >&2; exit 1 ;;
esac
if ! [ "$runs" -gt 0 ]; then
    echo "KUI_EDITING_REPEATS must be a positive integer" >&2
    exit 1
fi

previous=$("$adb" shell settings get secure default_input_method)
if [ -z "$previous" ] || [ "$previous" = null ] || [ "$previous" = "$method" ]; then
    echo "Select a regular keyboard before running the editing regression" >&2
    exit 1
fi
subtype=$("$adb" shell settings get secure selected_input_method_subtype)
history=$("$adb" shell settings get secure input_methods_subtype_history)
spell_checker=$("$adb" shell settings get secure selected_spell_checker)
spell_subtype=$("$adb" shell settings get secure selected_spell_checker_subtype)
spell_enabled=$("$adb" shell settings get secure spell_checker_enabled)
out=$(mktemp -d)
installed=false

restore_setting() {
    # The history contains semicolons. Pass it as data to the device shell.
    if [ "$2" = null ]; then
        "$adb" shell settings delete secure "$1"
    else
        printf '%s\n' "$2" | "$adb" shell -T "IFS= read -r value && settings put secure $1 \"\$value\""
    fi
}

cleanup() {
    result=$?
    if [ "$installed" = true ]; then
        "$adb" shell ime set "$previous" >/dev/null || result=1
        restore_setting selected_spell_checker "$spell_checker" >/dev/null || result=1
        restore_setting selected_spell_checker_subtype "$spell_subtype" >/dev/null || result=1
        restore_setting spell_checker_enabled "$spell_enabled" >/dev/null || result=1
        "$adb" uninstall "$fixture" >/dev/null || result=1
        restore_setting input_methods_subtype_history "$history" >/dev/null || result=1
        restore_setting selected_input_method_subtype "$subtype" >/dev/null || result=1
    fi
    rm -rf "$out"
    exit "$result"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

mkdir "$out/classes"
javac --release 8 -classpath "$platform" -d "$out/classes" EditingIme.java EditingSpellChecker.java
jar cf "$out/ime.jar" -C "$out/classes" .
"$tools/d8" --min-api 28 --lib "$platform" --output "$out" "$out/ime.jar"
"$tools/aapt2" compile --dir res -o "$out/resources.zip"
"$tools/aapt2" link -o "$out/base.apk" --manifest AndroidManifest.xml -I "$platform" "$out/resources.zip"
(cd "$out" && zip -q base.apk classes.dex)
"$tools/zipalign" -f 4 "$out/base.apk" "$out/aligned.apk"
"$tools/apksigner" sign --ks "$HOME/.android/debug.keystore" --ks-pass pass:android --out "$out/ime.apk" "$out/aligned.apk"
"$adb" install --no-incremental -r "$out/ime.apk"
installed=true
# Package installation can finish before the input-method service sees it.
available() { "$adb" shell ime list -a -s | grep -Fxq "$method"; }
for attempt in $(seq 1 25); do
    if available; then break; fi
    sleep 0.2
done
if ! available; then
    echo "Android did not register the test keyboard" >&2
    exit 1
fi
"$adb" shell ime enable "$method"
"$adb" shell ime set "$method"
"$adb" shell settings put secure selected_spell_checker "$fixture/.EditingSpellChecker"
"$adb" shell settings put secure selected_spell_checker_subtype 2026
"$adb" shell settings put secure spell_checker_enabled 1
if [ "$("$adb" shell settings get secure default_input_method)" != "$method" ]; then
    echo "Android did not select the test keyboard" >&2
    exit 1
fi

trial=1
while [ "$trial" -le "$runs" ]; do
    "$adb" shell am force-stop "$app"
    "$adb" shell run-as "$app" rm -f files/editing-result
    "$adb" shell am start -W -n "$app/dev.kui.KuiActivity"
    printf 'Editing trial %s/%s\n' "$trial" "$runs"
    sh ../../wait.sh "$adb" shell run-as "$app" cat files/editing-result
    "$adb" shell run-as "$app" cat shared_prefs/kui.spelling.xml | python3 -c \
        'import sys, xml.etree.ElementTree as ET; assert ET.parse(sys.stdin).find("boolean[@name=\"kuizzzword\"]").get("value") == "true"'
    trial=$((trial + 1))
done
