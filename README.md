# Digma

Digma is a single-user, Figma-like vector editor built around a Rust engine compiled to WebAssembly and rendered in the browser with WebGPU.
Still a work in progress

## What it includes

- Rust workspace with a pure editor engine, a WebGPU renderer, and a wasm-bindgen bridge.
- SolidJS + TypeScript frontend powered by Vite.
- Interactive canvas flow for selection, rectangle creation, panning, zooming, undo/redo, and fill color changes.

## Repository layout

```text
crates/
  engine/         Pure Rust editor logic with tests
  renderer_wgpu/  WebGPU renderer for wasm32
  app_wasm/       wasm-bindgen bridge consumed by the web app
  cli/            Placeholder native binary
web/              SolidJS frontend that loads the generated WASM package
```

## Prerequisites

- Rust toolchain with `cargo`
- `wasm-pack`
- `pnpm`
- A WebGPU-capable browser

## Quick start

Install frontend dependencies:

```bash
pnpm -C web install
```

Build the WASM package into the frontend source tree:

```bash
make build-wasm
```

Start the development server:

```bash
make run-web
```

You can also run the commands directly:

```bash
wasm-pack build crates/app_wasm --target web --release --out-dir ../../web/src/wasm/app_wasm
pnpm -C web dev
```

## Common commands

```bash
# Rust
cargo build
cargo test
cargo test -p engine
cargo fmt
cargo clippy -- -D warnings

# Frontend
pnpm -C web build
pnpm -C web test:run
```

## Editor controls

- `Select` tool for picking and manipulating existing shapes.
- `Rectangle` tool for drawing a new rectangle, then automatically returning to select mode.
- Hold `Space` and drag to pan the camera.
- Use the mouse wheel to zoom toward the pointer.
- Use `Cmd/Ctrl + Z` to undo and `Cmd/Ctrl + Shift + Z` or `Cmd/Ctrl + Y` to redo.
- Use the color input to change the fill of the current selection.

## Architecture notes

- `crates/engine` owns editor state, input processing, history, selection, dragging, resizing, and render-scene generation.
- `crates/renderer_wgpu` draws the engine output with WebGPU.
- `crates/app_wasm` exposes an async `App::new(...)` constructor and a `tick(...)` bridge for the browser.
- `web/` collects browser input events, forwards batched input into WASM, and renders the application shell.

## Notes

- `renderer_wgpu` and `app_wasm` are intended for `wasm32` builds.
- The generated WASM artifacts live under `web/src/wasm/app_wasm`.
- The `cli` crate is currently a placeholder.
