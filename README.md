# Streamline Desktop

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




