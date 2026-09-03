// Drives a running kui window through the macOS accessibility API — the
// same one VoiceOver uses — and checks what it answers. Real platform
// coverage for the access tree: nothing here talks to kui's own types,
// it only asks the OS what it can see and change.
//
//   cargo build -p kui --example accessibility
//   ./target/debug/examples/accessibility &
//   swift scripts/ax-audit.swift $!
//
// The calling terminal needs Accessibility permission (System Settings >
// Privacy & Security > Accessibility); without it every query fails with
// -25211 and the script says so. Exits nonzero on the first failed check.

import ApplicationServices
import Foundation

// ----------------------------------------------------------------- checks

var failures = 0
var checks = 0

func check(_ what: String, _ got: Any?, _ want: Any?) {
    checks += 1
    let g = String(describing: got ?? "nil")
    let w = String(describing: want ?? "nil")
    if g == w {
        print("  ok    \(what): \(g)")
    } else {
        print("  FAIL  \(what): got \(g), want \(w)")
        failures += 1
    }
}

func check(_ what: String, _ cond: Bool, _ detail: String = "") {
    checks += 1
    if cond {
        print("  ok    \(what)\(detail.isEmpty ? "" : ": \(detail)")")
    } else {
        print("  FAIL  \(what)\(detail.isEmpty ? "" : ": \(detail)")")
        failures += 1
    }
}

// ------------------------------------------------------------- AX helpers

func attr(_ el: AXUIElement, _ name: String) -> CFTypeRef? {
    var out: CFTypeRef?
    let err = AXUIElementCopyAttributeValue(el, name as CFString, &out)
    if err == .apiDisabled || err == .notImplemented, name == kAXRoleAttribute as String {
        FileHandle.standardError.write(
            "ax-audit: the accessibility API is disabled for this process (\(err.rawValue)).\n"
                .data(using: .utf8)!)
        exit(2)
    }
    return err == .success ? out : nil
}

func str(_ el: AXUIElement, _ name: String) -> String? {
    attr(el, name) as? String
}

func num(_ el: AXUIElement, _ name: String) -> Double? {
    (attr(el, name) as? NSNumber)?.doubleValue
}

func children(_ el: AXUIElement) -> [AXUIElement] {
    (attr(el, kAXChildrenAttribute as String) as? [AXUIElement]) ?? []
}

func actions(_ el: AXUIElement) -> [String] {
    var out: CFArray?
    guard AXUIElementCopyActionNames(el, &out) == .success else { return [] }
    return (out as? [String]) ?? []
}

/// A parameterized attribute taking a character range.
func rangeAttr(_ el: AXUIElement, _ name: String, _ loc: Int, _ len: Int) -> CFTypeRef? {
    var range = CFRange(location: loc, length: len)
    guard let param = AXValueCreate(.cfRange, &range) else { return nil }
    var out: CFTypeRef?
    let err = AXUIElementCopyParameterizedAttributeValue(el, name as CFString, param, &out)
    return err == .success ? out : nil
}

/// A parameterized attribute taking an integer.
func intAttr(_ el: AXUIElement, _ name: String, _ i: Int) -> CFTypeRef? {
    var out: CFTypeRef?
    let param = NSNumber(value: i)
    let err = AXUIElementCopyParameterizedAttributeValue(el, name as CFString, param, &out)
    return err == .success ? out : nil
}

func asRange(_ v: CFTypeRef?) -> CFRange? {
    guard let v, CFGetTypeID(v) == AXValueGetTypeID() else { return nil }
    var r = CFRange()
    return AXValueGetValue(v as! AXValue, .cfRange, &r) ? r : nil
}

func asRect(_ v: CFTypeRef?) -> CGRect? {
    guard let v, CFGetTypeID(v) == AXValueGetTypeID() else { return nil }
    var r = CGRect.zero
    return AXValueGetValue(v as! AXValue, .cgRect, &r) ? r : nil
}

func setRange(_ el: AXUIElement, _ name: String, _ loc: Int, _ len: Int) -> Bool {
    var range = CFRange(location: loc, length: len)
    guard let v = AXValueCreate(.cfRange, &range) else { return false }
    return AXUIElementSetAttributeValue(el, name as CFString, v) == .success
}

// ---------------------------------------------------------------- the tree

struct Found {
    let el: AXUIElement
    let role: String
    let title: String
    let value: String
    let desc: String
    let depth: Int
}

var all: [Found] = []

func walk(_ el: AXUIElement, _ depth: Int) {
    if depth > 24 { return }
    let role = str(el, kAXRoleAttribute as String) ?? "?"
    let title = str(el, kAXTitleAttribute as String) ?? ""
    // AXValue is a string for text, a number for sliders and switches.
    var value = str(el, kAXValueAttribute as String) ?? ""
    if value.isEmpty, let n = num(el, kAXValueAttribute as String) {
        value = String(n)
    }
    let desc = str(el, kAXDescriptionAttribute as String) ?? ""
    all.append(Found(el: el, role: role, title: title, value: value, desc: desc, depth: depth))
    for c in children(el) { walk(c, depth + 1) }
}

