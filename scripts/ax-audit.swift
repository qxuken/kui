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

// -------------------------------------------------- announcement observer
// The one part of the access story with no attribute to read: an
// announcement is a *notification* the app posts on its window
// (NSAccessibilityAnnouncementRequested), which is exactly what VoiceOver
// listens for. AccessKit derives it from a tree diff — a live node that was
// added, or whose label changed — so observing it is the only way to know
// the round trip works end to end.
// See docs/adr/0008-live-regions-and-announcements.md.

/// What the callback collects: (text, priority). A C function pointer
/// cannot capture, so this is a global.
var announcements: [(String, Int)] = []

let announceCallback: AXObserverCallbackWithInfo = { _, _, _, info, _ in
    guard let info = info as? [String: Any] else { return }
    let text = (info[kAXAnnouncementKey as String] as? String) ?? ""
    let priority = (info[kAXPriorityKey as String] as? NSNumber)?.intValue ?? 0
    announcements.append((text, priority))
}

/// Registers for AXAnnouncementRequested on `els` and returns the observer,
/// which must stay alive for the notifications to arrive.
func watchAnnouncements(_ pid: pid_t, _ els: [AXUIElement]) -> AXObserver? {
    var observer: AXObserver?
    guard AXObserverCreateWithInfoCallback(pid, announceCallback, &observer) == .success,
        let observer
    else { return nil }
    CFRunLoopAddSource(
        CFRunLoopGetCurrent(), AXObserverGetRunLoopSource(observer), .defaultMode)
    for el in els {
        AXObserverAddNotification(
            observer, el, kAXAnnouncementRequestedNotification as CFString, nil)
    }
    return observer
}

/// Spins the run loop until `n` announcements have arrived or time is up.
func waitForAnnouncements(_ n: Int, seconds: Double = 2.0) {
    let deadline = Date().addingTimeInterval(seconds)
    while announcements.count < n, Date() < deadline {
        CFRunLoopRunInMode(.defaultMode, 0.05, true)
    }
}

