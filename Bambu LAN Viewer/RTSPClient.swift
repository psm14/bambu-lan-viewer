//
//  RTSPClient.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/14/26.
//

import CryptoKit
import Foundation
import Network

struct RtspCredentials {
    let username: String
    let password: String
}

enum RTSPClientError: LocalizedError {
    case notConnected
    case connectionFailed
    case invalidResponse
    case requestFailed(status: Int, message: String)

    var errorDescription: String? {
        switch self {
        case .notConnected:
            return "RTSP connection not ready"
        case .connectionFailed:
            return "RTSP connection failed"
        case .invalidResponse:
            return "Invalid RTSP response"
        case .requestFailed(let status, let message):
            return "RTSP error \(status): \(message)"
        }
    }
}

struct RTSPSessionInfo {
    let sdpInfo: SDPInfo
    let rtpChannel: Int
    let rtcpChannel: Int
}

final class RTSPClient: @unchecked Sendable {
    var onInterleavedPacket: ((Int, Data) -> Void)?
    var onError: ((Error) -> Void)?

    private let queue: DispatchQueue
    private let credentials: RtspCredentials?
    private var authenticator: RtspAuthenticator?
    private var connection: NWConnection?
    private var parser = RTSPStreamParser()
    private var pendingResponses: [Int: CheckedContinuation<RTSPResponse, Error>] = [:]
    private var cseq: Int = 1
    private var sessionId: String?
    private var sessionTimeout: TimeInterval?
    private var keepaliveTimer: DispatchSourceTimer?
    private var keepaliveUri: String?
    private var connectContinuation: CheckedContinuation<Void, Error>?

    init(queue: DispatchQueue, credentials: RtspCredentials?) {
        self.queue = queue
        self.credentials = credentials
        if let credentials {
            self.authenticator = RtspAuthenticator(credentials: credentials)
        }
    }

    func start(url: URL) async throws -> RTSPSessionInfo {
        try await connectIfNeeded(to: url)

        let describeResponse = try await sendRequestWithRetry(
            method: "DESCRIBE",
            uri: url.absoluteString,
            headers: ["Accept": "application/sdp"]
        )
        guard describeResponse.statusCode == 200 else {
            throw RTSPClientError.requestFailed(status: describeResponse.statusCode, message: describeResponse.reasonPhrase)
        }
        guard let sdpInfo = SDPParser.parse(describeResponse.body) else {
            throw RTSPClientError.invalidResponse
        }

        let setupUri = sdpInfo.resolvedVideoControlURL(baseURL: url)
        let setupResponse = try await sendRequestWithRetry(
            method: "SETUP",
            uri: setupUri,
            headers: ["Transport": "RTP/AVP/TCP;unicast;interleaved=0-1"]
        )
        guard setupResponse.statusCode == 200 else {
            throw RTSPClientError.requestFailed(status: setupResponse.statusCode, message: setupResponse.reasonPhrase)
        }

        let channels = parseInterleavedChannels(from: setupResponse) ?? (0, 1)
        let playUri = sdpInfo.resolvedPlayURL(baseURL: url)
        let playResponse = try await sendRequestWithRetry(method: "PLAY", uri: playUri, headers: ["Range": "npt=0-"])
        guard playResponse.statusCode == 200 else {
            throw RTSPClientError.requestFailed(status: playResponse.statusCode, message: playResponse.reasonPhrase)
        }

        keepaliveUri = playUri
        startKeepalive()

        return RTSPSessionInfo(sdpInfo: sdpInfo, rtpChannel: channels.0, rtcpChannel: channels.1)
    }

    func stop() {
        queue.async { [weak self] in
            guard let self else { return }
            self.keepaliveTimer?.cancel()
            self.keepaliveTimer = nil
            if let uri = self.keepaliveUri {
                Task {
                    _ = try? await self.sendRequestWithRetry(method: "TEARDOWN", uri: uri, headers: [:])
                }
            }
            self.connection?.cancel()
            self.connection = nil
        }
    }

