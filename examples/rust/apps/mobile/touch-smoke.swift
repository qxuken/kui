// Run on macOS against a booted simulator with the spike installed.
// swift touch-smoke.swift ios <UDID> <display scale>   (AXe on PATH)
// swift touch-smoke.swift android <adb serial>         (adb on PATH)
import AppKit
import Foundation
import Vision

struct Failure: Error, CustomStringConvertible {
    var description: String
    init(_ message: String) { description = message }
}

func run(_ args: [String], capture: Bool = false) throws -> Data {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
    process.arguments = args
    let pipe = Pipe()
    if capture { process.standardOutput = pipe }
    try process.run()
    let data = capture ? pipe.fileHandleForReading.readDataToEndOfFile() : Data()
    process.waitUntilExit()
    guard process.terminationStatus == 0 else { throw Failure("Command failed: \(args)") }
    return data
}

// AXe supports rotation on the Duo. For a standard iPhone use the same
// Rotate menu as the user, targeting the selected simulator window.
func rotatePhone(_ device: String, left: Bool) throws {
    let data = try run(["xcrun", "simctl", "list", "devices", "booted", "--json"], capture: true)
    struct Devices: Decodable {
        struct Device: Decodable { let name, udid: String }
        let devices: [String: [Device]]
    }
    let devices = try JSONDecoder().decode(Devices.self, from: data).devices.values.flatMap { $0 }
    guard let name = devices.first(where: { $0.udid == device })?.name else { throw Failure("Unknown simulator") }
    func attribute(_ item: AXUIElement, _ name: String) -> CFTypeRef? {
        var value: CFTypeRef?
        AXUIElementCopyAttributeValue(item, name as CFString, &value)
        return value
    }
    func menuItem(_ element: AXUIElement, _ title: String) -> AXUIElement? {
        if attribute(element, kAXTitleAttribute) as? String == title { return element }
        for child in attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? [] {
            if let found = menuItem(child, title) { return found }
        }
        return nil
    }
    let windows = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]] ?? []
    for window in windows {
        guard let owner = window[kCGWindowOwnerName as String] as? String,
              ["Device Hub", "Simulator"].contains(owner),
              let pid = window[kCGWindowOwnerPID as String] as? Int32 else { continue }
        let app = AXUIElementCreateApplication(pid)
        guard let focused = attribute(app, kAXFocusedWindowAttribute),
              let title = attribute(focused as! AXUIElement, kAXTitleAttribute) as? String,
              title.hasPrefix(name), let bar = attribute(app, kAXMenuBarAttribute),
              let item = menuItem(bar as! AXUIElement, left ? "Rotate Left" : "Rotate Right"),
              AXUIElementPerformAction(item, kAXPressAction as CFString) == .success else { continue }
        return
    }
    throw Failure("Select the target simulator window in Device Hub or Simulator and enable macOS Accessibility for the test")
}

