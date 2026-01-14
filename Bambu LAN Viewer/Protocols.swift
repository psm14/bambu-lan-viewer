//
//  Protocols.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/13/26.
//

import Foundation
import Security
import UIKit

struct MqttMessage: Equatable {
    let topic: String
    let payload: Data
}

enum MqttConnectionState: Equatable {
    case disconnected
    case connecting
    case connected
    case failed(message: String)
}

protocol MqttTransport: AnyObject {
    var onMessage: ((MqttMessage) -> Void)? { get set }
    var onConnectionStateChanged: ((MqttConnectionState) -> Void)? { get set }
    var onTrustEvaluation: ((SecTrust, @escaping (Bool) -> Void) -> Void)? { get set }

    func connect(host: String, port: Int, username: String, password: String, useTLS: Bool)
    func disconnect()
    func subscribe(topic: String)
    func unsubscribe(topic: String)
    func publish(topic: String, payload: Data, qos: Int, retain: Bool)
}

enum VideoState: Equatable {
    case stopped
    case buffering
    case playing
    case failed(message: String)
}

protocol VideoTransport: AnyObject {
    var onStateChanged: ((VideoState) -> Void)? { get set }

    func attach(to view: UIView)
    func play(url: URL, username: String, password: String)
    func stop()
}

protocol TrustStore: AnyObject {
    func pinnedSPKIHash(for printerID: UUID) async -> String?
    func setPinnedSPKIHash(_ hash: String, for printerID: UUID) async
    func clearPinnedSPKIHash(for printerID: UUID) async
}

protocol SecretStore: AnyObject {
    func secret(for printerID: UUID) async -> String?
    func setSecret(_ secret: String, for printerID: UUID) async
    func clearSecret(for printerID: UUID) async
}

protocol PrinterConfigStoring: AnyObject {
    func load() async -> PrinterConfig?
    func save(_ config: PrinterConfig) async
    func clear() async
}
