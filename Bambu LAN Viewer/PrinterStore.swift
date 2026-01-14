//
//  PrinterStore.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/13/26.
//

import Combine
import Foundation

@MainActor
final class PrinterStore: ObservableObject {
    @Published private(set) var state: PrinterState
    @Published private(set) var config: PrinterConfig

    private let service: PrinterService

    init(
        config: PrinterConfig,
        environment: AppEnvironment,
        mqttTransport: MqttTransport
    ) {
        self.config = config
        self.state = PrinterState.empty
        self.service = PrinterService(
            config: config,
            mqttTransport: mqttTransport,
            configStore: environment.configStore,
            secretStore: environment.secretStore,
            trustStore: environment.trustStore
        )

        Task { [weak self] in
            guard let self else { return }
            await service.setStateHandler { [weak self] state in
                self?.state = state
            }
            await service.setConfigHandler { [weak self] config in
                self?.config = config
            }
        }
    }

    func connect() {
        Task {
            await service.connect()
        }
    }

    func disconnect() {
        Task {
            await service.disconnect()
        }
    }

    func pause() {
        Task {
            await service.pause()
        }
    }

    func resume() {
        Task {
            await service.resume()
        }
    }

    func stopPrint() {
        Task {
            await service.stopPrint()
        }
    }

    func setLight(_ isOn: Bool) {
        Task {
            await service.setLight(isOn)
        }
    }
}