    private func connectIfNeeded(to url: URL) async throws {
        if connection != nil {
            return
        }

        let host = url.host ?? ""
        let portValue = UInt16(url.port ?? 322)
        let port = NWEndpoint.Port(rawValue: portValue) ?? NWEndpoint.Port(rawValue: 322)!

        let parameters: NWParameters
        if url.scheme?.lowercased() == "rtsps" {
            let tcp = NWProtocolTCP.Options()
            let tls = NWProtocolTLS.Options()
            sec_protocol_options_set_verify_block(tls.securityProtocolOptions, { _, _, completion in
                completion(true)
            }, queue)
            parameters = NWParameters(tls: tls, tcp: tcp)
        } else {
            let tcp = NWProtocolTCP.Options()
            parameters = NWParameters(tls: nil, tcp: tcp)
        }

        let connection = NWConnection(host: NWEndpoint.Host(host), port: port, using: parameters)
        self.connection = connection

        connection.stateUpdateHandler = { [weak self] state in
            self?.handleState(state)
        }

        try await withCheckedThrowingContinuation { continuation in
            connectContinuation = continuation
            connection.start(queue: queue)
            receiveLoop()
        }
    }

    private func handleState(_ state: NWConnection.State) {
        switch state {
        case .ready:
            connectContinuation?.resume(returning: ())
            connectContinuation = nil
        case .failed(let error):
            connectContinuation?.resume(throwing: error)
            connectContinuation = nil
            failAllRequests(error)
            onError?(error)
        case .cancelled:
            let error = RTSPClientError.connectionFailed
            connectContinuation?.resume(throwing: error)
            connectContinuation = nil
            failAllRequests(error)
            onError?(error)
        default:
            break
        }
    }

    private func receiveLoop() {
        connection?.receive(minimumIncompleteLength: 1, maximumLength: 16 * 1024) { [weak self] data, _, isComplete, error in
            guard let self else { return }
            if let data, !data.isEmpty {
                let events = self.parser.append(data)
                for event in events {
                    self.handle(event)
                }
            }

            if let error {
                self.failAllRequests(error)
                self.onError?(error)
                return
            }

            if isComplete {
                let error = RTSPClientError.connectionFailed
                self.failAllRequests(error)
                self.onError?(error)
                return
            }

            self.receiveLoop()
        }
    }

    private func handle(_ event: RTSPStreamEvent) {
        switch event {
        case .interleaved(let channel, let payload):
            onInterleavedPacket?(channel, payload)
        case .response(let response):
            if let sessionInfo = parseSessionInfo(from: response) {
                sessionId = sessionInfo.id
                sessionTimeout = sessionInfo.timeout
            }
            if response.statusCode == 401 {
                _ = authenticator?.updateChallenge(from: response)
            }
            if let cseq = response.cseq, let continuation = pendingResponses.removeValue(forKey: cseq) {
                continuation.resume(returning: response)
            }
        }
    }

    private func failAllRequests(_ error: Error) {
        let pending = pendingResponses
        pendingResponses.removeAll()
        for continuation in pending.values {
            continuation.resume(throwing: error)
        }
    }

    private func sendRequestWithRetry(method: String, uri: String, headers: [String: String]) async throws -> RTSPResponse {
        var attempts = 0
        var lastResponse: RTSPResponse?

        while attempts < 2 {
            let response = try await sendRequest(method: method, uri: uri, headers: headers)
            lastResponse = response
            if response.statusCode == 401, authenticator?.updateChallenge(from: response) == true {
                attempts += 1
                continue
            }
            return response
        }

        if let lastResponse {
            return lastResponse
        }
        throw RTSPClientError.invalidResponse
    }

    private func sendRequest(method: String, uri: String, headers: [String: String]) async throws -> RTSPResponse {
        try await withCheckedThrowingContinuation { continuation in
            queue.async { [weak self] in
                guard let self else {
                    continuation.resume(throwing: RTSPClientError.notConnected)
                    return
                }
                guard let connection = self.connection else {
                    continuation.resume(throwing: RTSPClientError.notConnected)
                    return
                }

                let cseq = self.cseq
                self.cseq += 1

                var requestHeaders = headers
                if let sessionId, requestHeaders["Session"] == nil, method != "DESCRIBE" {
                    requestHeaders["Session"] = sessionId
                }

                let authHeader = self.authenticator?.authorizationHeader(method: method, uri: uri)
                let requestData = self.buildRequest(method: method, uri: uri, cseq: cseq, headers: requestHeaders, authorizationHeader: authHeader)

                self.pendingResponses[cseq] = continuation
                connection.send(content: requestData, completion: .contentProcessed { error in
                    if let error {
                        self.pendingResponses.removeValue(forKey: cseq)
                        continuation.resume(throwing: error)
                    }
                })
            }
        }
    }

