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
- Basic style edits from the toolbar (fill color today; full inspector deferred).

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

## Current Progress (as of Mar 8, 2026)

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
- Marquee overlay rendering implemented in `OverlayScene`.
- Marquee drag now starts only on empty space (no hit), avoiding accidental marquee on top of shapes.
- Drag threshold added for empty-space marquee initiation (pending marquee state).
- `OverlayScene` now renders selection outlines/handles from engine selection state.
- Drag interactions refactored to a unified engine drag state machine (`DragState`): `Idle`, `PendingMarquee`, `Marquee`, `PendingSelectionMove`, `SelectionMove`.
- Move-selection path implemented with drag threshold and world-space delta.
- Selection move uses an origin snapshot (`Vec<(NodeId, Vec2)>`) to avoid cumulative drift while dragging.
- Clicking an already-selected shape (without shift) now preserves the multi-selection and starts pending move.
- Shift remains selection-modifier behavior (toggle/additive), not move-start behavior.
- Tests added for selection move behavior: single-drag (full flow, below-threshold no-op, cancel), multi-drag (all selected move by same delta, unselected rect untouched, no cumulative drift).
- Resize handles implemented: `PendingResize` / `Resize` drag states added; all four corners (TL, TR, BL, BR) resize with min-size clamping and no cumulative drift.
- `CursorStyle` enum added to `EngineOutput` (`Default`, `ResizeTlBr`, `ResizeTrBl`, `Move`); engine computes cursor from hover position and drag state each tick.
- `hover_screen_px` tracked in `Engine`; all `PointerMove` events (including hover-only) now forwarded to the engine from the frontend.
- Frontend reads `out.cursor` and applies the corresponding CSS cursor to the canvas element.
- Engine crate refactored from a single 1450-line `lib.rs` into focused modules: `types`, `camera`, `input`, `drag`, `engine` — `lib.rs` is now a 13-line re-export root.
- `ToolMode` now flows through `InputBatch` end-to-end (`Select`, `Rect`) so the engine can branch interaction behavior per active tool.
- Rectangle tool implemented in the engine with `PendingRectCreate` / `RectCreate` drag states, drag-threshold activation, live overlay preview, and commit-on-release creation.
- New rectangles are normalized from drag bounds, clamped to a minimum size, inserted into the document with a fresh `NodeId`, and auto-selected after creation.
- Frontend now exposes explicit Select / Rectangle toolbar controls and syncs tool changes into the per-frame input batch.
- Canvas input handling now supports space-drag panning in the frontend and automatically switches back to Select after finishing a rectangle creation gesture.
- Cursor behavior expanded for drawing/panning UX: rect creation shows a crosshair, while the frontend resolves grab / grabbing during space-pan interactions.
- Web app runtime split into focused frontend modules (`App`, `createCanvasInputController`, `createWasmApp`, `editorTypes`) instead of a monolithic entry flow.
- Tailwind CSS is now wired into the Vite frontend, and the app has a basic editor shell with header, toolbar, and framed canvas workspace.
- Toolbar fill color editing added; selected rectangles can now be recolored from the frontend.
- Frontend test setup added with Vitest + Solid Testing Library; smoke tests now cover app boot rendering and rectangle tool switching.
- Command history module added in `engine` (`history.rs`) with gesture-level commands for rect creation and rect geometry edits (`CreateRect`, `SetRectsGeometry`).
- Rectangle create / move / resize now finalize through the command system: create commits a `CreateRect` command, while move and resize capture before/after geometry and push a single history entry on pointer release.
- Engine now maintains undo/redo stacks and handles `InputEvent::Undo` / `InputEvent::Redo`, including redo invalidation on new edits.
- Drag-state snapshots were upgraded to carry stable IDs and prior selection where needed, keeping move/resize/create history tied to `NodeId` rather than transient rect indices.
- Pointer cancel now rolls active move/resize gestures back to their drag-start snapshot instead of silently leaving partial edits in the document.
- Frontend input types and canvas controller now emit undo/redo shortcut events and defer the post-rectangle auto-switch back to Select until after the current input batch is flushed.
- Verified status: `cargo test -p engine` passes (19 tests), and `pnpm -C web test:run` passes.

## Next Implementation Step

- Layers + ordering: implement a selection-synced layers list and undoable bring-forward / send-backward / delete commands, using `Document.rects` order as the single source of truth (Week 5).
