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
        XCTAssertTrue(keyboard.waitForExistence(timeout: 5))
        func typeOnKeyboard(_ text: String) {
            for letter in text {
                let key = keyboard.keys.matching(NSPredicate(format: "label ==[c] %@", String(letter))).firstMatch
                if key.exists && key.isHittable {
                    key.tap()
                } else {
                    app.typeKey(String(letter), modifierFlags: [])
                }
            }
        }
        typeOnKeyboard("www")
        for (orientation, suffix) in [(UIDeviceOrientation.landscapeLeft, "x"), (.portrait, "y")] {
            XCUIDevice.shared.orientation = orientation
            let ready = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
                let viewport = app.frame
                guard (viewport.width > viewport.height) == (orientation == .landscapeLeft),
                      field.exists, field.isHittable else { return false }
                let softwareKey = keyboard.keys.matching(NSPredicate(format: "label ==[c] %@", suffix)).firstMatch
                let bottom = softwareKey.exists && softwareKey.isHittable ? keyboard.frame.minY : viewport.maxY
                return field.frame.height > 0 && field.frame.minY >= viewport.minY &&
                    field.frame.maxY <= bottom + 1
            }, object: nil)
            XCTAssertEqual(XCTWaiter.wait(for: [ready], timeout: 8), .completed)
            typeOnKeyboard(suffix)
        }
        XCTAssertEqual((field.value as? String)?.lowercased(), "wwwxy")
        app.terminate()
        app.launch()
        scrollToField()
        XCTAssertEqual((field.value as? String)?.lowercased(), "wwwxy")
    }
}