func find(role: String, title: String? = nil, value: String? = nil) -> Found? {
    all.first {
        $0.role == role && (title == nil || $0.title == title!)
            && (value == nil || $0.value == value!)
    }
}

// -------------------------------------------------------------------- main

guard CommandLine.arguments.count > 1, let pid = Int32(CommandLine.arguments[1]) else {
    print("usage: swift scripts/ax-audit.swift <pid>")
    exit(64)
}
if !AXIsProcessTrusted() {
    FileHandle.standardError.write(
        "ax-audit: this process is not trusted for Accessibility; grant it in System Settings > Privacy & Security > Accessibility.\n"
            .data(using: .utf8)!)
    exit(2)
}

let app = AXUIElementCreateApplication(pid)
// The adapter builds the platform tree when something first asks for it,
// and the app needs a frame on screen; give it a moment either way.
var windows: [AXUIElement] = []
for _ in 0..<50 {
    windows = (attr(app, kAXWindowsAttribute as String) as? [AXUIElement]) ?? []
    if !windows.isEmpty, !children(windows[0]).isEmpty { break }
    usleep(200_000)
}
guard let window = windows.first else {
    FileHandle.standardError.write("ax-audit: the app exposes no window\n".data(using: .utf8)!)
    exit(1)
}
walk(window, 0)

print("=== tree (\(all.count) elements)")
for f in all {
    let bits = [
        f.title.isEmpty ? nil : "title=\(f.title.debugDescription)",
        f.value.isEmpty ? nil : "value=\(f.value.debugDescription)",
        f.desc.isEmpty ? nil : "desc=\(f.desc.debugDescription)",
    ].compactMap { $0 }.joined(separator: " ")
    print("\(String(repeating: "  ", count: f.depth))\(f.role) \(bits)")
}

// -- Structure and names -------------------------------------------------

print("\n=== roles and names")
check("the window is exposed", all.contains { $0.role == kAXWindowRole as String }, "AXWindow")
// AccessKit spells this role "Heading", not "AXHeading".
check("heading present", all.contains { $0.role == "Heading" && $0.title == "Controls" })
let press = find(role: kAXButtonRole as String, title: "count 0")
check("button named by its content", press != nil, "count 0")
check("icon button named by its label", find(role: kAXButtonRole as String, title: "Save") != nil)
let mute = find(role: kAXCheckBoxRole as String, title: "Mute")
check("switch exposed with a name", mute != nil)
let slider = find(role: kAXSliderRole as String, title: "Volume")
check("slider exposed with a name", slider != nil)
// The window title belongs to the window; a drawn titlebar that also
// named itself would have it read twice. (The AppKit traffic lights sit
// beside our content at depth 1, so only deeper elements are ours.)
let titleEchoes = all.filter { $0.depth > 1 && ($0.title == "kui — accessibility" || $0.value == "kui — accessibility") }
check("the window title is announced once", titleEchoes.count, 1)
// The latency HUD is a development overlay, not content.
check(
    "the latency HUD is not exposed",
    !all.contains { $0.value.contains("ms avg") || $0.value == "?" },
    all.filter { $0.value.contains("ms avg") || $0.value == "?" }.map(\.value).description)

// -- Values --------------------------------------------------------------

print("\n=== values")
if let mute { check("switch value is off", num(mute.el, kAXValueAttribute as String), 0.0) }
if let slider {
    check("slider value", num(slider.el, kAXValueAttribute as String), 3.0)
    check("slider min", num(slider.el, kAXMinValueAttribute as String), 0.0)
    check("slider max", num(slider.el, kAXMaxValueAttribute as String), 10.0)
}

// -- The built-in editor's text protocol ---------------------------------

print("\n=== built-in editor text")
let doc = all.first { $0.role == kAXTextAreaRole as String && $0.title == "Notes" }
    ?? all.first { $0.value == "hello world\nsecond line" }
if let doc {
    let el = doc.el
    check("value", str(el, kAXValueAttribute as String), "hello world\nsecond line")
    check("character count", num(el, kAXNumberOfCharactersAttribute as String), 23.0)
    check(
        "string for a range",
        rangeAttr(el, kAXStringForRangeParameterizedAttribute as String, 6, 5) as? String,
        "world")
    check(
        "string across the newline",
        rangeAttr(el, kAXStringForRangeParameterizedAttribute as String, 9, 5) as? String,
        "ld\nse")
    check(
        "line for an index",
        (intAttr(el, kAXLineForIndexParameterizedAttribute as String, 15) as? NSNumber)?.intValue,
        1)
    if let r = asRange(intAttr(el, kAXRangeForLineParameterizedAttribute as String, 1)) {
        check("range for line 1", "\(r.location),\(r.length)", "12,11")
    } else {
        check("range for line 1", false, "no range returned")
    }
    if let box = asRect(rangeAttr(el, kAXBoundsForRangeParameterizedAttribute as String, 0, 5)) {
        check("bounds for a range have extent", box.width > 0 && box.height > 0, "\(box)")
    } else {
        check("bounds for a range", false, "no rect returned")
    }
    // Selection: set it through the API, read it and the text back.
    check("selection is settable", setRange(el, kAXSelectedTextRangeAttribute as String, 6, 5))
    usleep(300_000)
    if let sel = asRange(attr(el, kAXSelectedTextRangeAttribute as String)) {
        check("selection reads back", "\(sel.location),\(sel.length)", "6,5")
    } else {
        check("selection reads back", false, "no range")
    }
    check("selected text", str(el, kAXSelectedTextAttribute as String), "world")
    check(
        "insertion point line",
        num(el, kAXInsertionPointLineNumberAttribute as String), 0.0)
} else {
    check("the built-in editor is exposed", false, "not found")
}

