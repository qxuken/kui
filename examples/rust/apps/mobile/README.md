# Native touchscreen spike

Both platforms run the same Rust `Screen` and `App` implementation. The counter
stays above a scrollable list containing the Increment button and a controlled
text field. A second field, **Bottom note**, follows row 48. Both platforms use the bundled Noto Sans face for Latin text. For Japanese, iOS asks Core Text for a Japanese fallback and Android API 29+ uses the NDK font matcher with the Japanese locale. API 28 reads the Japanese fallback from its system font configuration. The spike maps that installed face into its font catalog.

The phone floors are iOS 17.0 and Android 9 / API 28 on ARM64. The support matrix
records which configurations have been tested and which still need release
coverage. [Support matrix](../../../../docs/mobile-support-matrix.md)

## Run the iOS Simulator app

On Apple Silicon with Xcode, boot an iPhone simulator, then run from the checkout:

```sh
rustup target add aarch64-apple-ios-sim
examples/rust/apps/mobile/run-ios-sim.sh
```

With multiple simulators booted, pass the simulator UDID to the script.

## Run the Android emulator app

Use a booted ARM64 Android emulator, platform-tools, SDK Platform 35,
Build Tools 35.0.0, the NDK and a JDK. The script packages KUI’s Android
editor view with the Rust library built by cargo-apk, then installs and launches
the debug APK. Set `ANDROID_SERIAL` if multiple devices are connected.
For an x86_64 emulator, install the Rust target `x86_64-linux-android` and set `KUI_ANDROID_TARGET=x86_64-linux-android`. Android Studio is optional.

When using SwiftShader on x86_64, boot the emulator with
`emulator -avd YOUR_AVD -gpu swiftshader -feature -Vulkan`. Some SwiftShader
versions terminate the emulator on Vulkan shader creation. The mobile CI uses
these flags to test GLES on API 28 and 35. On devices, the host tries GLES
when Vulkan returns an error during renderer startup.

`dev.kui.KuiActivity` extends NativeActivity and gives Gboard a focused Android
text editor. KUI still draws the field and owns its value and selection. The
Android input connection forwards composition and committed text through the
existing KUI events.

`kui-native`'s mobile feature selects the `kui-winit` backend automatically
so TalkBack receives touch exploration events. Use the same feature in a
separate Android project. [Workspace manifest](../../../../Cargo.toml)

```sh
rustup target add aarch64-linux-android
cargo install cargo-apk --locked
export ANDROID_HOME="$HOME/Library/Android/sdk"
export ANDROID_NDK_ROOT="$ANDROID_HOME/ndk/27.2.12479018"
examples/rust/apps/mobile/run-android-emulator.sh
```

## Run the Tasks pack

Build the existing Tasks tutorial as a source archive. The launch scripts
embed that archive in the Tasks executable, install it under
`dev.kui.book.tasks` and launch it on the selected device.

```sh
pack_dir=$(mktemp -d)
cargo run --locked -p kui-cli -- pack examples/lua/tutorial/tasks --format kui --out "$pack_dir"
export KUI_TASKS_PACK="$pack_dir/tasks-1.0.0.kui"
examples/rust/apps/mobile/run-ios-sim.sh "$UDID"
examples/rust/apps/mobile/run-android-emulator.sh
```

Tap **New task**, type with the software keyboard and tap **Add** while the
keyboard is still open. Check the new row and tap its checkbox. With enough
tasks to fill the viewport, swipe the list to its last row. Terminate Tasks
and launch it again. The task and its checked state should reappear.

The embedded runner expects a source archive with a Lua entry and the
matching KUI version. It extracts the archive into the app's private cache
and keeps the task file in persistent storage through `kui.data_dir`. The
browser build uses the same Lua project and Kit definitions. Use
`kui pack --format web` with the matching browser runner for that build.

## Verify the interaction

1. Begin with zero taps. Put a finger in the middle of Increment and swipe up.
   On a quick flick the rows continue moving after release and the counter stays at zero. Touch the list again to stop it.
2. Scroll back to the top and tap Increment. The counter shows one.
3. Tap the field immediately below Increment. Tap letters on the on-screen
   keyboard and check that the field shows those letters and the count stays
   at one. On iOS, turn off **Simulate Hardware Keyboard** in the simulator’s
   keyboard menu if the on-screen keyboard is hidden. On Android, dismiss the
   keyboard with Back and tap the still-focused field to show it again.
4. Scroll to the end and tap the field below **Bottom note**. The field should
   move above the docked keyboard. Type a few letters with the software keys.
   Tap a list row on iOS or press Back on Android. The field should keep the
   text, and the same rows should occupy the full list height again. Repeat
   with the keyboard open and rotate to landscape and back. The bottom field
   and its text should remain visible above the keyboard in both orientations.
5. Send the app to the background and reopen it while its process is alive.
   The text and count should still be visible.

