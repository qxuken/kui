#!/bin/sh
# Run against a booted emulator with the regular mobile example installed.
set -eu
cd "$(dirname "$0")"
: "${ANDROID_HOME:?Set ANDROID_HOME to the Android SDK}"
tools="$ANDROID_HOME/build-tools/35.0.0"
platform="$ANDROID_HOME/platforms/android-35/android.jar"
adb="$ANDROID_HOME/platform-tools/adb"
out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
mkdir "$out/classes"
javac --release 8 -classpath "$platform" -d "$out/classes" Check.java
jar cf "$out/check.jar" -C "$out/classes" .
"$tools/d8" --min-api 28 --lib "$platform" --output "$out" "$out/check.jar"
"$tools/aapt2" link -o "$out/base.apk" --manifest AndroidManifest.xml -I "$platform"
(cd "$out" && zip -q base.apk classes.dex)
"$tools/zipalign" -f 4 "$out/base.apk" "$out/aligned.apk"
"$tools/apksigner" sign --ks "$HOME/.android/debug.keystore" --ks-pass pass:android --out "$out/check.apk" "$out/aligned.apk"
"$adb" install --no-incremental -r "$out/check.apk"
"$adb" shell am start -S -n dev.kui.mobile.spike/dev.kui.KuiActivity --ez dev.kui.mobile.spike.ephemeral true
result=$("$adb" shell am instrument -w dev.kui.probe/.Check)
echo "$result"
echo "$result" | grep -q 'INSTRUMENTATION_CODE: -1'
if [ "${KUI_TALKBACK_REGRESSION:-0}" = 1 ]; then
    sh talkback.sh "$out"
fi
