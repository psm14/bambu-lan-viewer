## tdd: “bambu cam + status” web service (rtsp → hls) with tokio backend + react frontend

### goal

a small self-hosted web app that:

* connects to your bambu printers over lan/vpn
* reads print status + controls over mqtt
* ingests the camera stream via rtsp/rtsps
* republishes video as **LL-HLS (CMAF)** for browser playback
* runs behind cloudflare zero trust (auth handled externally)
* deploys via docker-compose: backend, frontend (static), cloudflared

---

## 1) repository layout (inside your existing ios app directory)

```
/<ios-app-root>/
  ios/                    (existing)
  backend/                (new: rust)
  frontend/               (new: react)
  docker-compose.yml      (new)
  cloudflared/            (new: config template)
  .env                    (gitignored; tunnel token)
```

---

## 2) architecture overview

### backend responsibilities (rust, tokio)

* printer registry:

  * sqlite-backed list of printer configs (name, ip, serial, access code)
  * per-printer workers for mqtt + rtsp + hls
* mqtt client:

  * connect to printer (ip, serial, lan access code)
  * subscribe to `device/<serial>/report`
  * publish to `device/<serial>/request` for commands
* rtsp client:

  * connect to camera stream (likely rtsps/digest)
  * bootstrap the RTSP URL from `print.ipcam.rtsp_url` in MQTT reports (override via env if needed)
  * receive rtp/h264 access units
* hls packager:

  * segment into **CMAF fMP4** (init + `.m4s` fragments)
  * maintain rolling window playlist `stream.m3u8`
  * serve `stream.m3u8` + `init.mp4` + `segNNNN.m4s`
* http api (axum):

  * `GET /api/printers` (list printers)
  * `POST /api/printers` (create printer)
  * `GET /api/printers/:id` (fetch printer)
  * `PUT /api/printers/:id` (update printer)
  * `DELETE /api/printers/:id` (remove printer)
  * `GET /api/printers/:id/status` (latest printer state)
  * `GET /api/printers/:id/status/stream` (server-sent events status stream)
  * `POST /api/printers/:id/command` (pause/resume/stop/light)
  * `GET /hls/:id/stream.m3u8`
  * `GET /hls/:id/init.mp4`
  * `GET /hls/:id/segXXXX.m4s`
  * `GET /healthz`

### frontend responsibilities (react)

* show status and controls
* manage printers list (add/edit/switch) via `/api/printers`
* play video:

* safari/ios: native `<video src="/hls/:id/stream.m3u8">`
  * chrome/firefox/edge: use **hls.js** (mse)

### deployment

* backend container exposes http (e.g. 8080) on an internal docker network
* frontend container serves static build (e.g. nginx/caddy) on internal network
* cloudflared tunnels external hostnames to these internal services
* no app-level auth (rely on zero trust)

### configuration

* `DATABASE_URL` (or `DB_PATH`): sqlite path for printer configs (default `data/printers.db`)
  * container-friendly example: `sqlite:///data/printers.db`
* `HLS_OUTPUT_DIR`: base directory; each printer writes to `HLS_OUTPUT_DIR/<printerId>/`
* `HLS_PART_DURATION_SECS` (default `0.333`)

---

## 3) hls strategy (CMAF LL-HLS)

### segmenting rules

* segment duration target: **2 seconds** (start here; 1s is fine later)
* cut segments **only at IDR keyframes** so clients can join quickly
* keep a rolling playlist window of ~6–10 segments (12–20 seconds)
* emit CMAF parts (≈0.333s each) inside every segment for low latency

### file storage

**in-memory segment store**

* store bytes in a ring buffer keyed by sequence number
* serve from memory
* more code, but no filesystem concerns

start with **disk**. it’s robust and easy.

### playlist format

* `#EXTM3U`
* `#EXT-X-VERSION:9`
* `#EXT-X-TARGETDURATION:2` (rounded up)
* `#EXT-X-MEDIA-SEQUENCE:<first_seq_in_window>`
* `#EXT-X-PART-INF:PART-TARGET=<max_part_duration>`
* `#EXT-X-SERVER-CONTROL:CAN-BLOCK-RELOAD=YES,...`
* `#EXT-X-MAP:URI="init.mp4"`
* for each segment:

  * `#EXT-X-PART:DURATION=...,URI="seg000123.m4s",BYTERANGE="..."` (one per part)
  * `#EXTINF:<duration>,`
  * `seg000123.m4s`

### client buffering expectations

* safari is forgiving, but wants consistent playlist + segment availability
* hls.js wants cors ok (same origin here so easy)

---

## 4) rtsp ingest options (choose one)

### implement full rtsps + digest in rust

* use the existing Swift RTSPS implementation as a reference to implement the Rust version

---

## 5) mqtt command shape (important)

you already discovered commands require a top-level `user_id` wrapper (at least for ledctrl). treat this as a general rule for control commands.

backend should:

* implement a request builder that always emits:

  * `{"user_id":"1", "<namespace>": {...}}`
* keep `sequence_id` as a string and monotonic per connection

---

## 6) api contract

### printers

`GET /api/printers`

```json
[
  {
    "id": 1,
    "name": "Studio X1",
    "host": "192.168.1.10",
    "serial": "00M1234ABC",
    "accessCode": "12345678",
    "rtspUrl": null
  }
]
```

`POST /api/printers`

```json
{
  "name": "Studio X1",
  "host": "192.168.1.10",
  "serial": "00M1234ABC",
  "accessCode": "12345678",
  "rtspUrl": "rtsps://..."
}
```

`GET /api/printers/:id`

`PUT /api/printers/:id` (same shape as POST; `accessCode` optional to keep current)

`DELETE /api/printers/:id` → `204 No Content`

### status

`GET /api/printers/:id/status`

```json
{
  "connected": true,
  "jobState": "RUNNING",
  "percent": 42,
  "remainingMinutes": 123,
  "nozzleC": 215.3,
  "bedC": 60.0,
  "chamberC": 38.2,
  "light": "off",
  "lastUpdate": "2026-01-18T20:15:00Z"
}
```

`GET /api/printers/:id/status/stream` (server-sent events)

* event: `status`
* data: JSON payload matching `GET /api/printers/:id/status`

### commands

`POST /api/printers/:id/command`

```json
{ "type": "pause" }
```

```json
{ "type": "light", "on": false }
```

responses:

* 200 with `{ "ok": true }`
* 4xx/5xx with `{ "ok": false, "error": "..." }`

### video

* `GET /hls/:id/stream.m3u8`
* `GET /hls/:id/init.mp4`
* `GET /hls/:id/seg000123.m4s`

---

## 7) docker-compose topology

* `backend`:

  * exposes `8080` internally
  * volume mount `./data:/data` (sqlite db at `/data/printers.db`, hls at `/data/hls`)
* `frontend`:

  * serves build on `8080` internally
  * reverse proxy to backend `/api` and `/hls` (optional) OR let cloudflared route multiple services
* `cloudflared`:

  * `TUNNEL_TOKEN` in `.env` (gitignored)
  * routes hostnames to `http://frontend:8080` and `http://backend:8080`
