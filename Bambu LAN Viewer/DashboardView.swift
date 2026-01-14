//
//  DashboardView.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/13/26.
//

import SwiftUI

struct DashboardView: View {
    @Environment(\.appEnvironment) private var environment
    @Environment(\.scenePhase) private var scenePhase
    @ObservedObject var store: PrinterStore
    let onForget: () -> Void
    @State private var lanCode: String?
    @State private var videoState: VideoState = .stopped
    @State private var showStopConfirmation = false
    @State private var pendingJobAction: PendingJobAction?
    @State private var pendingJobToken: UUID?
    @State private var lightOverride: Bool?
    @State private var pendingLightToken: UUID?
    @AppStorage("debugMqttLogging") private var debugMqttLogging = false
    @AppStorage("debugVideoLogging") private var debugVideoLogging = false

    var body: some View {
        Form {
            Section {
                videoSection
            }

            Section("Status") {
                LabeledContent("Job", value: jobStatusText)

                if let progress = store.state.progress01 {
                    ProgressView(value: progress) {
                        Text("Progress")
                    } currentValueLabel: {
                        Text(progressText)
                    }
                } else {
                    LabeledContent("Progress", value: "Unknown")
                }

                LabeledContent("Nozzle", value: formatTemp(current: store.state.nozzleC, target: store.state.nozzleTargetC))
                LabeledContent("Bed", value: formatTemp(current: store.state.bedC, target: store.state.bedTargetC))
                LabeledContent("Chamber", value: formatTemp(current: store.state.chamberC, target: store.state.chamberTargetC))
                LabeledContent("Light", value: lightStatusText)
            }

            Section("Controls") {
                HStack {
                    Button(pauseResumeTitle) {
                        handlePauseResume()
                    }
                    .disabled(!canPauseResume)
                    .buttonStyle(.borderless)

                    Spacer()

                    Button(stopButtonTitle, role: .destructive) {
                        showStopConfirmation = true
                    }
                    .disabled(!canStop)
                    .buttonStyle(.borderless)
                }

                Toggle("Light", isOn: lightToggleBinding)
                    .disabled(!canToggleLight)
            }

            Section("Connection") {
                HStack {
                    Text("Status")
                    Spacer()
                    Text(connectionStatus)
                        .foregroundStyle(connectionColor)
                }

                if let lastUpdate = store.state.lastUpdate {
                    HStack {
                        Text("Last Update")
                        Spacer()
                        Text(lastUpdate, style: .relative)
                    }
                } else {
                    LabeledContent("Last Update", value: "No data yet")
                }

                if case .failed(let message) = store.state.connection {
                    Text(message)
                        .foregroundStyle(.red)
                }

                if store.state.connection == .certMismatch {
                    Text("Printer identity changed. Clear trust to reconnect.")
                        .foregroundStyle(.red)
                }

                Button(isConnected ? "Disconnect" : "Connect") {
                    if isConnected {
                        store.disconnect()
                    } else {
                        store.connect()
                    }
                }

                #if DEBUG
                Toggle("Video Debug Logs", isOn: $debugVideoLogging)
                Toggle("MQTT Debug Logs", isOn: $debugMqttLogging)
                #endif
            }

            Section("Printer") {
                LabeledContent("Name", value: store.config.name)
                LabeledContent("IP", value: store.config.ip)
                LabeledContent("Serial", value: store.config.serial ?? "Not detected")
                LabeledContent("MQTT Port", value: String(store.config.mqttPort))
                LabeledContent("RTSPS Path", value: store.config.rtspsPath)
            }

            Section {
                Button("Forget Printer", role: .destructive, action: onForget)
            }
        }
        .navigationTitle(store.config.name)
        .alert("Stop Print?", isPresented: $showStopConfirmation) {
            Button("Stop Print", role: .destructive) {
                setPendingJobAction(.stop)
                store.stopPrint()
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This will end the current print job.")
        }
        .onChange(of: store.state.job) { _, newValue in
            handleJobUpdate(newValue)
        }
        .onChange(of: store.state.lightOn) { _, newValue in
            guard newValue != nil else { return }
            lightOverride = nil
            pendingLightToken = nil
        }
        .task {
            store.connect()
            lanCode = await environment.secretStore.secret(for: store.config.id)
        }
    }

    private var isConnected: Bool {
        store.state.connection == .connected || store.state.connection == .connecting
    }

    private var connectionStatus: String {
        switch store.state.connection {
        case .disconnected:
            return "Disconnected"
        case .connecting:
            return "Connecting"
        case .connected:
            return "Connected"
        case .failed(let message):
            return "Failed: \(message)"
        case .certMismatch:
            return "Cert mismatch"
        }
    }

    private var connectionColor: Color {
        switch store.state.connection {
        case .connected:
            return .green
        case .connecting:
            return .orange
        case .failed, .certMismatch:
            return .red
        case .disconnected:
            return .secondary
        }
    }

    private var jobStatusText: String {
        switch store.state.job {
        case .idle:
            return "Idle"
        case .printing:
            return "Printing"
        case .paused:
            return "Paused"
        case .finished:
            return "Finished"
        case .error(let message):
            return "Error: \(message)"
        }
    }

    private var progressText: String {
        guard let progress = store.state.progress01 else { return "Unknown" }
        return "\(Int(progress * 100.0))%"
    }

    private var lightStatusText: String {
        guard let lightOn = store.state.lightOn else { return "Unknown" }
        return lightOn ? "On" : "Off"
    }

    private func formatTemp(current: Double?, target: Double?) -> String {
        guard let current else {
            if let target {
                return String(format: "— / %.1f C", target)
            }
            return "Unknown"
        }

        if let target {
            return String(format: "%.1f C / %.1f C", current, target)
        }
        return String(format: "%.1f C", current)
    }

    private var pauseResumeTitle: String {
        if let pendingJobAction {
            switch pendingJobAction {
            case .pause:
                return "Pausing..."
            case .resume:
                return "Resuming..."
            case .stop:
                return "Pause"
            }
        }
        switch store.state.job {
        case .paused:
            return "Resume"
        default:
            return "Pause"
        }
    }

    private var stopButtonTitle: String {
        if pendingJobAction == .stop {
            return "Stopping..."
        }
        return "Stop"
    }

    private var canPauseResume: Bool {
        isConnected && pendingJobAction == nil && (store.state.job == .printing || store.state.job == .paused)
    }

    private var canStop: Bool {
        isConnected && pendingJobAction == nil && (store.state.job == .printing || store.state.job == .paused)
    }

    private var canToggleLight: Bool {
        isConnected && pendingLightToken == nil
    }

    private var lightToggleBinding: Binding<Bool> {
        Binding(
            get: { lightOverride ?? store.state.lightOn ?? false },
            set: { newValue in
                lightOverride = newValue
                scheduleLightTimeout()
                store.setLight(newValue)
            }
        )
    }

    private func handlePauseResume() {
        switch store.state.job {
        case .paused:
            setPendingJobAction(.resume)
            store.resume()
        case .printing:
            setPendingJobAction(.pause)
            store.pause()
        default:
            break
        }
    }

    private func handleJobUpdate(_ job: JobState) {
        guard let pending = pendingJobAction else { return }
        switch pending {
        case .pause:
            if job == .paused {
                clearPendingJob()
            }
        case .resume:
            if job == .printing {
                clearPendingJob()
            }
        case .stop:
            if job == .idle || job == .finished {
                clearPendingJob()
            } else if case .error = job {
                clearPendingJob()
            }
        }
    }

    private func setPendingJobAction(_ action: PendingJobAction) {
        pendingJobAction = action
        let token = UUID()
        pendingJobToken = token
        Task { [token] in
            try? await Task.sleep(nanoseconds: 5_000_000_000)
            await MainActor.run {
                if pendingJobToken == token {
                    clearPendingJob()
                }
            }
        }
    }

    private func clearPendingJob() {
        pendingJobAction = nil
        pendingJobToken = nil
    }

    private func scheduleLightTimeout() {
        let token = UUID()
        pendingLightToken = token
        Task { [token] in
            try? await Task.sleep(nanoseconds: 3_000_000_000)
            await MainActor.run {
                if pendingLightToken == token {
                    pendingLightToken = nil
                    lightOverride = nil
                }
            }
        }
    }

    @ViewBuilder
    private var videoSection: some View {
        let isActive = store.state.connection == .connected && lanCode != nil && scenePhase == .active
        VStack(alignment: .leading, spacing: 8) {
            if let url = videoURL, let lanCode {
                ZStack {
                    RTSPPlayerView(
                        url: url,
                        username: store.config.username,
                        password: lanCode,
                        isActive: isActive,
                        trustStore: environment.trustStore,
                        printerID: store.config.id,
                        onStateChanged: { state in
                            videoState = state
                        }
                    )
                    .frame(height: 220)
                    .overlay(
                        RoundedRectangle(cornerRadius: 12)
                            .stroke(Color.white.opacity(0.08), lineWidth: 1)
                    )

                    if !isActive {
                        overlayLabel(text: "Video paused")
                    } else if case .buffering = videoState {
                        overlayLabel(text: "Buffering...")
                    } else if case .failed(let message) = videoState {
                        overlayLabel(text: message)
                    }
                }
            } else {
                ZStack {
                    Rectangle()
                        .fill(Color.black.opacity(0.9))
                        .frame(height: 220)
                        .overlay(
                            RoundedRectangle(cornerRadius: 12)
                                .stroke(Color.white.opacity(0.08), lineWidth: 1)
                        )
                    Text("Video unavailable")
                        .foregroundStyle(.white)
                }
            }

            #if DEBUG
            if debugVideoLogging, let url = videoURL {
                Text(url.absoluteString)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }
            #endif
        }
    }

    private var videoURL: URL? {
        let path = store.config.rtspsPath.trimmingCharacters(in: .whitespacesAndNewlines)
        if path.lowercased().hasPrefix("rtsp://") || path.lowercased().hasPrefix("rtsps://") {
            return URL(string: path)
        }
        return URL(string: "rtsps://\(store.config.ip)\(path)")
    }

    private func overlayLabel(text: String) -> some View {
        Text(text)
            .font(.caption)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(Color.black.opacity(0.6))
            .foregroundStyle(.white)
            .clipShape(Capsule())
    }
}

private enum PendingJobAction {
    case pause
    case resume
    case stop
}

#Preview {
    NavigationStack {
        DashboardView(
            store: PrinterStore(
                config: PrinterConfig(name: "Bambu X1C", ip: "192.168.1.42"),
                environment: .preview,
                mqttTransport: CocoaMqttTransport()
            ),
            onForget: {}
        )
    }
}
