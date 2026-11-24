#!/bin/sh
set -eu
cd "$(dirname "$0")/../../../.."
: "${ANDROID_HOME:?Set ANDROID_HOME to the Android SDK}"
build_tools="$ANDROID_HOME/build-tools/35.0.0"
platform="$ANDROID_HOME/platforms/android-35/android.jar"
set --
package=kui-mobile-spike
app_id=dev.kui.mobile.spike
manifest=examples/rust/apps/mobile/Cargo.toml
editing=${KUI_EDITING_REGRESSION:-0}
if [ -n "${KUI_TASKS_PACK:-}" ]; then
    KUI_TASKS_PACK=$(python3 -c 'import os; print(os.path.abspath(os.environ["KUI_TASKS_PACK"]))')
    export KUI_TASKS_PACK
    package=kui-tasks-mobile
    app_id=dev.kui.book.tasks
    manifest=examples/rust/apps/mobile/tasks/Cargo.toml
    editing=0
else
    if [ "${KUI_CANDIDATE_TRIAL:-0}" = 1 ]; then set -- --features candidate-trial; fi
    if [ "$editing" = 1 ]; then set -- --features editing-regression; fi
    if [ "${KUI_NAVIGATION_TRIAL:-0}" = 1 ]; then set -- --features navigation-trial; fi
fi
if [ -n "${KUI_ANDROID_TARGET:-}" ]; then set -- "$@" --target "$KUI_ANDROID_TARGET"; fi
cargo apk build "$@" --lib --manifest-path "$manifest"
target=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
out="$target/debug/apk"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/classes"
# Compile the delegate from the exact AccessKit version in Cargo.lock.
accesskit=$(cargo metadata --locked --format-version 1 | python3 -c 'import json, pathlib, sys; print(next(pathlib.Path(p["manifest_path"]).parent for p in json.load(sys.stdin)["packages"] if p["name"] == "accesskit_android"))')
set -- crates/kui-native/android/dev/kui/KuiActivity.java "$accesskit/java/dev/accesskit/android/Delegate.java"
if [ "$editing" = 1 ]; then set -- "$@" examples/rust/apps/mobile/regression/EditingProbe.java; fi
javac --release 8 -classpath "$platform" -d "$tmp/classes" "$@"
jar cf "$tmp/editor.jar" -C "$tmp/classes" .
"$build_tools/d8" --min-api 28 --lib "$platform" --output "$tmp" "$tmp/editor.jar"
cp "$out/$package-unaligned.apk" "$tmp/mobile.apk"
(cd "$tmp" && zip -q mobile.apk classes.dex)
"$build_tools/zipalign" -P 16 -f 4 "$tmp/mobile.apk" "$tmp/aligned.apk"
apk="$out/$package.apk"
"$build_tools/apksigner" sign --ks "$HOME/.android/debug.keystore" --ks-pass pass:android --out "$apk" "$tmp/aligned.apk"
adb="$ANDROID_HOME/platform-tools/adb"
"$adb" install --no-incremental -r "$apk"
if [ "$editing" = 1 ]; then
    sh examples/rust/apps/mobile/regression/android/editing-ime/run.sh
else
    "$adb" shell am start -S -n "$app_id/dev.kui.KuiActivity"
fi
