# Native iOS and Android hosts for kui

At `bf0718d8`, a single-window Rust app is a good candidate for a native port. Keep the app model, widget tree, layout, editor, hit testing and renderer. Build one small mobile runner with an iOS entry point and an Android entry point. The first milestone is the same tappable counter on two devices. The milestones that determine whether ordinary apps can ship are reliable resume, editable text and screen-reader navigation. [Core session](../crates/kui-core/src/session.rs) · [Renderer interface](../crates/kui-wgpu/src/lib.rs) · [Counter app](../examples/rust/tutorial/03_counter.rs)

The [mobile readiness plan](mobile-readiness.md) tracks the next implementation milestones and physical-device acceptance checks.

## Spike result

The [runnable example](../examples/rust/apps/mobile/README.md) now uses the same Rust `App` on an iPhone 18 Pro simulator (iOS 27.2) and a Pixel 5 ARM64 emulator (Android API 35). On both, swiping from the Increment button moved the list while the count stayed at zero. Tapping incremented it once. A flick continued past the finger’s release position. The field accepted taps on the on-screen keyboard, and the text and count survived a background/foreground cycle with the process alive. The docked keyboard resizes the list and the runner reveals its focused bottom field. Dismissal restores the full list viewport. The Android host uses NativeActivity with a custom AccessKit view. The GameActivity approach below was considered during the initial investigation. See the [readiness plan](mobile-readiness.md) for current results.

## Crate boundary and app shape

I would start with a mobile runner module inside `kui-native`. Move the platform-independent `App` callbacks and wake contract out of the desktop GUI module, preserve the app-facing `App` and `Launcher` API, and select the runner by target. Expose Android’s required activity handle at the platform entry point. Restrict desktop-only dependencies and window management to their supported targets. Share one model, one core, redraw scheduling and touch translation across phones, with two small target modules for startup, native text services, insets and lifecycle. A separate `kui-mobile` crate makes sense if the mobile host needs independent releases or embedding APIs. It can still consume the same callbacks, core and renderer. [Native app callbacks](../crates/kui-native/src/gui.rs) · [Native feature boundary](../crates/kui-native/src/lib.rs) · [Web app callbacks](../crates/kui-web/src/lib.rs)

For example, the counter keeps its integer and handles the same increment message. On a phone its root fills the available area, the button accepts a tap and the view can use `ui.viewport()` to collapse a wide row into a column. The runner converts physical pixels to logical coordinates, feeds input to the core, routes the resulting messages, builds the next frame and presents the display list and glyph atlas. A settings/detail screen can be app state within that same window. The host maps Android Back and an iOS navigation affordance to the app's back action, with a modal dismissed before its underlying screen. Desktop titlebars, detached windows and menus need product-specific equivalents. [Counter implementation](../examples/rust/tutorial/03_counter.rs) · [Viewport](../crates/kui-core/src/ui.rs) · [Core input](../crates/kui-core/src/input.rs) · [Custom runner example](../crates/kui-wgpu/src/lib.rs)

A Kit app using the runner’s Lua helpers could keep the same message on both platforms and choose spacing by available width. This sketch assumes the bundled mobile Kit host described below. [Kit task example](../examples/lua/tutorial/tasks/main.lua) · [Responsive Kit layout](../addons/kit/host/lua/kit/presentation.lua)

```lua
local kit = require("kit.ui")

function init()
  return { count = 0 }
end

function update(msg)
  if msg.kind == "add" then model.count = model.count + 1 end
end

function view(env)
  return column {
    width = "grow", height = "grow", pad = env.viewport_w < 600 and 16 or 32,
    text(tostring(model.count), { size = 32 }),
    kit.button { label = "Add", on_press = { kind = "add" } },
  }
end
```

