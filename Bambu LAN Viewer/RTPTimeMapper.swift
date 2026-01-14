//
//  RTPTimeMapper.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/14/26.
//

import CoreMedia

final class RTPTimeMapper {
    private var firstTimestamp: UInt32?

    func reset() {
        firstTimestamp = nil
    }

    func presentationTime(for rtpTimestamp: UInt32) -> CMTime {
        if firstTimestamp == nil {
            firstTimestamp = rtpTimestamp
        }
        let base = firstTimestamp ?? rtpTimestamp
        let delta = rtpTimestamp &- base
        return CMTime(value: CMTimeValue(Int64(delta)), timescale: 90000)
    }
}
