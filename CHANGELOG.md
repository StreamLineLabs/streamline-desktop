# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


- style: align sidebar icons with design system (2026-03-05)

- refactor: migrate state management to zustand (2026-03-06)

- fix: resolve dark mode rendering issue on macOS (2026-03-06)
## [Unreleased]

## [0.2.0] - 2026-02-28

### Added
- Tauri 2 + React 19 desktop application
- Cross-platform support (macOS, Linux, Windows)
- Embedded Streamline server (zero-config)
- System tray integration with menu controls
- Web dashboard embedded via WebView
- Server lifecycle management (start/stop/restart)
- Consumer group monitoring view
- Message viewer with JSON formatting
- Connection settings panel
- Topic browser component
- Unit tests for connection manager

### Fixed
- Correct window title and icon configuration
- Resolve Tauri IPC handler registration

### Changed
- Extract Tauri command bindings
- Optimize data processing loop
- Clean up module organization

### Infrastructure
- Vite build pipeline with React plugin
- Rust backend via Tauri
- TypeScript strict mode configuration
- Apache 2.0 license

