# Repository Guidelines

## Project Structure & Module Organization
- `Bambu LAN Viewer/`: SwiftUI app source (entry point in `Bambu_LAN_ViewerApp.swift`, UI in `ContentView.swift`).
- `Bambu LAN Viewer/Assets.xcassets/`: App icons and asset catalogs.
- `Bambu LAN Viewer.xcodeproj/`: Xcode project configuration and schemes.
- `MobileVLCKit.xcframework/`: Embedded VLC framework used for RTSP(S) video playback.
- `Docs/`: Design and protocol references (e.g., `DesignDoc.md`, `MQTTProtocol.md`).

## Build, Test, and Development Commands
- `open "Bambu LAN Viewer.xcodeproj"`: Open the project in Xcode.
- `xcodebuild -project "Bambu LAN Viewer.xcodeproj" -scheme "Bambu LAN Viewer" build`: CLI build for CI or quick checks.
- `xcodebuild -project "Bambu LAN Viewer.xcodeproj" -scheme "Bambu LAN Viewer" test`: Run tests (add a test target first; none exist yet).
- In Xcode, use Product > Run to launch on a simulator or device.

## Coding Style & Naming Conventions
- Language: Swift + SwiftUI; keep code compatible with current Xcode defaults.
- Indentation: 4 spaces, no tabs; let Xcode reformat on save when possible.
- Naming: Types in UpperCamelCase, properties/functions in lowerCamelCase.
- File naming: match the primary type (for example, `ContentView.swift` holds `ContentView`).

## Testing Guidelines
- There is no test target in the repository yet.
- When adding tests, use XCTest and create a dedicated test target in Xcode.
- Naming: `FeatureTests.swift` with test methods named `test_<behavior>`.

## Commit & Pull Request Guidelines
- Commit messages follow a short, imperative style (examples from history: "Add docs from chat", "Add VLC library").
- PRs should include: a one-paragraph summary, test steps or commands run, and screenshots for UI changes.
- Update `Docs/` when a change affects protocols or architecture decisions.

## Architecture & Reference Docs
- The intended architecture and Phase 1 scope are captured in `Docs/DesignDoc.md`.
- MQTT and device notes live in `Docs/MQTTProtocol.md` and related references; align implementation with these docs when adding networking.
- Track implementation progress in `Docs/Phase1Checklist.md`.
