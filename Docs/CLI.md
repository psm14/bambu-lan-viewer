# CLI Guide

`bambuctl` is a generic command-line client for the Bambu LAN Viewer HTTP API. It talks to the same backend API that the web UI uses.

## Build

From `backend/server`:

```bash
cargo build --release --bin bambuctl
```

Resulting binary:

```text
backend/server/target/release/bambuctl
```

## Configuration

By default, `bambuctl` uses:

- `BAMBUCTL_BASE_URL=http://127.0.0.1:8080`

You can override it with either environment variables or flags.

Optional auth environment variables:

- `BAMBUCTL_USERNAME`
- `BAMBUCTL_PASSWORD`

These are only needed if you place the API behind HTTP Basic Auth later.

## Common usage

Against a local backend:

```bash
cargo run --bin bambuctl -- printers list
cargo run --bin bambuctl -- printers status --id 1
```

Against the Tailscale deployment:

```bash
BAMBUCTL_BASE_URL=https://bambu-lan-viewer.<tailnet>.ts.net \
cargo run --bin bambuctl -- printers list
```

## Commands

### List printers

```bash
bambuctl printers list
```

### Get printer status

```bash
bambuctl printers status --id 1
```

### Turn the chamber light on or off

```bash
bambuctl printers light --id 1 --on
bambuctl printers light --id 1 --off
```

### Pause, resume, stop, home

```bash
bambuctl printers pause --id 1
bambuctl printers resume --id 1
bambuctl printers stop --id 1
bambuctl printers home --id 1
```

### Jog axes

```bash
bambuctl printers move --id 1 --axis x --distance 5
bambuctl printers move --id 1 --axis z --distance 1 --feed-rate 300
```

### Set temperatures

```bash
bambuctl printers nozzle-temp --id 1 --celsius 220
bambuctl printers bed-temp --id 1 --celsius 60
```

### Extrude / retract

```bash
bambuctl printers extrude --id 1 --mm 5
bambuctl printers extrude --id 1 --mm -5
```

## JSON output

Use `--json` to emit compact JSON:

```bash
bambuctl --json printers status --id 1
```

## Notes

- `bambuctl` is intentionally generic and contains no Hermes-specific behavior.
- It is suitable for shell scripts, cron jobs, or external automation clients.
- For Tailscale deployments, prefer the `https://<hostname>.<tailnet>.ts.net` URL so browser/secure-context features and HTTPS semantics stay aligned with the web UI.
