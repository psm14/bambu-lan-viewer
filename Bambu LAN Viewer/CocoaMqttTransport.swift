//
//  CocoaMqttTransport.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/13/26.
//

import CocoaMQTT
import Foundation
import Security

final class CocoaMqttTransport: NSObject, MqttTransport {
    var onMessage: ((MqttMessage) -> Void)?
    var onConnectionStateChanged: ((MqttConnectionState) -> Void)?
    var onTrustEvaluation: ((SecTrust, @escaping (Bool) -> Void) -> Void)?

    private var mqtt: CocoaMQTT?

    func connect(host: String, port: Int, username: String, password: String, useTLS: Bool) {
        let clientID = "bambu-lan-viewer-\(UUID().uuidString)"
        let mqtt = CocoaMQTT(clientID: clientID, host: host, port: UInt16(port))
        mqtt.username = username
        mqtt.password = password
        mqtt.keepAlive = 60
        mqtt.cleanSession = true
        mqtt.enableSSL = useTLS
        mqtt.delegate = self
        if useTLS {
            mqtt.allowUntrustCACertificate = true
        }
        _ = mqtt.connect()
        self.mqtt = mqtt
        onConnectionStateChanged?(.connecting)
    }

    func disconnect() {
        mqtt?.disconnect()
    }

    func subscribe(topic: String) {
        mqtt?.subscribe(topic, qos: .qos0)
    }

    func unsubscribe(topic: String) {
        mqtt?.unsubscribe(topic)
    }

    func publish(topic: String, payload: Data, qos: Int, retain: Bool) {
        let message = CocoaMQTTMessage(topic: topic, payload: [UInt8](payload), qos: qosValue(from: qos), retained: retain)
        mqtt?.publish(message)
    }

    private func qosValue(from qos: Int) -> CocoaMQTTQoS {
        switch qos {
        case 1:
            return .qos1
        case 2:
            return .qos2
        default:
            return .qos0
        }
    }
}

extension CocoaMqttTransport: CocoaMQTTDelegate {
    func mqtt(_ mqtt: CocoaMQTT, didConnectAck ack: CocoaMQTTConnAck) {
        if ack == .accept {
            onConnectionStateChanged?(.connected)
        } else {
            onConnectionStateChanged?(.failed(message: "Connect rejected: \(String(describing: ack))"))
        }
    }

    func mqtt(_ mqtt: CocoaMQTT, didReceiveMessage message: CocoaMQTTMessage, id: UInt16) {
        let payload = Data(message.payload)
        onMessage?(MqttMessage(topic: message.topic, payload: payload))
    }

    func mqtt(_ mqtt: CocoaMQTT, didPublishMessage message: CocoaMQTTMessage, id: UInt16) {
    }

    func mqtt(_ mqtt: CocoaMQTT, didPublishAck id: UInt16) {
    }

    func mqtt(_ mqtt: CocoaMQTT, didSubscribeTopics success: NSDictionary, failed: [String]) {
    }

    func mqtt(_ mqtt: CocoaMQTT, didUnsubscribeTopics topics: [String]) {
    }

    func mqttDidPing(_ mqtt: CocoaMQTT) {
    }

    func mqttDidReceivePong(_ mqtt: CocoaMQTT) {
    }

    func mqttDidDisconnect(_ mqtt: CocoaMQTT, withError err: Error?) {
        if let err {
            onConnectionStateChanged?(.failed(message: err.localizedDescription))
        } else {
            onConnectionStateChanged?(.disconnected)
        }
    }

    func mqtt(_ mqtt: CocoaMQTT, didReceive trust: SecTrust, completionHandler: @escaping (Bool) -> Void) {
        guard let onTrustEvaluation else {
            completionHandler(true)
            return
        }

        onTrustEvaluation(trust, completionHandler)
    }
}
