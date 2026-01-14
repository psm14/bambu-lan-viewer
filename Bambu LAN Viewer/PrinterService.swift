//
//  PrinterService.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/13/26.
//

import Foundation
import Security

actor PrinterService {
    private let mqtt: MqttTransport
    private let configStore: any PrinterConfigStoring
    private let secretStore: any SecretStore
    private let trustStore: any TrustStore

    private var config: PrinterConfig
    private var state: PrinterState

    private var onStateChanged: ((PrinterState) -> Void)?
    private var onConfigChanged: ((PrinterConfig) -> Void)?
    private var systemSequenceId: Int = 0
    private var printSequenceId: Int = 100
    private let userId: String = "1"
    private var desiredConnection = false
    private var reconnectTask: Task<Void, Never>?
    private var reconnectAttempt: Int = 0
    private var connectionTimeoutTask: Task<Void, Never>?
    private var connectionAttemptToken: UUID?

    init(
        config: PrinterConfig,
        mqttTransport: MqttTransport,
        configStore: any PrinterConfigStoring,
        secretStore: any SecretStore,
        trustStore: any TrustStore
    ) {
        self.config = config
        self.state = PrinterState.empty
        let mqtt = mqttTransport
        self.mqtt = mqtt
        self.configStore = configStore
        self.secretStore = secretStore
        self.trustStore = trustStore

        mqtt.onMessage = { [weak self] message in
            Task { await self?.handleMessage(message) }
        }
        mqtt.onConnectionStateChanged = { [weak self] newState in
            Task { await self?.handleConnectionState(newState) }
        }
        mqtt.onTrustEvaluation = { [weak self] trust, completion in
            Task {
                guard let self else {
                    completion(false)
                    return
                }
                let allowed = await self.evaluateTrust(trust)
                completion(allowed)
            }
        }
    }

    func setStateHandler(_ handler: @escaping (PrinterState) -> Void) async {
        onStateChanged = handler
        await notifyStateChanged()
    }

    func setConfigHandler(_ handler: @escaping (PrinterConfig) -> Void) async {
        onConfigChanged = handler
        await notifyConfigChanged()
    }

    func connect() async {
        desiredConnection = true
        reconnectTask?.cancel()
        reconnectTask = nil
        await startConnection()
    }

    private func startConnection() async {
        guard state.connection != .connecting, state.connection != .connected else { return }
        guard let password = await secretStore.secret(for: config.id) else {
            await updateConnection(.failed(message: "Missing LAN access code."))
            return
        }
        await updateConnection(.connecting)
        scheduleConnectionTimeout()
        mqtt.connect(host: config.ip, port: config.mqttPort, username: config.username, password: password, useTLS: true)
    }

    func disconnect() async {
        desiredConnection = false
        reconnectTask?.cancel()
        reconnectTask = nil
        cancelConnectionTimeout()
        mqtt.disconnect()
        await updateConnection(.disconnected)
    }

    func pause() async {
        await send(command: .pause)
    }

    func resume() async {
        await send(command: .resume)
    }

    func stopPrint() async {
        await send(command: .stop)
    }

    func setLight(_ isOn: Bool) async {
        await send(command: .setLight(isOn))
    }

    private func handleConnectionState(_ mqttState: MqttConnectionState) async {
        switch mqttState {
        case .connecting:
            await updateConnection(.connecting)
        case .connected:
            reconnectAttempt = 0
            reconnectTask?.cancel()
            reconnectTask = nil
            cancelConnectionTimeout()
            await updateConnection(.connected)
            subscribeToReportTopic()
        case .disconnected:
            await updateConnection(.disconnected)
            cancelConnectionTimeout()
            scheduleReconnect()
        case .failed(let message):
            await updateConnection(.failed(message: message))
            cancelConnectionTimeout()
            scheduleReconnect()
        }
    }

    private func subscribeToReportTopic() {
        if let serial = config.serial, !serial.isEmpty {
            mqtt.subscribe(topic: "device/\(serial)/report")
        } else {
            mqtt.subscribe(topic: "device/+/report")
        }
    }

    private func handleMessage(_ message: MqttMessage) async {
        logReport(message)
        if config.serial?.isEmpty ?? true {
            if let serial = SerialNumberDetector.detectSerial(topic: message.topic, payload: message.payload) {
                config.serial = serial
                await configStore.save(config)
                await notifyConfigChanged()
                mqtt.unsubscribe(topic: "device/+/report")
                mqtt.subscribe(topic: "device/\(serial)/report")
            }
        }

        if let report = try? JSONDecoder().decode(PrinterReport.self, from: message.payload) {
            let patch = report.toPatch()
            state.apply(patch: patch)

            if let rtspURL = report.print?.rtspURL, !rtspURL.isEmpty, config.rtspsPath != rtspURL {
                config.rtspsPath = rtspURL
                await configStore.save(config)
                await notifyConfigChanged()
            }
        }

        state.lastUpdate = Date()
        await notifyStateChanged()
    }

    private func evaluateTrust(_ trust: SecTrust) async -> Bool {
        guard let hash = TrustHasher.publicKeyHashBase64(from: trust) else {
            await updateConnection(.failed(message: "Unable to read printer certificate."))
            return false
        }

        if let pinned = await trustStore.pinnedSPKIHash(for: config.id) {
            if pinned == hash {
                return true
            }
            await updateConnection(.certMismatch)
            return false
        }

        await trustStore.setPinnedSPKIHash(hash, for: config.id)
        return true
    }

    private func updateConnection(_ connection: ConnectionState) async {
        state.connection = connection
        await notifyStateChanged()
    }

    private func notifyStateChanged() async {
        await MainActor.run { [state, onStateChanged] in
            onStateChanged?(state)
        }
    }

    private func notifyConfigChanged() async {
        await MainActor.run { [config, onConfigChanged] in
            onConfigChanged?(config)
        }
    }

    private func scheduleReconnect() {
        guard desiredConnection else { return }
        guard reconnectTask == nil else { return }
        reconnectAttempt += 1
        let delay = min(10.0, pow(2.0, Double(reconnectAttempt)) * 0.5)
        reconnectTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
            await self?.performReconnect()
        }
    }

    private func performReconnect() async {
        reconnectTask = nil
        guard desiredConnection else { return }
        await startConnection()
    }

    private func scheduleConnectionTimeout() {
        cancelConnectionTimeout()
        let token = UUID()
        connectionAttemptToken = token
        connectionTimeoutTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 12_000_000_000)
            await self?.handleConnectionTimeout(token)
        }
    }

    private func cancelConnectionTimeout() {
        connectionTimeoutTask?.cancel()
        connectionTimeoutTask = nil
        connectionAttemptToken = nil
    }

    private func handleConnectionTimeout(_ token: UUID) async {
        guard desiredConnection else { return }
        guard connectionAttemptToken == token else { return }
        guard state.connection == .connecting else { return }
        mqtt.disconnect()
        await updateConnection(.failed(message: "Connection timed out."))
        scheduleReconnect()
    }

    private func send(command: PrinterCommand) async {
        guard state.connection == .connected else {
            logCommand(command, message: "Not connected; skipping command.")
            return
        }
        guard let serial = config.serial, !serial.isEmpty else {
            logCommand(command, message: "Missing serial; skipping command.")
            return
        }
        let payloads = commandPayloads(for: command)
        guard !payloads.isEmpty else {
            logCommand(command, message: "Unable to encode payload.")
            return
        }

        let topic = "device/\(serial)/request"
        for payload in payloads {
            mqtt.publish(topic: topic, payload: payload, qos: 1, retain: false)
            logCommand(command, message: "Published to \(topic) payload=\(payloadSummary(payload))")
        }
    }

    private func commandPayloads(for command: PrinterCommand) -> [Data] {
        let objects: [[String: Any]]
        switch command {
        case .pause:
            objects = [[
                "user_id": userId,
                "print": [
                    "sequence_id": nextPrintSequence(),
                    "command": "pause"
                ]
            ]]
        case .resume:
            objects = [[
                "user_id": userId,
                "print": [
                    "sequence_id": nextPrintSequence(),
                    "command": "resume"
                ]
            ]]
        case .stop:
            objects = [[
                "user_id": userId,
                "print": [
                    "sequence_id": nextPrintSequence(),
                    "command": "stop"
                ]
            ]]
        case .setLight(let isOn):
            let modeString = isOn ? "on" : "off"
            objects = [[
                "system": [
                    "sequence_id": nextSystemSequence(),
                    "command": "ledctrl",
                    "led_node": "chamber_light",
                    "led_mode": modeString,
                    "led_on_time": 500,
                    "led_off_time": 500,
                    "loop_times": 0,
                    "interval_time": 0
                ],
                "user_id": userId
            ]]
        }

        return objects.compactMap { object in
            try? JSONSerialization.data(withJSONObject: object, options: [])
        }
    }

    private func nextSystemSequence() -> String {
        systemSequenceId += 1
        return String(systemSequenceId)
    }

    private func nextPrintSequence() -> String {
        printSequenceId += 1
        return String(printSequenceId)
    }
}

private func payloadSummary(_ payload: Data) -> String {
#if DEBUG
    if let text = String(data: payload, encoding: .utf8) {
        return text
    }
#endif
    return "<\(payload.count) bytes>"
}

private func logCommand(_ command: PrinterCommand, message: String) {
#if DEBUG
    guard UserDefaults.standard.bool(forKey: "debugMqttLogging") else { return }
    print("[MQTT] command \(command) - \(message)")
#endif
}

private func logReport(_ message: MqttMessage) {
#if DEBUG
    guard UserDefaults.standard.bool(forKey: "debugMqttLogging") else { return }
    if let text = String(data: message.payload, encoding: .utf8) {
        print("[MQTT] \(message.topic): \(text)")
    } else {
        print("[MQTT] \(message.topic): <\(message.payload.count) bytes>")
    }
#endif
}
