//
//  PrinterReport.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/13/26.
//

import Foundation

struct PrinterReport: Decodable {
    struct PrintInfo: Decodable {
        let gcodeState: String?
        let mcPercent: Double?
        let layerNumber: Int?
        let totalLayerNumber: Int?
        let nozzleTemper: Double?
        let nozzleTargetTemper: Double?
        let bedTemper: Double?
        let bedTargetTemper: Double?
        let chamberTemper: Double?
        let chamberTargetTemper: Double?
        let infoTemp: Double?
        let deviceChamberTemp: Double?
        let lightsReportEntries: [LightEntry]?
        let rtspURL: String?

        private enum CodingKeys: String, CodingKey {
            case gcodeState = "gcode_state"
            case mcPercent = "mc_percent"
            case layerNumber = "layer_num"
            case totalLayerNumber = "total_layer_num"
            case nozzleTemper = "nozzle_temper"
            case nozzleTargetTemper = "nozzle_target_temper"
            case bedTemper = "bed_temper"
            case bedTargetTemper = "bed_target_temper"
            case chamberTemper = "chamber_temper"
            case chamberTargetTemper = "chamber_target_temper"
            case info
            case device
            case lightsReport = "lights_report"
            case ipcam
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            gcodeState = try container.decodeIfPresent(String.self, forKey: .gcodeState)
            mcPercent = try container.decodeIfPresent(LossyDouble.self, forKey: .mcPercent)?.value
            layerNumber = try container.decodeIfPresent(LossyInt.self, forKey: .layerNumber)?.value
            totalLayerNumber = try container.decodeIfPresent(LossyInt.self, forKey: .totalLayerNumber)?.value
            nozzleTemper = try container.decodeIfPresent(LossyDouble.self, forKey: .nozzleTemper)?.value
            nozzleTargetTemper = try container.decodeIfPresent(LossyDouble.self, forKey: .nozzleTargetTemper)?.value
            bedTemper = try container.decodeIfPresent(LossyDouble.self, forKey: .bedTemper)?.value
            bedTargetTemper = try container.decodeIfPresent(LossyDouble.self, forKey: .bedTargetTemper)?.value
            chamberTemper = try container.decodeIfPresent(LossyDouble.self, forKey: .chamberTemper)?.value
            chamberTargetTemper = try container.decodeIfPresent(LossyDouble.self, forKey: .chamberTargetTemper)?.value
            infoTemp = try container.decodeIfPresent(Info.self, forKey: .info)?.temp
            deviceChamberTemp = try container.decodeIfPresent(Device.self, forKey: .device)?.ctc?.info?.temp
            lightsReportEntries = try container.decodeIfPresent([LightEntry].self, forKey: .lightsReport)
            rtspURL = try container.decodeIfPresent(Ipcam.self, forKey: .ipcam)?.rtspURL
        }

        struct Info: Decodable {
            let temp: Double?

            private enum CodingKeys: String, CodingKey {
                case temp
            }

            init(from decoder: Decoder) throws {
                let container = try decoder.container(keyedBy: CodingKeys.self)
                temp = try container.decodeIfPresent(LossyDouble.self, forKey: .temp)?.value
            }
        }

        struct Device: Decodable {
            let ctc: Ctc?
        }

        struct Ctc: Decodable {
            let info: Info?
        }

        struct Ipcam: Decodable {
            let rtspURL: String?

            private enum CodingKeys: String, CodingKey {
                case rtspURL = "rtsp_url"
            }
        }
    }

    struct TempInfo: Decodable {
        let nozzle: Double?
        let nozzleTarget: Double?
        let bed: Double?
        let bedTarget: Double?
        let chamber: Double?
        let chamberTarget: Double?

        private enum CodingKeys: String, CodingKey {
            case nozzle = "nozzle_temper"
            case nozzleTarget = "nozzle_target_temper"
            case bed = "bed_temper"
            case bedTarget = "bed_target_temper"
            case chamber = "chamber_temper"
            case chamberTarget = "chamber_target_temper"
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            nozzle = try container.decodeIfPresent(LossyDouble.self, forKey: .nozzle)?.value
            nozzleTarget = try container.decodeIfPresent(LossyDouble.self, forKey: .nozzleTarget)?.value
            bed = try container.decodeIfPresent(LossyDouble.self, forKey: .bed)?.value
            bedTarget = try container.decodeIfPresent(LossyDouble.self, forKey: .bedTarget)?.value
            chamber = try container.decodeIfPresent(LossyDouble.self, forKey: .chamber)?.value
            chamberTarget = try container.decodeIfPresent(LossyDouble.self, forKey: .chamberTarget)?.value
        }
    }

