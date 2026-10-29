# kui-access

kui's access tree to the platform's accessibility API (NSAccessibility, UI Automation, AT-SPI) through [AccessKit](https://accesskit.dev).

The core (`kui-core`) derives the access tree, announcements and the requests assistive technology sends back as plain data. This crate hands them to the platform:

- the translation every bridge shares: the tree as an AccessKit `TreeUpdate`, a platform request as the core's `AccessRequest`, and the `Feed` that decides when a window's tree is sent;
- `HostBridge` (feature `platform`, on by default): AccessKit's platform adapter on a window a host made itself — an `NSView` or `NSWindow` on macOS, an `HWND` on Windows, AT-SPI on Linux — with no winit.

The runner (`kui-native`) uses the translation for its own windows. A Rust host with its own window attaches a `HostBridge`; a C host calls `kui_access_attach` from `kui.h`.

```rust,no_run
# fn demo(ns_view: *mut std::ffi::c_void, core: &mut kui_core::Core) {
let mut access = unsafe { kui_access::HostBridge::attach(ns_view) }.expect("a bridge");
// Each turn, before input: what a client asked for, as the events it raised.
for ev in access.apply(core) {
    let _ = ev;
}
// ...build and finish the frame...
access.publish(core);
# }
```