    private func buildRequest(method: String, uri: String, cseq: Int, headers: [String: String], authorizationHeader: String?) -> Data {
        var lines: [String] = [
            "\(method) \(uri) RTSP/1.0",
            "CSeq: \(cseq)",
            "User-Agent: BambuLANViewer/1.0"
        ]
        for (key, value) in headers {
            lines.append("\(key): \(value)")
        }
        if let authorizationHeader {
            lines.append(authorizationHeader)
        }
        let request = lines.joined(separator: "\r\n") + "\r\n\r\n"
        return Data(request.utf8)
    }

    private func startKeepalive() {
        keepaliveTimer?.cancel()
        keepaliveTimer = nil

        guard let keepaliveUri else { return }

        let interval: TimeInterval
        if let timeout = sessionTimeout, timeout > 0 {
            interval = max(1.0, min(timeout * 0.5, timeout - 1.0))
        } else {
            interval = 5.0
        }

        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + interval, repeating: interval)
        timer.setEventHandler { [weak self] in
            guard let self else { return }
            Task {
                do {
                    _ = try await self.sendRequestWithRetry(method: "OPTIONS", uri: keepaliveUri, headers: [:])
                } catch {
                    self.onError?(error)
                }
            }
        }
        keepaliveTimer = timer
        timer.resume()
    }

    private func parseInterleavedChannels(from response: RTSPResponse) -> (Int, Int)? {
        guard let transport = response.header("transport") else { return nil }
        let parts = transport.split(separator: ";")
        for part in parts {
            let trimmed = part.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.lowercased().hasPrefix("interleaved=") {
                let value = trimmed.dropFirst("interleaved=".count)
                let numbers = value.split(separator: "-")
                if numbers.count == 2, let rtp = Int(numbers[0]), let rtcp = Int(numbers[1]) {
                    return (rtp, rtcp)
                }
            }
        }
        return nil
    }

    private func parseSessionInfo(from response: RTSPResponse) -> (id: String, timeout: TimeInterval?)? {
        guard let sessionHeader = response.header("session") else { return nil }
        let parts = sessionHeader.split(separator: ";", omittingEmptySubsequences: true)
        guard let sessionId = parts.first?.trimmingCharacters(in: .whitespacesAndNewlines), !sessionId.isEmpty else { return nil }
        var timeout: TimeInterval?
        for part in parts.dropFirst() {
            let trimmed = part.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.lowercased().hasPrefix("timeout=") {
                let raw = trimmed.dropFirst("timeout=".count)
                timeout = TimeInterval(raw)
            }
        }
        return (String(sessionId), timeout)
    }
}

private final class RtspAuthenticator {
    private let credentials: RtspCredentials
    private var digestChallenge: DigestChallenge?
    private var nonceCount: Int = 0
    private var cnonce: String = UUID().uuidString.replacingOccurrences(of: "-", with: "")

    init(credentials: RtspCredentials) {
        self.credentials = credentials
    }

    func updateChallenge(from response: RTSPResponse) -> Bool {
        guard response.statusCode == 401 else { return false }
        guard let headerValue = response.header("www-authenticate") else { return false }
        guard let challenge = DigestChallenge.from(headerValue: headerValue) else { return false }
        digestChallenge = challenge
        nonceCount = 0
        cnonce = UUID().uuidString.replacingOccurrences(of: "-", with: "")
        return true
    }

    func authorizationHeader(method: String, uri: String) -> String? {
        if let challenge = digestChallenge {
            return digestAuthorization(method: method, uri: uri, challenge: challenge)
        }
        return basicAuthorization()
    }

