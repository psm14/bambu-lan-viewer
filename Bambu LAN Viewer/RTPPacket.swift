//
//  RTPPacket.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/14/26.
//

import Foundation

struct RTPPacket {
    let payloadType: Int
    let marker: Bool
    let sequenceNumber: UInt16
    let timestamp: UInt32
    let ssrc: UInt32
    let payload: Data

    init?(data: Data) {
        guard data.count >= 12 else { return nil }
        let b0 = data[data.startIndex]
        let b1 = data[data.startIndex.advanced(by: 1)]

        let version = b0 >> 6
        guard version == 2 else { return nil }

        let padding = (b0 & 0x20) != 0
        let hasExtension = (b0 & 0x10) != 0
        let csrcCount = Int(b0 & 0x0F)

        marker = (b1 & 0x80) != 0
        payloadType = Int(b1 & 0x7F)

        sequenceNumber = UInt16(data[data.startIndex.advanced(by: 2)]) << 8 | UInt16(data[data.startIndex.advanced(by: 3)])
        timestamp = UInt32(data[data.startIndex.advanced(by: 4)]) << 24
            | UInt32(data[data.startIndex.advanced(by: 5)]) << 16
            | UInt32(data[data.startIndex.advanced(by: 6)]) << 8
            | UInt32(data[data.startIndex.advanced(by: 7)])
        ssrc = UInt32(data[data.startIndex.advanced(by: 8)]) << 24
            | UInt32(data[data.startIndex.advanced(by: 9)]) << 16
            | UInt32(data[data.startIndex.advanced(by: 10)]) << 8
            | UInt32(data[data.startIndex.advanced(by: 11)])

        var offset = 12 + csrcCount * 4
        if hasExtension {
            guard data.count >= offset + 4 else { return nil }
            let extensionLength = Int(UInt16(data[data.startIndex.advanced(by: offset + 2)]) << 8
                | UInt16(data[data.startIndex.advanced(by: offset + 3)]))
            offset += 4 + extensionLength * 4
        }

        guard data.count >= offset else { return nil }
        var payloadEnd = data.count
        if padding {
            guard let last = data.last else { return nil }
            let paddingLength = Int(last)
            payloadEnd = max(offset, data.count - paddingLength)
        }
        payload = Data(data[data.startIndex.advanced(by: offset)..<data.startIndex.advanced(by: payloadEnd)])
    }
}
