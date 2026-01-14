//
//  H264DecoderVT.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/14/26.
//

import AVFoundation
import Foundation
import VideoToolbox

final class H264DecoderVT {
    var onFrame: ((CVImageBuffer, CMTime) -> Void)?

    private let decodeQueue = DispatchQueue(label: "com.bambu.lanviewer.h264-decode")
    private var formatDescription: CMVideoFormatDescription?
    private var decompressionSession: VTDecompressionSession?
    private var currentSps: Data?
    private var currentPps: Data?
    private var hasKeyframe = false

    func reset() {
        decodeQueue.async { [weak self] in
            self?.invalidateSession()
            self?.currentSps = nil
            self?.currentPps = nil
            self?.hasKeyframe = false
        }
    }

    func setParameterSets(sps: Data, pps: Data) {
        decodeQueue.async { [weak self] in
            guard let self else { return }
            if self.currentSps == sps && self.currentPps == pps {
                return
            }
            self.currentSps = sps
            self.currentPps = pps
            self.createFormatDescription(sps: sps, pps: pps)
            self.invalidateSession()
            self.createSessionIfPossible()
            self.hasKeyframe = false
        }
    }

    func decode(accessUnit: AccessUnit, pts: CMTime) {
        decodeQueue.async { [weak self] in
            guard let self else { return }
            guard self.decompressionSession != nil else { return }

            let containsIDR = accessUnit.nals.contains { ($0.first ?? 0) & 0x1F == 5 }
            if !self.hasKeyframe {
                guard containsIDR else { return }
                self.hasKeyframe = true
            }

            guard let formatDescription = self.formatDescription else { return }
            guard let sampleBuffer = self.makeSampleBuffer(accessUnit: accessUnit, pts: pts, formatDescription: formatDescription) else { return }

            var infoFlags = VTDecodeInfoFlags()
            let flags = VTDecodeFrameFlags._EnableAsynchronousDecompression
            let status = VTDecompressionSessionDecodeFrame(
                self.ensureSession(),
                sampleBuffer: sampleBuffer,
                flags: flags,
                infoFlagsOut: &infoFlags
            ) { [weak self] status, _, imageBuffer, presentationTimeStamp, _ in
                guard status == noErr, let imageBuffer else { return }
                self?.onFrame?(imageBuffer, presentationTimeStamp)
            }

            if status != noErr {
                self.invalidateSession()
                self.createSessionIfPossible()
            }
        }
    }

    private func ensureSession() -> VTDecompressionSession {
        if let session = decompressionSession {
            return session
        }
        createSessionIfPossible()
        return decompressionSession!
    }

    private func createFormatDescription(sps: Data, pps: Data) {
        var formatDescription: CMVideoFormatDescription?
        sps.withUnsafeBytes { spsBytes in
            pps.withUnsafeBytes { ppsBytes in
                let parameterSetPointers: [UnsafePointer<UInt8>] = [
                    spsBytes.bindMemory(to: UInt8.self).baseAddress!,
                    ppsBytes.bindMemory(to: UInt8.self).baseAddress!
                ]
                let parameterSetSizes: [Int] = [sps.count, pps.count]
                CMVideoFormatDescriptionCreateFromH264ParameterSets(
                    allocator: kCFAllocatorDefault,
                    parameterSetCount: 2,
                    parameterSetPointers: parameterSetPointers,
                    parameterSetSizes: parameterSetSizes,
                    nalUnitHeaderLength: 4,
                    formatDescriptionOut: &formatDescription
                )
            }
        }
        self.formatDescription = formatDescription
    }

    private func createSessionIfPossible() {
        guard let formatDescription else { return }

        var session: VTDecompressionSession?
        let destinationPixelBufferAttributes: [NSString: Any] = [
            kCVPixelBufferPixelFormatTypeKey: kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange
        ]
        var callbackRecord = VTDecompressionOutputCallbackRecord()

        let status = VTDecompressionSessionCreate(
            allocator: kCFAllocatorDefault,
            formatDescription: formatDescription,
            decoderSpecification: nil,
            imageBufferAttributes: destinationPixelBufferAttributes as CFDictionary,
            outputCallback: &callbackRecord,
            decompressionSessionOut: &session
        )

        guard status == noErr, let session else { return }
        VTSessionSetProperty(session, key: kVTDecompressionPropertyKey_RealTime, value: kCFBooleanTrue)
        VTSessionSetProperty(session, key: kVTDecompressionPropertyKey_ThreadCount, value: NSNumber(value: 2))
        decompressionSession = session
    }

    private func invalidateSession() {
        if let session = decompressionSession {
            VTDecompressionSessionInvalidate(session)
        }
        decompressionSession = nil
    }

    private func makeSampleBuffer(accessUnit: AccessUnit, pts: CMTime, formatDescription: CMVideoFormatDescription) -> CMSampleBuffer? {
        let avccData = buildAvccData(from: accessUnit.nals)
        var blockBuffer: CMBlockBuffer?
        let status = CMBlockBufferCreateWithMemoryBlock(
            allocator: kCFAllocatorDefault,
            memoryBlock: nil,
            blockLength: avccData.count,
            blockAllocator: kCFAllocatorDefault,
            customBlockSource: nil,
            offsetToData: 0,
            dataLength: avccData.count,
            flags: 0,
            blockBufferOut: &blockBuffer
        )
        guard status == kCMBlockBufferNoErr, let blockBuffer else { return nil }

        let replaceStatus = avccData.withUnsafeBytes { bytes in
            CMBlockBufferReplaceDataBytes(
                with: bytes.baseAddress!,
                blockBuffer: blockBuffer,
                offsetIntoDestination: 0,
                dataLength: avccData.count
            )
        }
        guard replaceStatus == kCMBlockBufferNoErr else { return nil }

        var timing = CMSampleTimingInfo(duration: .invalid, presentationTimeStamp: pts, decodeTimeStamp: .invalid)
        var sampleBuffer: CMSampleBuffer?
        let sampleStatus = CMSampleBufferCreateReady(
            allocator: kCFAllocatorDefault,
            dataBuffer: blockBuffer,
            formatDescription: formatDescription,
            sampleCount: 1,
            sampleTimingEntryCount: 1,
            sampleTimingArray: &timing,
            sampleSizeEntryCount: 0,
            sampleSizeArray: nil,
            sampleBufferOut: &sampleBuffer
        )
        guard sampleStatus == noErr else { return nil }
        return sampleBuffer
    }

    private func buildAvccData(from nals: [Data]) -> Data {
        var data = Data()
        for nal in nals {
            var length = UInt32(nal.count).bigEndian
            data.append(Data(bytes: &length, count: 4))
            data.append(nal)
        }
        return data
    }
}