    private func basicAuthorization() -> String? {
        let basicValue = "\(credentials.username):\(credentials.password)"
        guard let data = basicValue.data(using: .utf8) else { return nil }
        return "Authorization: Basic \(data.base64EncodedString())"
    }

    private func digestAuthorization(method: String, uri: String, challenge: DigestChallenge) -> String? {
        guard challenge.algorithm == nil || challenge.algorithm?.lowercased() == "md5" else {
            return nil
        }
        let realm = challenge.realm
        let nonce = challenge.nonce
        let ha1 = md5Hex("\(credentials.username):\(realm):\(credentials.password)")
        let ha2 = md5Hex("\(method):\(uri)")

        if let qop = challenge.qop {
            nonceCount += 1
            let ncString = String(format: "%08x", nonceCount)
            let response = md5Hex("\(ha1):\(nonce):\(ncString):\(cnonce):\(qop):\(ha2)")
            var header = "Authorization: Digest username=\"\(credentials.username)\", realm=\"\(realm)\", nonce=\"\(nonce)\", uri=\"\(uri)\", response=\"\(response)\", qop=\(qop), nc=\(ncString), cnonce=\"\(cnonce)\""
            if let opaque = challenge.opaque {
                header += ", opaque=\"\(opaque)\""
            }
            if let algorithm = challenge.algorithm {
                header += ", algorithm=\(algorithm)"
            }
            return header
        }

        let response = md5Hex("\(ha1):\(nonce):\(ha2)")
        var header = "Authorization: Digest username=\"\(credentials.username)\", realm=\"\(realm)\", nonce=\"\(nonce)\", uri=\"\(uri)\", response=\"\(response)\""
        if let opaque = challenge.opaque {
            header += ", opaque=\"\(opaque)\""
        }
        if let algorithm = challenge.algorithm {
            header += ", algorithm=\(algorithm)"
        }
        return header
    }
}

private struct DigestChallenge {
    let realm: String
    let nonce: String
    let qop: String?
    let algorithm: String?
    let opaque: String?

    static func from(headerValue: String) -> DigestChallenge? {
        let lower = headerValue.lowercased()
        guard lower.hasPrefix("digest") else { return nil }
        let parameters = headerValue.dropFirst("digest".count).trimmingCharacters(in: .whitespaces)
        return DigestChallenge.from(parameterString: parameters)
    }

    static func from(parameterString: String) -> DigestChallenge? {
        var parameters: [String: String] = [:]
        var current = ""
        var inQuotes = false

        for character in parameterString {
            if character == "\"" {
                inQuotes.toggle()
                current.append(character)
                continue
            }
            if character == "," && !inQuotes {
                consume(&current, into: &parameters)
            } else {
                current.append(character)
            }
        }
        consume(&current, into: &parameters)

        guard let realm = parameters["realm"], let nonce = parameters["nonce"] else { return nil }
        let qop = parameters["qop"]?.components(separatedBy: ",").first { $0.trimmingCharacters(in: .whitespacesAndNewlines) == "auth" } ?? parameters["qop"]
        let algorithm = parameters["algorithm"]
        let opaque = parameters["opaque"]
        return DigestChallenge(realm: realm, nonce: nonce, qop: qop, algorithm: algorithm, opaque: opaque)
    }

    private static func consume(_ buffer: inout String, into parameters: inout [String: String]) {
        let trimmed = buffer.trimmingCharacters(in: .whitespacesAndNewlines)
        buffer.removeAll(keepingCapacity: true)
        guard !trimmed.isEmpty else { return }

        let parts = trimmed.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: true)
        guard parts.count == 2 else { return }
        let key = parts[0].trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        var value = parts[1].trimmingCharacters(in: .whitespacesAndNewlines)
        if value.hasPrefix("\""), value.hasSuffix("\""), value.count >= 2 {
            value = String(value.dropFirst().dropLast())
        }
        parameters[key] = value
    }
}

private func md5Hex(_ string: String) -> String {
    let digest = Insecure.MD5.hash(data: Data(string.utf8))
    return digest.map { String(format: "%02x", $0) }.joined()
}