/// Lets anything still in flight land, then clears — so the next check
/// reads what its own press produced and not the tail of the last one.
func settleAnnouncements() {
    let deadline = Date().addingTimeInterval(0.5)
    while Date() < deadline { CFRunLoopRunInMode(.defaultMode, 0.05, true) }
    announcements = []
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

// -- Selection and disclosure --------------------------------------------
// AccessKit maps these three facts very differently on macOS, and the
// point of checking them here is that only the OS can say which landed:
// a tab list becomes an AXTabGroup whose AXTabs are AXRadioButtons with
// an AXTabButton subrole, and a tab's `selected` arrives as its AXValue,
// not as AXSelected (accesskit_macos treats a tab as checkable and keeps
// AXSelected for item-like nodes such as list rows). VoiceOver derives
// "1 of 3" from AXTabs itself, which is why the tree's `set_size` has no
// macOS attribute behind it.

print("\n=== selection")
let tabGroup = all.first { $0.role == "AXTabGroup" }
check("the tab list is an AXTabGroup", tabGroup != nil)
// A radio is an AXRadioButton too, so the tabs are the ones carrying the
// AXTabButton subrole — which is also the distinction the next check is
// about, made load-bearing here rather than only asserted.
let tabs = all.filter {
    $0.role == kAXRadioButtonRole as String
        && str($0.el, kAXSubroleAttribute as String) == "AXTabButton"
}
check("three tabs are exposed", tabs.count, 3)
check("tabs are named by their content", tabs.map(\.title), ["General", "Network", "About"])
check(
    "a tab carries the AXTabButton subrole",
    tabs.first.flatMap { str($0.el, kAXSubroleAttribute as String) }, "AXTabButton")
if let tabGroup {
    check(
        "the tab list is horizontal, from its dir",
        str(tabGroup.el, "AXOrientation"), "AXHorizontalOrientation")
    // AXTabs is what a reader walks to count them; it is the tab list's
    // Tab children, so it must not pick up the labels inside them.
    let exposed = (attr(tabGroup.el, "AXTabs") as? [AXUIElement]) ?? []
    check("the tab group exposes its tabs", exposed.count, 3)
    check(
        "and only the tabs",
        exposed.allSatisfy { str($0, kAXRoleAttribute as String) == kAXRadioButtonRole as String })
}
// The whole point of the `selected` row: before it every tab read 0, and
// a reader could not say which one the window was showing.
check(
    "the shown tab reads as on, the others off",
    tabs.map { num($0.el, kAXValueAttribute as String) ?? -1 }, [1.0, 0.0, 0.0])
// `selected` is not `toggled`, and on macOS a tab's state is its AXValue:
// AXSelected is for item-like nodes (a list row), and accesskit_macos
// excludes tabs from it, so a tab must not answer to it.
check(
    "a tab does not answer AXSelected",
    tabs.allSatisfy { (attr($0.el, "AXSelected") as? Bool) != true })

// A list row takes the other spelling. `Role::ListItem` becomes an
// AXGroup (macOS has no row role outside tables), and because a row is
// item-like there, `selected` arrives as AXSelected — the attribute a tab
// deliberately does not answer. AXSelected is also settable, which is the
// interesting half: accesskit_macos only honours it on a node that is
// already *selectable*, and a node is selectable only when it carries the
// state at all.
let list = all.first { $0.role == "AXList" }
check("the list is an AXList", list != nil)
let rows = list.map { children($0.el) } ?? []
check("three rows are exposed", rows.count, 3)
if rows.count == 3 {
    check(
        "a row is an AXGroup (macOS has no row role outside tables)",
        rows.allSatisfy { str($0, kAXRoleAttribute as String) == kAXGroupRole as String })
    // The row's text child is its content; the row itself is unnamed, so
    // nothing is announced twice.
    check("a row is unnamed, its text is the content", rows.allSatisfy { str($0, kAXTitleAttribute as String) == nil })
    check(
        "the picked row reads as selected",
        rows.map { (attr($0, "AXSelected") as? Bool) ?? false }, [false, true, false])
    // The state a tab carries in AXValue, a row carries in AXSelected:
    // the two roles must not answer each other's attribute.
    check(
        "a row does not carry its state in AXValue",
        rows.allSatisfy { num($0, kAXValueAttribute as String) == nil })
}

print("\n=== disclosure")
let disclosure = find(role: kAXButtonRole as String, title: "Advanced")
check("the disclosure is exposed with a stable name", disclosure != nil, "Advanced")
if let disclosure {
    // The tree carries `expanded` (kui-core's tests pin it) and UIA and
    // AT-SPI receive it, but accesskit_macos 0.27 maps no disclosure
    // state at all — there is no isAccessibilityExpanded in it. So a
    // VoiceOver user currently hears nothing about a disclosure being
    // shut. This check exists to fail the day that changes, so the note
    // in docs/adr/0001-accessibility-as-data.md can come out.
    check(
        "macOS exposes no AXExpanded yet (AccessKit 0.27 maps none)",
        attr(disclosure.el, "AXExpanded") == nil)
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
    // VoiceOver puts keyboard focus on the element under its cursor before
    // pressing it. The press renames the button; the node must survive the
    // rename, or the reader is left holding a dead element and the app
    // reports no focus at all.
    AXUIElementSetAttributeValue(press.el, kAXFocusedAttribute as CFString, kCFBooleanTrue)
    usleep(400_000)
    AXUIElementPerformAction(press.el, kAXPressAction as CFString)
    usleep(400_000)
    all = []
    walk(window, 0)
    check(
        "pressing renamed the button",
        find(role: kAXButtonRole as String, title: "count 1") != nil, "count 1")
    var focusedNow: CFTypeRef?
    AXUIElementCopyAttributeValue(app, kAXFocusedUIElementAttribute as CFString, &focusedNow)
    check(
        "the pressed button keeps focus through its rename",
        focusedNow.flatMap { str($0 as! AXUIElement, kAXTitleAttribute as String) }, "count 1")
    check(
        "the reader's element is still alive after the press",
        str(press.el, kAXRoleAttribute as String) != nil)
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
// Pressing a tab: the app switches, and the next frame moves the state
// from one tab to another. Both halves matter — a tab that turned on
// without its sibling turning off is what a reader announces as two open
// tabs, which is the failure `checked` would have given us.
if let network = all.first(where: { $0.role == kAXRadioButtonRole as String && $0.title == "Network" }) {
    check("a tab advertises press", actions(network.el).contains(kAXPressAction as String))
    AXUIElementPerformAction(network.el, kAXPressAction as CFString)
    usleep(400_000)
    all = []
    walk(window, 0)
    // The subrole again: the theme radios are AXRadioButtons too, and only
    // the tabs carry AXTabButton.
    let after = all.filter {
        $0.role == kAXRadioButtonRole as String
            && str($0.el, kAXSubroleAttribute as String) == "AXTabButton"
    }
    check(
        "pressing a tab moved the selection",
        after.map { num($0.el, kAXValueAttribute as String) ?? -1 }, [0.0, 1.0, 0.0])
    check("and renamed nothing", after.map(\.title), ["General", "Network", "About"])
} else {
    check("the tab is pressable", false, "Network not found")
}
// Picking a row. Two routes lead here and they do not behave alike:
// AXPress goes through the row's click payload like any button, while
// setting AXSelected is honoured only on a node accesskit_macos already
// considers selectable — `is_selected().is_some()`. A row that carries no
// selected state at all is therefore unselectable through that route,
// which is what the second half of this checks.
if let list = all.first(where: { $0.role == "AXList" }) {
    let rowsNow = { children(list.el) }
    let selection = { rowsNow().map { (attr($0, "AXSelected") as? Bool) ?? false } }
    let sent = rowsNow()[2]
    check("a row advertises press", actions(sent).contains(kAXPressAction as String))
    AXUIElementPerformAction(sent, kAXPressAction as CFString)
    usleep(400_000)
    all = []
    walk(window, 0)
    check("pressing a row moved the selection", selection(), [false, false, true])
    // Setting AXSelected on the row that already has the state: it is
    // selectable, so the request reaches the app as a click and the
    // selection is unchanged (it was already there).
    check(
        "the selected row answers AXSelected",
        (attr(rowsNow()[2], "AXSelected") as? Bool) ?? false)
    // Setting it on a row that carries no state: accesskit_macos drops
    // the request, because a row without the attribute is not selectable.
    // kui gives an unpicked row no state, so this is every other row —
    // a reader cannot select through AXSelected, only through AXPress.
    // Pinned so it fails if either side changes.
    AXUIElementSetAttributeValue(rowsNow()[0], "AXSelected" as CFString, kCFBooleanTrue)
    usleep(400_000)
    all = []
    walk(window, 0)
    check(
        "AXSelected does not move a selection onto a stateless row",
        selection(), [false, false, true])
    // AXPress still does, so the row is reachable either way.
    AXUIElementPerformAction(rowsNow()[0], kAXPressAction as CFString)
    usleep(400_000)
    all = []
    walk(window, 0)
    check("but AXPress does", selection(), [true, false, false])
} else {
    check("the list is pressable", false, "AXList not found")
}
// Pressing the disclosure: macOS is told nothing about the state itself
// (see the AXExpanded note above), so what a reader can observe is the
// panel arriving in the tree. That is the whole platform story today.
if let disclosure = find(role: kAXButtonRole as String, title: "Advanced") {
    let panel = { all.contains { $0.title == "Nothing here yet." || $0.value == "Nothing here yet." } }
    check("the disclosure starts shut", !panel())
    AXUIElementPerformAction(disclosure.el, kAXPressAction as CFString)
    usleep(400_000)
    all = []
    walk(window, 0)
    check("pressing it revealed the panel", panel())
    check(
        "the disclosure keeps its name across the state change",
        find(role: kAXButtonRole as String, title: "Advanced") != nil, "Advanced")
    AXUIElementPerformAction(disclosure.el, kAXPressAction as CFString)
    usleep(400_000)
    all = []
    walk(window, 0)
    check("pressing it again took the panel away", !panel())
} else {
    check("the disclosure is pressable", false, "Advanced not found")
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
/// What focus landed on, for a failure message: an unnamed node reports
/// its role and its first text child, so "nil" never has to be guessed at.
func focusedDesc() -> String {
    guard let el = attr(app, kAXFocusedUIElementAttribute as String) else { return "nothing" }
    let e = el as! AXUIElement
    let role = str(e, kAXRoleAttribute as String) ?? "?"
    let title = str(e, kAXTitleAttribute as String)
    let inner = children(e).compactMap { str($0, kAXValueAttribute as String) }.first
    return "\(role) \(title ?? inner ?? "unnamed")"
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
        check("Tab moved focus to the next control", focusedTitle() ?? focusedDesc(), "Save")
    } else {
        check("Tab keystroke", false, "could not build a CGEvent")
    }
} else {
    check("the button is focusable", false, "count 1 not found")
}

// -- Composites (docs/adr/0007-composite-keyboard-patterns.md) -----------
// The four container roles whose items are focusable are one Tab stop
// each with the arrows moving inside. Only the OS can answer two halves
// of that: whether the platform sees a radio group and a menu at all (and
// with the orientation the container's `dir` derived), and whether a real
// Tab keystroke leaves a tab bar in one step instead of walking its tabs.

print("\n=== composites")

/// Posts one key by virtual keycode and lets the app draw the next frame.
func postKey(_ code: CGKeyCode) -> Bool {
    guard let down = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: true),
          let up = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: false)
    else { return false }
    down.postToPid(pid)
    up.postToPid(pid)
    usleep(400_000)
    return true
}
let kTab: CGKeyCode = 0x30
let kRight: CGKeyCode = 0x7C
let kEscape: CGKeyCode = 0x35

