# Cloudflare Deployment Guide

This guide deploys Bambu LAN Viewer in production using:
- `docker-compose.yml`
- Cloudflare Tunnel (`cloudflared`)
- Cloudflare Access JWT enforcement in the backend (`CF_ACCESS_ENABLED=true`)

## Architecture

1. Browser connects to `https://app.example.com` through Cloudflare.
2. Cloudflare Tunnel forwards:
   - `/api/*` to `http://backend:8080`
   - all other paths (`/*`) to `http://frontend:80`
3. Cloudflare Access injects `cf-access-jwt-assertion`.
4. Backend validates that JWT using your Zero Trust team domain and app AUD.

## Prerequisites

1. Domain managed by Cloudflare (for example `example.com`).
2. Cloudflare Zero Trust account with a team domain (for example `yourteam.cloudflareaccess.com`).
3. Access application created for your app hostname.
4. Cloudflare Tunnel created with public hostnames configured.
5. Docker and Docker Compose on the deployment host.

## Repository Files

1. Compose stack: `/Users/user/dev/Bambu LAN Viewer/docker-compose.yml`
2. Runtime secret env file (local only): `/Users/user/dev/Bambu LAN Viewer/.cloudflared.env`

`.cloudflared.env` is gitignored and should never be committed.

## 1) Configure `.cloudflared.env`

Create `/Users/user/dev/Bambu LAN Viewer/.cloudflared.env`:

```env
TUNNEL_TOKEN=your_tunnel_connector_token
CF_ACCESS_TEAM_DOMAIN=yourteam.cloudflareaccess.com
CF_ACCESS_AUD=your_access_application_aud
```

Notes:
1. `CF_ACCESS_TEAM_DOMAIN` is used to derive JWKS and issuer.
2. `CF_ACCESS_AUD` should match the exact AUD/tag from your Access application.

## 2) Start the Stack

From `/Users/user/dev/Bambu LAN Viewer`:

```bash
docker compose up -d --build
```

Services:
1. `backend` on internal Docker network.
2. `frontend` on internal Docker network.
3. `cloudflared` connector to Cloudflare Tunnel.

## 3) Verify

```bash
docker compose ps
docker compose logs --tail=100 cloudflared
docker compose logs --tail=100 backend
```

Check externally by opening your configured hostname (for example `https://app.example.com`).

Expected behavior:
1. You are prompted by Cloudflare Access (if not already authenticated).
2. App loads and can call `/api/session`.
3. Live video works (WebSocket upgrade on `/api/printers/:id/video/cmaf`).

## 4) Updating / Restarting

```bash
docker compose pull
docker compose up -d
```

Rebuild app images when code changed:

```bash
docker compose up -d --build
```

## 5) Troubleshooting

### `401 Unauthorized` on `/api/*`
1. Confirm the request passed through Cloudflare Access.
2. Verify `CF_ACCESS_AUD` matches the Access application AUD exactly.
3. Verify `CF_ACCESS_TEAM_DOMAIN` is correct (example: `yourteam.cloudflareaccess.com`).

### Cloudflare `502/504`
1. Confirm tunnel is healthy in Cloudflare UI.
2. Confirm public hostname routes target `http://backend:8080` and `http://frontend:80`.
3. Confirm API route appears before catchall route.

### UI loads but API fails
1. `/api/*` is probably not routed to backend.
2. Recheck Tunnel public hostname path routing.

### Video stream does not start
1. Confirm `/api/*` route includes WebSocket-capable path handling.
2. Ensure traffic still goes through backend for `/api/printers/:id/video/cmaf`.

## Appendix A: Cloudflare Web UI Setup Checklist

This appendix lists the Cloudflare objects to create and where to find them in the dashboard.

### A1) Zero Trust Organization

Create/verify your Zero Trust team domain.

1. Open Cloudflare dashboard and enter Zero Trust.
2. Locate your team domain (format: `name.cloudflareaccess.com`).
3. Use this value as `CF_ACCESS_TEAM_DOMAIN`.

### A2) Authentication Login Method

Create at least one login method for Access.

1. Go to `Access` -> `Authentication` -> `Login methods`.
2. Add your IdP (Google, Microsoft Entra ID, GitHub, Okta, or One-time PIN).
3. Complete IdP setup and test user login.

Without this, Access applications cannot authenticate users.

### A3) Access Application

Create a self-hosted Access app for the Bambu UI hostname.

1. Go to `Access` -> `Applications` -> `Add an application`.
2. Choose `Self-hosted`.
3. Name: `Bambu LAN Viewer` (or your preferred name).
4. Application domain: your app host (for example `app.example.com`).
5. Path: `/*`.
6. Create at least one Allow policy (email, email domain, or group).
7. Save.

After saving:
1. Open the application details.
2. Copy the Application Audience (AUD/tag).
3. Put that value into `CF_ACCESS_AUD`.

### A4) Access Policy (who can reach the app)

At minimum, create one Allow policy:

1. Policy action: `Allow`.
2. Include rule example: `Emails ending in @example.com`.
3. Optional hardening:
   - Require MFA.
   - Require specific identity groups.
   - Require approved device posture.

This policy is evaluated before the request reaches your backend.

### A5) Tunnel

Create a Cloudflare Tunnel connector for this deployment host.

1. Go to `Networks` -> `Tunnels` -> `Create a tunnel`.
2. Choose `Cloudflared`.
3. Name it (example `bambu-lan-viewer-prod`).
4. In connector setup, choose Docker and copy the token value.
5. Use that token in `TUNNEL_TOKEN`.

This repository uses token-based tunnel auth (no local credentials file required).

### A6) Tunnel Public Hostnames (Routing)

On the created tunnel, add public hostnames/routes:

1. Route 1:
   - Hostname: `app.example.com`
   - Path: `/api/*`
   - Service: `http://backend:8080`
2. Route 2:
   - Hostname: `app.example.com`
   - Path: `/*` (or empty catchall)
   - Service: `http://frontend:80`

Important:
1. API route must be evaluated before catchall.
2. Both routes use internal Docker service names because `cloudflared` runs in the same compose network.

### A7) Profile / Device Controls (Optional Hardening)

If your org uses managed devices, add profile-based restrictions.

1. Configure device enrollment/profile policies in Zero Trust device settings.
2. Create posture checks (OS, disk encryption, client presence, etc.).
3. In Access application policy, add `Require` rules for those posture checks.

This is optional, but useful when you want app access limited to trusted devices.

### A8) Final Cloudflare-Side Validation

Before launching, confirm:

1. Access application exists and has an active Allow policy.
2. Tunnel status is healthy.
3. Public hostname routing includes both `/api/*` and catchall to frontend.
4. `CF_ACCESS_TEAM_DOMAIN` and `CF_ACCESS_AUD` in `.cloudflared.env` match UI values.
