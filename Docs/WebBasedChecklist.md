# implementation checklist

## phase 0: scaffolding

* [x] create `backend/` rust workspace (edition 2021)
* [x] add deps:

  * tokio
  * axum (http server)
  * serde + serde_json
  * tracing + tracing-subscriber
  * rumqttc (mqtt)
  * bytes
  * parking_lot or tokio::sync (state sharing)
* [x] create `frontend/` react app (vite)
* [x] add hls.js dependency for non-safari playback
* [x] add sqlite-backed printer registry (multi-printer support)
* [ ] add top-level `docker-compose.yml`
* [ ] add `cloudflared/` with template config or rely on tunnel token

## phase 1: mqtt status + commands (no video yet)

* [x] implement `backend/src/mqtt.rs`

  * connect to printer
  * subscribe report topic
  * parse relevant fields into `PrinterState`
  * store latest state in `Arc<RwLock<...>>` or a tokio watch channel
* [x] implement `backend/src/commands.rs`

  * request builder that always wraps `user_id`
  * implement:

    * pause/resume/stop (print.command)
    * light on/off (system.ledctrl with required fields)
* [x] implement `backend/src/http.rs` (axum routes)

  * `GET /api/printers`
  * `POST /api/printers`
  * `GET /api/printers/:id`
  * `PUT /api/printers/:id`
  * `DELETE /api/printers/:id`
  * `GET /api/printers/:id/status`
  * `POST /api/printers/:id/command`
  * `GET /healthz`
* [x] frontend:

  * subscribe to `/api/printers/:id/status/stream` (SSE) for updates
  * buttons call `/api/printers/:id/command`
  * add printer selector + add/edit drawer

## phase 2: rtsp ingest to access units (still no hls)

* [x] implement `backend/src/rtsp/` module

  * rtsp client (tcp interleaved)
  * sdp parsing (extract SPS/PPS if available)
  * rtp parsing + h264 depacketizer (FU-A, STAP-A, single NAL)
  * output `AccessUnit { pts90k: u64, is_idr: bool, nals: Vec<Vec<u8>> }` via async channel
  * [x] derive RTSP URL from MQTT report (`print.ipcam.rtsp_url`) with optional env override
* [ ] add a debug endpoint `GET /api/video_debug` returning:

  * fps, last pts, last idr time, etc.

## phase 3: hls packager (CMAF LL-HLS) + serving

* [x] implement minimal CMAF writer:

  * [x] `init.mp4` (ftyp + moov with avcC)
  * [x] fragment parts (`styp` + `moof` + `mdat`)
* [x] implement segmenter:

  * [x] maintain current segment buffer + start pts
  * [x] flush segment on (elapsed>=2s && is_idr)
  * [x] write `seg%06d.m4s` and update `stream.m3u8` atomically (write temp then rename)
  * [x] delete segments older than window
* [x] serve hls:

  * [x] `GET /hls/:id/stream.m3u8` from disk
  * [x] `GET /hls/:id/init.mp4` from disk
  * [x] `GET /hls/:id/:segment.m4s` from disk
  * [x] set content-types:

    * m3u8: `application/vnd.apple.mpegurl`
    * mp4/m4s: `video/mp4`
  * [x] disable caching headers (or very short) to avoid stale playlist issues through proxies

## phase 4: frontend video player

* [x] add `<video>` component
* [x] safari path:

  * if `video.canPlayType('application/vnd.apple.mpegurl')` is truthy, set `src=/hls/:id/stream.m3u8`
* [x] else use hls.js:

  * attach media
  * loadSource `/hls/:id/stream.m3u8`
* [x] add UI “reload video” button
* [x] show stale indicator if `/api/printers/:id/status.lastUpdate` old

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
* [ ] ensure websocket not required (hls + SSE is plain http)

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

## stretch goal: ll-hls (now available)

* [x] emit CMAF LL-HLS playlist (`/hls/:id/stream.m3u8`) with `init.mp4` + `.m4s`