// A radio group: the pattern with no example before this ADR, and the one
// whose arrows must also check the radio they land on.
all = []
walk(window, 0)
let group = all.first { $0.role == kAXRadioGroupRole as String }
check("the radio group is an AXRadioGroup", group != nil)
if let group {
    check("named by its label", group.title, "Theme")
    // AXOrientation is derived from the container's own `dir`, so a row
    // of radios announces itself horizontal. AppKit takes an
    // NSAccessibilityOrientation from the app and hands a *string* to the
    // client, which is the kind of thing only a real platform run says.
    check("and horizontal, from its dir", str(group.el, "AXOrientation"), "AXHorizontalOrientation")
    let radios = children(group.el)
    check("three radios are exposed", radios.count, 3)
    check(
        "named by their content",
        radios.compactMap { str($0, kAXTitleAttribute as String) },
        ["Light", "Dark", "Auto"])
    // A radio takes AXValue like a checkbox, not AXSelected like a row —
    // the group is what makes it one of a set.
    check(
        "the checked radio reads as on",
        radios.map { num($0, kAXValueAttribute as String) ?? -1 }, [0.0, 1.0, 0.0])

    // Focus the checked one — where Tab would enter — and press Right.
    // Both halves, as with the tab press: a radio group where the new one
    // turns on without the old one turning off is two checked radios.
    AXUIElementSetAttributeValue(radios[1], kAXFocusedAttribute as CFString, kCFBooleanTrue)
    usleep(400_000)
    if postKey(kRight) {
        all = []
        walk(window, 0)
        let after = all.first { $0.role == kAXRadioGroupRole as String }.map { children($0.el) } ?? []
        check(
            "an arrow key moved the checked radio",
            after.map { num($0, kAXValueAttribute as String) ?? -1 }, [0.0, 0.0, 1.0])
    } else {
        check("arrow keystroke", false, "could not build a CGEvent")
    }
}

