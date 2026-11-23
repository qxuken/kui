import XCTest

final class MobileChecks: XCTestCase {
    func testEditorSurvivesRotationAndRelaunch() throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "dev.kui.mobile.spike")
        app.launchEnvironment["KUI_TEST_DRAFTS"] = UUID().uuidString
        XCUIDevice.shared.orientation = .portrait
        defer { XCUIDevice.shared.orientation = .portrait }
        app.launch()
        let increment = app.buttons["Increment"]
        XCTAssertTrue(increment.waitForExistence(timeout: 10))
        increment.tap()
        XCTAssertTrue(app.staticTexts["Button taps: 1"].waitForExistence(timeout: 5))

        let field = app.descendants(matching: .any).matching(NSPredicate(
            format: "label == %@ AND elementType != %d", "Bottom note", XCUIElement.ElementType.staticText.rawValue
        )).firstMatch
        func scrollToField() {
            for _ in 0..<12 {
                if field.exists && field.isHittable && field.frame.height > 0 { break }
                app.swipeUp()
            }
            XCTAssertTrue(field.isHittable)
        }
        scrollToField()
        field.tap()
        let keyboard = app.keyboards.firstMatch
        func softwareKey(_ letter: String) -> XCUIElement {
            keyboard.keys.matching(NSPredicate(format: "label ==[c] %@", letter)).firstMatch
        }
        func waitUntil(_ condition: @escaping () -> Bool) -> Bool {
            let ready = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in condition() }, object: nil)
            return XCTWaiter.wait(for: [ready], timeout: 12) == .completed
        }
        // Fresh simulators can show the QuickPath introduction in place of the keys.
        let introduction = app.buttons["Continue"].firstMatch
        let w = softwareKey("w")
        XCTAssertTrue(waitUntil { w.isHittable || introduction.isHittable }, "Waiting for the software keyboard")
        if introduction.isHittable {
            introduction.tap()
        }

        func type(_ key: XCUIElement, expecting text: String) {
            XCTAssertTrue(waitUntil { key.isHittable }, "Waiting for a software key to enter \(text)")
            key.tap()
            XCTAssertTrue(waitUntil { (field.value as? String)?.lowercased() == text }, "Expected \(text) in Bottom note")
        }
        for text in ["w", "ww", "www"] {
            type(w, expecting: text)
        }
        for (orientation, suffix, text) in [(UIDeviceOrientation.landscapeLeft, "x", "wwwx"), (.portrait, "y", "wwwxy")] {
            XCUIDevice.shared.orientation = orientation
            let key = softwareKey(suffix)
            let ready = waitUntil {
                let viewport = app.frame
                guard (viewport.width > viewport.height) == (orientation == .landscapeLeft),
                      key.isHittable, field.isHittable else { return false }
                let editor = field.frame
                return !editor.isEmpty && viewport.insetBy(dx: -1, dy: -1).contains(editor) &&
                    !editor.intersects(keyboard.frame.insetBy(dx: 1, dy: 1))
            }
            if !ready {
                let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
                attachment.name = "Bottom note after rotation"
                add(attachment)
            }
            XCTAssertTrue(ready, "Expected Bottom note visible in \(orientation). App \(app.frame), field \(field.frame), keyboard \(keyboard.frame)")
            type(key, expecting: text)
        }
        XCTAssertEqual((field.value as? String)?.lowercased(), "wwwxy")
        app.terminate()
        app.launch()
        scrollToField()
        XCTAssertEqual((field.value as? String)?.lowercased(), "wwwxy")
    }
}
