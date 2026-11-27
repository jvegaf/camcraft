---

name: tauri-dev
description: "Trigger: Tauri, src-tauri, IPC, commands, invoke, capabilities, Tauri plugin, desktop build. Develop Camcraft's Tauri 2 backend and frontend bridge."
license: Apache-2.0
---

## Activation Contract

Use when working on `src-tauri/`, Tauri commands/events, capabilities, plugins, desktop packaging, or the frontend↔Rust IPC bridge.

## Project Layout

- `src-tauri/src/main.rs` — desktop entrypoint; keep thin.
- `src-tauri/src/lib.rs` — Tauri builder, plugin registration, command registration.
- `src-tauri/capabilities/` — capability/permission files. Keep least privilege.
- `src-tauri/tauri.conf.json` — app config, windows, bundle, security.
- Frontend calls Rust via `@tauri-apps/api/core` `invoke()`.

## Hard Rules

- **Contract first**: define command name, input/output types, and error shape before wiring UI.
- **Typed boundary**: Rust commands return `Result<T, E>` with serializable errors; frontend gets typed wrappers, not scattered `invoke` strings.
- **Least privilege**: add capabilities/plugins only when needed; never widen permissions "just in case".
- **No domain duplication**: CAM/domain logic belongs in Rust domain crates/modules; Tauri commands are adapters.
- **Logging**: use `tauri-plugin-log` for backend diagnostics; avoid `println!` in app code.

## Command Pattern

Rust:

```rust
#[tauri::command]
fn load_scene(id: String) -> Result<SceneSnapshot, AppError> { /* ... */ }
```

Register in `lib.rs` with `tauri::generate_handler![load_scene]`.

Frontend wrapper:

```ts
import { invoke } from '@tauri-apps/api/core';
export const loadScene = (id: string) =>
  invoke<SceneSnapshot>('load_scene', { id });
```

## Checks

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test  --manifest-path src-tauri/Cargo.toml
pnpm tauri dev
```

## Output Contract

Report: command/capability changed, files touched, IPC contract, verification run, and any permission/security impact.
