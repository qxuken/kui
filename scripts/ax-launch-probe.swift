// Times what a screen reader experiences while a kui app *launches* — the
// gap `scripts/ax-audit.swift` deliberately waits out.
//
// The audit polls until the tree is there and then checks what it says; this
// asks the opposite question, and asks it from t=0: how long does each
// accessibility request take while the app is still starting, and does any of
// them take long enough for the client to give up? VoiceOver saying
// "<app> is not responding" is exactly that giving-up — a request the app did
// not answer inside the client's patience — so a slow request here is that
// announcement, measured without a screen reader running (F13).
//
// What it found (F13, 2026-09-06): every kui launch on macOS has exactly one
// long request, 0.23s warm and 0.43s cold, starting the moment the process
// becomes AX-addressable and ending when the first frame is done. The whole
// of `resumed` — window creation, GPU init — sits inside it. Both bindings
// do it, so run the Rust example beside the Node one before blaming either:
//
//   swift scripts/ax-launch-probe.swift -- node dist/counter-window.mjs
//   swift scripts/ax-launch-probe.swift -- ./target/release/examples/accessibility
//
// The calling terminal needs Accessibility permission (System Settings >
// Privacy & Security > Accessibility). Exits nonzero if any request the app
// was addressable for took longer than `--patience`.
//
// `--timeout S`  per-request messaging timeout (default 10s). Left long on
//                purpose: the point is to measure the real latency rather
//                than clip it, and `--patience` is where the verdict lives.
// `--patience S` how long a client would wait before giving up (default
//                0.5s). Any request slower than this is counted as one that
//                would have been announced; the exit status is that count.
// `--for S`      how long to keep probing after launch (default 12s).
// `--every MS`   polling interval (default 20ms).
// `--quiet`      only state changes and the summary, not every poll.

import ApplicationServices
import Foundation

// ------------------------------------------------------------------ arguments

var timeout: Double = 10.0
var patience: Double = 0.5
var duration: Double = 12.0
var every: Double = 0.020
var quiet = false
var command: [String] = []

var args = Array(CommandLine.arguments.dropFirst())
while let a = args.first {
    args.removeFirst()
    switch a {
    case "--timeout": timeout = Double(args.removeFirst()) ?? timeout
    case "--patience": patience = Double(args.removeFirst()) ?? patience
    case "--for": duration = Double(args.removeFirst()) ?? duration
    case "--every": every = (Double(args.removeFirst()) ?? 20) / 1000
    case "--quiet": quiet = true
    case "--": command = args; args = []
    default:
        FileHandle.standardError.write("ax-launch-probe: unknown argument \(a)\n".data(using: .utf8)!)
        exit(64)
    }
}
guard !command.isEmpty else {
    print("usage: swift scripts/ax-launch-probe.swift [--timeout S] [--patience S] [--for S] [--every MS] [--quiet] -- <command> [args...]")
    exit(64)
}
if !AXIsProcessTrusted() {
    FileHandle.standardError.write(
        "ax-launch-probe: this process is not trusted for Accessibility; grant it in System Settings > Privacy & Security > Accessibility.\n"
            .data(using: .utf8)!)
    exit(2)
}

/// One clock for both processes: the child's `KUI_TRACE` stamps are the same
/// `seconds.microseconds` since the epoch, so the two interleave by sorting.
func now() -> Double { Date().timeIntervalSince1970 }

func stamp(_ t: Double, _ what: String) {
    print(String(format: "ax-probe %.6f %@", t, what))
}

// -------------------------------------------------------------------- launch

let proc = Process()
proc.executableURL = URL(fileURLWithPath: "/usr/bin/env")
proc.arguments = command
// Whatever the child prints goes to our stderr, so its own stamps (add some
// while diagnosing) interleave with these lines on one clock.
proc.standardOutput = FileHandle.standardError
proc.standardError = FileHandle.standardError

let t0 = now()
do { try proc.run() } catch {
    FileHandle.standardError.write("ax-launch-probe: cannot launch: \(error)\n".data(using: .utf8)!)
    exit(1)
}
let pid = proc.processIdentifier
stamp(t0, "launched pid \(pid): \(command.joined(separator: " "))")

let app = AXUIElementCreateApplication(pid)
AXUIElementSetMessagingTimeout(app, Float(timeout))

// ------------------------------------------------------------------- probing

func name(_ e: AXError) -> String {
    switch e {
    case .success: return "success"
    case .cannotComplete: return "cannotComplete(-25204)"
    case .invalidUIElement: return "invalidUIElement"
    case .attributeUnsupported: return "attributeUnsupported"
    case .noValue: return "noValue"
    case .apiDisabled: return "apiDisabled"
    case .notImplemented: return "notImplemented"
    case .failure: return "failure"
    default: return "error(\(e.rawValue))"
    }
}

struct Ask {
    let what: String
    let at: Double
    let took: Double
    let err: AXError
    let detail: String

    /// `cannotComplete` means two different things and the latency tells them
    /// apart. Returned at once, the process has no accessibility connection
    /// yet — it is not an app the AX system can address, which every process
    /// is for its first moments. Returned after a wait, the connection exists
    /// and the app did not answer: that is the timeout a screen reader
    /// announces.
    var unaddressable: Bool { err == .cannotComplete && took < 0.05 }
    var timedOut: Bool { err == .cannotComplete && took >= 0.05 }
}

