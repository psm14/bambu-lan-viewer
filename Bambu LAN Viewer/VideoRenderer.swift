//
//  VideoRenderer.swift
//  Bambu LAN Viewer
//
//  Created by Patrick McLaughlin on 1/14/26.
//

import AVFoundation
import Foundation
import UIKit

final class VideoRenderer {
    private let displayLayer: AVSampleBufferDisplayLayer
    private let renderQueue = DispatchQueue(label: "com.bambu.lanviewer.video-render")

    init(displayLayer: AVSampleBufferDisplayLayer) {
        self.displayLayer = displayLayer
        self.displayLayer.videoGravity = .resizeAspect
    }

    func enqueue(imageBuffer: CVImageBuffer, pts: CMTime) {
        renderQueue.async { [weak self] in
            guard let self else { return }
            let renderer = self.displayLayer.sampleBufferRenderer
            guard renderer.isReadyForMoreMediaData else { return }

            var formatDescription: CMVideoFormatDescription?
            let status = CMVideoFormatDescriptionCreateForImageBuffer(
                allocator: kCFAllocatorDefault,
                imageBuffer: imageBuffer,
                formatDescriptionOut: &formatDescription
            )
            guard status == noErr, let formatDescription else { return }

            var timing = CMSampleTimingInfo(duration: .invalid, presentationTimeStamp: pts, decodeTimeStamp: .invalid)
            var sampleBuffer: CMSampleBuffer?
            let sampleStatus = CMSampleBufferCreateReadyWithImageBuffer(
                allocator: kCFAllocatorDefault,
                imageBuffer: imageBuffer,
                formatDescription: formatDescription,
                sampleTiming: &timing,
                sampleBufferOut: &sampleBuffer
            )
            guard sampleStatus == noErr, let sampleBuffer else { return }

            renderer.enqueue(sampleBuffer)
        }
    }

    func reset() {
        renderQueue.async { [weak self] in
            guard let self else { return }
            let renderer = self.displayLayer.sampleBufferRenderer
            renderer.flush(removingDisplayedImage: true, completionHandler: nil)
        }
    }
}

final class VideoRenderView: UIView {
    override class var layerClass: AnyClass {
        AVSampleBufferDisplayLayer.self
    }

    var displayLayer: AVSampleBufferDisplayLayer {
        guard let layer = layer as? AVSampleBufferDisplayLayer else {
            return AVSampleBufferDisplayLayer()
        }
        return layer
    }

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .black
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        backgroundColor = .black
    }
}