func smoke() throws {
    var args = Array(CommandLine.arguments.dropFirst())
    let restoreWindows = args.last == "--restore-windows"
    if restoreWindows { args.removeLast() }
    let windows = args.last == "--windows"
    if windows { args.removeLast() }
    let rotate = args.last == "--rotate"
    if rotate { args.removeLast() }
    var flickCount: Int?
    if let index = args.firstIndex(of: "--flicks") {
        guard !rotate, index == args.count - 2, let count = Int(args[index + 1]), count > 0 else {
            throw Failure("Use --flicks COUNT with a positive repeat count")
        }
        flickCount = count
        args.removeLast(2)
    }
    var keyboardCycles: Int?
    if let index = args.firstIndex(of: "--keyboard") {
        guard !rotate, flickCount == nil, args.first == "android", index == args.count - 2,
              let count = Int(args[index + 1]), count > 0 else {
            throw Failure("Use android SERIAL --keyboard COUNT with a positive repeat count")
        }
        keyboardCycles = count
        args.removeLast(2)
    }
    guard args.count >= 2, ["android", "ios"].contains(args[0]) else {
        throw Failure("Supply android SERIAL or ios UDID DISPLAY_SCALE")
    }
    let ios = args[0] == "ios"
    let device = args[1]
    let scale = ios ? Double(args.last!) ?? 0 : 1
    guard scale > 0, args.count == (ios ? 3 : 2) else { throw Failure("Invalid device arguments") }
    let adb = ["adb", "-s", device]
    let sim = ["xcrun", "simctl"]
    let bundle = "dev.kui.mobile.spike"

    func launch() throws {
        _ = try run(ios ? ["env", "SIMCTL_CHILD_KUI_TEST_DRAFTS=\(UUID().uuidString)"] + sim + ["launch", "--terminate-running-process", device, bundle]
                         : adb + ["shell", "am", "start", "-S", "-n", bundle + "/dev.kui.KuiActivity", "--ez", "dev.kui.mobile.spike.ephemeral", "true"])
    }

    // Locate the actual rendered button and rows. This also catches a mismatch
    // between the host's touch coordinates and the renderer's viewport.
    var landscape = false
    func labels() throws -> [String: CGPoint] {
        let png: Data
        if ios {
            let file = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".png")
            defer { try? FileManager.default.removeItem(at: file) }
            _ = try run(sim + ["io", device, "screenshot", file.path])
            png = try Data(contentsOf: file)
        } else {
            png = try run(adb + ["exec-out", "screencap", "-p"], capture: true)
        }
        guard let image = NSBitmapImageRep(data: png)?.cgImage else { throw Failure("Invalid screenshot") }
        landscape = image.width > image.height
        let request = VNRecognizeTextRequest()
        try VNImageRequestHandler(cgImage: image).perform([request])
        var found = [String: CGPoint]()
        for result in request.results ?? [] {
            guard let text = result.topCandidates(1).first?.string else { continue }
            let box = result.boundingBox
            let point = CGPoint(x: box.midX * Double(image.width) / scale,
                                  y: (1 - box.midY) * Double(image.height) / scale)
            // A Gboard suggestion can repeat the field value below the field.
            if point.y < (found[text]?.y ?? .infinity) { found[text] = point }
        }
        return found
    }

    if restoreWindows {
        guard ios else { throw Failure("Window restoration requires an iPad simulator") }
        func output(_ args: [String]) throws -> String {
            String(decoding: try run(args, capture: true), as: UTF8.self)
                .trimmingCharacters(in: .whitespacesAndNewlines)
        }
        let data = try output(sim + ["get_app_container", device, bundle, "data"])
        let dir = URL(fileURLWithPath: data).appendingPathComponent("Library/Application Support/KUI/tablet")
        let notes = ["A", "B"]
        func drafts() throws -> [String] {
            try notes.map { note in
                let path = dir.appendingPathComponent("\(note).txt")
                return FileManager.default.fileExists(atPath: path.path)
                    ? try String(contentsOf: path, encoding: .utf8) : "Draft \(note)"
            }
        }
        func axe(_ command: String, _ arguments: [String]) throws {
            _ = try run(["axe", command, "--udid", device] + arguments)
        }
        func rows(_ note: String, _ screen: [String: CGPoint]) -> [(Int, CGPoint)] {
            let prefix = "Note \(note) paragraph "
            return screen.compactMap { label, point in
                guard label.hasPrefix(prefix), let n = Int(label.dropFirst(prefix.count)) else { return nil }
                return (n, point)
            }.sorted { $0.0 < $1.0 }
        }
        let start = try labels(), original = try drafts(), carets = [2, 5]
        guard let a = start["Note A"], let b = start["Note B"], abs(a.x - b.x) > 100 else {
            throw Failure("Arrange A and B side by side")
        }
        for (i, note) in notes.enumerated() {
            let visible = rows(note, start)
            guard visible.count >= 6, original[i].count >= carets[i] else {
                throw Failure("Need six visible rows and at least \(carets[i]) characters in \(note)")
            }
            let middle = visible[visible.count / 2].1
            // Aim at different numbered rows even when a previous run scrolled the lists.
            let step = (visible.last!.1.y - visible[0].1.y) / Double(visible.last!.0 - visible[0].0)
            let target = i == 0 ? 6 : 18
            let dy = min(280, max(-280, Double(visible[0].0 - target) * step))
            if abs(dy) < 1 { continue }
            try axe("swipe", ["--start-x", "\(Int(middle.x))", "--end-x", "\(Int(middle.x))",
                              "--start-y", "\(Int(middle.y))", "--end-y", "\(Int(middle.y + dy))", "--duration", "0.55"])
        }
        for (i, note) in notes.enumerated() {
            let title = [a,b][i]
            guard let button = start["Open \(notes[1-i]) in New Window"] else { throw Failure("Missing button in \(note)") }
            try axe("tap", ["-x", "\(Int(title.x))", "-y", "\(Int(button.y + (button.y - title.y)))"])
            try axe("key-combo", ["--modifiers", "227", "--key", "80"])
            for _ in 0..<carets[i] { try axe("key", ["79"]) }
        }
        let before = try labels()
        let markers = try notes.map { note -> (Int, CGPoint) in
            let visible = rows(note, before)
            guard visible.count >= 4 else { throw Failure("Missing paragraphs in \(note)") }
            return visible[visible.count / 2]
        }
        guard markers[0].0 != markers[1].0 else { throw Failure("Expected different paragraphs in the two windows") }
        let services = try output(sim + ["spawn", device, "launchctl", "list"])
        guard let line = services.split(separator: "\n").first(where: { $0.contains("UIKitApplication:\(bundle)[") }),
              let first = line.split(whereSeparator: { $0.isWhitespace }).first, let pid = Int32(first), pid > 1 else {
            throw Failure("Could not find running spike process")
        }
        _ = try run(["kill", "-KILL", "\(pid)"])
        let launched = try output(sim + ["launch", device, bundle])
        guard let newPID = launched.split(separator: " ").last.flatMap({ Int32($0) }), newPID > 1, newPID != pid else {
            throw Failure("Expected a new spike process after SIGKILL")
        }
        let deadline = Date().addingTimeInterval(20)
        var restored = false
        repeat {
            let screen = try labels()
            restored = notes.enumerated().allSatisfy { i, note in
                let (n, old) = markers[i]
                guard let point = screen["Note \(note) paragraph \(n)"] else { return false }
                return abs(point.y - old.y) < 2 && abs(point.x - old.x) < 2
            }
            if restored { break }
        } while Date() < deadline
        guard restored else { throw Failure("The two saved paragraphs did not return to their original screen positions") }
        var tested = Set<Int>()
        for probe in ["q", "z"] {
            try axe("type", [probe])
            let timeout = Date().addingTimeInterval(3)
            var changed = try drafts()
            while changed == original && Date() < timeout {
                Thread.sleep(forTimeInterval: 0.05)
                changed = try drafts()
            }
            guard let i = changed.indices.first(where: { changed[$0] != original[$0] }) else { throw Failure("Restored editor did not accept typing") }
            let position = original[i].index(original[i].startIndex, offsetBy: carets[i])
            let expected = original[i][..<position] + probe + original[i][position...]
            guard changed[i] == expected, changed[1-i] == original[1-i], tested.insert(i).inserted else {
                throw Failure("Unexpected insertion in \(notes[i]): \(changed[i])")
            }
            try axe("key", ["42"])
            let cleanupDeadline = Date().addingTimeInterval(3)
            while try drafts() != original && Date() < cleanupDeadline { Thread.sleep(forTimeInterval: 0.05) }
            guard try drafts() == original else { throw Failure("Probe cleanup failed") }
            print("Note \(notes[i]): paragraph \(markers[i].0) at y \(Int(markers[i].1.y.rounded())), insertion at character \(carets[i]).")
            if tested.count == 1 {
                // Cycle the key window without touching either field or moving its caret.
                try axe("key-combo", ["--modifiers", "227", "--key", "53"])
            }
        }
        return
    }

    if windows {
        guard ios else { throw Failure("The two-window check requires an iPad simulator") }
        let before = try labels()
        guard let a = before["Note A"], let b = before["Note B"], abs(a.x - b.x) > 100 else {
            throw Failure("Arrange Note A and Note B side by side and dismiss the keyboard")
        }
        func paragraphs(_ title: String) -> [(Int, CGPoint)] {
            before.compactMap { name, point in
                let prefix = "\(title) paragraph "
                guard name.hasPrefix(prefix), let n = Int(name.dropFirst(prefix.count)) else { return nil }
                return (n, point)
            }.sorted { $0.0 < $1.0 }
        }
        let rowsA = paragraphs("Note A"), rowsB = paragraphs("Note B")
        guard rowsA.count >= 4, rowsB.count >= 4 else { throw Failure("Too few visible paragraphs in the two windows") }
        let markerA = rowsA[rowsA.count / 2], markerB = rowsB[rowsB.count / 2]
        let rowHeight = (rowsB.last!.1.y - rowsB.first!.1.y) / Double(rowsB.last!.0 - rowsB.first!.0)
        let delta = rowsB.last!.0 >= 46 ? 3 * rowHeight : -3 * rowHeight
        _ = try run(["axe", "swipe", "--udid", device, "--start-x", String(Int(b.x)), "--end-x", String(Int(b.x)),
                     "--start-y", String(Int(markerB.1.y)), "--end-y", String(Int(markerB.1.y + delta)), "--duration", "0.35"])
        let deadline = Date().addingTimeInterval(6)
        repeat {
            let after = try labels()
            guard let stationary = after["Note A paragraph \(markerA.0)"], abs(stationary.y - markerA.1.y) < 2 else {
                throw Failure("Scrolling B moved A's paragraph \(markerA.0)")
            }
            if let moved = after["Note B paragraph \(markerB.0)"], abs(moved.y - markerB.1.y) >= rowHeight {
                print("iPad: scrolled B while A's paragraph \(markerA.0) stayed at y \(Int(stationary.y)).")
                return
            }
        } while Date() < deadline
        throw Failure("B's list did not scroll")
    }

    func ready() throws -> [String: CGPoint] {
        let deadline = Date().addingTimeInterval(25)
        var lastButton: CGPoint?
        repeat {
            let screen = try labels()
            if let button = screen["Increment"], screen["Button taps: 0"] != nil {
                if let lastButton, abs(lastButton.y - button.y) < 1 { return screen }
                lastButton = button
            }
            Thread.sleep(forTimeInterval: 0.2)
        } while Date() < deadline
        throw Failure("Spike did not display Increment and a zero count")
    }

    func checkFlick(_ before: [String: CGPoint], trial: Int) throws {
        guard let button = before["Increment"], let first = before["List row 1"],
              let second = before["List row 2"], let marker = before["List row 6"] else {
            throw Failure("List rows missing from the initial frame")
        }
        // The short path crosses finger slop but both endpoints are inside
        // the button, so releasing outside cannot explain a zero tap count.
        let inside = flickCount != nil && trial.isMultiple(of: 2)
        let distance = Int((second.y - first.y) * (inside ? 0.4 : 1.3))
        let x = String(Int(button.x)), y = Int(button.y) + (inside ? distance / 2 : 0)
        let duration = inside ? 75 : 160
        _ = try run(ios ? ["axe", "swipe", "--udid", device, "--start-x", x, "--start-y", String(y),
                           "--end-x", x, "--end-y", String(y - distance), "--duration", String(Double(duration) / 1000), "--delta", "3",
                           "--post-delay", "0"]
                        : adb + ["shell", "input", "swipe", x, String(y), x, String(y - distance), String(duration)])
        let deadline = Date().addingTimeInterval(8)
        var previous: CGFloat?
        var travel: CGFloat = 0
        repeat {
            let after = try labels()
            if after.keys.contains(where: { label in
                let prefix = "Button taps: "
                guard label.hasPrefix(prefix), let count = Int(label.dropFirst(prefix.count)) else { return false }
                return count > 0
            }) {
                throw Failure("Flick \(trial): expected zero button taps")
            }
            // OCR can miss a label while the list is moving. Require two
            // valid measurements of the final position and the zero count.
            if after["Button taps: 0"] != nil, let moved = after["List row 6"] {
                travel = marker.y - moved.y
                if let previous, abs(travel - previous) < 2 && travel > Double(distance) * 1.1 {
                    print("Flick \(trial): finger \(distance), list \(Int(travel)), zero clicks.")
                    fflush(stdout)
                    return
                }
                previous = travel
            } else {
                previous = nil
            }
            Thread.sleep(forTimeInterval: 0.08)
        } while Date() < deadline
        throw Failure("Flick \(trial): list travelled \(travel) for a finger displacement of \(distance)")
    }

    try launch()
    var before = try ready()
    func tapPoint(_ point: CGPoint) throws {
        let x = String(Int(point.x)), y = String(Int(point.y))
        _ = try run(ios ? ["axe", "tap", "--udid", device, "-x", x, "-y", y]
                        : adb + ["shell", "input", "tap", x, y])
    }

    if let count = keyboardCycles {
        guard let button = before["Increment"], let row = before["List row 1"],
              let marker = before["List row 14"] else { throw Failure("Start in portrait with the full list visible") }
        let field = CGPoint(x: 2 * button.x, y: (button.y + row.y) / 2)
        func inField(_ point: CGPoint) -> Bool { point.y > button.y && point.y < row.y }
        func waitFor(_ description: String, _ check: ([String: CGPoint]) -> Bool) throws -> [String: CGPoint] {
            let until = Date().addingTimeInterval(6)
            repeat {
                let screen = try labels()
                if check(screen) { return screen }
                Thread.sleep(forTimeInterval: 0.08)
            } while Date() < until
            throw Failure(description)
        }
        let sdk = String(decoding: try run(adb + ["shell", "getprop", "ro.build.version.sdk"], capture: true), as: UTF8.self)
        let snapshots = (Int(sdk.trimmingCharacters(in: .whitespacesAndNewlines)) ?? 0) >= 33
        func servedConnection() throws -> String {
            let ime = String(decoding: try run(adb + ["shell", "dumpsys", "input_method"], capture: true), as: UTF8.self)
            guard ime.contains("mServedView=dev.kui.KuiActivity$Editor{"),
                  let id = ime.range(of: #"mServedInputConnection=RemoteInputConnectionImpl\{connection=dev\.kui\.KuiActivity\$Connection@[0-9a-f]+"#, options: .regularExpression) else {
                throw Failure("Android is not serving the KUI editor's input connection")
            }
            return String(ime[id])
        }
        for cycle in 1...count {
            try tapPoint(field)
            _ = try waitFor("Cycle \(cycle): missing on-screen D/A keys") { screen in
                ["d", "a"].allSatisfy { letter in
                    screen.contains { $0.key.lowercased() == letter && $0.value.y > row.y }
                }
            }
            let connection = try servedConnection()
            if cycle > 1 {
                _ = try run(adb + ["shell", "input", "keyevent", "DEL", "DEL", "DEL"])
                _ = try waitFor("Cycle \(cycle): previous text did not clear") { screen in
                    !screen.contains { inField($0.value) && $0.key.lowercased().contains("dad") }
                }
            }
            // Keys are already legible while Gboard is still sliding up. A
            // position from that frame can hit the next row by dispatch time.
            func keyRow(_ screen: [String: CGPoint]) -> [CGPoint]? {
                let letters = ["a", "d", "f"]
                let points = letters.compactMap { letter in
                    screen.first { $0.key.lowercased() == letter && $0.value.y > row.y }?.value
                }
                guard points.count == letters.count,
                      points[0].x < points[1].x, points[1].x < points[2].x,
                      points.allSatisfy({ abs($0.y - points[0].y) < (points[1].x - points[0].x) / 4 }) else { return nil }
                return points
            }
            var previous: [CGPoint]?
            let keys = try waitFor("Cycle \(cycle): keyboard row did not settle") { screen in
                let current = keyRow(screen)
                defer { previous = current }
                guard let current, let previous else { return false }
                return zip(current, previous).allSatisfy { abs($0.y - $1.y) < 2 && abs($0.x - $1.x) < 2 }
            }
            guard let points = keyRow(keys) else { throw Failure("Missing Gboard key row") }
            for point in [points[1], points[0], points[1]] { try tapPoint(point) }
            func hasWord(_ screen: [String: CGPoint]) -> Bool {
                screen.contains { inField($0.value) && $0.key.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "|")) == "dad" }
            }
            _ = try waitFor("Cycle \(cycle): Gboard did not type dad into the field", hasWord)
            if snapshots && cycle > 1 {
                guard try servedConnection() == connection else {
                    throw Failure("Cycle \(cycle): editing restarted the active input connection")
                }
            }
            _ = try run(adb + ["shell", "input", "keyevent", "BACK"])
            _ = try waitFor("Cycle \(cycle): keyboard did not close with the text retained") { screen in
                hasWord(screen) && abs((screen["List row 14"]?.y ?? 0) - marker.y) < 4 &&
                    !screen.contains { $0.key.lowercased() == "d" && $0.value.y > row.y }
            }
            print("Keyboard \(cycle)/\(count): typed dad and restored the list.")
            fflush(stdout)
        }
        return
    }
    if !rotate {
        for trial in 1...(flickCount ?? 1) {
            if trial > 1 {
                try launch()
                before = try ready()
            }
            try checkFlick(before, trial: trial)
        }
        try launch()
        guard let tap = try ready()["Increment"] else { throw Failure("Missing button") }
        _ = try run(ios ? ["axe", "tap", "--udid", device, "-x", String(Int(tap.x)), "-y", String(Int(tap.y))]
                        : adb + ["shell", "input", "tap", String(Int(tap.x)), String(Int(tap.y))])
        Thread.sleep(forTimeInterval: 0.3)
        guard try labels()["Button taps: 1"] != nil else { throw Failure("Tap did not increment once") }
        print("\(args[0]): intentional tap produced one click.")
        if flickCount != nil { return }
    }
    guard let first = before["List row 1"], let second = before["List row 2"] else { throw Failure("Missing initial rows") }
    let rowHeight = second.y - first.y
    for _ in 0..<5 {
        let x = String(Int(first.x)), start = String(Int(first.y + 10 * rowHeight))
        let end = String(Int(first.y))
        _ = try run(ios ? ["axe", "swipe", "--udid", device, "--start-x", x, "--start-y", start,
                           "--end-x", x, "--end-y", end, "--duration", "0.28"]
                        : adb + ["shell", "input", "swipe", x, start, x, end, "280"])
    }
    Thread.sleep(forTimeInterval: 0.7)
    let bottom = try labels()
    guard let label = bottom["Bottom note"], let last = bottom["List row 48"] else {
        throw Failure("Did not reach the bottom field")
    }
    try tapPoint(CGPoint(x: label.x, y: label.y + (label.y - last.y)))
    Thread.sleep(forTimeInterval: 0.8)
    let open = try labels()
    guard let lifted = open["Bottom note"], let row = open["List row 48"],
          label.y - lifted.y > rowHeight * 2,
          let key = open.first(where: { $0.key.lowercased() == "w" && $0.value.y > lifted.y }) else {
        throw Failure("Bottom field did not rise above the software keyboard: \(open.keys.sorted())")
    }
    for _ in 0..<3 { try tapPoint(key.value) }
    Thread.sleep(forTimeInterval: 0.4)
    let typed = try labels()
    guard typed.contains(where: { $0.key.lowercased().contains("www") &&
                                  $0.value.y > lifted.y && $0.value.y < key.value.y }) else {
        throw Failure("Tapping the on-screen W key did not enter text in the raised field: \(typed.keys.sorted())")
    }
    if rotate {
        guard !landscape else { throw Failure("Start the rotation check in portrait") }
        let savedRotation = ios ? "" : String(decoding: try run(adb + ["shell", "wm", "user-rotation"], capture: true), as: UTF8.self)
        var rotated = false
        defer {
            if ios && rotated { try? rotatePhone(device, left: false) }
            if !ios {
                _ = try? run(adb + ["shell", "wm", "user-rotation"] + savedRotation.split(whereSeparator: { $0.isWhitespace }).map(String.init))
            }
        }
        var expected = "www"
        for turnLeft in [true, false] {
            if ios { try rotatePhone(device, left: turnLeft) }
            else { _ = try run(adb + ["shell", "wm", "user-rotation", "lock", turnLeft ? "1" : "0"]) }
            rotated = turnLeft
            // Let the OS finish reconfiguring its IME. A transition snapshot
            // can still contain the old keyboard even after its dismissal starts.
            Thread.sleep(forTimeInterval: 3)
            let deadline = Date().addingTimeInterval(10)
            var nextKey: (String, CGPoint)?
            var lastKey: (String, CGPoint)?
            repeat {
                let screen = try labels()
                if landscape == turnLeft, let caption = screen["Bottom note"],
                   let value = screen.first(where: { $0.key.lowercased().contains(expected) }),
                   value.value.y > caption.y,
                   screen.filter({ $0.key.count == 1 && $0.key.first!.isLetter && $0.value.y > value.value.y }).count >= 3 {
                    // Type from the newly laid-out keyboard to verify that
                    // the same field still has focus after each rotation.
                    // A rotation snapshot can contain perfectly legible keys
                    // in the old layout. Require the same key and row twice.
                    let rows = ["asdfghjkl", "qwertyuiop"]
                    let keys = screen.filter { $0.key.count == 1 && $0.value.y > value.value.y }
                    var candidate: (String, CGPoint)?
                    for letters in rows {
                        let points = letters.compactMap { letter in keys.first { $0.key.lowercased() == String(letter) } }
                        guard points.count >= 3 else { continue }
                        let ordered = zip(points, points.dropFirst()).allSatisfy { $0.value.x < $1.value.x }
                        let sameRow = points.allSatisfy { abs($0.value.y - points[0].value.y) < 14 }
                        if ordered && sameRow {
                            let key = points[points.count / 2]
                            candidate = (key.key.lowercased(), key.value)
                            break
                        }
                    }
                    if let candidate, let lastKey, candidate.0 == lastKey.0,
                       abs(candidate.1.x - lastKey.1.x) < 2 && abs(candidate.1.y - lastKey.1.y) < 2 {
                        nextKey = candidate
                        break
                    }
                    lastKey = candidate
                }
                Thread.sleep(forTimeInterval: 0.3)
            } while Date() < deadline
            guard let (letter, point) = nextKey else { throw Failure("Expected the typed field above the keyboard after rotating") }
            print("Rotated \(turnLeft ? "landscape" : "portrait"); typing \(letter) at \(point).")
            try tapPoint(point)
            expected += letter
            // A rotated input connection and its first rendered frame can
            // settle at different times. Wait for the edited field itself.
            let editDeadline = Date().addingTimeInterval(6)
            var visibleField: [String] = []
            repeat {
                let edited = try labels()
                if let caption = edited["Bottom note"] {
                    visibleField = edited.filter { $0.value.y > caption.y && $0.value.y < point.y }.map(\.key)
                    if visibleField.contains(where: { $0.lowercased().contains(expected) }) { break }
                }
                Thread.sleep(forTimeInterval: 0.12)
            } while Date() < editDeadline
            guard visibleField.contains(where: { $0.lowercased().contains(expected) }) else {
                throw Failure("Expected \(expected) in the bottom field after rotating; saw \(visibleField)")
            }
        }
        print("\(args[0]): field and typed text visible above the keyboard through landscape and back to portrait.")
    }
    if ios {
        try tapPoint(row) // A list row takes focus away from the field.
    } else {
        _ = try run(adb + ["shell", "input", "keyevent", "BACK"])
    }
    Thread.sleep(forTimeInterval: 0.8)
    let closed = try labels()
    let markers = bottom.filter { $0.key.range(of: #"^List row [0-9]+$"#, options: .regularExpression) != nil }
    guard markers.count >= 4,
          markers.allSatisfy({ abs((closed[$0.key]?.y ?? -10000) - $0.value.y) < rowHeight * 0.1 }),
          let restored = closed["Bottom note"], abs(restored.y - label.y) < rowHeight * 0.1 else {
        throw Failure("Closing the keyboard did not restore the original list geometry")
    }
    print("\(args[0]): bottom label y \(Int(label.y)) → \(Int(lifted.y)) → \(Int(restored.y)); typed with the on-screen keyboard and restored \(markers.count) row positions.")

}

do {
    try smoke()
} catch {
    let args = Array(CommandLine.arguments.dropFirst())
    FileHandle.standardError.write(Data("\(args.joined(separator: " ")): \(error)\n".utf8))
    if args.count >= 2, ["ios", "android"].contains(args[0]) {
        let file = FileManager.default.temporaryDirectory.appendingPathComponent("kui-\(args[0])-\(UUID().uuidString).png")
        do {
            if args[0] == "ios" {
                _ = try run(["xcrun", "simctl", "io", args[1], "screenshot", file.path])
            } else {
                let data = try run(["adb", "-s", args[1], "exec-out", "screencap", "-p"], capture: true)
                try data.write(to: file)
            }
            FileHandle.standardError.write(Data("Failure screenshot: \(file.path)\n".utf8))
        } catch {
            FileHandle.standardError.write(Data("Could not capture the screen: \(error)\n".utf8))
        }
    }
    exit(1)
}
