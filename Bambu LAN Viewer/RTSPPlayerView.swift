//
//  RTSPPlayerView.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/14/26.
//

import SwiftUI
import UIKit

struct RTSPPlayerView: UIViewRepresentable {
    let url: URL?
    let username: String
    let password: String
    let isActive: Bool
    let trustStore: TrustStore
    let printerID: UUID
    var onStateChanged: ((VideoState) -> Void)?

    func makeCoordinator() -> Coordinator {
        Coordinator(
            trustStore: trustStore,
            printerID: printerID,
            onStateChanged: onStateChanged
        )
    }

    func makeUIView(context: Context) -> VideoRenderView {
        let view = VideoRenderView()
        context.coordinator.attach(to: view)
        return view
    }

    func updateUIView(_ uiView: VideoRenderView, context: Context) {
        context.coordinator.onStateChanged = onStateChanged
        context.coordinator.update(url: url, username: username, password: password, isActive: isActive)
    }

    static func dismantleUIView(_ uiView: VideoRenderView, coordinator: Coordinator) {
        coordinator.stop()
    }

    final class Coordinator: NSObject {
        private let transport: NativeVideoTransport
        private var currentURL: URL?
        private var currentUsername: String?
        private var currentPassword: String?
        private var isPlaying = false

        var onStateChanged: ((VideoState) -> Void)? {
            didSet {
                transport.onStateChanged = { [weak self] state in
                    self?.handleStateChange(state)
                    self?.onStateChanged?(state)
                }
            }
        }

        init(trustStore: TrustStore, printerID: UUID, onStateChanged: ((VideoState) -> Void)? = nil) {
            transport = NativeVideoTransport(trustStore: trustStore, printerID: printerID)
            self.onStateChanged = onStateChanged
            super.init()
            transport.onStateChanged = { [weak self] state in
                self?.handleStateChange(state)
                self?.onStateChanged?(state)
            }
        }

        func attach(to view: UIView) {
            transport.attach(to: view)
        }

        func update(url: URL?, username: String, password: String, isActive: Bool) {
            guard isActive, let url else {
                if isPlaying {
                    transport.stop()
                    isPlaying = false
                }
                return
            }

            if currentURL != url || currentUsername != username || currentPassword != password || !isPlaying {
                currentURL = url
                currentUsername = username
                currentPassword = password
                transport.play(url: url, username: username, password: password)
                isPlaying = true
            }
        }

        func stop() {
            transport.stop()
            isPlaying = false
        }

        private func handleStateChange(_ state: VideoState) {
            switch state {
            case .playing, .buffering:
                isPlaying = true
            case .stopped, .failed:
                isPlaying = false
            }
        }
    }
}
