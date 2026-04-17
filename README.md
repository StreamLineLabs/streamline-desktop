# Streamline Desktop

[![CI](https://github.com/streamlinelabs/streamline-desktop/actions/workflows/ci.yml/badge.svg)](https://github.com/streamlinelabs/streamline-desktop/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-24C8D8.svg)](https://v2.tauri.app)
[![Docs](https://img.shields.io/badge/docs-streamlinelabs.dev-blue.svg)](https://streamlinelabs.dev/docs/getting-started/desktop)
[![Release](https://img.shields.io/github/v/release/streamlinelabs/streamline-desktop?label=release)](https://github.com/streamlinelabs/streamline-desktop/releases)

> **The Redis of Streaming — on your desktop.**

Streamline Desktop wraps the [Streamline](https://github.com/streamlinelabs/streamline) server in a native desktop application powered by [Tauri 2](https://v2.tauri.app). It bundles the Streamline binary, manages its lifecycle, and provides a GUI for producing/consuming messages, inspecting topics, and running StreamQL queries.

## Features

- **Zero-config server** — starts an embedded Streamline instance automatically
- **System tray** — Start / Stop / Quit from the tray icon
- **Web dashboard** — embeds the Streamline HTTP dashboard in-app
- **Cross-platform** — macOS, Linux, Windows

## Prerequisites

| Tool | Version |
|------|---------|
| [Node.js](https://nodejs.org) | 18+ |
| [Rust](https://rustup.rs) | 1.80+ |
| [Tauri CLI prerequisites](https://v2.tauri.app/start/prerequisites/) | see docs |

## Getting Started

```bash
# Install frontend dependencies
npm install

# Run in development mode (hot-reload)
npm run dev

# Build a production bundle
npm run build
```

> **Note:** The `streamline` binary must be present in `src-tauri/` (or on your `PATH`) for the embedded server to start. During development you can run the Streamline server manually.

## Project Structure

```
streamline-desktop/
├── index.html              # HTML entry point
├── src/                    # React frontend
│   ├── main.tsx
│   └── App.tsx
├── src-tauri/              # Tauri / Rust backend
│   ├── tauri.conf.json
│   ├── Cargo.toml
│   └── src/main.rs
├── vite.config.ts
├── tsconfig.json
└── package.json
```

## System Requirements

- **macOS** 10.15+ (Catalina or later)
- **Linux** with WebKitGTK 4.1+
- **Windows** 10+ with WebView2

## License

Apache-2.0




## Development Setup

### Prerequisites
- Node.js 18+
- Rust 1.80+
- Tauri CLI: `cargo install tauri-cli`

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `STREAMLINE_DEFAULT_BROKER` | Default broker address | `localhost:9092` |
| `STREAMLINE_THEME` | UI theme (light/dark/system) | `system` |
| `STREAMLINE_LOG_LEVEL` | Log verbosity | `info` |
| `STREAMLINE_DATA_DIR` | Override embedded data directory | OS app-data dir |
| `STREAMLINE_HTTP_PORT` | HTTP/dashboard port for the embedded server | `9094` |
| `STREAMLINE_KAFKA_PORT` | Kafka protocol port for the embedded server | `9092` |
| `TAURI_PRIVATE_KEY` | Code-signing key (release builds only) | _unset_ |

## Architecture

```
 ┌─────────────────────────────┐
 │ React + Vite frontend (TS)  │  ← src/
 └──────────────┬──────────────┘
                │  Tauri IPC (invoke / events)
 ┌──────────────▼──────────────┐
 │ Tauri Rust backend          │  ← src-tauri/src/
 │  • lifecycle (start/stop)   │
 │  • tray menu                │
 │  • spawns ./streamline      │
 └──────────────┬──────────────┘
                │  TCP (Kafka) + HTTP
 ┌──────────────▼──────────────┐
 │ Embedded Streamline server  │  bundled binary
 └─────────────────────────────┘
```

The Rust backend launches the bundled `streamline` binary as a child process,
streams its stdout/stderr into the in-app log viewer, and exposes start/stop
controls to the frontend via Tauri commands.

## Inner Loop

| Action | Command | Notes |
|--------|---------|-------|
| Frontend hot-reload | `npm run dev` | Vite dev server proxied by Tauri |
| Backend rebuild | edit `src-tauri/src/*.rs` | Tauri recompiles on save |
| Type-check only | `npm run typecheck` | No bundle output |
| Lint | `npm run lint` | ESLint + Prettier |
| Production bundle | `npm run build` | `.dmg`, `.AppImage`, `.msi` in `src-tauri/target/release/bundle/` |
| Tauri-only build | `cargo tauri build` | Same output, more verbose |

Cold start (first build) typically takes 4–7 minutes due to Rust compilation;
incremental rebuilds during development are < 5 s for frontend changes and
< 30 s for backend changes.

## Bundled Binary

Release builds embed a platform-matched `streamline` binary into the app
bundle. To refresh it locally:

```bash
# From the streamline/ repo (next to streamline-desktop/)
cd ../streamline && cargo build --release
cp target/release/streamline ../streamline-desktop/src-tauri/binaries/streamline-$(rustc -vV | grep host | awk '{print $2}')
```

Tauri's bundler picks the binary matching the build target triple
automatically. See `src-tauri/tauri.conf.json` → `bundle.externalBin`.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| "streamline: command not found" on launch | Bundled binary missing for your target triple | Rebuild and copy as shown above |
| Blank window on Linux | Missing WebKitGTK 4.1 | `sudo apt install libwebkit2gtk-4.1-dev` |
| "WebView2 not installed" on Windows | Edge runtime absent | Install [WebView2 evergreen runtime](https://developer.microsoft.com/microsoft-edge/webview2/) |
| Tray icon not visible on macOS | Sandboxed environment without status-bar access | Run from `/Applications/`, not from Finder preview |
| Port 9092 already in use | Another Kafka/Streamline instance running | `lsof -i :9092` and stop the conflicting process, or set `STREAMLINE_KAFKA_PORT` |
| Build fails with `linker 'cc' not found` | Missing build essentials on Linux | `sudo apt install build-essential` |

Verbose logs:

```bash
RUST_LOG=streamline_desktop=debug,tauri=info npm run dev
```

## Releasing

Releases are produced by the GitHub Actions workflow `.github/workflows/release.yml`,
which runs on tag push (`v*.*.*`) and uploads platform installers to the
GitHub Release. Local dry-run:

```bash
npm run build
ls src-tauri/target/release/bundle/
```

Code-signing is required for notarized macOS / signed Windows builds; secrets
are configured at the org level (see `streamlinelabs/.github`).

## Roadmap

- [ ] Multi-cluster connection manager (currently single embedded server only)
- [ ] In-app StreamQL editor with syntax highlighting
- [ ] Topic browser with message preview / replay
- [ ] Auto-update channel via Tauri updater
- [ ] Linux `.deb` and `.rpm` packaging in addition to `.AppImage`

Track progress in the [project board](https://github.com/orgs/streamlinelabs/projects).

## Contributing

See the [org-wide CONTRIBUTING guide](https://github.com/streamlinelabs/.github/blob/main/CONTRIBUTING.md).
Desktop-specific notes:

- Keep the IPC surface (`#[tauri::command]` functions) minimal and typed
- Never spawn the embedded server directly from the frontend — always go through
  the Rust backend so lifecycle/cleanup is centralized
- UI strings live in `src/i18n/` (when present) — add new strings to all locale files

## Related Projects

- [`streamline`](https://github.com/streamlinelabs/streamline) — core server
- [`streamline-vscode`](https://github.com/streamlinelabs/streamline-vscode) — VS Code extension
- [`streamline-docs`](https://github.com/streamlinelabs/streamline-docs) — documentation site

## Status

**Beta.** Suitable for local development and demos; not recommended as a
production cluster manager. Breaking changes possible until 1.0.

