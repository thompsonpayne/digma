# Figma-Clone (Rust Engine + WebGPU) Plan (6-8 Weeks)

Goal: a usable single-user vector editor demo with a Rust core engine (WASM) and a WebGPU renderer, plus a DOM-based text overlay (no GPU text in MVP).

Non-goals (MVP): real-time collaboration, constraints/auto-layout, components/variants, boolean ops, full SVG fidelity.

## Architecture

### Crates / Modules

- `engine` (pure Rust library)
  - Document model + stable IDs
  - Commands + undo/redo
  - Tool state machines (select, rect, etc.)
  - Hit-testing + snapping
  - Produces a compact `RenderScene` + `OverlayScene`
  - Produces `TextOverlayScene` for the DOM text layer

- `renderer_wgpu` (Rust library)
  - WebGPU renderer (`wgpu` on wasm32)
  - Draws `RenderScene` (shapes) + `OverlayScene` (selection/handles)
  - Keeps GPU resources (pipelines, bind groups, buffers) and performs partial updates

- `app_wasm` (Rust `cdylib`)
  - `wasm-bindgen` bridge
  - Owns the `engine` instance + `renderer_wgpu` instance
  - Accepts per-frame input batches from JS and returns a small output payload:
    - render stats + dirty flags
    - `TextOverlayScene` (or a diff) for DOM updates

- `web/` (TypeScript)
  - UI: toolbar, layers panel, inspector, menus, shortcuts
  - Hosts the WebGPU canvas
  - Maintains the DOM text overlay layer (positioned HTML elements)
  - Feeds input batches to WASM each animation frame

### Core design choices

- **Stable `NodeId`** for all document nodes.
- **Command log** for all edits:
  - Each edit is `Command -> Vec<Op>`; undo applies inverse ops.
  - Enables deterministic undo/redo and later supports multiplayer.
- **Input batching** to reduce JS<->WASM overhead:
  - JS collects pointer/keyboard/wheel events into a frame batch.
  - WASM processes the batch once per frame: `tick(input_batch)`.
- **Dirty flags**:
  - Engine emits `DocDirty`, `SelectionDirty`, `TextDirty`, `RenderDirty`.
  - Renderer only re-uploads changed instance ranges.

## Data Model (MVP)

### Node types

- `Rect { size, corner_radius? }`
- `Ellipse { radii }` (optional)
- `Text { content, font, size, align, color }` (DOM overlay)

All nodes share:

- `transform`: position + scale + rotation (start with position+scale; rotation optional)
- `style`: fill, stroke, stroke_width
- `z_index` (or explicit ordering in a parent container)
- `visible`, `locked` (optional for MVP)

### Viewport / camera

- `Camera { pan, zoom }`
- Helpers to map screen <-> world coordinates.

## Rendering

### WebGPU primitives (MVP)

- Rectangles and ellipses rendered via instancing.
- Strokes (for selection outlines / handles) can be:
  - simple line shader with screen-space thickness, or
  - tessellated geometry via `lyon` (optional early).

### Overlay pass

- Selection bounds, handles, hover highlights.
- Drawn in a separate pass so it stays crisp and predictable.

## DOM Text Overlay

### Rationale

- Avoids GPU text shaping/atlas complexity in MVP.
- Enables acceptable "usable" text quickly.

### Approach

- Engine stores `Text` nodes and produces a `TextOverlayScene` each tick (or on dirty):
  - list of text items with stable IDs, content, font info, color
  - world-space transform + bounds
- JS maintains an absolutely-positioned overlay container above the canvas:
  - for each text item, create/update a `div` (or `span`) with CSS:
    - `position: absolute`
    - `transform: translate(...) scale(...)` based on camera + node transform
    - set `font-family`, `font-size`, `line-height`, `color`
  - pointer events for text items can be forwarded to the engine (or disabled and rely on canvas hit testing).

Notes:

- For MVP, measure text with DOM (`getBoundingClientRect`) and optionally feed the measured size back to the engine.
- If that round-trip is too expensive, approximate text bounds initially.

## Weekly Milestones (6-8 weeks)

### Week 1: Boot + Render Loop

- WebGPU canvas working on desktop + mobile.
- `engine.tick(input_batch)` wired up.
- Camera pan/zoom at 60fps.
- Render a handful of hardcoded rect instances.

Deliverable: "blank canvas" app that feels smooth.

### Current Progress (as of Feb 22, 2026)

- WebGPU canvas booted on wasm32 via `renderer_wgpu`.
- Input batching from `web/` to `app_wasm` tick loop.
- Camera pan/zoom implemented in `engine::Camera`.
- `RenderScene` struct added and emitted from the engine.
- Renderer consumes `RenderScene` via instanced drawing (unit quad + instance buffer).
- Camera uniform wired into WGSL and applied in the vertex shader.
- `OverlayScene` added in the engine and included in `EngineOutput`.
- Renderer uploads overlay instances and draws them in a second render pass.
- `app_wasm` bridges `overlay_scene` into the renderer.
- Document now stores rect nodes with stable `NodeId`.
- Hit-testing added for rects in world space.
- Click-to-select implemented (with shift-toggle).
- Marquee selection logic implemented (world-space AABB intersection, shift-additive).
- `OverlayScene` now renders selection outlines/handles from engine selection state.

### Week 2: Document + Commands + Persistence

- Document model with `NodeId` + ordering.
- Command system + undo/redo.
- Save/load JSON (localStorage + file import/export optional).

Deliverable: create a few shapes via dev controls and persist them.

### Week 3: Hit Testing + Selection

- AABB hit-testing in world space.
- Click-to-select, multi-select (shift), marquee select.
- Selection overlay rendering (bounds + handles).

Deliverable: selection feels reliable.

### Week 4: Tools + Transforms

- Rectangle tool (drag to create).
- Move selection (drag).
- Resize handles (scale).
- Inspector edits for x/y/w/h, fill/stroke.

Deliverable: basic drawing + editing workflow.

### Week 5: Layers + Ordering + Snapping

- Layers list UI with selection sync.
- Bring forward/backward.
- Snap-to-grid (toggle).
- Basic keyboard shortcuts (delete, undo/redo, copy/paste optional).

Deliverable: "mini Figma" workflow with layers.

### Week 6: Text Nodes (DOM Overlay) + Export

- Text tool: click to insert, edit content in UI.
- DOM overlay renders text and stays aligned through pan/zoom.
- Export PNG (render-to-texture + readback; include overlay text by compositing in JS).

Deliverable: shapes + text + export.

### Weeks 7-8 (optional polish)

Pick 1-2:

- Ellipses + rounded rects.
- Better snapping (to edges/centers).
- Improved performance (spatial index, partial buffer updates).
- Basic SVG export (shapes + text as `<text>`).
- Path tool (polyline) with stroke.

## Acceptance Criteria for MVP

- Pan/zoom remains smooth with ~1k rectangles.
- Create/move/resize rectangles with undo/redo.
- Multi-select + marquee selection.
- Layers reorder works.
- Text nodes render via DOM overlay and remain correctly aligned at various zoom levels.
- Save/load round-trip preserves document.

## Risks / Mitigations

- JS<->WASM overhead: batch inputs per frame; avoid per-node getters.
- WebGPU availability / browser differences: prefer `wgpu` wasm path; keep feature usage minimal in MVP.
- Text alignment issues: keep overlay container in the same stacking context; use a single source of truth for camera transforms.
- Export including text: composite canvas + overlay text via an offscreen canvas in JS.

## Next Implementation Step

- Wire marquee selection behavior into UI polish: additive drag UX (shift) + clean up selection updates.
