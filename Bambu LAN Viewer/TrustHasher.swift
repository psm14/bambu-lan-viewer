//
//  TrustHasher.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/14/26.
//

import CryptoKit
import Foundation
import Security

enum TrustHasher {
    static func publicKeyHashBase64(from trust: SecTrust) -> String? {
        let certificates = SecTrustCopyCertificateChain(trust) as? [SecCertificate]
        guard let certificate = certificates?.first else { return nil }
        guard let publicKey = SecCertificateCopyKey(certificate) else { return nil }
        var error: Unmanaged<CFError>?
        guard let keyData = SecKeyCopyExternalRepresentation(publicKey, &error) as Data? else { return nil }
        let digest = SHA256.hash(data: keyData)
        return Data(digest).base64EncodedString()
    }
}
