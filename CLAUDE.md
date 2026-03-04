# CLAUDE.md — Streamline Desktop

## What is this?

Streamline Desktop is a Tauri 2 application that bundles the Streamline server into a native desktop app. It provides a React-based UI shell with an embedded web dashboard and Tauri commands for server lifecycle management.

## Architecture

- **Frontend**: React 19 + Vite, rendered in a Tauri webview
- **Backend**: Rust (Tauri), manages a child `streamline` process
- **Communication**: Tauri IPC (`invoke`) between frontend ↔ Rust; Rust ↔ Streamline server via HTTP (`localhost:9094`) and Kafka protocol (`localhost:9092`)

## Build & Run Commands

```bash
npm install              # Install frontend dependencies
npm run dev              # Tauri dev mode (hot-reload)
npm run build            # Production build (creates installer)
npm run preview          # Preview the Vite build
```

Rust backend (from `src-tauri/`):

```bash
cargo build              # Build the Tauri backend
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Key Files

| File | Purpose |
|------|---------|
| `src-tauri/src/main.rs` | Tauri entry point, server lifecycle, tray, commands |
| `src-tauri/tauri.conf.json` | Tauri app configuration (window, bundle, security) |
| `src-tauri/Cargo.toml` | Rust dependencies |
| `src/App.tsx` | React shell (sidebar, iframe dashboard, settings) |
| `src/main.tsx` | React entry point |
| `vite.config.ts` | Vite config for React + Tauri |

## Tauri Commands (IPC)

| Command | Description |
|---------|-------------|
| `get_server_status` | Returns `{ running, pid, kafka_port, http_port }` |
| `start_server` | Spawns the embedded Streamline binary |
| `stop_server` | Kills the running server process |
| `get_topics` | Fetches topic list from the HTTP API |
| `get_server_info` | Fetches server version/uptime from the HTTP API |

## Conventions

- Follow the same Rust style as `streamline/` (clippy clean, `cargo fmt`)
- Frontend is intentionally minimal — the heavy lifting is in the Streamline web dashboard served via iframe
- The bundled `streamline` binary is declared in `tauri.conf.json` under `bundle.resources`
- Ports default to 9092 (Kafka) and 9094 (HTTP) to match the core server defaults