// The ring shrinks, and this is the check the ADR exists for: from a tab,
// one Tab lands on the disclosure below the bar — not on the second tab,
// which is what the ring did before a composite was one stop.
if let general = tabs.first(where: { $0.title == "General" }) {
    AXUIElementSetAttributeValue(general.el, kAXFocusedAttribute as CFString, kCFBooleanTrue)
    usleep(400_000)
    check("a tab takes focus", focusedTitle(), "General")
    if postKey(kTab) {
        check(
            "Tab left the whole tab bar in one step",
            focusedTitle() ?? focusedDesc(), "Advanced")
    } else {
        check("Tab keystroke", false, "could not build a CGEvent")
    }
} else {
    check("the tab bar is one stop", false, "General not found")
}

// A menu: `modal` plus `role="menu"`, which is what a context menu is
// here. Nothing exposes it until the app declares it, so the button that
// opens it is pressed first.
if let actions = find(role: kAXButtonRole as String, title: "Actions ▾") {
    AXUIElementPerformAction(actions.el, kAXPressAction as CFString)
    usleep(400_000)
    all = []
    walk(window, 0)
    let menu = all.first { $0.role == kAXMenuRole as String }
    check("the menu is an AXMenu", menu != nil)
    if let menu {
        check("named by its label", menu.title, "Actions")
        check("and vertical, from its dir", str(menu.el, "AXOrientation"), "AXVerticalOrientation")
        let items = children(menu.el)
        check(
            "holding three AXMenuItems",
            items.map { str($0, kAXRoleAttribute as String) ?? "?" },
            Array(repeating: kAXMenuItemRole as String, count: 3))
        check(
            "named by their content",
            items.compactMap { str($0, kAXTitleAttribute as String) },
            ["Rename", "Duplicate", "Archive"])
        // A menu is modal, so focus is inside it; the arrows move between
        // its items and — unlike a radio group — run none of them.
        check("focus entered the menu", focusedTitle(), "Rename")
        if postKey(kRight) {
            check("an arrow moved inside the menu", focusedTitle() ?? focusedDesc(), "Duplicate")
        }
        // Escape asks it to go away, and the app stops declaring it.
        _ = postKey(kEscape)
        all = []
        walk(window, 0)
        check("Escape closed the menu", !all.contains { $0.role == kAXMenuRole as String })
    }
} else {
    check("the menu opens", false, "Actions ▾ not found")
}

