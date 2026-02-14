# AGENTS.md — Antigravity Tools

Tauri v2 desktop app: Rust backend (`src-tauri/`) + React TypeScript frontend (`src/`).
Package name: `antigravity_tools`. License: CC-BY-NC-SA-4.0.

---

## Build & Dev Commands

### Frontend (project root)

```bash
npm run dev              # Vite dev server (port 1420)
npm run build            # tsc && vite build
npm run preview          # Preview production build
```

### Rust Backend (from `src-tauri/`)

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # Run all tests
cargo test <test_name>   # Run a single test by name
cargo test -- --nocapture  # Tests with stdout visible
cargo clippy             # Lint
cargo fmt                # Format
cargo fmt --check        # Check formatting without modifying
```

### Full Tauri App

```bash
npm run tauri dev        # Dev mode (frontend + backend hot-reload)
npm run tauri build      # Production build
npm run tauri:debug      # Dev with RUST_LOG=debug
```

### What's NOT configured

- No ESLint or Prettier
- No frontend test framework
- CI runs `cargo test` before release builds (`.github/workflows/release.yml`)

---

## Project Structure

```
src-tauri/src/
  main.rs             Entry point
  lib.rs              Module declarations, run(), Tauri setup
  error.rs            AppError enum (thiserror), AppResult<T>
  constants.rs        Global constants
  models/             Data models — serde Serialize/Deserialize
  modules/            Business logic (account, oauth, config, quota, scheduler, db)
  commands/           Tauri command handlers (#[tauri::command])
  proxy/              Axum-based API proxy server (port 8045)
    handlers/         Protocol handlers (claude, openai, gemini, audio, warmup)
    mappers/          Protocol conversion (request/response, streaming)
    common/           Shared utils (json_schema, model_mapping, rate_limiter)
    middleware/       Auth, CORS, logging
    tests/            Integration tests (quota, security, rate_limit, retry)
    server.rs         Axum server setup
    token_manager.rs  Account token management

src/
  main.tsx            React entry point
  App.tsx             Router setup (react-router-dom v7, createBrowserRouter)
  components/         Feature-based UI components
  pages/              Page-level components
  stores/             Zustand v5 stores (useXxxStore pattern)
  services/           API service layer
  types/              TypeScript type definitions
  locales/            i18n translations (12+ languages)
  utils/              Utilities (env detection, request wrapper)
  hooks/              Custom React hooks
```

---

## Rust Code Style

- **Edition**: 2021
- **Naming**: `snake_case` functions/variables, `PascalCase` types/structs/enums
- **Error handling**: `thiserror` for `AppError` enum, `anyhow` for proxy internals. Return `AppResult<T>` from commands. Use `#[from]` for automatic error conversion.
- **Async**: `tokio` (full features). Async throughout proxy handlers.
- **Serialization**: `serde` with `derive`. Use `#[serde(default)]` on config fields for backward compatibility. `serde_json` with `preserve_order`.
- **Logging**: `tracing` crate (`info!`, `warn!`, `error!`, `debug!`). Prefer structured fields.
- **Concurrency**: `Arc` for shared state, `parking_lot` mutexes, `DashMap` for concurrent maps, `tokio::sync` for async locks.
- **HTTP**: `reqwest` outbound, `axum` 0.7 for proxy server, `tower-http` middleware.
- **Database**: `rusqlite` (bundled SQLite). Use `COALESCE` for NULL safety. All data stored locally, encrypted.
- **Tauri commands**: `#[tauri::command]` attribute, placed in `commands/` module. Error types must implement `Serialize`.
- **Tests**: Located in `proxy/tests/`. Use `#[cfg(test)]` and `#[test]`. Pure Rust unit/integration tests.
- **Comments**: Mix of English and Chinese throughout the codebase.

### Key Rust Dependencies

tauri 2.x, axum 0.7, tokio, serde, reqwest, rusqlite, tracing, thiserror, anyhow, dashmap, parking_lot

---

## TypeScript/React Code Style

- **React 19**: Functional components and hooks only. No class components.
- **TypeScript**: Strict mode. ES2020 target. `noUnusedLocals` and `noUnusedParameters` enforced.
- **State**: Zustand v5 stores in `stores/`. Pattern: `useXxxStore`.
- **Routing**: react-router-dom v7, `createBrowserRouter` in `App.tsx`.
- **Styling**: TailwindCSS 3 + DaisyUI 5. Dark mode via `class` strategy. Use `clsx`/`tailwind-merge` for conditional classes.
- **i18n**: react-i18next with `useTranslation()` hook. No hardcoded UI strings. 12+ locale files in `src/locales/`.
- **Icons**: `lucide-react` (general), `@lobehub/icons` (model brand icons).
- **Imports**: ES modules. Use `type` keyword for type-only imports. React from `react`, Tauri API from `@tauri-apps/api`.
- **Tauri bridge**: `invoke` from `@tauri-apps/api/core`. Wrapped in `src/utils/request.ts` for web/Tauri compatibility.
- **Components**: Feature-based in `components/`. Pages in `pages/`. Services abstract API calls.
- **Charts**: Recharts. Virtualized lists via `@tanstack/react-virtual`.
- **Animations**: `framer-motion`.

### Key Frontend Dependencies

react 19, zustand 5, react-router 7, tailwindcss 3, daisyui 5, i18next, lucide-react, recharts, framer-motion, antd 5

---

## Architecture Notes

- This is a **Tauri v2 app** — Rust backend and frontend are tightly coupled via Tauri commands (`invoke`).
- The **proxy module** is the most complex subsystem. It bridges multiple AI API protocols (OpenAI, Claude, Gemini) through Axum on port 8045.
- `#[serde(default)]` is used heavily on config structs for backward compatibility during version upgrades.
- Account data is stored in **encrypted local SQLite**.
- The proxy server uses Axum with `tower-http` middleware for auth, CORS, and logging.

---

## Common Workflows

### Adding a Tauri Command

1. Create handler in `src-tauri/src/commands/` with `#[tauri::command]`
2. Register in `lib.rs` via `invoke_handler`
3. Call from frontend with `invoke("command_name", { args })` via `@tauri-apps/api/core`

### Adding a Proxy Handler

1. Add handler in `src-tauri/src/proxy/handlers/`
2. Add request/response mappers in `proxy/mappers/`
3. Register route in `proxy/server.rs`
4. Add tests in `proxy/tests/`

### Running a Single Test

```bash
# From src-tauri/
cargo test test_quota_protection          # By name
cargo test test_quota -- --nocapture      # With output
cargo test proxy::tests::                 # All proxy tests
```
