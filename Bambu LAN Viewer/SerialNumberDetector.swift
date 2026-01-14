//
//  SerialNumberDetector.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/13/26.
//

import Foundation

enum SerialNumberDetector {
    static func detectSerial(topic: String, payload: Data) -> String? {
        if let serial = serialFromTopic(topic) {
            return serial
        }

        return serialFromPayload(payload)
    }

    private static func serialFromTopic(_ topic: String) -> String? {
        let parts = topic.split(separator: "/")
        guard parts.count >= 3, parts[0] == "device" else { return nil }
        return String(parts[1])
    }

    private static func serialFromPayload(_ payload: Data) -> String? {
        guard let object = try? JSONSerialization.jsonObject(with: payload),
              let dict = object as? [String: Any] else {
            return nil
        }

        if let device = dict["device"] as? [String: Any],
           let serial = device["sn"] as? String {
            return serial
        }

        if let system = dict["system"] as? [String: Any],
           let serial = system["dev_id"] as? String {
            return serial
        }

        return nil
    }
}
