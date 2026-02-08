# Generic Deployment Guide

This guide covers a direct deployment without Cloudflare Tunnel or Tailscale.

Use this when you already have your own edge/reverse-proxy strategy and are comfortable managing:
1. TLS certificates and renewal
2. DNS and optional port-forwarding
3. Reverse proxy routing and security hardening

## Architecture

1. Browser connects to your own reverse proxy (for example Nginx) over HTTPS.
2. Reverse proxy routes traffic by path:
   - `/api` and `/api/*` -> backend
   - everything else -> frontend
3. Backend and frontend run in Docker via `/Users/user/dev/Bambu LAN Viewer/docker-compose.generic.yml`.

## Route Map (Required)

These path routes are required for the app to work correctly:

1. `/api` -> backend
2. `/api/*` -> backend
3. `/*` -> frontend

Backend target:
1. `http://127.0.0.1:18080` when using the generic compose file as-is.

Frontend target:
1. `http://127.0.0.1:18081` when using the generic compose file as-is.

## Start the Generic Stack

From `/Users/user/dev/Bambu LAN Viewer`:

```bash
docker compose -f docker-compose.generic.yml up -d --build
```

The generic compose file binds app services to loopback only:
1. backend: `127.0.0.1:18080`
2. frontend: `127.0.0.1:18081`

This is intended for a local reverse proxy on the same host.

## Nginx Example (TLS Termination + Path Routing)

Example `http` block snippet:

```nginx
map $http_upgrade $connection_upgrade {
    default upgrade;
    ''      close;
}
```

Example server blocks:

```nginx
server {
    listen 80;
    server_name app.example.com;
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl http2;
    server_name app.example.com;

    ssl_certificate /etc/letsencrypt/live/app.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/app.example.com/privkey.pem;

    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;

    location = /api {
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection $connection_upgrade;
        proxy_read_timeout 3600s;
        proxy_send_timeout 3600s;
        proxy_pass http://127.0.0.1:18080;
    }

    location /api/ {
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection $connection_upgrade;
        proxy_read_timeout 3600s;
        proxy_send_timeout 3600s;
        proxy_buffering off;
        proxy_pass http://127.0.0.1:18080;
    }

    location / {
        proxy_http_version 1.1;
        proxy_pass http://127.0.0.1:18081;
    }
}
```

Why this matters:
1. WebSockets are required for the CMAF video stream endpoint.
2. Long read/send timeouts reduce broken live streams.
3. API and non-API routes must go to different upstream services.

## Caddy Example (TLS Termination + Path Routing)

Example `Caddyfile`:

```caddy
app.example.com {
    encode zstd gzip

    @api path /api /api/*
    reverse_proxy @api 127.0.0.1:18080
    reverse_proxy 127.0.0.1:18081
}
```

Notes:
1. Caddy handles TLS and certificate renewal automatically when DNS and ports are set correctly.
2. WebSocket upgrades are handled automatically by `reverse_proxy`.
3. The API matcher must be declared before the frontend catchall route.

## Traefik Example (TLS Termination + Path Routing)

Example static config (`traefik.yml`):

```yaml
entryPoints:
  web:
    address: ":80"
  websecure:
    address: ":443"

providers:
  file:
    filename: /etc/traefik/dynamic/bambu.yml

certificatesResolvers:
  letsencrypt:
    acme:
      email: you@example.com
      storage: /var/lib/traefik/acme.json
      httpChallenge:
        entryPoint: web
```

Example dynamic config (`/etc/traefik/dynamic/bambu.yml`):

```yaml
http:
  routers:
    bambu-api:
      entryPoints:
        - websecure
      rule: Host(`app.example.com`) && (Path(`/api`) || PathPrefix(`/api/`))
      service: bambu-backend
      priority: 100
      tls:
        certResolver: letsencrypt

    bambu-frontend:
      entryPoints:
        - websecure
      rule: Host(`app.example.com`)
      service: bambu-frontend
      priority: 1
      tls:
        certResolver: letsencrypt

  services:
    bambu-backend:
      loadBalancer:
        servers:
          - url: http://127.0.0.1:18080
    bambu-frontend:
      loadBalancer:
        servers:
          - url: http://127.0.0.1:18081
```

Notes:
1. Traefik handles WebSocket upgrades automatically for proxied HTTP services.
2. Higher priority on the API router ensures `/api` traffic is not captured by the frontend router.
3. If Traefik runs in the same Docker network as app services, you can route to `http://backend:8080` and `http://frontend:80`.

## Optional: Proxy Container on Docker Network

If your reverse proxy runs as a Docker container on the same Compose network, you can route directly to service names:
1. backend upstream: `http://backend:8080`
2. frontend upstream: `http://frontend:80`

In that model, host port bindings can be removed or adjusted.

## Security Notes

1. `CF_ACCESS_ENABLED=false` in this deployment mode.
2. Restrict inbound firewall rules to ports you explicitly need (typically 443 and optionally 80).
3. Keep the backend API non-public except through your reverse proxy.
4. Add your own auth layer if your edge is internet-facing.

## Verify

```bash
docker compose -f docker-compose.generic.yml ps
docker compose -f docker-compose.generic.yml logs --tail=100 backend
docker compose -f docker-compose.generic.yml logs --tail=100 frontend
```

Functional checks:
1. `https://app.example.com/` returns frontend.
2. `https://app.example.com/api/session` returns backend session payload.
3. Video endpoint can establish WebSocket connections through your proxy.

## Troubleshooting

### UI works but API 404/502
1. `/api` and `/api/*` are likely not routed to backend.
2. Confirm path ordering and proxy upstream target.

### API works but video fails
1. Confirm WebSocket upgrade headers are passed.
2. Confirm long proxy timeouts are configured.

### TLS/certificate issues
1. Validate DNS points to your proxy host.
2. Validate certificate files and renewal automation.
3. Confirm firewall/router forwarding for 443.
