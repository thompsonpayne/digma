# Rust Engine Refactor Plan for Future Realtime Collaboration

## Goal

Prepare the Rust engine for future realtime collaboration without throwing away the current single-player editor architecture.

The engine is already in a good starting state because the core editor logic is isolated from the browser and renderer, but the current implementation is still optimized for a local-only editor loop. The main refactor goal is to separate:

- shared document state
- local interaction state
- deterministic document operations
- history/undo semantics
- persistence and sync boundaries

This will let the current app remain simple today while making it possible to add a server-authoritative or room-authoritative collaboration model later.

## Why Refactor Now

Today, `Engine` owns both persistent document data and local editor session state.

Current responsibilities mixed inside `Engine` include:

- document data: `doc`
- camera/view state: `camera`
- local selection state: `selected`
- transient pointer interaction state: `drag_state`, `hover_screen_px`
- local history: `undo_stack`, `redo_stack`

This is a clean shape for a local editor, but it becomes a problem for collaboration because not all of this state should be shared across clients.

In a Figma-like system:

- the document model is shared
- camera is local to each client
- selection is usually local to each client, though remote presence may expose it separately
- drag previews are local transient state
- undo/redo must be scoped to user intent, not just a global document stack

If we do not separate these concerns early, later multiplayer work will force a much more painful rewrite.

## Current Engine Issues That Will Matter for Realtime

### 1. Shared and local state are coupled

`Engine` currently mutates the document directly while also owning local interaction state. This makes it difficult to:

- replay document operations on another client
- apply remote operations without disturbing local UI state
- make the back-end authoritative for ordering

### 2. Input handling and document mutation are tightly combined

`tick(&InputBatch)` currently does all of the following in one pass:

- interprets user input
- updates local drag state
- mutates the document
- maintains local undo/redo
- produces render output

For real-time collaboration, document mutation must become more explicit and serializable.

### 3. Commands are close to collaboration ops, but not quite there

`ToolCommand` is a strong starting point, but it is still designed around local editor history. Some variants include local selection bookkeeping, and the command set is shaped around undo semantics rather than a network protocol.

### 4. ID generation is local-only

`Document::alloc_id()` currently allocates IDs from a local counter. That works for one editor instance, but in multiplayer it risks collisions unless IDs become server-issued or client-scoped.

### 5. Undo/redo is local-stack based

The current undo model assumes one linear local change stream. Collaborative editors need a more explicit model for:

- which user created which operation
- how local undo maps to previously acknowledged operations
- what happens when remote operations interleave

### 6. Rendering output is derived from the entire engine state

This is fine for now, but the long-term engine boundary should make it clear which output depends on shared document data and which depends on local interaction state.

## Target Architecture

The engine should move toward five distinct layers.

### 1. Document Core

Pure shared document state.

Responsibilities:

- nodes and their properties
- z-order
- stable node identifiers
- document metadata
- deterministic application of document operations

Must not own:

- camera
- hover state
- drag previews
- local selection UI
- local undo stacks

Suggested shape:

```rust
pub struct DocumentModel {
    pub next_id: u64,
    pub rects: Vec<RectNode>,
}

impl DocumentModel {
    pub fn apply_op(&mut self, op: &DocumentOp) -> ApplyResult {
        // deterministic shared mutation only
    }
}
```

### 2. Local Editor Session

Per-client ephemeral editing state.

Responsibilities:

- camera
- local selection
- hover state
- current drag interaction
- in-progress shape creation
- maybe local tool mode

Suggested shape:

```rust
pub struct EditorSession {
    pub camera: Camera,
    pub selected: Vec<NodeId>,
    pub drag_state: DragState,
    pub hover_screen_px: Option<Vec2>,
}
```

This state should be safe to discard and reconstruct without losing the actual document.

### 3. Intent Interpreter

A layer that converts raw input events into higher-level editor intents and candidate document operations.

Examples:

- pointer drag over selected rects -> `MoveRectsIntent`
- color picker change -> `SetFillIntent`
- delete key -> `DeleteNodesIntent`

This layer should manage local interaction rules, thresholds, and gestures, but it should not directly mutate the shared document model when that mutation can instead be expressed as an explicit operation.

### 4. Document Operations Layer

A serializable operation model that becomes the source of truth for persistence, sync, and deterministic replay.

Suggested examples:

```rust
pub enum DocumentOp {
    CreateRect {
        id: NodeId,
        pos: Vec2,
        size: Vec2,
        color: [f32; 4],
    },
    SetRectsGeometry {
        changes: Vec<RectGeometryPatch>,
    },
    SetRectsFill {
        changes: Vec<RectFillPatch>,
    },
    ReorderNodes {
        node_ids: Vec<NodeId>,
        placement: ReorderPlacement,
    },
    DeleteNodes {
        node_ids: Vec<NodeId>,
    },
}
```

