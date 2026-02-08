# Tailscale Deployment Guide

This guide deploys Bambu LAN Viewer in production using:
- `docker-compose.tailscale.yml`
- Tailscale container networking
- `tailscale serve` path-based proxy routing

## Architecture

1. `tailscale` container joins your tailnet.
2. `tailscale serve` publishes HTTPS on the Tailscale cert domain.
3. Requests are routed by path:
   - `/api` and `/api/*` -> `http://backend:8080`
   - all other paths -> `http://frontend:80`
4. Backend runs with `CF_ACCESS_ENABLED=false` (security comes from Tailscale ACLs and tailnet identity).

## Prerequisites

1. A Tailscale tailnet.
2. MagicDNS and HTTPS certificates enabled in tailnet settings.
3. A Tailscale auth key for this deployment.
4. Docker and Docker Compose on the deployment host.

## Repository Files

1. Compose stack: `/Users/user/dev/Bambu LAN Viewer/docker-compose.tailscale.yml`
2. Serve routing config: `/Users/user/dev/Bambu LAN Viewer/.tailscale/serve.json`
3. Env template: `/Users/user/dev/Bambu LAN Viewer/.tailscale.env.example`
4. Local env file you create: `/Users/user/dev/Bambu LAN Viewer/.tailscale.env`

`.tailscale.env` is gitignored and should never be committed.

## 1) Create `.tailscale.env`

```bash
cp /Users/user/dev/Bambu LAN Viewer/.tailscale.env.example /Users/user/dev/Bambu LAN Viewer/.tailscale.env
```

Edit `/Users/user/dev/Bambu LAN Viewer/.tailscale.env`:

```env
TS_AUTHKEY=tskey-auth-REPLACE_ME
TS_HOSTNAME=bambu-lan-viewer
TS_EXTRA_ARGS=
```

## 2) Review Serve Routes

Default config in `/Users/user/dev/Bambu LAN Viewer/.tailscale/serve.json`:

1. HTTPS on `:443`.
2. `/api` + `/api/` proxied to backend.
3. `/` proxied to frontend.
4. Funnel disabled by default.

If you intentionally want internet exposure via Funnel, set `AllowFunnel` to `true` and enable Funnel in Tailscale.

## 3) Start the Stack

From `/Users/user/dev/Bambu LAN Viewer`:

```bash
docker compose -f docker-compose.tailscale.yml up -d --build
```

## 4) Verify

```bash
docker compose -f docker-compose.tailscale.yml ps
docker compose -f docker-compose.tailscale.yml logs --tail=100 tailscale
docker compose -f docker-compose.tailscale.yml exec tailscale tailscale status
docker compose -f docker-compose.tailscale.yml exec tailscale tailscale serve status
```

Use the HTTPS URL reported by `tailscale serve status` to access the UI.

## 5) Updating / Restarting

```bash
docker compose -f docker-compose.tailscale.yml pull
docker compose -f docker-compose.tailscale.yml up -d
```

Rebuild app images when code changed:

```bash
docker compose -f docker-compose.tailscale.yml up -d --build
```

## 6) Troubleshooting

### App not reachable from tailnet
1. Check `tailscale status` output for node connectivity.
2. Verify auth key is valid and not expired.
3. Verify ACLs allow your user/device to reach this node.

### UI loads but API fails
1. Verify `/api` and `/api/` handlers still point to `http://backend:8080`.
2. Confirm backend service is healthy in compose logs.

### HTTPS URL missing
1. Confirm MagicDNS and HTTPS certificates are enabled in tailnet.
2. Confirm `tailscale serve status` shows an active HTTPS serve config.

