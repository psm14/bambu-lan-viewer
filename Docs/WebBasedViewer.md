## tdd: “bambu cam + status” web service (rtsp → hls) with tokio backend + react frontend

### goal

a small self-hosted web app that:

* connects to your bambu printer over lan/vpn
* reads print status + controls over mqtt
* ingests the camera stream via rtsp/rtsps
* republishes video as **regular hls** (mpeg-ts) for browser playback
* runs behind cloudflare zero trust (auth handled externally)
* deploys via docker-compose: backend, frontend (static), cloudflared

stretch goal later: ll-hls.

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

* mqtt client:

  * connect to printer (ip, serial, lan access code)
  * subscribe to `device/<serial>/report`
  * publish to `device/<serial>/request` for commands
* rtsp client:

  * connect to camera stream (likely rtsps/digest; you can keep your existing “strip tls/digest” proxy as a separate module or integrate directly)
  * receive rtp/h264 access units
* hls packager:

  * segment into **mpeg-ts** files (1–2s each)
  * maintain rolling window playlist `stream.m3u8`
  * serve `stream.m3u8` + `segNNNN.ts`
* http api (axum):

  * `GET /api/status` (latest printer state)
  * `POST /api/command` (pause/resume/stop/light)
  * `GET /hls/stream.m3u8`
  * `GET /hls/segXXXX.ts`
  * `GET /healthz`

### frontend responsibilities (react)

* show status and controls
* play video:

  * safari/ios: native `<video src="/hls/stream.m3u8">`
  * chrome/firefox/edge: use **hls.js** (mse)

### deployment

* backend container exposes http (e.g. 8080) on an internal docker network
* frontend container serves static build (e.g. nginx/caddy) on internal network
* cloudflared tunnels external hostnames to these internal services
* no app-level auth (rely on zero trust)

---

## 3) hls strategy (phase 1: regular hls mpeg-ts)

### segmenting rules

* segment duration target: **2 seconds** (start here; 1s is fine later)
* cut segments **only at IDR keyframes** so clients can join quickly
* keep a rolling playlist window of ~6–10 segments (12–20 seconds)

### file storage

**in-memory segment store**

* store bytes in a ring buffer keyed by sequence number
* serve from memory
* more code, but no filesystem concerns

start with **disk**. it’s robust and easy.

### playlist format

* `#EXTM3U`
* `#EXT-X-VERSION:3`
* `#EXT-X-TARGETDURATION:2` (rounded up)
* `#EXT-X-MEDIA-SEQUENCE:<first_seq_in_window>`
* for each segment:

  * `#EXTINF:<duration>,`
  * `seg000123.ts`

### client buffering expectations

* safari is forgiving, but wants consistent playlist + segment availability
* hls.js wants cors ok (same origin here so easy)

---

## 4) rtsp ingest options (choose one)

### implement full rtsps + digest in rust

* use the existing Swift RTSPS implementation as a reference to implement the Rust version

---

## 5) mpeg-ts muxing (what you need to implement)

if you’re doing this bespoke, you need a minimal TS muxer for h264:

### minimum TS features

* PAT + PMT tables
* one video PID
* PES packetization of h264 access units
* PTS (90kHz clock) monotonic
* TS packetization into 188-byte packets

### h264 formatting

* ensure SPS/PPS appear regularly:

  * at least at segment boundaries, ideally before each IDR in a new segment
* you can:

  * carry SPS/PPS from SDP (`sprop-parameter-sets`) and inject them at start of each segment
  * or observe in-band NALs and cache latest SPS/PPS

### timestamping

* base PTS on RTP timestamps when available
* if you lose RTP timestamp continuity, fall back to local clock and keep monotonic PTS (players hate backwards time)

### segment cutting

* maintain `current_segment_bytes`
* when:

  * elapsed >= target_duration AND
  * the current access unit contains an IDR (NAL type 5)
    → finalize segment and start a new one

---

## 6) mqtt command shape (important)

you already discovered commands require a top-level `user_id` wrapper (at least for ledctrl). treat this as a general rule for control commands.

backend should:

* implement a request builder that always emits:

  * `{"user_id":"1", "<namespace>": {...}}`
* keep `sequence_id` as a string and monotonic per connection

---

## 7) api contract

### status

`GET /api/status`

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

### commands

`POST /api/command`

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

* `GET /hls/stream.m3u8`
* `GET /hls/seg000123.ts`

---

## 8) docker-compose topology

* `backend`:

  * exposes `8080` internally
  * volume mount `./data/hls:/data/hls` (or a docker volume)
* `frontend`:

  * serves build on `8080` internally
  * reverse proxy to backend `/api` and `/hls` (optional) OR let cloudflared route multiple services
* `cloudflared`:

  * `TUNNEL_TOKEN` in `.env` (gitignored)
  * routes hostnames to `http://frontend:8080` and `http://backend:8080`