These ops should be:

- deterministic
- serializable
- replayable
- independent from local UI-only state

### 5. History / Collaboration Layer

Local history should be rebuilt around document operations and inverse operations, instead of tightly coupling history to direct engine mutation.

Long-term responsibilities:

- capture inverse ops for local undo
- track operation author/session
- reconcile local optimistic ops with server ordering
- support snapshots plus operation replay

## Refactor Principles

### Keep the renderer boundary stable

The existing renderer pipeline should change as little as possible. The engine can still output `RenderScene` and `OverlayScene`, but those outputs should be computed from a split model:

- `RenderScene` from shared document state
- `OverlayScene` from local session state plus document state

### Do not introduce CRDT complexity too early

The first refactor should not attempt to fully solve multiplayer conflict resolution. It should only create the boundaries needed to support a future collaboration protocol.

### Favor deterministic operation application

If the same sequence of document operations is replayed on two machines, the resulting document should match exactly.

### Preserve current single-player behavior during the refactor

The app should keep working as a local editor while the architecture is being split underneath.

## Proposed Phases

## Current Progress

Status as of 2026-03-29:

- Phase 1: completed
- Phase 2: in progress
- Phase 3: not started
- Phase 4: not started
- Phase 5: not started
- Phase 6: not started

## Phase 1 - Separate Shared Document State from Local Session State

### Objective

Split `Engine` into two conceptual parts without changing visible behavior.

### Current status

**Completed.** All Phase 1 objectives have been achieved.

Completed items:

- Introduced `DocumentModel` in `types.rs` and kept `Document` as a compatibility alias
- Introduced `EditorSession` in `session.rs`
- Changed `Engine` into a composition root that owns `document: DocumentModel` and `session: EditorSession`
- Kept undo/redo on `Engine` for now, matching the Phase 1 goal of boundary cleanup without redesigning history yet
- Moved document-only helpers onto `DocumentModel`:
  - `alloc_id()`
  - `check_collide_rects()`
  - `rect_index()`
  - `rect()`
  - `rect_mut()`
  - `reorder_selected()`
- Moved local selection policy onto `EditorSession` with `apply_selection()`
- Moved the local pointer interaction entry points onto `EditorSession`:
  - `pointer_down()`
  - `pointer_move()`
  - `pointer_cancel()`
- Moved local interaction and preview helpers onto `EditorSession`:
  - `check_collide_handle()`
  - `update_marquee_drag()`
  - `update_marquee_selection()`
  - `update_move_drag()`
  - `apply_selection_drag()`
  - `update_resize_drag()`
  - `apply_selection_resize()`
  - `update_rect_create_drag()`
  - `rollback_active_drag()`
- Moved pure resize math into `drag.rs` as `compute_resize()`
- Verified current behavior still passes the engine test suite with `cargo test -p engine`

Not yet extracted (deferred to later phases):

- `compute_cursor()` — read-only cursor style computation
- overlay construction — read-only scene construction for selection, marquee, and rect-create previews
- These are cosmetic read-only helpers that don't affect the document/session boundary

Notes from implementation:

- `geometry_change_for_rect()` still lives on `Engine` because it depends on history-layer types (`RectGeometry`, `RectGeometryChange`), so moving it now would blur the Phase 1 boundary
- `PointerUp` logic still lives on `Engine` for the same reason: it currently produces history-oriented `ToolCommand` values rather than document-level operations
- `EditorSession` interaction helpers currently take `&DocumentModel`, `&mut DocumentModel`, or rect slices depending on whether the interaction is read-only or preview-mutating; this keeps the session/document ownership split explicit without forcing a larger module restructure yet
- The larger `document/` and `editor/` module restructure is intentionally deferred until the ownership split is stable
- After the current refactor, `Engine::tick()` is now significantly simplified — it mostly delegates to `session.pointer_down()`, `session.pointer_move()`, and `session.pointer_cancel()` for local interaction, while keeping history/commit logic inline. This makes the Phase 2 boundary (where `PointerUp` commit points become explicit `DocumentOp` calls) much clearer to identify.

### Changes

- Introduce `DocumentModel` for shared document contents
- Introduce `EditorSession` for camera, selection, hover, drag state
- Change `Engine` into a thin composition root that owns both
- Move any method that only reads or mutates document data onto `DocumentModel`
- Move interaction-only logic onto `EditorSession` or helper modules

### Expected result

The code still runs locally, but it becomes obvious which state is collaboration-relevant and which is not.

### Notes

This is the most important structural step. Without it, later work will stay tangled.

## Phase 2 - Introduce Explicit Document Operations

### Objective

Replace direct document mutations hidden inside `tick()` with explicit document operations.

### Current status

In progress. Most document mutations now route through `DocumentModel::apply_op()`.

Completed so far:

