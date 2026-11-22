# Run a KUI app on iOS and Android

Start with Tasks, the Lua project from [Apps in Lua](runner.md). The mobile
example opens it in `kui-native` with the same Kit controls used by the
desktop and browser runners. Run the commands from the repository checkout.

## Build the Tasks pack

```sh
pack_dir=$(mktemp -d)
cargo run --locked -p kui-cli -- pack examples/lua/tutorial/tasks \
  --format kui --out "$pack_dir"
export KUI_TASKS_PACK="$pack_dir/tasks-1.0.0.kui"
```

The example embeds that archive in its executable. Use a source pack with a Lua
entry and the same KUI version as the mobile build. Keep tasks and settings in
`kui.data_dir`. The host extracts bundled files into a private cache and exposes
them as read-only through `kui.fs`. Rebuilding a pack leaves the data directory
for its project ID in place.

## Launch on an iPhone simulator

On an Apple Silicon Mac with Xcode, boot an iPhone simulator. Run
`xcrun simctl list devices booted` and set `UDID` to its identifier.

```sh
rustup target add aarch64-apple-ios-sim
examples/rust/apps/mobile/run-ios-sim.sh "$UDID"
```

If tapping the field shows the caret without the software keyboard, use
Simulator's **Keyboard** menu to show the software keyboard.

## Launch on an Android emulator

Boot an ARM64 Android emulator. Install platform-tools, SDK Platform 35,
Build Tools 35.0.0, NDK 27.2.12479018 and a JDK. The emulator can run Android 9
(API 28) while the app builds against SDK 35.

```sh
rustup target add aarch64-linux-android
cargo install --locked --version 0.10.0 cargo-apk
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export ANDROID_NDK_ROOT="$ANDROID_HOME/ndk/27.2.12479018"
"$ANDROID_HOME/platform-tools/adb" devices -l
```

Set `ANDROID_SERIAL` to the emulator's serial if more than one device is listed.
The launch script builds, signs and installs the debug APK.

```sh
examples/rust/apps/mobile/run-android-emulator.sh
```

## Check typing, scrolling and saved data

1. Tap **New task**, type with the on-screen keyboard, then tap **Add**. The
   task should appear even if the keyboard was still composing the word.
2. Check its checkbox and add enough tasks to fill the view. Swipe up from a
   row. The list should scroll without toggling that row.
3. Terminate Tasks, reopen it, and check that the first task is still checked.
   Both launchers install Tasks as `dev.kui.book.tasks`.

```sh
# iOS cold launch
xcrun simctl terminate "$UDID" dev.kui.book.tasks
xcrun simctl launch "$UDID" dev.kui.book.tasks

# Android cold launch
adb="$ANDROID_HOME/platform-tools/adb"
"$adb" shell am force-stop dev.kui.book.tasks
"$adb" shell am start -n dev.kui.book.tasks/dev.kui.KuiActivity
```

## Try the same app in a browser

Set up the [book's WASI tools](setup.md#building-this-book), then build the
browser runner from this checkout and pack the same Tasks project with it.

```sh
nu scripts/web-runner.nu --out "$pack_dir/kui-web.zip"
cargo run --locked -p kui-cli -- pack examples/lua/tutorial/tasks \
  --format web --runner "$pack_dir/kui-web.zip" --out "$pack_dir/web"
python3 -m http.server 8000 --bind 127.0.0.1 --directory "$pack_dir/web"
```

Open `http://127.0.0.1:8000`, add and check a task, then reload the page.
The browser keeps app data by origin and project ID. The mobile and browser
hosts provide Kit, JSON, local files and task dispatch. Keep portable
`kui.spawn` callbacks short so they can finish between frames.
[Browser runner overview](runner.md#build-for-the-browser)

## Use the mobile host from Rust

Enable `kui-native`'s `mobile` feature and set `default-features = false` in the
mobile target's dependencies. Call `mobile::run` with a `Core` and your `App`.
Android also passes its `AndroidApp` from `android_main`. Use
`mobile::run_with_extensions` to pass Kit or another extension list.
Use `mobile::session_with_japanese_fallback` with bundled Noto Sans bytes to
use the installed Japanese face. The Tasks host and the Rust example share
this font setup.

The workspace patches `winit` so TalkBack receives touch exploration events.
Android apps built in a separate workspace need the same patch until their
`winit` version includes the fix.
[Workspace Cargo.toml](https://github.com/qxuken/kui/blob/main/Cargo.toml)

Build your view inside `ui.safe_area()` using a filling root and a scrollable
container for fields that may move above the keyboard. Continue using click
and edit events. The host handles gesture recognition, scroll momentum and
the docked keyboard's effect on the viewport. A confirmed tap on a button
commits the focused field's marked text before delivering the click. Moving
focus directly to another editor cancels the previous composition.

For an interactive check, run either simulator script with
`KUI_TASKS_PACK=` to select the Rust touchscreen example. Flick over
**Increment** and check that the count stays at zero. Open **Bottom note**,
type, rotate with the keyboard open, and dismiss it. Check that the field
keeps its text and that the list gets its full height back. The example
also includes speech, Japanese conversion and two-field editing checks.
[Rust example and its test commands](https://github.com/qxuken/kui/blob/main/examples/rust/apps/mobile/README.md)

## Open two notes on iPad

Use the scene example on a booted iPad simulator.

```sh
KUI_TASKS_PACK= KUI_IPAD_SCENES=1 \
  examples/rust/apps/mobile/run-ios-sim.sh "$IPAD_UDID"
```

Choose **Open B in New Window** and arrange A and B side by side. The app
shares the note store and font session. The factory passed to
`mobile::run_scenes` makes a separate core and view for each persistent
UIKit session ID. Store the open note, scroll and caret under that ID.
Test by leaving each window at a different paragraph and caret, then
terminating and relaunching. Repeat after closing one window. Only the
survivor should return in the second test.

The examples target iOS 17.0 and Android API 28. See the support matrix for
the tested runtimes, device results and open release checks.
[Mobile support matrix](https://github.com/qxuken/kui/blob/main/docs/mobile-support-matrix.md)