| Platform | Installable app and rendering |
| --- | --- |
| iOS | An Xcode app target links a Rust static library and calls its exported start function from the app entry point. Let winit own UIKit startup and create the window in `resumed`. Use the existing renderer on Metal. Forward view size, scale, safe area and keyboard occlusion to layout. [winit iOS integration](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform/ios.rs) · [UIKit safe area](https://developer.apple.com/documentation/uikit/uiview/safeareainsets) · [wgpu platform matrix](https://github.com/gfx-rs/wgpu#supported-platforms) |
| Android | A Gradle app packages the Rust `cdylib` for the target ABI, its assets and a GameActivity. Use winit’s `android-game-activity` feature, implement `android_main(AndroidApp)` and give that handle to the event-loop builder. Start with Vulkan. Drop the surface before `suspended` returns and create a fresh surface after resume. Keep the model alive across surface loss and persist state separately for process recreation. Handle system bars, cutouts and the keyboard through window insets. [winit Android integration](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform/android.rs) · [Surface lifecycle](https://github.com/rust-windowing/winit/blob/v0.30.13/src/application.rs) · [Android insets](https://developer.android.com/develop/ui/views/layout/edge-to-edge) · [wgpu platform matrix](https://github.com/gfx-rs/wgpu#supported-platforms) |

The lockfile selects winit 0.30.13 and wgpu 30.0.1. On both phone targets the default `kui-native` graph still includes arboard, rfd, X11 and Wayland. Its manifest selects the latter two on every target except macOS and Windows. The GUI loop opens the main pane on resume and also contains desktop-only pump APIs. With the pinned Rust 1.99.1 toolchain, `cargo check --locked --lib -p kui-core -p kui-wgpu --target TARGET` passed on both `aarch64-apple-ios-sim` and `aarch64-linux-android`. System-only `kui-native --no-default-features` also passed on both. Adding `--features gui` stopped in `wayland-sys` during cross-target pkg-config setup on the simulator, and in arboard 3.6.1 on Android because its platform clipboard implementation was missing. No device build was linked, installed or run as part of these checks. [Lockfile](../Cargo.lock) · [Native dependencies](../crates/kui-native/Cargo.toml) · [GUI loop](../crates/kui-native/src/gui.rs)

## How much of input carries over?

Keep the existing activation message for ordinary controls. A second public `on_tap` would force app authors to decide which handler to register for the same action. The host needs contact IDs and cancellation internally so it can recognize that action reliably.

| Interaction | Reuse and host work |
| --- | --- |
| Tap, button, toggle, drag | Convert one contact's position to logical pixels. Use cursor move and primary down/up for a tap. Track contact ID, apply finger slop, and distinguish a drag from a scroll before pressing. Clear capture and hover on cancellation, interruption or a second contact. Audit a dedicated primary-press cancel operation if a release can commit a drag or click. The existing `DragCancel` handles external file drags. A cancelled finger must never activate the underlying button. The web runner already implements this policy in JavaScript. [Core input contract](../crates/kui-core/src/input.rs) · [Web touch handling](../crates/kui-web/canvas.js) · [Web touch regression cases](../examples/web/touch-smoke.mjs) |
| Swipe, flick, pinch | Reuse `prefers_scroll_at`, `prefers_drag_at`, per-axis scroll latching and `ScrollGesture`. The host computes deltas, tracks velocity and stops momentum on the next touch. The web code supplies a reference implementation for flicks. `InputEvent::Pinch` already routes zoom to a handler. Recognizing two fingers, choosing the focal point and cancelling a pending tap belong in the mobile adapter. Apps needing simultaneous independent touches or stylus identity would need an extended public input model. [Input variants and hit tests](../crates/kui-core/src/input.rs) · [Scroll tests](../crates/kui-core/tests/scroll_gestures.rs) · [Web momentum](../crates/kui-web/canvas.js) |
| Keyboard and text | Reuse key routing, focus, selection, `Preedit`, `Commit` and caret geometry. The native keyboard needs two-way synchronization of selection, composing range, surrounding text and replacement edits, including conversion between platform offsets and core byte ranges. In pinned winit, the iOS view implements basic `UIKeyInput`. Prototype a UIKit text responder or native text control and the Android GameTextInput bridge against the same core editor. Validate autocorrection, deletion, emoji and CJK composition on phones. [Core IME tests](../crates/kui-core/tests/ime.rs) · [winit UIKit implementation](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/ios/view.rs) · [Apple UITextInput](https://developer.apple.com/documentation/uikit/uitextinput) · [Android GameTextInput](https://developer.android.com/games/agdk/add-support-for-text-input) |
| Screen reader | Reuse kui’s semantic tree, actions and AccessKit translation. Pinned `accesskit_winit` 0.34.1 pulls in the iOS adapter on iOS. Android requires its optional `accesskit_android` feature, absent from the current default graph, and its adapter expects GameActivity’s input surface view. Exercise focus, activation, edit announcements and hit bounds in VoiceOver and TalkBack. [Translation](../crates/kui-access/src/translate.rs) · [Pinned adapter manifest](https://github.com/AccessKit/accesskit/blob/ce8164ba92995cfa86005b6259115e08c8244253/adapters/winit/Cargo.toml) · [Pinned Android integration](https://github.com/AccessKit/accesskit/blob/ce8164ba92995cfa86005b6259115e08c8244253/adapters/winit/src/platform_impl/android.rs) |

Start with a bundled font and image supplied as bytes. Shipping adds font fallback and locale coverage, text scaling, light/dark appearance, reduced motion, clipboard, app storage and restoration. Core currently accepts `MacOs`, `Windows`, `Linux` and `Other` for platform conditions. Both phones can use `Other` for the spike. A Kit/Lua app could package scripts and assets with the app and use the same host. Its `view(env)` can switch layout by `env.viewport_w`, as Kit’s presentation module already does. Budget a separate integration pass for `kui-lua`, its non-WASI dependency on native system helpers, and any app-specific runner modules or native extensions. [Platform conditions](../crates/kui-core/src/rules.rs) · [Session](../crates/kui-core/src/session.rs) · [Environment](../crates/kui-core/src/env.rs) · [Kit responsive layout](../addons/kit/host/lua/kit/presentation.lua) · [Lua dependencies](../crates/kui-lua/Cargo.toml) · [Runner modules](../crates/kui-cli/src/modules)

## Estimate and first experiment

These are cumulative engineering estimates for one engineer familiar with kui and Rust, with access to Xcode, the Android SDK/NDK and both physical phones. They cover a shared Rust UI, one foreground window and bundled assets. App-specific integrations, additional language bindings and store review time are outside the ranges.

| Milestone on both platforms | Cumulative effort | Acceptance check |
| --- | --- | --- |
| Counter on devices | 3–7 engineer-days | Install both packages, draw text and a button, increment once per tap, rotate and return from the background with count and rendering intact. |
| Usable single-window app | 3–6 engineer-weeks | Scroll a nested list, drag a control, open a detail screen and go back, edit a field with a software keyboard, respect safe areas and exercise basic screen-reader actions on each phone. |
| Reusable mobile release | 8–16 engineer-weeks | Cover composition and selection, accessibility navigation and editing, fallback fonts, resume and process restoration, representative GPU/device testing, signed packaging examples and CI. Re-estimate after the device spike, especially for rich editing or a wider Android GPU range. |

Use one shared screen with a counter inside a scrolling list and an editor near the bottom. First prove native presentation and tap on both phones with the bundled font. Then start a swipe on the button, lift or interrupt mid-drag, rotate with the keyboard open, compose a word, and background/foreground repeatedly. Verify the button fires only on an intentional tap, the editor stays visible and its text survives, the surface presents again and a screen reader can find and activate the button. Record device, OS, graphics backend and any missing native callbacks. These results determine whether the pinned stack is sufficient and which native bridges need implementation. [Web touch smoke](../examples/web/touch-smoke.mjs) · [Web keyboard smoke](../examples/web/mobile-keyboard-smoke.mjs) · [Core editor tests](../crates/kui-core/tests/ime.rs)
