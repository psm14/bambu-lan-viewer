//
//  AddPrinterView.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/13/26.
//

import SwiftUI

struct AddPrinterView: View {
    let onConnect: (PrinterConfig, String) async -> Void

    @State private var name: String = ""
    @State private var ipAddress: String = ""
    @State private var lanCode: String = ""
    @State private var isConnecting = false
    @State private var errorMessage: String?

    var body: some View {
        Form {
            Section("Printer") {
                TextField("Name (optional)", text: $name)
                    .textInputAutocapitalization(.words)
                TextField("IP Address", text: $ipAddress)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .keyboardType(.numbersAndPunctuation)
            }

            Section("LAN Access Code") {
                SecureField("Enter LAN access code", text: $lanCode)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
            }

            if let errorMessage {
                Section {
                    Text(errorMessage)
                        .foregroundStyle(.red)
                }
            }

            Section {
                Button {
                    Task { await connect() }
                } label: {
                    HStack {
                        if isConnecting {
                            ProgressView()
                        }
                        Text(isConnecting ? "Connecting..." : "Connect")
                    }
                }
                .disabled(!canConnect || isConnecting)
            }
        }
        .navigationTitle("Add Printer")
    }

    private var canConnect: Bool {
        !ipAddress.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && !lanCode.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private func connect() async {
        errorMessage = nil
        let trimmedIP = ipAddress.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedCode = lanCode.trimmingCharacters(in: .whitespacesAndNewlines)

        guard !trimmedIP.isEmpty else {
            errorMessage = "IP address is required."
            return
        }

        guard !trimmedCode.isEmpty else {
            errorMessage = "LAN access code is required."
            return
        }

        isConnecting = true
        defer { isConnecting = false }

        let displayName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let resolvedName = displayName.isEmpty ? "Bambu Printer" : displayName
        let config = PrinterConfig(name: resolvedName, ip: trimmedIP)
        await onConnect(config, trimmedCode)
    }
}

#Preview {
    NavigationStack {
        AddPrinterView { _, _ in }
    }
}