    struct LightsReport: Decodable {
        let chamberLight: Bool?

        private enum CodingKeys: String, CodingKey {
            case chamberLight = "chamber_light"
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            chamberLight = try container.decodeIfPresent(LossyBool.self, forKey: .chamberLight)?.value
        }
    }

    struct LightEntry: Decodable {
        let node: String?
        let mode: String?
    }

    let print: PrintInfo?
    let temp: TempInfo?
    let lightsReport: LightsReport?
    let lightsReportEntries: [LightEntry]?

    private enum CodingKeys: String, CodingKey {
        case print
        case temp
        case temperature
        case lights
        case lightsReport = "lights_report"
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        print = try container.decodeIfPresent(PrintInfo.self, forKey: .print)
        temp = try container.decodeIfPresent(TempInfo.self, forKey: .temp)
            ?? container.decodeIfPresent(TempInfo.self, forKey: .temperature)
        if let entries = try? container.decode([LightEntry].self, forKey: .lightsReport) {
            lightsReportEntries = entries
            lightsReport = nil
        } else {
            lightsReportEntries = nil
            lightsReport = try container.decodeIfPresent(LightsReport.self, forKey: .lightsReport)
                ?? container.decodeIfPresent(LightsReport.self, forKey: .lights)
        }
    }

    func toPatch() -> PrinterStatePatch {
        var patch = PrinterStatePatch()

        if let gcodeState = print?.gcodeState,
           let job = JobState.from(gcodeState: gcodeState) {
            patch.job = job
        }

        if let percent = print?.mcPercent {
            patch.progress01 = max(0.0, min(1.0, percent / 100.0))
        }

        if let nozzle = temp?.nozzle ?? print?.nozzleTemper {
            patch.nozzleC = nozzle
        }

        if let nozzleTarget = temp?.nozzleTarget ?? print?.nozzleTargetTemper {
            patch.nozzleTargetC = nozzleTarget
        }

        if let bed = temp?.bed ?? print?.bedTemper {
            patch.bedC = bed
        }

        if let bedTarget = temp?.bedTarget ?? print?.bedTargetTemper {
            patch.bedTargetC = bedTarget
        }

        if let chamber = temp?.chamber ?? print?.chamberTemper ?? print?.deviceChamberTemp ?? print?.infoTemp {
            patch.chamberC = chamber
        }

        if let chamberTarget = temp?.chamberTarget ?? print?.chamberTargetTemper {
            patch.chamberTargetC = chamberTarget
        }

        if let lightValue = lightsReport?.chamberLight {
            patch.lightOn = lightValue
        } else if let entries = lightsReportEntries ?? print?.lightsReportEntries {
            if let chamberEntry = entries.first(where: { $0.node == "chamber_light" }) {
                if let mode = chamberEntry.mode?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
                    if mode == "off" {
                        patch.lightOn = false
                    } else if mode == "on" || mode == "flashing" {
                        patch.lightOn = true
                    }
                }
            }
        }

        return patch
    }
}

private struct LossyDouble: Decodable {
    let value: Double?

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let double = try? container.decode(Double.self) {
            value = double
        } else if let int = try? container.decode(Int.self) {
            value = Double(int)
        } else if let string = try? container.decode(String.self), let double = Double(string) {
            value = double
        } else {
            value = nil
        }
    }
}

private struct LossyInt: Decodable {
    let value: Int?

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let int = try? container.decode(Int.self) {
            value = int
        } else if let double = try? container.decode(Double.self) {
            value = Int(double)
        } else if let string = try? container.decode(String.self), let int = Int(string) {
            value = int
        } else {
            value = nil
        }
    }
}

private struct LossyBool: Decodable {
    let value: Bool?

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let bool = try? container.decode(Bool.self) {
            value = bool
        } else if let int = try? container.decode(Int.self) {
            value = int != 0
        } else if let string = try? container.decode(String.self) {
            let lowered = string.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            if ["1", "true", "on", "yes"].contains(lowered) {
                value = true
            } else if ["0", "false", "off", "no"].contains(lowered) {
                value = false
            } else {
                value = nil
            }
        } else {
            value = nil
        }
    }
}