The macOS smoke test uses Vision to locate the rendered button and list rows.
It restarts the sample, flicks from the button, checks that the list travelled
farther than the finger while the count stayed at zero, then restarts and
checks that a tap increments once. At the end of the list it opens the bottom
field, taps the on-screen W key, dismisses the keyboard and compares the
visible row positions with their original positions. Use an English QWERTY
software keyboard. Use AXe on PATH for iOS or adb for Android.
Pass the simulated iPhone’s display scale to convert screenshot pixels to
touch coordinates (3 for the iPhone 18 Pro). The default run covers flick, tap and editing. The `--rotate` run focuses on the
keyboard and orientation regression. It types another letter after each turn
and checks that the same field retains the full text. Start it in portrait.
On iOS select the target simulator window and give the test process macOS
Accessibility access to its Rotate menu. On Android the test temporarily
locks rotation and restores the original rotation setting on exit.

On Android, `--keyboard N` opens the same field N times in one app session.
Each cycle checks the input connection, taps D, A and D on Gboard, dismisses
it with Back, and checks the rendered field and the restored list.

`--flicks N` alternates a flick starting on the button with a shorter flick
whose start and end are both inside it. Each trial launches afresh, checks
the counter, and waits for the list’s measured position to settle. The test
exits on the first failure. After all trials it checks that one intentional
tap increments the counter once.

```sh
swift examples/rust/apps/mobile/touch-smoke.swift ios "$UDID" 3 --rotate
swift examples/rust/apps/mobile/touch-smoke.swift android "$ANDROID_SERIAL" --rotate
swift examples/rust/apps/mobile/touch-smoke.swift android "$ANDROID_SERIAL" --keyboard 30
swift examples/rust/apps/mobile/touch-smoke.swift android "$ANDROID_SERIAL" --flicks 30
cargo test -p kui-native --lib mobile_touch
```

### Two-field editing regression

With the same build prerequisites and a booted simulator or connected Android device, run either command.
Each installs a test build, waits for the app’s assertions and exits nonzero on
failure. Run the launch script again without the flag to restore the normal
scrolling example.

```sh
KUI_EDITING_REGRESSION=1 examples/rust/apps/mobile/run-ios-sim.sh "$UDID"
KUI_EDITING_REGRESSION=1 examples/rust/apps/mobile/run-android-emulator.sh
```

The driver invokes UIKit’s active text responder or Android’s served input
connection (API 28+). Android temporarily selects a test keyboard that leaves
the probe in control of composition. The runner removes it and restores the
device’s keyboard settings on exit. Set `KUI_EDITING_REPEATS` to run multiple
cold launches with the same test keyboard. The probe selects UTF-16 offsets 1–3 in
`a🙂b`, inserts `é`, and checks the model and KUI caret before inserting `!`.
It starts marked text `か` in the second field, moves focus back to the first,
delivers a late commit to the old responder, and types `Z` through the new one.
The assertions require `aé!Zb`, an unchanged second field, and exactly three
model changes. On Android the probe then marks part of an existing word,
checks that switching fields preserves it, and accepts `日本` in a fresh
composition. Both probes switch to `ime_off` mid-composition and reopen
the editor. The iOS probe also checks teardown on UIKit's termination
notification. The touch smoke covers tapping the software keys.

### On-screen autocorrection and Japanese conversion

Install the interactive trial on the booted device with either command.

```sh
KUI_CANDIDATE_TRIAL=1 examples/rust/apps/mobile/run-ios-sim.sh "$UDID"
KUI_CANDIDATE_TRIAL=1 examples/rust/apps/mobile/run-android-emulator.sh
```

Tap **Start trial** before each case. It resets the field to `say d|rain now`
with the caret at UTF-8 byte 5. The vertical bar here denotes the caret.
Use the software keys and wait for the keyboard to finish opening.

| Case | Actions | Observed in the iPhone 18 Pro simulator and Pixel 5 API 35 emulator |
| --- | --- | --- |
| Automatic correction | On English (US) with autocorrection enabled, tap Space, `d o n t`, then Space. Tap `x` to probe the caret. | iOS produced `say d don’t x\|rain now` (byte 15). Gboard produced `say d don't x\|rain now` (byte 13). The first space gives the inserted word its own boundary inside the original word. |
| Existing prefix | Starting after the existing `d`, tap `o n t` and Space. | Both keyboards preserved the literal spelling, producing `say dont \|rain now` (byte 9). |
| Japanese candidate | Switch to Japanese Romaji on iOS or Japanese QWERTY on Gboard. Tap `n i h o n n` and choose **日本** from the candidate bar. Switch back to English and tap `x`. | Both committed `say d日本\|rain now` (byte 11), then `say d日本x\|rain now` (byte 12). |

The readout shows the app’s committed value with non-ASCII scalars in hex.
Verify that the field visibly renders にほん during composition and 日本 after
selection. The readout should show `[65E5][672C]` once, immediately before the marker. Candidate rankings
and apostrophe styles may vary with keyboard settings and learned vocabulary.

