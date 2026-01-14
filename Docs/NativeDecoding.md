below is a concrete, ios-native, low-overhead plan that does **rtsp (over tcp interleaved)** → **rtp/h264 depacketize** → **videotoolbox decode** → **avsamplebufferdisplaylayer render**, with *no* vlc/gstreamer. the client itself handles tls termination, digest/basic auth, and keepalive inside the rtsp layer.

---

# 0) target architecture

### pipeline

```
rtsp client (tcp)  ──>  rtp interleaved ($) packets ──>  h264 depacketizer
         │                                              │
         └─ keepalive / control                          └─ nal units (annex-b or avcc)
                                                            │
                                                            v
                                                     videotoolbox decoder
                                                            │
                                                            v
                                               avsamplebufferdisplaylayer
```

### components

1. `RTSPClient` (network + protocol control)
2. `RTPInterleavedDemux` (splits `$`-framed rtp/rtcp from tcp stream)
3. `H264RTPDepacketizer` (rtp payload → NAL units, reassembly)
4. `H264DecoderVT` (VideoToolbox decode)
5. `VideoRenderer` (AVSampleBufferDisplayLayer + timing)

keep each component small and testable.

---

# 1) constraints & decisions

### use rtsp over tcp interleaved

* avoids udp issues on ios/vpn/vlan
* you parse a single tcp stream:

  * rtsp text responses (`\r\n\r\n` + optional body)
  * interleaved rtp/rtcp frames (`$` prefix)

### assume h264 baseline/main/high

* printer camera is h264, typically 1080p-ish
* decode via hardware (VideoToolbox)

### rendering choice

* use `AVSampleBufferDisplayLayer` (simpler than metal)
* you feed CMSampleBuffers with proper timing

---

# 2) RTSP client implementation plan (tcp, interleaved)

## 2.1 create a TCP connection

use `Network.framework`:

* `NWConnection(host:port:using: .tcp)`
* set `stateUpdateHandler`
* start on a background queue

provide:

* `send(_ bytes: Data)`
* `receiveLoop()` (continuous async reads, e.g., 4–16KB chunks)

## 2.2 minimal RTSP request/response engine

implement:

* CSeq incrementing
* session id capture
* response parsing (status line, headers, optional body)

### requests you need

1. `OPTIONS` (optional but useful)
2. `DESCRIBE` (get SDP)
3. `SETUP` (interleaved transport)
4. `PLAY`
5. keepalive: `OPTIONS` or `GET_PARAMETER` periodically
6. `TEARDOWN` on stop

### required headers

* `CSeq: <n>`
* `User-Agent: <your-app>`
* for SETUP/PLAY:

  * `Session: <id>` (after SETUP)
* for DESCRIBE:

  * `Accept: application/sdp`

### SETUP transport header (must request interleaved)

for video track:

* `Transport: RTP/AVP/TCP;unicast;interleaved=0-1`

(if the stream has audio too, you can ignore it initially; just choose the video `m=` entry from SDP.)

## 2.3 parsing RTSP responses while interleaved data is present

this is the “hard” part: after PLAY, server may send interleaved rtp/rtcp *between* rtsp responses.

write a single stream parser that can handle both:

### parsing algorithm

maintain a `Data` buffer.

while buffer not empty:

* if buffer[0] == `$`:

  * need at least 4 bytes
  * `$` (0x24), channel (1 byte), length (2 bytes big-endian)
  * need `4 + length` bytes total
  * emit `InterleavedPacket(channel, payload)`
  * remove from buffer
* else:

  * look for `\r\n\r\n` (end of headers)
  * parse status + headers
  * if `Content-Length` exists:

    * wait until you have headers + body
  * emit `RTSPResponse`

this single parser prevents “10s starvation” style bugs caused by mishandling `$` frames.

## 2.4 keepalive

many servers expect keepalive roughly every 10–30 seconds.

after PLAY:

* start a timer (e.g. 10s)
* send `OPTIONS` *or* `GET_PARAMETER` with the current `Session:`
* if you get 200 OK, keep going
* if you get error/timeout, reconnect

---

# 3) SDP parsing (get H264 config + control URL)

from DESCRIBE body (SDP):

* find the video `m=video` section
* parse:

  * `a=control:<track-url>`
  * `a=rtpmap:<pt> H264/90000`
  * `a=fmtp:<pt> ... sprop-parameter-sets=<base64 sps>,<base64 pps> ...`
  * optionally: `profile-level-id=...`

### output of SDP parser

