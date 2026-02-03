# Design: Bespoke Low-Latency Video Streaming via Chunked CMAF over HTTP

## Overview

This document describes a **bespoke, low-latency video streaming design** for a personal, low-fan-out system (1–5 clients) that:

- Ingests an RTSP (H.264) camera stream
- Remuxes it into CMAF/fMP4 fragments
- Streams fragments over a **single long-lived HTTP response** using **chunked transfer encoding**
- Plays back in browsers using **(Managed) MediaSource Extensions**
- Falls back to **HLS / LL-HLS** for maximum compatibility

This design avoids WebSockets and WebRTC while achieving **lower latency than LL-HLS** in practice, especially behind Cloudflare Tunnel (`cloudflared`).

Target environment:
- iOS 26+ Safari (ManagedMediaSource available)
- Desktop Chrome / Firefox / Edge
- Deployment via Docker Compose behind Cloudflare Zero Trust
- Personal use (no horizontal scalability requirements)

---

## Goals

- Sub-3s end-to-end latency when possible
- Works reliably through HTTP-only tunnels/proxies (Cloudflare Tunnel)
- Minimal moving parts (no TURN, no UDP)
- Fully hardware-decoded playback in the browser
- Clean fallback path (HLS / LL-HLS)

Non-goals:
- Massive fan-out scalability
- DVR / rewind / recording
- Legacy iOS (<26) support
- Standards-perfect LL-HLS compliance (stretch goal only)

---

## High-Level Architecture

```

RTSP Camera
│
│  (RTSP over TCP, interleaved)
▼
RTSP Client (Tokio)
│
│  Access Units (H.264)
▼
CMAF Muxer (fMP4)
│
│  Init Segment + Fragments
▼
HTTP Chunked Stream
│
│  (Cloudflared Tunnel)
▼
Browser Fetch Stream
│
│  (Managed) MediaSource
▼ <video> (Hardware Decode)

```

---

## Transport Choice Rationale

### Why Not WebRTC?
- Requires UDP or TURN for reliable internet access
- Cloudflare Tunnel is HTTP-first; WebRTC is awkward behind it
- More signaling, ICE, and NAT complexity than desired

### Why Not WebSockets?
- Often works, but:
  - More fragile through proxies/CDNs
  - Upgrade semantics sometimes flaky behind tunnels
- HTTP chunked streaming is simpler and more proxy-friendly

### Why HTTP Chunked?
- Plain HTTP/1.1 streaming
- No protocol upgrade required
- Cloudflare Tunnel handles it cleanly
- Works with `fetch()` + `ReadableStream`

---

## Stream Format

### Endpoint

```

GET /api/video/cmaf

```

### Response Headers

```

Content-Type: video/mp4
Cache-Control: no-store
Connection: keep-alive
Transfer-Encoding: chunked

```

### Body Structure

The response body is an **infinite stream** of length-prefixed MP4 fragments.

```

[ u32be length ][ init segment bytes ]
[ u32be length ][ moof+mdat fragment ]
[ u32be length ][ moof+mdat fragment ]
...

````

**Why length-prefixed framing?**
- Avoids parsing MP4 box boundaries from a byte stream
- Client can trivially buffer + append
- Robust against partial TCP reads

---

## CMAF / fMP4 Details

### Init Segment
- `ftyp` + `moov`
- Contains:
  - AVC codec configuration
  - SPS / PPS
  - Timescale
- Sent **once per connection**

### Media Fragments
- Each fragment is:
  - `moof` + `mdat`
- Fragment duration:
  - Target: **100–300 ms**
- Cut rules:
  - Prefer IDR (keyframe) boundaries
  - Ensure periodic SPS/PPS availability

### Timestamping
- Use RTP timestamps when available (90 kHz clock)
- Maintain monotonic PTS
- Avoid backward time jumps (Safari will stall)

---

## Server-Side Design (Rust + Tokio)

### Streaming Handler Skeleton

```rust
async fn stream_cmaf(req: Request) -> impl IntoResponse {
    let (tx, body) = hyper::Body::channel();

    tokio::spawn(async move {
        // 1. Send init segment
        send_chunk(&tx, init_segment).await;

        // 2. Stream fragments
        while let Some(fragment) = fragment_rx.recv().await {
            send_chunk(&tx, fragment).await;
        }
    });

    Response::builder()
        .header("Content-Type", "video/mp4")
        .header("Cache-Control", "no-store")
        .body(body)
}
````