// -- The app-owned editor ------------------------------------------------

print("\n=== app-owned editor text")
let code = all.first { $0.title == "Source" }
if let code {
    let el = code.el
    check("value is the lines it drew", str(el, kAXValueAttribute as String), "fn main() {\n    greet()\n}")
    check("character count", num(el, kAXNumberOfCharactersAttribute as String), 25.0)
    // "fn main() {\n    greet()\n}": the second line's four spaces start
    // at 12, so "greet" is 16..21.
    check(
        "string for a range",
        rangeAttr(el, kAXStringForRangeParameterizedAttribute as String, 16, 5) as? String,
        "greet")
    check(
        "line for an index",
        (intAttr(el, kAXLineForIndexParameterizedAttribute as String, 24) as? NSNumber)?.intValue,
        2)
    check(
        "the gutter is not part of the text",
        !(str(el, kAXValueAttribute as String) ?? "").contains("1"),
        (str(el, kAXValueAttribute as String) ?? "").debugDescription)
} else {
    check("the app-owned editor is exposed", false, "not found")
}

// -- Actions change the app ---------------------------------------------

print("\n=== actions")
if let press {
    check("button advertises press", actions(press.el).contains(kAXPressAction as String))
    AXUIElementPerformAction(press.el, kAXPressAction as CFString)
    usleep(400_000)
    all = []
    walk(window, 0)
    check(
        "pressing renamed the button",
        find(role: kAXButtonRole as String, title: "count 1") != nil, "count 1")
}
if let slider = find(role: kAXSliderRole as String, title: "Volume") {
    check("slider advertises increment", actions(slider.el).contains(kAXIncrementAction as String))
    AXUIElementPerformAction(slider.el, kAXIncrementAction as CFString)
    usleep(400_000)
    check("increment raised the value", num(slider.el, kAXValueAttribute as String), 4.0)
    AXUIElementPerformAction(slider.el, kAXDecrementAction as CFString)
    usleep(400_000)
    check("decrement lowered it", num(slider.el, kAXValueAttribute as String), 3.0)
}
if let mute = find(role: kAXCheckBoxRole as String, title: "Mute") {
    AXUIElementPerformAction(mute.el, kAXPressAction as CFString)
    usleep(400_000)
    check("pressing the switch toggled it", num(mute.el, kAXValueAttribute as String), 1.0)
}
// Moving the caret in the app-owned editor: the app applies it and the
// next frame reports the new position, so the round trip is observable.
if let code = all.first(where: { $0.title == "Source" }) {
    check(
        "app-owned editor accepts a selection",
        setRange(code.el, kAXSelectedTextRangeAttribute as String, 3, 0))
    usleep(400_000)
    if let sel = asRange(attr(code.el, kAXSelectedTextRangeAttribute as String)) {
        check("the app moved its caret", "\(sel.location),\(sel.length)", "3,0")
    } else {
        check("the app moved its caret", false, "no range")
    }
}

// -- Keyboard focus (docs/adr/0002-keyboard-focus-as-data.md) ------------
// A reader moves keyboard focus by setting AXFocused; the core lands it
// where Tab would, and reports it back as the application's focused
// element. Then a real Tab keystroke moves it on to the next control.

print("\n=== keyboard focus")
func focusedTitle() -> String? {
    guard let el = attr(app, kAXFocusedUIElementAttribute as String) else { return nil }
    let e = el as! AXUIElement
    return str(e, kAXTitleAttribute as String)
}
if let press = find(role: kAXButtonRole as String, title: "count 1") {
    check(
        "the button accepts focus",
        AXUIElementSetAttributeValue(press.el, kAXFocusedAttribute as CFString, kCFBooleanTrue)
            == .success)
    usleep(400_000)
    check("the app reports it focused", focusedTitle(), "count 1")
    check(
        "AXFocused reads back",
        (attr(press.el, kAXFocusedAttribute as String) as? Bool) ?? false)
    // Tab: the next control in tree order (the icon button).
    if let down = CGEvent(keyboardEventSource: nil, virtualKey: 0x30, keyDown: true),
       let up = CGEvent(keyboardEventSource: nil, virtualKey: 0x30, keyDown: false)
    {
        down.postToPid(pid)
        up.postToPid(pid)
        usleep(400_000)
        check("Tab moved focus to the next control", focusedTitle(), "Save")
    } else {
        check("Tab keystroke", false, "could not build a CGEvent")
    }
} else {
    check("the button is focusable", false, "count 1 not found")
}

print("\n\(checks - failures)/\(checks) checks passed")
exit(failures == 0 ? 0 : 1)
