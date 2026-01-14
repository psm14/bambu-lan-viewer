//
//  Models.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/13/26.
//

import Foundation

struct PrinterConfig: Codable, Equatable, Identifiable {
    var id: UUID
    var name: String
    var ip: String
    var serial: String?
    var mqttPort: Int
    var rtspsPath: String
    var username: String

    init(
        id: UUID = UUID(),
        name: String,
        ip: String,
        serial: String? = nil,
        mqttPort: Int = 8883,
        rtspsPath: String = "/streaming/live/1",
        username: String = "bblp"
    ) {
        self.id = id
        self.name = name
        self.ip = ip
        self.serial = serial
        self.mqttPort = mqttPort
        self.rtspsPath = rtspsPath
        self.username = username
    }

    private enum CodingKeys: String, CodingKey {
        case id
        case name
        case ip
        case serial
        case mqttPort
        case rtspsPath
        case username
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decodeIfPresent(UUID.self, forKey: .id) ?? UUID()
        name = try container.decodeIfPresent(String.self, forKey: .name) ?? "Bambu Printer"
        ip = try container.decodeIfPresent(String.self, forKey: .ip) ?? ""
        serial = try container.decodeIfPresent(String.self, forKey: .serial)
        mqttPort = try container.decodeIfPresent(Int.self, forKey: .mqttPort) ?? 8883
        rtspsPath = try container.decodeIfPresent(String.self, forKey: .rtspsPath) ?? "/streaming/live/1"
        username = try container.decodeIfPresent(String.self, forKey: .username) ?? "bblp"
    }
}

enum ConnectionState: Equatable {
    case disconnected
    case connecting
    case connected
    case failed(message: String)
    case certMismatch
}

enum JobState: Equatable {
    case idle
    case printing
    case paused
    case finished
    case error(message: String)
}

extension JobState {
    static func from(gcodeState: String) -> JobState? {
        switch gcodeState.uppercased() {
        case "IDLE", "STOPPED":
            return .idle
        case "RUNNING":
            return .printing
        case "PAUSE":
            return .paused
        case "FINISH":
            return .finished
        case "FAILED":
            return .error(message: "Failed")
        default:
            return nil
        }
    }
}

struct PrinterState: Equatable {
    var connection: ConnectionState
    var job: JobState
    var progress01: Double?
    var nozzleC: Double?
    var nozzleTargetC: Double?
    var bedC: Double?
    var bedTargetC: Double?
    var chamberC: Double?
    var chamberTargetC: Double?
    var lightOn: Bool?
    var lastUpdate: Date?

    init(
        connection: ConnectionState = .disconnected,
        job: JobState = .idle,
        progress01: Double? = nil,
        nozzleC: Double? = nil,
        nozzleTargetC: Double? = nil,
        bedC: Double? = nil,
        bedTargetC: Double? = nil,
        chamberC: Double? = nil,
        chamberTargetC: Double? = nil,
        lightOn: Bool? = nil,
        lastUpdate: Date? = nil
    ) {
        self.connection = connection
        self.job = job
        self.progress01 = progress01
        self.nozzleC = nozzleC
        self.nozzleTargetC = nozzleTargetC
        self.bedC = bedC
        self.bedTargetC = bedTargetC
        self.chamberC = chamberC
        self.chamberTargetC = chamberTargetC
        self.lightOn = lightOn
        self.lastUpdate = lastUpdate
    }

    static let empty = PrinterState()
}

struct PrinterStatePatch: Equatable {
    var job: JobState?
    var progress01: Double?
    var nozzleC: Double?
    var nozzleTargetC: Double?
    var bedC: Double?
    var bedTargetC: Double?
    var chamberC: Double?
    var chamberTargetC: Double?
    var lightOn: Bool?
}

extension PrinterState {
    mutating func apply(patch: PrinterStatePatch) {
        if let job = patch.job {
            self.job = job
        }
        if let progress01 = patch.progress01 {
            self.progress01 = progress01
        }
        if let nozzleC = patch.nozzleC {
            self.nozzleC = nozzleC
        }
        if let nozzleTargetC = patch.nozzleTargetC {
            self.nozzleTargetC = nozzleTargetC
        }
        if let bedC = patch.bedC {
            self.bedC = bedC
        }
        if let bedTargetC = patch.bedTargetC {
            self.bedTargetC = bedTargetC
        }
        if let chamberC = patch.chamberC {
            self.chamberC = chamberC
        }
        if let chamberTargetC = patch.chamberTargetC {
            self.chamberTargetC = chamberTargetC
        }
        if let lightOn = patch.lightOn {
            self.lightOn = lightOn
        }
    }
}

enum PrinterCommand: Equatable {
    case pause
    case resume
    case stop
    case setLight(Bool)
}
