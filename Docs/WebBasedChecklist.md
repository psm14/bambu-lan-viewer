# implementation checklist

## phase 0: scaffolding

* [ ] create `backend/` rust workspace (edition 2021)
* [ ] add deps:

  * tokio
  * axum (http server)
  * serde + serde_json
  * tracing + tracing-subscriber
  * rumqttc (mqtt)
  * bytes
  * parking_lot or tokio::sync (state sharing)
* [ ] create `frontend/` react app (vite)
* [ ] add hls.js dependency for non-safari playback
* [ ] add top-level `docker-compose.yml`
* [ ] add `cloudflared/` with template config or rely on tunnel token

## phase 1: mqtt status + commands (no video yet)

* [ ] implement `backend/src/mqtt.rs`

  * connect to printer
  * subscribe report topic
  * parse relevant fields into `PrinterState`
  * store latest state in `Arc<RwLock<...>>` or a tokio watch channel
* [ ] implement `backend/src/commands.rs`

  * request builder that always wraps `user_id`
  * implement:

    * pause/resume/stop (print.command)
    * light on/off (system.ledctrl with required fields)
* [ ] implement `backend/src/http.rs` (axum routes)

  * `GET /api/status`
  * `POST /api/command`
  * `GET /healthz`
* [ ] frontend:

  * poll `/api/status` every 2–5s
  * buttons call `/api/command`

## phase 2: rtsp ingest to access units (still no hls)

* [ ] implement `backend/src/rtsp/` module

  * rtsp client (tcp interleaved)
  * sdp parsing (extract SPS/PPS if available)
  * rtp parsing + h264 depacketizer (FU-A, STAP-A, single NAL)
  * output `AccessUnit { pts90k: u64, is_idr: bool, nals: Vec<Vec<u8>> }` via async channel
* [ ] add a debug endpoint `GET /api/video_debug` returning:

  * fps, last pts, last idr time, etc.

## phase 3: hls packager (mpeg-ts) + serving

* [ ] implement minimal mpeg-ts muxer:

  * [ ] PAT/PMT generation
  * [ ] PES packetization for H264
  * [ ] PTS stamping
  * [ ] TS packetization (188-byte packets)
* [ ] implement segmenter:

  * [ ] maintain current segment buffer + start pts
  * [ ] flush segment on (elapsed>=2s && is_idr)
  * [ ] write `seg%06d.ts`
  * [ ] update `stream.m3u8` atomically (write temp then rename)
  * [ ] delete segments older than window
* [ ] serve hls:

  * [ ] `GET /hls/stream.m3u8` from disk
  * [ ] `GET /hls/:segment.ts` from disk
  * [ ] set content-types:

    * m3u8: `application/vnd.apple.mpegurl`
    * ts: `video/mp2t`
  * [ ] disable caching headers (or very short) to avoid stale playlist issues through proxies

## phase 4: frontend video player

* [ ] add `<video>` component
* [ ] safari path:

  * if `video.canPlayType('application/vnd.apple.mpegurl')` is truthy, set `src=/hls/stream.m3u8`
* [ ] else use hls.js:

  * attach media
  * loadSource `/hls/stream.m3u8`
* [ ] add UI toggle “compatibility mode” / “reload video”
* [ ] show stale indicator if `/api/status.lastUpdate` old

## phase 5: docker-compose + cloudflared

* [ ] `docker-compose.yml`:

  * backend build
  * frontend build (static)
  * cloudflared uses `.env` for `TUNNEL_TOKEN`
  * shared internal network
* [ ] cloudflare zero trust:

  * route hostname to frontend
  * route `/api` and `/hls` either via:

    * frontend reverse proxy (nginx) OR
    * separate hostname to backend
* [ ] ensure websocket not required (hls + polling is plain http)

## phase 6: hardening / polish

* [ ] watchdog reconnection loops for mqtt + rtsp
* [ ] metrics: segment creation rate, current latency estimate
* [ ] handle multiple clients:

  * hls is naturally fan-out but ensure segmenter keeps up
* [ ] optional: reduce latency:

  * 1s segments
  * smaller playlist window
* [ ] optional: notifications:

  * backend can send push/email/etc later (outside scope here)

---

## stretch goal: ll-hls (later)

* switch to fmp4/cmaf muxing and `#EXT-X-PART`
* increased complexity; do only after regular hls is stable
