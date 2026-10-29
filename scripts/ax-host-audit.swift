// Walks a host's own window through the macOS accessibility API — the one
// VoiceOver uses — and checks that kui's bridge on it answers (backlog
// F269): examples/c/tools/access_host.c, a GLFW window with no GPU and
// kui_access_attach on its NSView. Before F269 such a window showed the
// platform its traffic lights and its title and nothing else.
//
//   cargo run -p kui-devtools --bin cbuild
//   ./target/debug/access_host & swift scripts/ax-host-audit.swift $!; wait $!
//
// It asks only the OS, never kui: the controls by name, a press on Harvest
// read back as the count the host's model now shows, the checkbox's value
// after a press, and focus set the way VoiceOver's cursor sets it read back
// as the name the host says kui focused. Then it closes the window through
// its close button, and the host exits with what it saw (7 is all three).
// `--keep` leaves the window open.
//
// The calling terminal needs Accessibility permission (System Settings >
// Privacy & Security > Accessibility). The tree appears lazily — nothing is
// derived until a client asks — so the walk polls until the controls are
// there. Exits nonzero on a failed check.

import ApplicationServices
import Foundation

var failures = 0
var checks = 0

func check(_ what: String, _ cond: Bool, _ detail: String = "") {
    checks += 1
    print("  \(cond ? "ok  " : "FAIL")  \(what)\(detail.isEmpty ? "" : ": \(detail)")")
    if !cond { failures += 1 }
}

func attr(_ el: AXUIElement, _ name: String) -> CFTypeRef? {
    var out: CFTypeRef?
    return AXUIElementCopyAttributeValue(el, name as CFString, &out) == .success ? out : nil
}

func str(_ el: AXUIElement, _ name: String) -> String? {
    attr(el, name) as? String
}

func children(_ el: AXUIElement) -> [AXUIElement] {
    (attr(el, kAXChildrenAttribute as String) as? [AXUIElement]) ?? []
}

struct Found {
    let el: AXUIElement
    let role: String
    let title: String
    let value: String
    let depth: Int
}

/// The window's tree, depth first.
func walk(_ el: AXUIElement, _ depth: Int = 0, into out: inout [Found]) {
    if depth > 16 { return }
    let role = str(el, kAXRoleAttribute as String) ?? "?"
    let title = str(el, kAXTitleAttribute as String) ?? str(el, kAXDescriptionAttribute as String) ?? ""
    var value = str(el, kAXValueAttribute as String) ?? ""
    if value.isEmpty, let n = attr(el, kAXValueAttribute as String) as? NSNumber {
        value = n.stringValue
    }
    out.append(Found(el: el, role: role, title: title, value: value, depth: depth))
    for c in children(el) { walk(c, depth + 1, into: &out) }
}

func tree(_ window: AXUIElement) -> [Found] {
    var out: [Found] = []
    walk(window, into: &out)
    return out
}

func find(_ all: [Found], role: String, title: String? = nil, value: String? = nil) -> Found? {
    all.first {
        $0.role == role && (title == nil || $0.title == title!) && (value == nil || $0.value == value!)
    }
}

/// Polls the window until `pred` holds over its tree, or time is up.
func waitFor(_ window: AXUIElement, seconds: Double = 3, _ pred: ([Found]) -> Bool) -> [Found] {
    let deadline = Date().addingTimeInterval(seconds)
    var all = tree(window)
    while !pred(all), Date() < deadline {
        usleep(50_000)
        all = tree(window)
    }
    return all
}

// -------------------------------------------------------------------- main

let args = CommandLine.arguments.dropFirst()
guard let pidArg = args.first(where: { Int32($0) != nil }), let pid = Int32(pidArg) else {
    print("usage: swift scripts/ax-host-audit.swift <pid> [--keep]")
    exit(64)
}
let keep = args.contains("--keep")
if !AXIsProcessTrusted() {
    FileHandle.standardError.write(
        "ax-host-audit: this process is not trusted for Accessibility; grant it in System Settings > Privacy & Security > Accessibility.\n"
            .data(using: .utf8)!)
    exit(2)
}

let app = AXUIElementCreateApplication(pid)
var window: AXUIElement?
for _ in 0..<50 {
    if let w = (attr(app, kAXWindowsAttribute as String) as? [AXUIElement])?.first {
        window = w
        break
    }
    usleep(200_000)
}
guard let window else {
    FileHandle.standardError.write("ax-host-audit: the app exposes no window\n".data(using: .utf8)!)
    exit(1)
}

// The first ask is what activates the bridge; the host answers with its
// next frame, which the ask woke it for.
var all = waitFor(window, seconds: 10) { find($0, role: kAXButtonRole as String, title: "Harvest") != nil }
print("the window, as the platform sees it:")
for f in all {
    let pad = String(repeating: "  ", count: f.depth + 1)
    let title = f.title.isEmpty ? "" : " \"\(f.title)\""
    let value = f.value.isEmpty ? "" : " = \(f.value)"
    print("\(pad)\(f.role)\(title)\(value)")
}
print("")

let harvest = find(all, role: kAXButtonRole as String, title: "Harvest")
let plant = find(all, role: kAXButtonRole as String, title: "Plant")
let water = find(all, role: kAXCheckBoxRole as String, title: "Auto-water")
check("the Harvest button is in the window", harvest != nil)
check("the Plant button is in the window", plant != nil)
check("the Auto-water checkbox is in the window", water != nil, water.map { "value \($0.value)" } ?? "")
check("and the host's text", find(all, role: kAXStaticTextRole as String, value: "Harvested 0") != nil)

if let harvest {
    AXUIElementPerformAction(harvest.el, kAXPressAction as CFString)
    all = waitFor(window) { find($0, role: kAXStaticTextRole as String, value: "Harvested 1") != nil }
    check(
        "AXPress on Harvest is a click: the host counted it",
        find(all, role: kAXStaticTextRole as String, value: "Harvested 1") != nil)
}
if let water {
    AXUIElementPerformAction(water.el, kAXPressAction as CFString)
    all = waitFor(window) { find($0, role: kAXCheckBoxRole as String, title: "Auto-water", value: "1") != nil }
    let now = find(all, role: kAXCheckBoxRole as String, title: "Auto-water")
    check("AXPress on the checkbox toggles it", now?.value == "1", "value \(now?.value ?? "?")")
}
if let plant {
    AXUIElementSetAttributeValue(plant.el, kAXFocusedAttribute as CFString, kCFBooleanTrue)
    all = waitFor(window) { find($0, role: kAXStaticTextRole as String, value: "Focus: Plant") != nil }
    check(
        "setting AXFocused on Plant moves kui's focus there",
        find(all, role: kAXStaticTextRole as String, value: "Focus: Plant") != nil)
    // Whether the platform says so too is the window's key state: an
    // element of a window that is not key is not focused to AppKit.
    let focused = (attr(plant.el, kAXFocusedAttribute as String) as? Bool) ?? false
    print("  info  Plant's AXFocused: \(focused)")
}

if !keep,
    let close = attr(window, kAXCloseButtonAttribute as String),
    CFGetTypeID(close) == AXUIElementGetTypeID()
{
    AXUIElementPerformAction(close as! AXUIElement, kAXPressAction as CFString)
}

print("")
print("\(checks - failures)/\(checks) checks passed")
exit(failures == 0 ? 0 : 1)
