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
Three production deployment methods are included:
1. Cloudflare Tunnel + Cloudflare Access (`docker-compose.yml`)
2. Tailscale Serve (`docker-compose.tailscale.yml`)
3. Generic reverse proxy / direct edge (`docker-compose.generic.yml`)

Cloudflare and Tailscale stacks keep host ports closed by default. The generic stack binds loopback ports for an external reverse proxy. For full setup instructions:
- [Cloudflare Deployment Guide](Docs/DeploymentCloudflare.md)
- [Tailscale Deployment Guide](Docs/DeploymentTailscale.md)
- [Generic Deployment Guide](Docs/DeploymentGeneric.md)

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
- `CF_ACCESS_ENABLED`: Enable Cloudflare Access enforcement. Default `false` (set to `true` in `docker-compose.yml`, `false` in `docker-compose.tailscale.yml` and `docker-compose.generic.yml`).
- `CF_ACCESS_TEAM_DOMAIN`: Cloudflare Access team domain (used to derive JWKS/issuer).
- `CF_ACCESS_JWKS_URL`: Override JWKS URL (optional).
- `CF_ACCESS_AUD`: Access application audience (optional but recommended).
- `CF_ACCESS_ISSUER`: Override issuer (optional).
- `CF_ACCESS_DEV_USER_EMAIL`: Placeholder user email when Access is disabled. Default `admin@local`.

Frontend:
- `VITE_API_BASE`: Base URL for API calls. Leave empty when frontend and backend share the same origin.

For the full backend configuration list, see `backend/server/src/config.rs`.
