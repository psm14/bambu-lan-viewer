//
//  ContentView.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/13/26.
//

import SwiftUI

struct ContentView: View {
    @Environment(\.appEnvironment) private var environment
    @State private var printerStore: PrinterStore?
    @State private var isLoading = true

    var body: some View {
        NavigationStack {
            Group {
                if isLoading {
                    ProgressView("Loading...")
                } else if let printerStore {
                    DashboardView(store: printerStore, onForget: forgetPrinter)
                } else {
                    AddPrinterView(onConnect: connect)
                }
            }
        }
        .task {
            await loadConfig()
        }
    }

    private func loadConfig() async {
        guard isLoading else { return }
        if let savedConfig = await environment.configStore.load() {
            printerStore = makeStore(for: savedConfig)
        }
        isLoading = false
    }

    private func connect(config: PrinterConfig, lanCode: String) async {
        await environment.configStore.save(config)
        await environment.secretStore.setSecret(lanCode, for: config.id)
        printerStore = makeStore(for: config)
    }

    private func forgetPrinter() {
        guard let printerStore else { return }
        Task {
            let config = printerStore.config
            printerStore.disconnect()
            await environment.configStore.clear()
            await environment.secretStore.clearSecret(for: config.id)
            await environment.trustStore.clearPinnedSPKIHash(for: config.id)
            await MainActor.run {
                self.printerStore = nil
            }
        }
    }

    @MainActor
    private func makeStore(for config: PrinterConfig) -> PrinterStore {
        PrinterStore(
            config: config,
            environment: environment,
            mqttTransport: CocoaMqttTransport()
        )
    }
}

#Preview {
    ContentView()
        .environment(\.appEnvironment, .preview)
}
