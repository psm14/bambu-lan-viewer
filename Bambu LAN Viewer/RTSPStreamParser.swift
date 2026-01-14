//
//  RTSPStreamParser.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/14/26.
//

import Foundation

enum RTSPStreamEvent {
    case response(RTSPResponse)
    case interleaved(channel: Int, payload: Data)
}

struct RTSPResponse {
    let statusCode: Int
    let reasonPhrase: String
    let headers: [String: String]
    let body: Data

    var cseq: Int? {
        guard let value = headers["cseq"], let parsed = Int(value) else { return nil }
        return parsed
    }

    func header(_ name: String) -> String? {
        return headers[name.lowercased()]
    }
}

final class RTSPStreamParser {
    private var buffer = Data()

    func append(_ data: Data) -> [RTSPStreamEvent] {
        buffer.append(data)
        var events: [RTSPStreamEvent] = []

        while !buffer.isEmpty {
            if buffer.first == 0x24 {
                guard let interleaved = extractInterleavedFrame() else { break }
                events.append(.interleaved(channel: interleaved.channel, payload: interleaved.payload))
                continue
            }

            guard let response = extractResponse() else { break }
            events.append(.response(response))
        }

        return events
    }

    private func extractInterleavedFrame() -> (channel: Int, payload: Data)? {
        guard buffer.count >= 4 else { return nil }
        guard buffer[buffer.startIndex] == 0x24 else { return nil }

        let channel = Int(buffer[buffer.startIndex.advanced(by: 1)])
        let lengthHigh = buffer[buffer.startIndex.advanced(by: 2)]
        let lengthLow = buffer[buffer.startIndex.advanced(by: 3)]
        let length = Int(lengthHigh) << 8 | Int(lengthLow)
        let total = 4 + length
        guard buffer.count >= total else { return nil }

        let payloadStart = buffer.startIndex.advanced(by: 4)
        let payloadEnd = payloadStart.advanced(by: length)
        let payload = Data(buffer[payloadStart..<payloadEnd])
        buffer.removeSubrange(..<payloadEnd)
        return (channel, payload)
    }

    private func extractResponse() -> RTSPResponse? {
        let separator = Data("\r\n\r\n".utf8)
        guard let headerRange = buffer.range(of: separator) else { return nil }

        let headerEnd = headerRange.upperBound
        let headerData = Data(buffer[..<headerEnd])
        guard let headerText = String(data: headerData, encoding: .utf8) else { return nil }

        let lines = headerText.components(separatedBy: "\r\n").filter { !$0.isEmpty }
        guard let statusLine = lines.first else { return nil }

        let statusParts = statusLine.split(separator: " ", maxSplits: 2, omittingEmptySubsequences: true)
        let statusCode = statusParts.count > 1 ? Int(statusParts[1]) ?? -1 : -1
        let reasonPhrase = statusParts.count > 2 ? String(statusParts[2]) : ""

        var headers: [String: String] = [:]
        for line in lines.dropFirst() {
            let parts = line.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: true)
            guard parts.count == 2 else { continue }
            let key = parts[0].trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            let value = parts[1].trimmingCharacters(in: .whitespacesAndNewlines)
            headers[key] = value
        }

        let contentLength = Int(headers["content-length"] ?? "0") ?? 0
        let totalLength = headerEnd + contentLength
        guard buffer.count >= totalLength else { return nil }

        let bodyRange = headerEnd..<totalLength
        let body = Data(buffer[bodyRange])
        buffer.removeSubrange(..<totalLength)

        return RTSPResponse(statusCode: statusCode, reasonPhrase: reasonPhrase, headers: headers, body: body)
    }
}
