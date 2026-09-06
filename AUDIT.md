# Clean Code and SRP Audit

## Summary

- **Highest-leverage split:** separate Streamline process lifecycle from Tauri command/tray wiring in `src-tauri/src/main.rs`; ops changes should not collide with IPC or HTTP contract changes.
- The 1,511-line Rust entry point currently serves OS process management, Streamline HTTP adaptation, settings persistence, Tauri IPC, and desktop boot actors.
- The 897-line React `App.tsx` owns the shell plus six operational feature screens; the tab components have independent local state and can move without inventing abstraction.
- The Streamline HTTP wire shapes and conversion decisions are now characterized with literal core responses, making that contract layer the safest first extraction.
- Long transform/rendering sections are not automatically violations; extraction is limited to units that remove an actor and remain independently testable.

## Findings

| ID | Location | Category | Severity | Actors in conflict | Cost | Size | Behavior risk |
|---|---|---|---|---|---|---|---|
| DESKTOP-SRP-1 | `src-tauri/src/main.rs:25-1065` | SRP | P1 | Ops/process lifecycle, Streamline API consumers, desktop settings, and Tauri shell integration | Core route changes, sidecar packaging changes, settings migrations, and tray UX all edit one entry point. Lifecycle methods use `ServerProcess`; API adapters use wire DTOs; settings uses filesystem paths; tray setup uses neither domain. Split into `server_lifecycle`, `streamline_api`, and a thin Tauri composition root. | L | Medium |
| DESKTOP-SRP-2 | `src/App.tsx:1-897` | SRP | P1 | Desktop shell/navigation, topic operations, consumer operations, schema operations, and settings UX | Each product area carries independent state and IPC calls, so a schema-screen change requires reviewing the same file as server controls and topic creation. Move coherent tab components into feature files while retaining the shell as orchestrator. | L | Medium |
| DESKTOP-SRP-3 | `src/types.ts:1-225` | SRP | P2 | IPC contract, preview/test environment, and visual design system | Native command changes, fixture changes, and color/style changes collide. Split into `ipc.ts`, `models.ts`, and `theme.ts`; avoid a vague shared utility module. | M | Low |
| DESKTOP-CC-1 | `src-tauri/src/main.rs:1037-1063` | Error handling | P1 | Settings persistence | Directory creation and malformed settings are silently ignored, making corruption indistinguishable from first launch. Changing recovery behavior requires a user-visible policy and migration test. | S | Medium |
| DESKTOP-CC-2 | `src-tauri/src/main.rs:556-615` | Duplication / dependencies | P2 | Streamline HTTP transport | GET, POST, and DELETE build independent clients and repeat status/body handling. A timeout, TLS, or error-format change must be repeated. Extract a concrete `StreamlineApi` transport after preserving exact error strings. | M | Medium |
| DESKTOP-CC-3 | `src-tauri/src/main.rs:1067-1160` | Mixed abstraction | P2 | Tauri shell and server lifecycle | Menu construction, async task dispatch, process cleanup, auto-start, and command registration share one function. Extract lifecycle first; keep router-style registration visible in the composition root. | M | Low |
| DESKTOP-CC-4 | `src/App.tsx:37-135` | Mixed abstraction / hidden effects | P2 | Polling policy, notifications, and product actions | The root component owns polling timers, toast expiration, server lifecycle, produce, and consume workflows alongside layout. Feature hooks are justified only where they own a complete workflow and tests. | M | Medium |
| DESKTOP-SUP-1 | `src-tauri/src/main.rs:498-1029` | Layering | P1 | Tauri IPC vs. core HTTP contract | IPC handlers know core paths and wire fields directly. A core contract change risks changing exported IPC payloads. Keep dedicated wire DTOs and map them to frozen desktop DTOs. | M | Medium |

## Ordered Refactor Sequence

1. **DESKTOP-SRP-1 / DESKTOP-SUP-1:** move the characterized Streamline wire DTOs, parsers, topic policy, and record-merging decisions into `streamline_api.rs` unchanged.
2. Make command handlers consume the extracted contract module while preserving every Tauri command name and serialized payload.
3. Add lifecycle state-transition tests using an injectable child-process boundary.
4. Extract `ServerProcess` and startup/cleanup policy into `server_lifecycle.rs`; keep Tauri command wrappers in `main.rs`.
5. Add React interaction tests for navigation, topic creation, producing, consuming, groups, schemas, and settings.
6. **DESKTOP-SRP-2:** move one characterized feature tab per commit, starting with the self-contained schema and consumer-group screens.
7. **DESKTOP-SRP-3:** separate IPC preview behavior from domain models and theme constants after component imports are stable.

## Deferred

- **DESKTOP-CC-1:** invalid-settings recovery is observable behavior and needs a chosen UX (fail startup, quarantine the file, or show a repair prompt).
- Process execution remains concrete until lifecycle tests can substitute a child process; adding an interface with one production implementation now would be over-extraction.
- React tab extraction is deferred because current tests validate imports and preview fixtures, not user interactions or asynchronous state transitions.
- Real notarization, Windows signing, and same-tag core release sequencing still require external credentials and release policy.

## Out of Scope

- Tauri command names, argument names, IPC payloads, Streamline routes, settings JSON, and sidecar naming remain frozen.
- `DashboardTab`, `ProduceTab`, and the small shared form components remain functions; they have one presentation actor and no independent state requiring classes or service wrappers.
- No shared cross-repository API package is introduced. Desktop adapts the existing core contract locally so the repositories keep independent release units.
- The schema-enabled core sidecar is built by release automation, but core feature policy and `_schemas` protection in the server remain separate core decisions.
