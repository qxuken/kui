#!/bin/sh
# From a checkout on Apple Silicon, with the chosen simulator booted.
set -eu
cd "$(dirname "$0")/../../../.."
device=${1:-booted}
build_target=aarch64-apple-ios-sim
IPHONEOS_DEPLOYMENT_TARGET=$(/usr/libexec/PlistBuddy -c 'Print :MinimumOSVersion' examples/rust/apps/mobile/Info.plist)
export IPHONEOS_DEPLOYMENT_TARGET
set --
package=kui-mobile-spike
app_id=dev.kui.mobile.spike
app_name=KUIMobile
editing=${KUI_EDITING_REGRESSION:-0}
scenes=${KUI_IPAD_SCENES:-0}
if [ -n "${KUI_TASKS_PACK:-}" ]; then
    KUI_TASKS_PACK=$(python3 -c 'import os; print(os.path.abspath(os.environ["KUI_TASKS_PACK"]))')
    export KUI_TASKS_PACK
    package=kui-tasks-mobile
    app_id=dev.kui.book.tasks
    app_name=Tasks
    editing=0
    scenes=0
else
    if [ "${KUI_CANDIDATE_TRIAL:-0}" = 1 ]; then set -- --features candidate-trial; fi
    if [ "$editing" = 1 ]; then set -- --features editing-regression; fi
    if [ "${KUI_NAVIGATION_TRIAL:-0}" = 1 ]; then set -- --features navigation-trial; fi
    if [ "$scenes" = 1 ]; then set -- --features ipad-scenes; fi
fi
cargo build "$@" --locked -p "$package" --bin "$package" --target "$build_target"
out=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')/"$build_target/debug"
bundle="$out/$app_name.app"
mkdir -p "$bundle"
cp "$out/$package" examples/rust/apps/mobile/Info.plist "$bundle/"
plist="$bundle/Info.plist"
if [ "$package" = kui-tasks-mobile ]; then
    /usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $app_id" -c "Set :CFBundleName Tasks" \
        -c "Set :CFBundleExecutable $package" -c "Set :CFBundleShortVersionString 1.0.0" "$plist"
fi
if [ "$scenes" = 1 ]; then
    /usr/libexec/PlistBuddy -c 'Set :UIApplicationSceneManifest:UIApplicationSupportsMultipleScenes true' "$plist"
    /usr/libexec/PlistBuddy -c 'Add :UIApplicationSceneManifest:UISceneConfigurations:UIWindowSceneSessionRoleApplication:0:UISceneDelegateClassName string KUISceneDelegate' "$plist"
    /usr/libexec/PlistBuddy -c 'Add :UISupportedInterfaceOrientations:3 string UIInterfaceOrientationPortraitUpsideDown' "$plist"
fi
codesign --force --sign - "$bundle"
xcrun simctl install "$device" "$bundle"
if [ "$editing" = 1 ]; then
    data=$(xcrun simctl get_app_container "$device" "$app_id" data)
    rm -f "$data/tmp/editing-result"
fi
xcrun simctl launch --terminate-running-process "$device" "$app_id"
if [ "$editing" = 1 ]; then
    sh examples/rust/apps/mobile/regression/wait.sh cat "$data/tmp/editing-result"
fi
