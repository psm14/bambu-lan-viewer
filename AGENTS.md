# Repository Guidelines

## Project Structure & Module Organization
- `frontend/`: Webapp frontend (primary interface).
- `backend/`: Webapp backend (primary interface).
- `docker-compose.yml`: Local orchestration for the full stack.

## Build, Test, and Development Commands
- `npm --prefix frontend run dev`: Start the frontend dev server.
- `npm --prefix frontend run build`: Build the frontend.
- `cargo run --manifest-path backend/Cargo.toml`: Run the backend service.
- `cargo test --manifest-path backend/Cargo.toml`: Run backend tests.
- `docker compose up`: Run the full stack via Docker Compose.

## Coding Style & Naming Conventions
- Backend: Rust; use `rustfmt` conventions and idiomatic naming.
- Frontend: React (Vite); follow existing component and file naming in `frontend/`.

## Testing Guidelines
- Backend: use `cargo test` and add tests alongside modules where appropriate.
- Frontend: no test harness is set up yet; add one if needed.
