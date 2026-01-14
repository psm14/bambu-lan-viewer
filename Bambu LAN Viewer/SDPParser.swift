//
//  SDPParser.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/14/26.
//

import Foundation

struct SDPInfo {
    let videoControl: String?
    let sessionControl: String?
    let payloadType: Int?
    let sps: Data?
    let pps: Data?

    func resolvedVideoControlURL(baseURL: URL) -> String {
        if let videoControl {
            return resolve(control: videoControl, baseURL: baseURL)
        }
        return baseURL.absoluteString
    }

    func resolvedPlayURL(baseURL: URL) -> String {
        if let sessionControl, sessionControl != "*" {
            return resolve(control: sessionControl, baseURL: baseURL)
        }
        return baseURL.absoluteString
    }

    private func resolve(control: String, baseURL: URL) -> String {
        let lower = control.lowercased()
        if lower.hasPrefix("rtsp://") || lower.hasPrefix("rtsps://") {
            return control
        }
        if control == "*" {
            return baseURL.absoluteString
        }
        if let url = URL(string: control, relativeTo: baseURL) {
            return url.absoluteString
        }
        return baseURL.absoluteString
    }
}

enum SDPParser {
    static func parse(_ data: Data) -> SDPInfo? {
        let text = String(decoding: data, as: UTF8.self)
        let lines = text.split(whereSeparator: { $0 == "\n" || $0 == "\r" })

        var sessionControl: String?
        var videoControl: String?
        var payloadType: Int?
        var sps: Data?
        var pps: Data?
        var inVideo = false

        for lineSub in lines {
            let line = lineSub.trimmingCharacters(in: .whitespaces)
            if line.hasPrefix("m=") {
                inVideo = line.lowercased().hasPrefix("m=video")
                if inVideo {
                    let parts = line.split(separator: " ")
                    if parts.count >= 4, let pt = Int(parts[3]) {
                        payloadType = pt
                    }
                }
                continue
            }

            if line.hasPrefix("a=control:") {
                let value = String(line.dropFirst("a=control:".count))
                if inVideo {
                    videoControl = value
                } else {
                    sessionControl = value
                }
                continue
            }

            if inVideo, line.hasPrefix("a=rtpmap:") {
                let value = String(line.dropFirst("a=rtpmap:".count))
                let parts = value.split(separator: " ")
                if parts.count >= 2, parts[1].uppercased().hasPrefix("H264") {
                    payloadType = Int(parts[0])
                }
                continue
            }

            if inVideo, line.hasPrefix("a=fmtp:") {
                let value = String(line.dropFirst("a=fmtp:".count))
                let parts = value.split(separator: " ", maxSplits: 1, omittingEmptySubsequences: true)
                guard parts.count == 2 else { continue }
                let params = parts[1].split(separator: ";")
                for param in params {
                    let pair = param.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: true)
                    guard pair.count == 2 else { continue }
                    let key = pair[0].trimmingCharacters(in: .whitespacesAndNewlines)
                    let val = pair[1].trimmingCharacters(in: .whitespacesAndNewlines)
                    if key == "sprop-parameter-sets" {
                        let sets = val.split(separator: ",")
                        if sets.count >= 1 {
                            sps = Data(base64Encoded: String(sets[0]))
                        }
                        if sets.count >= 2 {
                            pps = Data(base64Encoded: String(sets[1]))
                        }
                    }
                }
                continue
            }
        }

        return SDPInfo(videoControl: videoControl, sessionControl: sessionControl, payloadType: payloadType, sps: sps, pps: pps)
    }
}
