//
//  NativeVideoTransport.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/14/26.
//

import AVFoundation
import Foundation
import UIKit

final class NativeVideoTransport: VideoTransport, @unchecked Sendable {
    var onStateChanged: ((VideoState) -> Void)?

    private let queue = DispatchQueue(label: "com.bambu.lanviewer.rtsp")
    private let trustStore: TrustStore
    private let printerID: UUID
    private var client: RTSPClient?
    private var depacketizer = H264RTPDepacketizer()
    private var decoder = H264DecoderVT()
    private var renderer: VideoRenderer?
    private var timeMapper = RTPTimeMapper()
    private var rtpChannel: Int = 0
    private var expectedPayloadType: Int?
    private var lastState: VideoState?

    init(trustStore: TrustStore, printerID: UUID) {
        self.trustStore = trustStore
        self.printerID = printerID
        decoder.onFrame = { [weak self] imageBuffer, pts in
            self?.render(imageBuffer: imageBuffer, pts: pts)
        }
        depacketizer.onParameterSets = { [weak self] sps, pps in
            self?.decoder.setParameterSets(sps: sps, pps: pps)
        }
    }

    func attach(to view: UIView) {
        guard let renderView = view as? VideoRenderView else { return }
        renderer = VideoRenderer(displayLayer: renderView.displayLayer)
    }

    func play(url: URL, username: String, password: String) {
        renderer?.reset()
        report(.buffering)
        queue.async { [weak self] in
            guard let self else { return }
            self.client?.stop()
            self.client = nil
            self.depacketizer.reset()
            self.decoder.reset()
            self.timeMapper.reset()
            self.expectedPayloadType = nil
            self.rtpChannel = 0

            let credentials = RtspCredentials(username: username, password: password)
            let client = RTSPClient(
                queue: self.queue,
                credentials: credentials,
                onTrustEvaluation: { [weak self] trust, completion in
                    guard let self else {
                        completion(false)
                        return
                    }
                    Task {
                        let allowed = await self.evaluateTrust(trust)
                        completion(allowed)
                    }
                }
            )
            self.client = client

            client.onInterleavedPacket = { [weak self] channel, payload in
                self?.handleInterleaved(channel: channel, payload: payload)
            }
            client.onError = { [weak self] error in
                self?.handleError(error)
            }

            Task { [weak self] in
                guard let self else { return }
                do {
                    let session = try await client.start(url: url)
                    self.queue.async { [weak self] in
                        guard let self else { return }
                        self.expectedPayloadType = session.sdpInfo.payloadType
                        self.rtpChannel = session.rtpChannel
                    }
                    if let sps = session.sdpInfo.sps, let pps = session.sdpInfo.pps {
                        self.decoder.setParameterSets(sps: sps, pps: pps)
                    }
                } catch {
                    self.handleError(error)
                }
            }
        }
    }

    func stop() {
        queue.async { [weak self] in
            guard let self else { return }
            self.client?.stop()
            self.client = nil
            self.decoder.reset()
            self.depacketizer.reset()
            self.timeMapper.reset()
        }
        renderer?.reset()
        report(.stopped)
    }

    private func handleInterleaved(channel: Int, payload: Data) {
        guard channel == rtpChannel else { return }
        guard let packet = RTPPacket(data: payload) else { return }
        if let expectedPayloadType, packet.payloadType != expectedPayloadType {
            return
        }
        let accessUnits = depacketizer.handle(packet: packet)
        for accessUnit in accessUnits {
            let pts = timeMapper.presentationTime(for: accessUnit.rtpTimestamp)
            decoder.decode(accessUnit: accessUnit, pts: pts)
        }
    }

    private func render(imageBuffer: CVImageBuffer, pts: CMTime) {
        renderer?.enqueue(imageBuffer: imageBuffer, pts: pts)
        report(.playing)
    }

    private func handleError(_ error: Error) {
        queue.async { [weak self] in
            self?.client?.stop()
            self?.client = nil
        }
        report(.failed(message: error.localizedDescription))
    }

    private func report(_ state: VideoState) {
        guard state != lastState else { return }
        lastState = state
        DispatchQueue.main.async { [weak self] in
            self?.onStateChanged?(state)
        }
    }

    private func evaluateTrust(_ trust: SecTrust) async -> Bool {
        guard let hash = TrustHasher.publicKeyHashBase64(from: trust) else {
            report(.failed(message: "Unable to read printer certificate."))
            return false
        }

        if let pinned = await trustStore.pinnedSPKIHash(for: printerID) {
            if pinned == hash {
                return true
            }
            report(.failed(message: "Certificate mismatch."))
            return false
        }

        await trustStore.setPinnedSPKIHash(hash, for: printerID)
        return true
    }
}