Each `send_chunk` writes:

```
[u32be length][payload]
```

---

## Client-Side Design (React)

### Capability Detection

```js
const supportsMMS = !!window.ManagedMediaSource;
const supportsHLS = video.canPlayType("application/vnd.apple.mpegurl");
```

Selection logic:

1. Prefer ManagedMediaSource + CMAF stream
2. Fallback to native HLS / LL-HLS
3. Optional manual toggle in UI

---

## ManagedMediaSource Playback Flow

```js
const mediaSource = new ManagedMediaSource();
video.src = URL.createObjectURL(mediaSource);

mediaSource.addEventListener("sourceopen", async () => {
  const sb = mediaSource.addSourceBuffer(
    'video/mp4; codecs="avc1.42E01E"'
  );

  const res = await fetch("/api/video/cmaf", { cache: "no-store" });
  const reader = res.body.getReader();

  let buffer = new Uint8Array();

  while (true) {
    const { value, done } = await reader.read();
    if (done) break;

    buffer = concat(buffer, value);

    while (buffer.length >= 4) {
      const len = readU32BE(buffer);
      if (buffer.length < 4 + len) break;

      const chunk = buffer.slice(4, 4 + len);
      buffer = buffer.slice(4 + len);

      await appendWithBackpressure(sb, chunk);
    }
  }
});
```

### Backpressure & Latency Control

* Append only when `sourceBuffer.updating === false`
* Keep buffered range small:

  * Target: **1–2 seconds**
* Periodically trim old data:

  ```js
  if (video.buffered.end(0) - video.currentTime > 2.0) {
    sb.remove(0, video.currentTime - 0.5);
  }
  ```

---

## iOS / Safari Caveats

### ManagedMediaSource Requirements

* Exists on iOS 26+ Safari
* May silently fail unless an **AirPlay alternative** is available
  * Easiest fix: keep a native HLS URL available as fallback
* Avoid aggressive buffering; Safari enforces safety margins

### Known Behavior

* Lower latency than LL-HLS in many cases
* Still not guaranteed sub-second; ~1–3s is realistic
* Must handle silent stalls gracefully (fallback button)

---

## Cloudflare Tunnel Compatibility

* Chunked HTTP streaming works well
* No WebSocket upgrade required
* Avoid extremely tiny fragments (<50 ms)
* Disable caching headers explicitly

---

## Fallback Strategy

Always keep **HLS / LL-HLS** available:

* Native HLS for iOS Safari
* hls.js for desktop browsers
* UI option: “Switch to compatibility mode”

This ensures:

* Playback never hard-fails
* Debugging is easier
* You can iterate on bespoke path safely

---

## Trade-Off Summary

| Approach           | Latency | Compatibility | Complexity |
| ------------------ | ------- | ------------- | ---------- |
| WebRTC             | ★★★★☆   | Medium        | High       |
| LL-HLS             | ★★☆☆☆   | High          | Medium     |
| Chunked CMAF + MMS | ★★★☆☆   | Medium-High   | Medium     |
| Regular HLS        | ★☆☆☆☆   | Very High     | Low        |

---

## Stretch Goals

* Adaptive fragment duration based on RTT
* Switch to LL-HLS parts using same CMAF muxer
* Unified client abstraction for HLS / CMAF / WebRTC
* Optional auth token binding per stream

---

## Conclusion

For a **personal, low-fan-out system**, chunked CMAF over HTTP + ManagedMediaSource is a sweet spot:

* Lower latency than LL-HLS
* Far simpler than WebRTC
* Proxy-friendly
* Fully hardware-decoded

Keep HLS as a safety net, and you get the best of both worlds without turning your printer cam into a media startup.
