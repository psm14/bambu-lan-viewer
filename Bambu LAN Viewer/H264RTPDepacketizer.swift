//
//  H264RTPDepacketizer.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/14/26.
//

import Foundation

struct AccessUnit {
    let nals: [Data]
    let rtpTimestamp: UInt32
}

final class H264RTPDepacketizer {
    var onParameterSets: ((Data, Data) -> Void)?

    private var currentAccessUnit: [Data] = []
    private var currentTimestamp: UInt32?
    private var fuBuffer: Data?
    private var fuSequence: UInt16?
    private var sps: Data?
    private var pps: Data?

    func reset() {
        currentAccessUnit.removeAll()
        currentTimestamp = nil
        fuBuffer = nil
        fuSequence = nil
        sps = nil
        pps = nil
    }

    func handle(packet: RTPPacket) -> [AccessUnit] {
        var output: [AccessUnit] = []
        if let currentTimestamp, currentTimestamp != packet.timestamp, !currentAccessUnit.isEmpty {
            output.append(AccessUnit(nals: currentAccessUnit, rtpTimestamp: currentTimestamp))
            currentAccessUnit.removeAll()
            self.currentTimestamp = nil
        }

        let nals = extractNals(from: packet)
        for nal in nals {
            append(nal: nal, timestamp: packet.timestamp)
        }

        if packet.marker, let currentTimestamp, !currentAccessUnit.isEmpty {
            output.append(AccessUnit(nals: currentAccessUnit, rtpTimestamp: currentTimestamp))
            currentAccessUnit.removeAll()
            self.currentTimestamp = nil
        }

        return output
    }

    private func append(nal: Data, timestamp: UInt32) {
        if currentTimestamp == nil {
            currentTimestamp = timestamp
        }

        let nalType = nal.first.map { $0 & 0x1F } ?? 0
        if nalType == 7 {
            sps = nal
            notifyParameterSetsIfNeeded()
        } else if nalType == 8 {
            pps = nal
            notifyParameterSetsIfNeeded()
        }

        currentAccessUnit.append(nal)
    }

    private func notifyParameterSetsIfNeeded() {
        guard let sps, let pps else { return }
        onParameterSets?(sps, pps)
    }

    private func extractNals(from packet: RTPPacket) -> [Data] {
        let payload = packet.payload
        guard let firstByte = payload.first else { return [] }
        let nalType = firstByte & 0x1F

        switch nalType {
        case 1...23:
            return [payload]
        case 24:
            return extractStapA(payload: payload)
        case 28:
            return extractFuA(payload: payload, sequence: packet.sequenceNumber, timestamp: packet.timestamp)
        default:
            return []
        }
    }

    private func extractStapA(payload: Data) -> [Data] {
        guard payload.count > 1 else { return [] }
        var index = 1
        var nals: [Data] = []

        while index + 2 <= payload.count {
            let size = Int(UInt16(payload[payload.startIndex.advanced(by: index)]) << 8
                | UInt16(payload[payload.startIndex.advanced(by: index + 1)]))
            index += 2
            guard index + size <= payload.count else { break }
            let nal = Data(payload[payload.startIndex.advanced(by: index)..<payload.startIndex.advanced(by: index + size)])
            nals.append(nal)
            index += size
        }

        return nals
    }

    private func extractFuA(payload: Data, sequence: UInt16, timestamp: UInt32) -> [Data] {
        guard payload.count > 2 else { return [] }

        let fuIndicator = payload[payload.startIndex]
        let fuHeader = payload[payload.startIndex.advanced(by: 1)]
        let start = (fuHeader & 0x80) != 0
        let end = (fuHeader & 0x40) != 0
        let nalType = fuHeader & 0x1F
        let nalHeader = (fuIndicator & 0xE0) | nalType

        if start {
            fuBuffer = Data([nalHeader])
            fuBuffer?.append(payload[payload.startIndex.advanced(by: 2)...])
            fuSequence = sequence
            return []
        }

        guard var buffer = fuBuffer else { return [] }
        if let lastSequence = fuSequence {
            let expected = lastSequence &+ 1
            guard sequence == expected else {
                fuBuffer = nil
                fuSequence = nil
                return []
            }
        }
        buffer.append(payload[payload.startIndex.advanced(by: 2)...])
        fuBuffer = buffer
        fuSequence = sequence

        if end {
            fuBuffer = nil
            fuSequence = nil
            return [buffer]
        }

        return []
    }
}
