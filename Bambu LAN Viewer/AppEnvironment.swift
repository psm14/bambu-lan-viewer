//
//  AppEnvironment.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/13/26.
//

import SwiftUI

struct AppEnvironment {
    let configStore: any PrinterConfigStoring
    let secretStore: any SecretStore
    let trustStore: any TrustStore

    static let live = AppEnvironment(
        configStore: UserDefaultsPrinterConfigStore(),
        secretStore: KeychainSecretStore(),
        trustStore: KeychainTrustStore()
    )

    static let preview = AppEnvironment(
        configStore: InMemoryPrinterConfigStore(),
        secretStore: InMemorySecretStore(),
        trustStore: InMemoryTrustStore()
    )
}

private struct AppEnvironmentKey: EnvironmentKey {
    static let defaultValue = AppEnvironment.live
}

extension EnvironmentValues {
    var appEnvironment: AppEnvironment {
        get { self[AppEnvironmentKey.self] }
        set { self[AppEnvironmentKey.self] = newValue }
    }
}
