# CLAUDE.md — Streamline Desktop

## What is this?

Streamline Desktop is a Tauri 2 application that bundles the Streamline server into a native desktop app. It provides a React-based UI shell that renders its own operations screens against the Streamline HTTP API, plus Tauri commands for server lifecycle management. The Streamline web dashboard is not embedded.

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
| `src/App.tsx` | React shell (sidebar, native dashboard/topics/produce/consume/groups/schemas, settings) |
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
| `save_settings` / `load_settings` | Persist/read validated settings (loopback host, distinct non-zero ports, absolute data dir) |
| `get_settings_warning` | Reports unreadable/invalid persisted settings that were quarantined |
| `take_startup_error` | Drains a background/tray server-start failure for the UI to display once |

## Conventions

- Follow the same Rust style as `streamline/` (clippy clean, `cargo fmt`)
- Frontend is intentionally minimal — screens are rendered natively from Tauri commands that call the Streamline HTTP API; there is no embedded/iframed web dashboard
- The bundled `streamline` sidecar is declared in `src-tauri/tauri.release.conf.json` under `bundle.externalBin`
- Packaged (release) builds require that bundled sidecar and fail closed without it; `STREAMLINE_BINARY`/`PATH` fallbacks exist only in debug builds
- Tagged releases require `package.json`, both root entries in
  `package-lock.json`, `src-tauri/Cargo.toml`, the `streamline-desktop` package
  block in `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json` to match the
  exact tag. SemVer prerelease versions such as `0.4.0-rc.1` are published as
  GitHub prereleases.
- Tag releases bundle the matching core tag; manual release-workflow runs require an explicit `streamline_ref` input (default `main`)
- Ports default to 9092 (Kafka) and 9094 (HTTP) to match the core server defaults
