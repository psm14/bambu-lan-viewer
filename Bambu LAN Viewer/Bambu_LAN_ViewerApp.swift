//
//  Bambu_LAN_ViewerApp.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/13/26.
//

import SwiftUI

@main
struct Bambu_LAN_ViewerApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(\.appEnvironment, .live)
        }
    }
}