* `videoControlURL` (absolute or relative to base RTSP URL)
* `payloadType` (pt)
* initial `sps`/`pps` (decoded bytes) if provided

store SPS/PPS because it lets you initialize VideoToolbox immediately and helps late joiners.

---

# 4) RTP interleaved demux

server will send `$` packets on channels:

* channel 0 = RTP (usually)
* channel 1 = RTCP

don’t assume; map based on SETUP response or your request:

* if you requested `interleaved=0-1`, treat 0 as RTP, 1 as RTCP

for phase 1:

* process RTP only
* ignore/pass-through RTCP (but still parse and discard so the stream stays aligned)

---

# 5) H264 RTP depacketizer

you need to turn RTP payloads into complete NAL units.

## 5.1 parse RTP header

RTP header is 12 bytes minimum:

* version/padding/extension/cc
* marker/payload-type
* sequence number
* timestamp
* ssrc
* csrc list (if cc>0)
* extension (if x=1)

extract:

* `seq` (UInt16)
* `ts` (UInt32)
* `payload` (Data slice)

store `rtpTimestamp` to convert to presentation time.

## 5.2 support H264 packetization modes

handle at least:

* **single nal unit** (1–23)
* **FU-A** (type 28) fragmentation (most common)
* optionally **STAP-A** (type 24) (less common)

### single nal

payload[0] nal header:

* nal_type = payload[0] & 0x1F
  emit one NAL = payload

### FU-A

payload layout:

* byte0 = FU indicator
* byte1 = FU header
* start bit (S), end bit (E)
* nal type = FU header & 0x1F
  reconstruct original nal header:
* original nal header = (FU indicator & 0xE0) | nal_type

reassembly state:

* keyed by (ssrc) or just single stream
* expect increasing seq numbers; if gap occurs, drop current fragment

on S:

* start a buffer: [original nal header] + payload[2...]
  on middle:
* append payload[2...]
  on E:
* append payload[2...], emit complete nal, clear buffer

### STAP-A

payload contains multiple NALs:

* skip first byte (stap-a header)
* then repeated:

  * 16-bit size
  * NAL bytes

emit each NAL.

## 5.3 access unit / frame boundaries

VideoToolbox prefers full frames (access units). easiest approach:

* build an “access unit” by grouping NALs until you hit an IDR/non-IDR boundary using RTP marker bit.

common practice:

* when RTP header `marker == 1`, treat it as end-of-access-unit and flush collected NALs as one sample.

so depacketizer outputs:

* `AccessUnit(nals: [Data], rtpTimestamp: UInt32)`

---

# 6) VideoToolbox decode (hardware)

## 6.1 create format description

you need `CMVideoFormatDescription` for H264.

inputs:

* SPS, PPS as raw NAL payloads (without start codes)
  sources:
* from SDP `sprop-parameter-sets`
* or from in-band NALs (type 7/8) seen in stream

build avcc “parameter sets”:

* store `sps`, `pps`
* call `CMVideoFormatDescriptionCreateFromH264ParameterSets`

## 6.2 create VTDecompressionSession

* output pixel format: `kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange` (NV12)
* callback provides decoded `CVImageBuffer`

configure for low latency:

* `kVTDecompressionPropertyKey_RealTime = true`
* `kVTDecompressionPropertyKey_ThreadCount` maybe 2

## 6.3 feed compressed samples

for each `AccessUnit`:

* convert NAL list to **AVCC** format for VT:

  * length-prefixed NAL units (4-byte big-endian length) concatenated
* create `CMBlockBuffer` / `CMSampleBuffer` with the format description
* set presentation timestamp based on RTP timestamp:

  * clock rate 90000
  * keep `firstRtpTimestamp` and `firstHostTime`
  * pts = (rtpTs - firstRtpTs) / 90000 seconds

then:

* `VTDecompressionSessionDecodeFrame`
* in callback, hand decoded frame to renderer

### keyframe handling

* wait until you have SPS/PPS and an IDR before starting decode
* if format changes (new SPS/PPS), recreate session

---

# 7) Rendering with AVSampleBufferDisplayLayer

## 7.1 setup

* `AVSampleBufferDisplayLayer`
* set:

  * `videoGravity = .resizeAspect`
* add as sublayer to a `UIView` backing your SwiftUI `UIViewRepresentable`

## 7.2 display decoded frames

you have two options:

### option A: bypass re-encode, display decoded CVPixelBuffer

create a `CMSampleBuffer` from the decoded image buffer and enqueue:

