//
//  Storage.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/13/26.
//

import Foundation
import Security

private enum KeychainKey {
    static func lanCode(for printerID: UUID) -> String {
        "lanCode-\(printerID.uuidString)"
    }

    static func spkiHash(for printerID: UUID) -> String {
        "spkiHash-\(printerID.uuidString)"
    }
}

private struct KeychainStore {
    let service: String

    func readString(account: String) -> String? {
        var query: [String: Any] = baseQuery(account: account)
        query[kSecReturnData as String] = kCFBooleanTrue
        query[kSecMatchLimit as String] = kSecMatchLimitOne

        var item: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        guard status != errSecItemNotFound else { return nil }
        guard status == errSecSuccess, let data = item as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    func saveString(_ value: String, account: String) {
        let data = Data(value.utf8)
        var query: [String: Any] = baseQuery(account: account)
        query[kSecValueData as String] = data
        query[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly

        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecDuplicateItem else { return }

        let updateAttributes: [String: Any] = [
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        ]
        SecItemUpdate(baseQuery(account: account) as CFDictionary, updateAttributes as CFDictionary)
    }

    func delete(account: String) {
        SecItemDelete(baseQuery(account: account) as CFDictionary)
    }

    private func baseQuery(account: String) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account
        ]
    }
}

actor KeychainSecretStore: SecretStore {
    private let store: KeychainStore

    init(service: String = Bundle.main.bundleIdentifier ?? "com.bambu.lanviewer") {
        self.store = KeychainStore(service: service)
    }

    func secret(for printerID: UUID) async -> String? {
        store.readString(account: KeychainKey.lanCode(for: printerID))
    }

    func setSecret(_ secret: String, for printerID: UUID) async {
        store.saveString(secret, account: KeychainKey.lanCode(for: printerID))
    }

    func clearSecret(for printerID: UUID) async {
        store.delete(account: KeychainKey.lanCode(for: printerID))
    }
}

actor KeychainTrustStore: TrustStore {
    private let store: KeychainStore

    init(service: String = Bundle.main.bundleIdentifier ?? "com.bambu.lanviewer") {
        self.store = KeychainStore(service: service)
    }

    func pinnedSPKIHash(for printerID: UUID) async -> String? {
        store.readString(account: KeychainKey.spkiHash(for: printerID))
    }

    func setPinnedSPKIHash(_ hash: String, for printerID: UUID) async {
        store.saveString(hash, account: KeychainKey.spkiHash(for: printerID))
    }

    func clearPinnedSPKIHash(for printerID: UUID) async {
        store.delete(account: KeychainKey.spkiHash(for: printerID))
    }
}

actor UserDefaultsPrinterConfigStore: PrinterConfigStoring {
    private let defaults: UserDefaults
    private let storageKey: String

    init(defaults: UserDefaults = .standard, storageKey: String = "printerConfig") {
        self.defaults = defaults
        self.storageKey = storageKey
    }

    func load() async -> PrinterConfig? {
        guard let data = defaults.data(forKey: storageKey) else { return nil }
        return try? JSONDecoder().decode(PrinterConfig.self, from: data)
    }

    func save(_ config: PrinterConfig) async {
        guard let data = try? JSONEncoder().encode(config) else { return }
        defaults.set(data, forKey: storageKey)
    }

    func clear() async {
        defaults.removeObject(forKey: storageKey)
    }
}

actor InMemorySecretStore: SecretStore {
    private var secrets: [UUID: String]

    init(secrets: [UUID: String] = [:]) {
        self.secrets = secrets
    }

    func secret(for printerID: UUID) async -> String? {
        secrets[printerID]
    }

    func setSecret(_ secret: String, for printerID: UUID) async {
        secrets[printerID] = secret
    }

    func clearSecret(for printerID: UUID) async {
        secrets[printerID] = nil
    }
}

actor InMemoryTrustStore: TrustStore {
    private var hashes: [UUID: String]

    init(hashes: [UUID: String] = [:]) {
        self.hashes = hashes
    }

    func pinnedSPKIHash(for printerID: UUID) async -> String? {
        hashes[printerID]
    }

    func setPinnedSPKIHash(_ hash: String, for printerID: UUID) async {
        hashes[printerID] = hash
    }

    func clearPinnedSPKIHash(for printerID: UUID) async {
        hashes[printerID] = nil
    }
}

actor InMemoryPrinterConfigStore: PrinterConfigStoring {
    private var config: PrinterConfig?

    init(config: PrinterConfig? = nil) {
        self.config = config
    }

    func load() async -> PrinterConfig? {
        config
    }

    func save(_ config: PrinterConfig) async {
        self.config = config
    }

    func clear() async {
        config = nil
    }
}
