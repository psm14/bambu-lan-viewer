# Bambu LAN Viewer

Self-hosted web app for Bambu printers on LAN/VPN. The backend connects to the printer over MQTT and RTSP(S), repackages video as CMAF fragments over WebSocket (MSE), and exposes a REST/SSE API. The frontend is a React app for status, controls, and live video.

**Local Development**
Prerequisites: Rust toolchain, Node.js, and npm.

1. Start the backend.
```bash
cd backend
cargo run
```
2. Start the frontend (point it at the backend).
```bash
cd frontend
npm install
VITE_API_BASE=http://localhost:8080 npm run dev
```
3. Open the UI at `http://localhost:5173`.

If you prefer, set `VITE_API_BASE` in `frontend/.env.local` instead of in the command line.

**Production (Docker Compose)**
This compose setup is designed to run behind a Cloudflare Tunnel and Cloudflare Access.

1. Configure `.cloudflared.env` with your Cloudflare values.
2. Build and start the stack.
```bash
docker compose up -d --build
```
3. Access the UI via the hostname(s) configured in your Cloudflare Tunnel.

Notes:
`docker-compose.yml` does not publish ports to the host. If you want direct local access without Cloudflare, add port mappings and set `CF_ACCESS_ENABLED=false` in `docker-compose.yml`.

**Cloudflare Tunnel Routing**
Configure your tunnel to send API traffic (including WebSockets) to the backend container, and everything else to the frontend.

Example `cloudflared` config (ingress order matters; catchall last):
```yaml
ingress:
  - hostname: app.example.com
    path: /api/*
    service: http://backend:8080
  - hostname: app.example.com
    service: http://frontend:80
  - service: http_status:404
```

If you configure this in the Cloudflare dashboard instead of a config file, create path-based routes with the same mapping and ensure the catchall (`/*`) points to `http://frontend:80`.

**Configuration**
Backend (selected):
- `DATABASE_URL` or `DB_PATH`: SQLite path. Default is `data/printers.db` (relative to the backend working directory).
- `HTTP_BIND`: HTTP listen address. Default `0.0.0.0:8080`.
- `CMAF_OUTPUT_DIR`: Output directory for CMAF scratch files when `CMAF_WRITE_FILES=true`. Default `cmaf`.
- `CMAF_TARGET_DURATION_SECS`: CMAF segment target duration. Default `2.0`.
- `CMAF_WINDOW_SEGMENTS`: CMAF segment window size. Default `6`.
- `CMAF_PART_DURATION_SECS`: CMAF fragment duration. Default `0.333`.
- `CMAF_WS_BACKLOG_SECS`: CMAF backlog seconds sent on WS connect. Default `3.0`.
- `CMAF_WRITE_FILES`: Write CMAF files/playlist to disk for debugging. Default `false`.
- `CF_ACCESS_ENABLED`: Enable Cloudflare Access enforcement. Default `false` (set to `true` in `docker-compose.yml`).
- `CF_ACCESS_TEAM_DOMAIN`: Cloudflare Access team domain (used to derive JWKS/issuer).
- `CF_ACCESS_JWKS_URL`: Override JWKS URL (optional).
- `CF_ACCESS_AUD`: Access application audience (optional but recommended).
- `CF_ACCESS_ISSUER`: Override issuer (optional).
- `CF_ACCESS_DEV_USER_EMAIL`: Placeholder user email when Access is disabled. Default `admin@local`.

Frontend:
- `VITE_API_BASE`: Base URL for API calls. Leave empty when frontend and backend share the same origin.

For the full backend configuration list, see `backend/server/src/config.rs`.