* `displayLayer.enqueue(sampleBuffer)`

this requires creating a `CMSampleBuffer` with:

* `CMVideoFormatDescriptionCreateForImageBuffer`
* `CMSampleBufferCreateReadyWithImageBuffer`

set its timing (`CMSampleTimingInfo`) to your computed PTS.

### option B: use `VTDecompressionSession` output callback with attachments

still enqueue as above; AVSampleBufferDisplayLayer wants sample buffers.

## 7.3 latency controls

* keep a small queue
* if you detect you’re falling behind (pts << now), drop frames:

  * only keep latest decoded frame
  * or drop until you reach near-real-time

---

# 8) Integration in your app

## 8.1 video engine actor

create an `actor VideoEngine`:

* `start(url: URL)` / `stop()`
* owns:

  * `RTSPClient`
  * `H264RTPDepacketizer`
  * `H264DecoderVT`
* emits:

  * decoded frames (callback or async stream)
  * connection state (connecting/playing/error)

## 8.2 swiftui wrapper

* `VideoView: UIViewRepresentable`
* internally contains a `UIView` with `AVSampleBufferDisplayLayer`

hook:

* when view appears, call `engine.attach(renderer:)`
* when state becomes connected, start
* on disappear/background, stop

---

# 9) Reconnect strategy

when stream fails:

* teardown connection
* exponential backoff (e.g., 0.5s, 1s, 2s, 4s max 10s)
* on reconnect:

  * redo DESCRIBE/SETUP/PLAY
  * reset depacketizer buffers
  * keep SPS/PPS cached but accept updates

---

# 10) Testing plan

## unit tests

* `RTSPParser`:

  * mixed rtsp headers + `$` interleaved frames
  * partial reads
* `RTPParser`:

  * header variations (cc, extension)
* `H264Depacketizer`:

  * single nal
  * FU-A start/middle/end
  * loss (missing seq) drops frame
  * STAP-A
* `TimestampMapper`:

  * rtp ts to pts monotonic

## integration tests (local)

* use a recorded rtp/h264 pcap converted into interleaved stream (or a small mock rtsp server) to validate end-to-end.

## on-device validation

* verify steady playback >5 minutes
* background/foreground stops cleanly
* reconnect works after toggling printer liveview / killing stream

---

# 11) Milestone checklist (Codex-friendly)

### milestone 1: RTSP control works

* [ ] NWConnection connect + read/write loop
* [ ] rtsp request builder (CSeq, headers)
* [ ] mixed stream parser (rtsp + `$`)
* [ ] DESCRIBE parses SDP, extracts video track control url, extracts SPS/PPS
* [ ] SETUP requests interleaved 0-1, captures Session
* [ ] PLAY starts, keepalive timer runs

### milestone 2: RTP/H264 depacketizer produces access units

* [ ] parse RTP header, extract seq/ts/marker/payload
* [ ] implement FU-A reassembly
* [ ] implement STAP-A (optional but recommended)
* [ ] group NALs into access units using marker bit
* [ ] surface SPS/PPS from in-band NALs if SDP absent

### milestone 3: VideoToolbox decoding works

* [ ] create CMVideoFormatDescription from SPS/PPS
* [ ] create VTDecompressionSession
* [ ] convert access unit to AVCC length-prefixed format
* [ ] decode frames, obtain CVPixelBuffer callback
* [ ] handle format change by recreating session

### milestone 4: Render in SwiftUI

* [ ] AVSampleBufferDisplayLayer setup in UIViewRepresentable
* [ ] create CMSampleBuffer from CVPixelBuffer and enqueue
* [ ] drop frames when behind to keep low latency
* [ ] stop/start on lifecycle events

### milestone 5: resiliency

* [ ] keepalive works (OPTIONS or GET_PARAMETER)
* [ ] reconnect loop on timeout/error
* [ ] guard against partial reads and parser resync
* [ ] metrics/logging: fps, decode time, last packet time

---

# 12) Notes on tls/auth in the client

with the proxy removed, the rtsp client is responsible for:

* tls termination (accept self-signed as needed)
* digest/basic auth injection
* keepalive requests (OPTIONS or GET_PARAMETER)

this plan avoids any heavyweight third-party frameworks and should keep your app size sane while giving you better control/latency than hls.

if you want, paste one SDP from your stream (DESCRIBE response body) and a sample RTP packetization pattern you’re seeing (FU-A vs single NAL), and i’ll tailor the depacketizer + timestamp mapping details to match it exactly.