The native runner uses existing click and scroll events. Unit tests exercise
release velocity, frame-rate independence, interruptions, and a swipe that
starts and ends inside the button. The spike covers one window, one active
finger, inertial panning and basic editing. The shared view applies
`ui.safe_area()` as per-edge padding. These logical distances exclude areas
already removed from the viewport by the keyboard. UIKit supplies the view’s
safe area, and Android supplies its system bar and display cutout insets.
Docked keyboard avoidance uses the UIKit keyboard layout guide on iOS 15+
and Android window insets. VoiceOver and TalkBack use the KUI semantic tree. The example saves each
field in the app’s private storage. Smoke tests use isolated drafts. Native
and web canvas hosts share kinetic scrolling in kui-core and honor the
platform reduced-motion setting.

### Accessibility, rotation and restoration

Install the regular example first. Run these checks from the repository root.

```sh
xcodebuild test -project examples/rust/apps/mobile/regression/ios/MobileChecks.xcodeproj \
  -scheme MobileChecks -destination "platform=iOS Simulator,id=$UDID" \
  -derivedDataPath target/mobile-spike/ios-checks -parallel-testing-enabled NO
examples/rust/apps/mobile/regression/android/run.sh
```

The iOS test activates Increment, edits the bottom note in both orientations
and verifies its value after relaunch. Disconnect the simulated hardware
keyboard to exercise the on-screen keys. The Android instrumentation reads
the accessibility tree, activates Increment and requests focus on Note. The
APK build includes the Java delegate from the AccessKit version in Cargo.lock
so Android 9 can resolve it through the app class loader. For spoken feedback,
enable VoiceOver or TalkBack and traverse the running example.

With TalkBack installed on the Android emulator, run the touch exploration and
swipe regression. It sends genuine virtual touchscreen input to TalkBack and
checks that touching Increment focuses the button and swiping right focuses Note.

```sh
KUI_TALKBACK_REGRESSION=1 examples/rust/apps/mobile/regression/android/run.sh
```

### Navigation and gesture interruption

Build the shared navigation trial with the `navigation-trial` feature. Its
draft uses a separate file in the app's private storage.

```sh
KUI_NAVIGATION_TRIAL=1 examples/rust/apps/mobile/run-ios-sim.sh "$UDID"
KUI_NAVIGATION_TRIAL=1 examples/rust/apps/mobile/run-android-emulator.sh
```

1. Open the note, open its editor and type a few characters. On Android,
   Back should close the software keyboard, then the modal editor, then
   the detail screen. On iOS, use **Hide keyboard**, **Close editor** and
   **Back** for the same sequence.
2. Background the app with the editor open. Reopen it in the same process
   and type another character. Cold-launch it and open the note again to
   verify the saved draft.
3. Open the detail screen in a fresh process. Drag the slider and check that
   both scroll offsets stay at zero. Scroll the inner list to its end, then
   start another swipe inside it. The second swipe should move the outer
   list while the inner list stays at its limit.
4. In a fresh detail screen, begin pressing **Increment** and background
   the app before lifting. Resume it and check for zero taps, then tap once
   and check for one. Repeat while moving the slider. Its last value should
   stay stable after resume and the next drag should move it again.

### iPad windows

Boot an iPad simulator and install the tablet variant. The simulator used for
the two-window check was the 13-inch iPad Pro (M5) with display scale 2.

```sh
KUI_IPAD_SCENES=1 examples/rust/apps/mobile/run-ios-sim.sh "$IPAD_UDID"
```

Choose **Open B in New Window** in Note A. Use iPadOS window controls or the
resize handle to arrange both windows side by side. Each note has its own
field and numbered list. With the keyboard dismissed, the following check
swipes B and verifies that a visible paragraph in A stays at the same height.

```sh
swift examples/rust/apps/mobile/touch-smoke.swift ios "$IPAD_UDID" 2 --windows
```

With A and B side by side, run the cold-launch regression on the same simulator.
Use drafts of at least two characters in A and five in B. The check moves both
carets and lists, sends SIGKILL, and launches the installed app again. It locates
each paragraph on screen within two points of its previous position, types at
both restored carets, and removes the probe characters after verifying both
drafts. It briefly drags the other window's title to give it keyboard focus
without moving its caret.

```sh
swift examples/rust/apps/mobile/touch-smoke.swift ios "$IPAD_UDID" 2 --restore-windows
```

To check editing, type distinct text in A and B, resize A, close it, and keep
typing in B. Choose **Open A in New Window** in B to check A’s saved draft.
The regular iPhone build uses the original single-window example.

On Android API 33+, the Gboard `--keyboard` check also compares the served
input connection before deleting the previous word and after entering the
next one. A same-field edit that replaces the connection fails the check.