var asks: [Ask] = []

/// One AX request, timed. The timing is the point: this is the number a
/// screen reader compares against its own patience.
@discardableResult
func ask(_ el: AXUIElement, _ attr: String, _ what: String) -> (CFTypeRef?, Ask) {
    let at = now()
    var out: CFTypeRef?
    let err = AXUIElementCopyAttributeValue(el, attr as CFString, &out)
    let took = now() - at
    var detail = ""
    if err == .success, let out {
        if let arr = out as? [AXUIElement] { detail = "\(arr.count) element(s)" }
        else if let s = out as? String { detail = s.debugDescription }
        else { detail = String(describing: out) }
    }
    let a = Ask(what: what, at: at, took: took, err: err, detail: detail)
    asks.append(a)
    return (out, a)
}

func line(_ a: Ask) -> String {
    String(
        format: "%@ took %.3fs -> %@%@", a.what, a.took, name(a.err),
        a.detail.isEmpty ? "" : " \(a.detail)")
}

var lastState = ""
var sawWindow = false
var firstWindowAt: Double?
var firstChildrenAt: Double?
var timeouts = 0

// Poll the way a screen reader arriving at a freshly launched app does: what
// windows are there, what is in the front one, and what has focus.
while now() - t0 < duration {
    if !proc.isRunning { stamp(now(), "child exited"); break }

    let (winsRef, winAsk) = ask(app, kAXWindowsAttribute as String, "AXWindows")
    var state = "windows=\(winAsk.err == .success ? winAsk.detail : name(winAsk.err))"
    var report = [line(winAsk)]

    if winAsk.timedOut { timeouts += 1 }

    if let wins = winsRef as? [AXUIElement], let win = wins.first {
        if !sawWindow {
            sawWindow = true
            firstWindowAt = winAsk.at
        }
        let (kidsRef, kidsAsk) = ask(win, kAXChildrenAttribute as String, "AXWindow.AXChildren")
        if kidsAsk.timedOut { timeouts += 1 }
        report.append(line(kidsAsk))
        let kids = (kidsRef as? [AXUIElement]) ?? []
        state += " children=\(kidsAsk.err == .success ? String(kids.count) : name(kidsAsk.err))"
        if firstChildrenAt == nil, !kids.isEmpty { firstChildrenAt = kidsAsk.at }

        let (_, roleAsk) = ask(win, kAXRoleAttribute as String, "AXWindow.AXRole")
        if roleAsk.timedOut { timeouts += 1 }
        report.append(line(roleAsk))
    }

    let (_, focusAsk) = ask(app, kAXFocusedUIElementAttribute as String, "AXFocusedUIElement")
    if focusAsk.timedOut { timeouts += 1 }
    report.append(line(focusAsk))
    state += " focus=\(name(focusAsk.err))"

    let slow = asks.suffix(report.count).contains { $0.took > 0.1 }
    if !quiet || state != lastState || slow {
        stamp(report.first.map { _ in asks[asks.count - report.count].at } ?? now(), report.joined(separator: " | "))
    }
    lastState = state
    Thread.sleep(forTimeInterval: every)
}

if proc.isRunning { proc.terminate() }

// ------------------------------------------------------------------- summary

print("\n=== summary")
print(String(format: "  requests            %d over %.1fs", asks.count, now() - t0))
if let t = firstWindowAt {
    print(String(format: "  first AXWindow      +%.3fs after launch", t - t0))
} else {
    print("  first AXWindow      never")
}
if let t = firstChildrenAt {
    print(String(format: "  first tree contents +%.3fs after launch", t - t0))
} else {
    print("  first tree contents never — the window stayed empty to accessibility")
}
let slowest = asks.max { $0.took < $1.took }
if let s = slowest {
    print(String(format: "  slowest request     %.3fs (%@ at +%.3fs) -> %@", s.took, s.what, s.at - t0, name(s.err)))
}
let over = [0.1, 0.25, 0.5, 1.0, 2.0].map { limit in
    (limit, asks.filter { $0.took > limit }.count)
}
print("  requests slower than: " + over.map { "\($0.0)s: \($0.1)" }.joined(separator: "  "))
let unaddressable = asks.filter(\.unaddressable).count
let lastUnaddressable = asks.last(where: { $0.unaddressable })
print(
    "  not addressable yet \(unaddressable) request(s)"
        + (lastUnaddressable.map { String(format: ", the last at +%.3fs", $0.at - t0) } ?? ""))
print("  answered too slowly \(timeouts) request(s) (no reply, connection up)")
let impatient = asks.filter { $0.took > patience }
print(String(format: "  over --patience %.2fs  %d request(s)", patience, impatient.count))
for a in impatient.prefix(10) {
    print(String(format: "      +%.3fs %@ waited %.3fs", a.at - t0, a.what, a.took))
}
if impatient.isEmpty {
    print("\n  Nothing here is what VoiceOver announces as \"not responding\": every")
    print("  request the app was addressable for came back inside the patience above.")
} else {
    print("\n  A request slower than a client's patience is what VoiceOver announces")
    print("  as \"<app> is not responding\".")
}
exit(impatient.isEmpty ? 0 : 1)
