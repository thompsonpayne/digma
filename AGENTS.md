# AGENTS.md — Digma Codebase Guide

## Project Overview

Digma is a single-user Figma-like vector editor built with a Rust/WASM engine and a WebGPU renderer.
Architecture: 4 Rust crates + a SolidJS/TypeScript frontend.

```
crates/
  engine/         # Pure Rust logic library — no I/O, no rendering, no_std-friendly
  renderer_wgpu/  # WebGPU renderer — wasm32 target only
  app_wasm/       # wasm-bindgen cdylib bridge exposing the API to the browser
  cli/            # Placeholder binary stub
web/              # SolidJS + Vite 6 frontend; imports WASM output from app_wasm
```

---

## Build Commands

### WASM (primary build)

```bash
# Build the WASM package and place it into the web src tree
wasm-pack build crates/app_wasm --target web --release --out-dir ../../web/src/wasm/app_wasm

# Shortcut via Makefile
make build-wasm
```

### Frontend dev server

```bash
pnpm -C web dev        # Start Vite dev server (requires wasm already built)
make run-web           # Makefile shortcut
```

### Frontend production build

```bash
pnpm -C web build      # Runs tsc -b then vite build
```

### Rust build (native, for development only)

```bash
cargo build            # Build all crates
cargo build -p engine  # Build only the engine crate
```

> Note: `renderer_wgpu` and `app_wasm` are `wasm32` targets only. `cargo build` on a native host
> will compile `engine` and `cli` successfully but will stub/fail for the wasm-only crates.

---

## Test Commands

### Run all Rust tests

```bash
cargo test
```

### Run tests for a single crate

```bash
cargo test -p engine
```

### Run a single test by name

```bash
cargo test -p engine -- test::world_screen_roundtrip_is_stable
cargo test -p engine -- test::hit_test_picks_topmost_rect
# General pattern: cargo test -p <crate> -- <module>::<test_name>
```

### Run tests matching a pattern

```bash
cargo test -p engine -- drag     # runs all tests with "drag" in the name
```

All tests live in `crates/engine/src/lib.rs` inside `#[cfg(test)] mod test { ... }`.
There are no JavaScript/TypeScript tests.

---

## Lint & Format Commands

### Rust

```bash
cargo fmt                   # Format all Rust code (no config file — uses rustfmt defaults)
cargo fmt --check           # Check without modifying (for CI)
cargo clippy                # Lint all crates
cargo clippy -- -D warnings # Treat warnings as errors
```

### TypeScript

```bash
pnpm -C web build           # tsc -b runs as part of the build, catching all type errors
```

TypeScript linting is enforced entirely by the compiler — there is no ESLint or Prettier config.
Formatting is left to editor defaults (2-space indent is used throughout the existing code).

---

## Code Style — Rust

### Naming conventions

- Types / Traits / Enum variants: `PascalCase` — `NodeId`, `RectNode`, `DragState`, `EngineOutput`
- Functions, methods, variables, fields: `snake_case` — `pan_by_screen_delta`, `drag_state`, `start_screen_px`
- Constants / statics: `SCREAMING_SNAKE_CASE`
- Use short, domain-meaningful names; avoid abbreviations except widely understood ones (`px`, `pos`, `id`)

### Imports

Group in this order, separated by blank lines:
1. `std` / `core` / `alloc`
2. Third-party crates (`serde`, `wgpu`, `wasm-bindgen`, …)
3. Local crate modules (`crate::`, `super::`)

Use selective imports (`use serde::{Deserialize, Serialize};`), not glob imports.
Re-export items at the crate root with `pub use` when they form the public API.

### Types and derive macros

- Always use explicit types on struct fields and function signatures; rely on inference only inside function bodies.
- Derive liberally: `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]` — add only what is needed.
- GPU-facing structs must use `#[repr(C)]` and derive `bytemuck::Pod + bytemuck::Zeroable`.
- Add explicit padding fields (`_pad0: f32`) for GPU uniform alignment; never rely on implicit padding.

### Error handling

- The `engine` crate is pure infallible logic — functions return values directly, never `Result`.
- WASM-facing functions in `renderer_wgpu` and `app_wasm` return `Result<T, JsValue>`.
- Convert errors with `.map_err(|e| JsValue::from_str(&format!("context: {e}")))`.
- Do not use `unwrap()` in library code; use `expect("reason")` only where a panic is truly impossible.

### Conditional compilation

- Gate all wasm32-only code with `#[cfg(target_arch = "wasm32")]`.
- Provide a non-wasm stub that returns an error or compiles to nothing, so `cargo build` on a native
  host does not break for development/test purposes.

### Documentation

- Use `///` doc comments on all public types, functions, and methods.
- Follow the pattern:
  ```rust
  /// One-line summary.
  ///
  /// # Arguments
  /// * `param` - description
  pub fn example(&self, param: Vec2) -> Vec2 { ... }
  ```
- Use `//` inline comments for non-obvious logic inside function bodies.

### GPU / wgpu specifics

- Always set `label: Some("descriptive label")` on every wgpu descriptor for GPU debugging.
- Keep shader source in a separate `.wgsl` file; include at compile time with `include_str!`.

### Tests

- Write tests in a `#[cfg(test)] mod test { ... }` block at the bottom of the file under test.
- Use small factory helpers (`fn engine_with_one_rect() -> Engine`) rather than duplicating setup.
- Add `assert_approx` / `assert_vec2_approx` helpers for floating-point comparisons; never use `==` on `f32`.
- Test names should read as sentences: `pan_by_screen_delta_moves_camera_origin`.

---

## Code Style — TypeScript / SolidJS

### Naming conventions

- Components: `PascalCase` function returning JSX — `App`, `CanvasView`
- Variables, functions, signals: `camelCase` — `canvasRef`, `createSignal`, `onMount`
- Type aliases: `PascalCase` — `Point`, `CameraView`, `InputEvent`
- File names: `PascalCase` for components (`App.tsx`), `camelCase` for utilities

### Imports

- Use ES module `import`/`export`; no CommonJS `require`.
- `verbatimModuleSyntax` is enabled — use `import type { Foo }` for type-only imports.
- Dynamic `import()` is required for the WASM module (loaded in `onMount`).

### Types

- `strict: true` and `noUncheckedSideEffectImports` are enabled — honour all strict-mode constraints.
- Prefer `type` over `interface` for local definitions.
- Use discriminated unions for variant data (e.g., `InputEvent` with a `kind` field).
- Non-null assertion (`!`) is acceptable only when the value is provably non-null at that point.
- Avoid `any`; use `unknown` when the type is genuinely unknown.

### SolidJS patterns

- Use `createSignal` / `createMemo` / `createEffect` for reactive state — not React hooks.
- Use `onMount` / `onCleanup` for side effects and teardown (not `useEffect`).
- Prefer the AbortController pattern for event listener cleanup:
  ```typescript
  const ac = new AbortController();
  window.addEventListener("pointermove", handler, { signal: ac.signal });
  onCleanup(() => ac.abort());
  ```

### Formatting

- 2-space indentation.
- No trailing semicolons omitted — use them consistently.
- No Prettier or ESLint config exists; match the style of surrounding code.