// -- Live regions and announcements --------------------------------------
// docs/adr/0008-live-regions-and-announcements.md. Two halves, and macOS
// spells them the same way underneath: a live node the adapter sees appear
// or get renamed becomes one AXAnnouncementRequested on the window. The
// region half is a *change*; the announcement half is a node kui invents
// for the purpose. Both are checked through the notification, because
// macOS exposes no attribute for either.

print("\n=== live regions and announcements")
all = []
walk(window, 0)
// The region is a plain box: the `live` row is the only reason it is in
// the tree at all, and its text is a child rather than its name — which is
// what a reader announces when it changes.
let statusText = all.first { $0.value == "No changes saved" || $0.title == "No changes saved" }
check("the live region's text is exposed", statusText != nil, "No changes saved")

// The window only: AccessKit posts the announcement on it, and an observer
// registered on the application element as well receives the same
// notification a second time.
let observer = watchAnnouncements(pid, [window])
check("the announcement notification can be observed", observer != nil)
if observer != nil {
    // Half one: pressing Save changes the region's text. Nothing announces
    // anything — the change *is* the announcement.
    settleAnnouncements()
    if let save = find(role: kAXButtonRole as String, title: "Save") {
        AXUIElementPerformAction(save.el, kAXPressAction as CFString)
        waitForAnnouncements(1)
        check(
            "changing a live region's text announces it",
            announcements.first?.0, "Saved 1 change")
        // Polite, not assertive: NSAccessibilityPriorityMedium is 50.
        check("politely", announcements.first?.1, 50)
        all = []
        walk(window, 0)
        check(
            "and the region still reads its new text",
            all.contains { $0.value == "Saved 1 change" || $0.title == "Saved 1 change" })
    } else {
        check("the Save button is there to press", false)
    }

    // Half two: an announcement with no node behind it. Nothing on screen
    // changes, so the notification is the whole of what a reader gets.
    settleAnnouncements()
    if let copy = find(role: kAXButtonRole as String, title: "Copy") {
        AXUIElementPerformAction(copy.el, kAXPressAction as CFString)
        waitForAnnouncements(1)
        check("an announcement with no node reaches the OS", announcements.first?.0, "Copied to clipboard")
        // The same message again. This is the check that pins decision 7:
        // the bridge mints a *fresh* node id per announcement, so the
        // adapter takes its node_added path. Reusing one id would take the
        // node_updated path, which fires only when the label changed — and
        // the second "Copied to clipboard" would be silent.
        settleAnnouncements()
        AXUIElementPerformAction(copy.el, kAXPressAction as CFString)
        waitForAnnouncements(1)
        check(
            "and the same message twice in a row is said twice",
            announcements.first?.0, "Copied to clipboard")
        // It is left standing in the tree afterwards: UIA reads the name
        // back after its own live-region event, so a node removed in the
        // update it announced in would be a race. macOS does not need it,
        // but the tree is one shape for all three platforms.
        all = []
        walk(window, 0)
        check(
            "the last announcement stays readable in the tree",
            all.contains { $0.title == "Copied to clipboard" || $0.value == "Copied to clipboard" })
    } else {
        check("the Copy button is there to press", false)
    }
}

print("\n\(checks - failures)/\(checks) checks passed")
exit(failures == 0 ? 0 : 1)