- Added `RectFillChange` struct to `history.rs`
- Fixed imports in `ops.rs` (added `RectFillChange` import)
- Added `apply_op()` method to `DocumentModel` in `types.rs`
- Added exports for new types in `lib.rs`: `RectFillChange`, `DocumentOp`, `ReorderPlacement`
- Updated `engine.rs` to use `apply_op()`:
  - `SetSelectionFill` → `DocumentOp::SetRectsFill`
  - `BringForward` → `DocumentOp::ReorderNodes`
  - `SendBackward` → `DocumentOp::ReorderNodes`
  - `DeleteSelected` → `DocumentOp::DeleteNodes` (forward path only, undo still uses `ToolCommand::Delete`)
- Updated `apply_command` to delegate `ToolCommand::Delete` forward path to `DocumentOp::DeleteNodes`

Still remaining:

- `PointerUp` → `SelectionMove`, `Resize`, `RectCreate` still use `ToolCommand` directly (the operations are built as `ToolCommand::SetRectsGeometry` and `ToolCommand::CreateRect`, which work but don't yet go through explicit `DocumentOp` application)
- These could be converted to `DocumentOp` variants, but the current flow works correctly
- `SetSelectionFill` currently has no undo support (direct apply without history entry)
- The remaining direct mutation in `apply_command` for `ToolCommand::SetRectsGeometry` and `ToolCommand::BringForward`/`SendBackward` could optionally be converted to delegate to `DocumentOp`

### Changes

- Introduce `DocumentOp`
- Replace direct mutation paths with `apply_op()` or `apply_ops()`
- Keep temporary local drag previews local until commit points where appropriate
- Convert existing `ToolCommand` logic into two layers:
  - document-level operation types
  - local history metadata and inversion support

### Expected result

All persistent document changes pass through a small, well-defined API.

### Why this matters

This is the future sync protocol boundary. Backend messages, persistence logs, and undo logic can all converge on this model.

## Phase 3 - Redesign History Around Invertible Operations

### Objective

Make undo/redo operation-based rather than ad hoc engine-state based.

### Changes

- Introduce an inversion mechanism for document operations
- Store history as applied op groups with inverse op groups
- Move selection restoration and other local UI concerns out of the shared document op format
- Decide which local UI state belongs in a separate local-history structure

### Expected result

Undo remains correct locally and becomes easier to adapt to collaborative semantics later.

### Important caveat

Collaborative undo is not just local stack rewind. The first target is a sound local abstraction, not final multiplayer undo behavior.

## Phase 4 - Introduce Stable Collaboration-Oriented IDs and Metadata

### Objective

Prepare all persistent document entities and operations for distributed creation.

### Changes

- Replace local-only sequential IDs or wrap them in a future-safe type
- Add operation metadata structure, for example:

```rust
pub struct OpEnvelope {
    pub op_id: OpId,
    pub actor_id: ActorId,
    pub base_version: DocumentVersion,
    pub op: DocumentOp,
}
```

- Keep the metadata layer separate from the pure document op if that keeps the core cleaner

### Expected result

The engine can accept document changes that were created elsewhere, not just locally.

## Phase 5 - Add Serialization, Snapshots, and Replay

### Objective

Support loading, saving, replaying, and eventually syncing documents through a backend.

### Changes

- Add snapshot serialization for the document model
- Add operation log serialization
- Add `load_snapshot()` and `replay_ops()` style APIs
- Add tests for snapshot roundtrip and deterministic replay

### Expected result

The engine becomes usable as a persistence and sync core, not just an in-memory editor loop.

## Phase 6 - Add Collaboration Integration Boundary

### Objective

Make it easy for a future network layer to drive the engine.

### Changes

- Add methods like:

```rust
pub fn apply_remote_op(&mut self, envelope: OpEnvelope)
pub fn produce_local_ops(&mut self, input: &InputBatch) -> Vec<OpEnvelope>
pub fn acknowledge_local_op(&mut self, op_id: OpId)
```

- Keep network transport outside the engine
- Keep the engine runtime-agnostic so it can run in WASM and potentially on the server

### Expected result

The future backend can order, validate, and broadcast operations while the client engine stays deterministic.

## Recommended Module Restructure

One possible target layout:

```text
crates/engine/src/
  lib.rs
  camera.rs
  input.rs
  types.rs
  document/
    mod.rs
    model.rs
    ops.rs
    apply.rs
    snapshot.rs
  editor/
    mod.rs
    session.rs
    intents.rs
    history.rs
    interaction.rs
  render_scene.rs
  engine.rs
```

Possible responsibilities:

- `document/model.rs`: shared persistent state
- `document/ops.rs`: operation definitions
- `document/apply.rs`: deterministic op application
- `document/snapshot.rs`: save/load helpers
- `editor/session.rs`: local ephemeral state
- `editor/intents.rs`: input-to-intent translation
- `editor/history.rs`: local undo/redo abstractions
- `editor/interaction.rs`: drag and gesture state transitions
- `engine.rs`: orchestration layer bridging input, session, document, and render output

## Concrete Refactor Tasks

### Task 1 - Extract document-only methods

Move methods that only inspect or mutate document contents away from the monolithic engine type.

Examples likely include:

- rect lookup helpers
- z-order manipulation helpers
- geometry application helpers
- document mutation helpers used by commands

### Task 2 - Extract session-only methods

Move methods that only depend on camera, hover, drag, and selection toward an editor session module.

Examples likely include:

- drag state transitions
- marquee interaction state
- hover updates
- local selection policy

### Task 3 - Replace direct mutation with operation application

When the editor commits a user action, convert it into one or more document operations and apply them through a single path.

### Task 4 - Separate visual preview from committed mutation

For interactions like dragging or resizing, decide whether the engine should:

- keep optimistic local preview state separate until commit, or
- apply temporary document changes plus rollback/inversion support

For collaboration, the first option is usually cleaner, though it may require more explicit preview structures.

### Task 5 - Introduce operation metadata incrementally

Do not force full multiplayer metadata into the first operation type if it complicates the local engine. It is acceptable to begin with pure `DocumentOp` and later wrap it in `OpEnvelope`.

## Suggested Data Ownership Rules

These rules should guide implementation decisions.

### Shared document state

Should include:

- nodes
- geometry
- fill/style properties
- ordering
- document-level metadata

Should not include:

- per-user camera
- local hover state
- pointer drag state
- local-only selection behavior

### Per-user local session state

Should include:

- viewport/camera
- active tool if tool choice is client-local
- current selection
- drag anchors and thresholds
- hover information
- optimistic preview bookkeeping

### Remote presence state

This is separate from both local session and persistent document state.

Should eventually include:

- collaborator cursors
- collaborator selections
- user display info
- room presence metadata

It may be rendered by the client, but it should not live inside the core persistent document model.

## Undo/Redo Strategy Recommendation

Recommended direction:

- model each committed user edit as one or more document operations
- capture the inverse operations at commit time
- store local undo history as operation groups
- keep selection restoration in local editor state, not in the shared op format unless absolutely necessary

This keeps the shared operation protocol cleaner while still supporting a familiar local editing experience.

Long-term, collaborative undo may need a separate policy layer, but this approach gets the engine into the right shape.

## ID Strategy Recommendation

Do not keep raw local sequential IDs as the long-term public model.

Safer options:

- server-assigned globally unique IDs
- client-scoped IDs such as `(actor_id, local_counter)`
- UUID-like IDs if storage and protocol overhead are acceptable

For now, the engine can keep a simple internal representation, but it should be wrapped in a type that can evolve without rewriting the entire document model.

## Testing Strategy

The refactor should be guarded by tests at three levels.

### 1. Document operation tests

Verify that applying document operations yields deterministic results.

Examples:

- create rect adds rect with expected properties
- set geometry updates only targeted nodes
- delete and reorder preserve invariants
- replaying the same op sequence twice yields the same document

### 2. History tests

Verify invertibility and local undo correctness.

Examples:

- applying op then inverse returns original document
- grouped drag commit undoes as one user action
- redo restores the same final document

### 3. Interaction tests

Verify that input interpretation still produces the same visible behavior.

Examples:

- drag threshold behavior remains stable
- marquee selection behavior remains stable
- local selection rules remain unchanged

## Migration Risks

### Risk 1 - Regressing local editing behavior

Mitigation:

- keep existing tests passing throughout
- preserve the current external engine API until the new layers settle

### Risk 2 - Over-designing for multiplayer too early

Mitigation:

- first refactor for boundaries, not full collaboration semantics
- keep CRDT/OT choices out of the engine until the product needs them

### Risk 3 - Mixing preview state with committed state again

Mitigation:

- enforce clear ownership rules for local session vs document state
- review new structs by asking whether they should survive serialization and replay

## Recommended Implementation Order

If this work starts soon, the order below is the safest:

1. Extract `DocumentModel` and `EditorSession`
2. Move document-only helpers out of `Engine`
3. Introduce `DocumentOp`
4. Route committed edits through `apply_op()`
5. Rebuild history around invertible ops
6. Add snapshot/replay support
7. Add op metadata and remote-application hooks

## Final Recommendation

The engine does not need to be replaced for realtime collaboration, but it does need a boundary cleanup.

The most important architectural decision is this:

- treat the document model as shared deterministic state
- treat editor interaction as local ephemeral state
- treat committed edits as explicit serializable operations

Once that split exists, the current Rust engine can remain the foundation of the product while a future backend handles ordering, presence, persistence, and broadcast.

That is the point where the engine becomes collaboration-ready instead of local-editor-only.
