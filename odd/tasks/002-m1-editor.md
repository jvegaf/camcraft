# Feature: 2.5D CAM application — Milestone M1 (2D editor shell)

Program plan: `odd/roadmap.md` (M1 = "2D editor shell"). This document is the executable
specification for M1 only. Read this file until `## Evidence — commit per task

| Task | Commit | Notes |
| --- | --- | --- |
| T1 | `c03c06e` | `cam-app` state, snapshot history, bounded at 128; 12 tests |
| T2 | `a0950f4` | camera; 11 tests incl. a round-trip property test |
| T3 | `527afc5` | commands; 10 tests, every refusal named |
| T4 | `5665faf` | scene + grid; 10 tests. Fixed: arc bounds were tessellation-approximate |
| T5 | `f9d3a60` | pick/marquee/snap; 13 tests. cam-geom gained public distance queries |
| T6 | `d4edea5` | egui shell; 16 headless frame tests. Fixed: entity id counter ignored loaded ids |
| T7 | `b71fedc` | input wiring; 10 tests with synthetic events. Fixed: status lagged a frame |
| T8 | `3c1fbf1` | eframe binary; built and launched under a compositor, screenshot in `docs/images/` |
| T9 | `dabe910` | this documentation close-out |

## Log` before implementing any task.

M0 (merged to `master` as `6bbc52b`) delivered the domain: geometry, document format, tool library,
post-processor and the headless CLI. M1 adds the window — and only the window.

## Specs

Owner requirements in force (verbatim, unchanged from M0; S1 restated by the name-hygiene sweep):

- **S1** — A desktop 2.5D CAM application (restated; the owner's wording named a third-party product).
- **S2** — "quiero que sea cross platform Windows/Linux"
- **S3** — Decision D1 (user, verbatim option label): "Rust + wgpu/egui (o Rust + Qt bindings)"
- **S4** — Decision D3 (user, verbatim option label): "Formato propio abierto (JSON/ZIP) + import
  DXF/SVG/PDF/EPS"
- **S5** — (this milestone's instruction, verbatim): "mergea primero lo que tenemos a master, y de
  master te sacas la de m1" then "dale con m1"

M1 delivers the interactive editor the roadmap names: *"Interactive viewport: pan/zoom/snap, entity
picking, undo/redo, layer dock"*.

### Derived acceptance criteria (M1)

- **A1** — `cam-app` owns application state (document, camera, selection, history, dirty flag) and
  depends on **no UI crate**. Every command is applied through it.
- **A2** — Undo and redo restore the document **exactly**: any command sequence followed by the same
  number of undos yields a document identical to the original, and redo re-applies it.
- **A3** — History is bounded (a documented maximum) and reports when old steps are dropped; it never
  grows without limit while the user works.
- **A4** — The camera maps world millimetres to screen points and back; the round trip is stable, zoom
  is clamped to documented limits, and zoom-to-fit frames the job with a margin.
- **A5** — A viewport click selects the entity **nearest** to the cursor within a pixel tolerance, not
  merely one whose bounding box contains it; ties resolve deterministically.
- **A6** — Snapping proposes candidates (endpoint, midpoint, centre, grid) within a pixel tolerance and
  picks the nearest of the enabled kinds; the snapping decision is pure math, testable headless.
- **A7** — The shell renders the layer dock, a property view of the selected entity, a command bar and
  a status bar, and drives the canvas from camera + scene state.
- **A8** — Every user-visible action in the shell is expressible as a `UiAction` handled by one
  function, so the shell's behaviour is testable without a display.
- **A9** — The whole shell runs **headless**: a test builds a full frame with a synthetic `RawInput`
  and asserts the resulting state change, so UI behaviour is verified in CI without a GPU.
- **A10** — The 2D draw list is **backend-agnostic**: what to paint is computed in `cam-view`
  independently of the painter, so a future `wgpu` backend consumes the same list unchanged.
- **A11** — A native window binary builds for Windows and Linux and opens the editor on a sample
  document. Launching it requires a display and is therefore verified by build + wiring, not by
  automation.
- **A12** — All M0 behaviour is unchanged: the existing 123 tests stay green and the CLI contract is
  untouched.

## Tasks

One line per task: ID — acceptance criteria — route — commit.

- **T1** — A1, A2, A3 — inline — `feat(app): application state with snapshot undo and redo`
- **T2** — A1, A4 — inline — `feat(app): camera with pan, zoom, clamping and fit`
- **T3** — A1, A5, A8 — inline — `feat(app): commands for job setup, drawing and layer edits`
- **T4** — A10 — inline — `feat(view): backend-agnostic scene built from the document`
- **T5** — A5, A6 — inline — `feat(view): pick, box select and snap candidates`
- **T6** — A7, A8, A9 — inline — `feat(ui): egui shell with dock, canvas, properties and status bar`
- **T7** — A8, A9 — inline — `feat(ui): pointer, wheel and keyboard wiring through UiAction`
- **T8** — A11 — inline — `feat(desktop): native window binary for windows and linux`
- **T9** — A7, A11 — inline — `docs: m1 quickstart for the editor`

Route notes: M1 is a dependency chain (T1→T2→T3→T4/T5→T6→T7→T8), so no two tasks are independent units
of behaviour; parallel writers do not apply.

### Task detail

**T1 — `cam-app` state and history.** `AppState { document, camera, selection, history, dirty }`.
`Command` enum with `apply(&mut self, &mut Document)`; `AppState::execute(command)` records a snapshot
before the change; `undo`/`redo` move along a bounded ring with `MAX_HISTORY` (documented, 128) and a
`history_dropped` counter; a `dirty` flag that clears on mark-saved. Undo/redo must be exact: a test
applies five commands, undoes five, and compares with the original document byte for byte via
`serde_json`.

**T2 — Camera.** `Camera { center: Point2, pixels_per_mm: f64, viewport: Vec2 }` with
`world_to_screen`, `screen_to_world`, `pan_by_pixels`, `zoom_at(anchor_screen, factor)`,
`zoom_to_fit(bounds, margin)`, `MIN_PIXELS_PER_MM`, `MAX_PIXELS_PER_MM`. Tests: round trip within
1e-9 screen points at several zoom levels; zoom-at-anchor keeps the world point under the cursor fixed;
clamping at both limits; fit frames the job inside the viewport with the margin.

**T3 — Commands.** At least: `SetJob { width, height, thickness }`, `AddRectangle { layer, at, size }`,
`AddCircle { layer, at, radius }`, `DeleteEntity { id }`, `SetLayerVisible { id, visible }`,
`SetEntityLayer` is out of scope. Each validates through `Document::validate` after applying; an invalid
result is refused and the document restored. Deterministic id generation: `E<n>` with a counter in
`AppState`, not a random UUID, so tests and golden files are stable.

**T4 — Scene.** `cam_view::Scene` = ordered `Vec<SceneItem>` where an item is a polyline (world points)
or an arc (centre, radius, start, sweep) plus a style token (`Selected`, `Normal`, `Construction`,
`Grid`). `build_scene(document, selection) -> Scene`, and `Scene::bounds`. Arcs stay arcs in the scene;
only the painter flattens them. Tests: entity count, arc preservation, selection styling, bounds.

**T5 — Picking and snapping.** `pick(scene, camera, screen_pos, tolerance_px) -> Option<EntityId>` by
point-to-segment distance over the scene's items, deterministic on ties (lowest id). `box_select` by
rectangle intersection. `snap_candidates(document, camera, world_pos, tolerance_px, kinds) -> Vec<Snap>`
with `SnapKind::{Endpoint, Midpoint, Center, Grid}` and `Snap::choose` picking the nearest. Tests: a click
just off an edge within tolerance selects it, just beyond does not, ties pick the lowest id, box select
catches only intersecting entities, snap picks the nearest candidate of the enabled kinds.

**T6 — Shell.** `cam_ui::Shell` with `show(&mut self, app: &mut AppState, ctx: &egui::Context) ->
ShellOutput { canvas_rect, actions }`. Panels: top command bar (New, Undo, Redo, Zoom to fit), left
layer dock (visibility toggles, entity counts), right properties (selected entity id, kind, area/perimeter),
bottom status bar (cursor world position, zoom, history depth), centre canvas. No business logic inside
widgets: they emit `UiAction` and `handle_action(app, action)` performs it. Test: run one frame headless
via `egui::Context::run` with an empty `RawInput` and assert the panels reported content.

**T7 — Input wiring.** Pointer primary click → pick → select; drag with primary from empty space → box
select; middle drag or space+drag → pan; wheel → zoom at cursor; `Ctrl+Z`/`Ctrl+Shift+Z` → undo/redo;
`Delete` → delete selection; `F` → zoom to fit. Test: synthetic `RawInput` with pointer press/release at
a screen position computed through the camera selects the expected entity; wheel event changes zoom at
the anchor; a `Ctrl+Z` key event reverts the last command.

**T8 — Desktop binary.** `cam-desktop` with `eframe`, opening a sample document (a job with a rectangle
and a circle) so the window shows something on first launch. Must build on Linux; the Windows build is
the CI's job once a pipeline exists.

**T9 — Docs.** README gains an "editor" section (how to run, what works, what does not), the roadmap
marks M1 delivered with the same evidence table style as M0, and this file records the commits.

### Decisions taken at plan time

- **D-M1-1 — Undo by document snapshot, not inverse commands.** Documents are kilobytes; a snapshot per
  command is simple and provably exact (A2), while inverse commands across geometry edits are the exact
  place where "undo left something behind" bugs are born. Bounded ring with a dropped counter covers A3.
  Revisit trigger: if documents exceed a few MB (imported dense DXF), switch to a persistent data
  structure or a command journal, keeping the same `Command` API.
- **D-M1-2 — The canvas is painted with `egui`'s painter in M1; the draw list is backend-agnostic.**
  The roadmap named `wgpu` for `cam-view`. Splitting the *scene* (which is testable math) from the
  *painter* keeps the wgpu decision intact while making M1 verifiable without a GPU: `cam-view` emits
  `Scene`, and a wgpu backend consumes the same `Scene` when the 3D simulation (M5) actually needs the
  GPU. Revisit trigger: entity counts that stall the CPU painter, or M5's heightfield view — whichever
  comes first. Recorded as a deviation from the roadmap's crate description in the Log (L2).

## Evidence — commit per task

| Task | Commit | Notes |
| --- | --- | --- |
| T1 | `c03c06e` | `cam-app` state, snapshot history, bounded at 128; 12 tests |
| T2 | `a0950f4` | camera; 11 tests incl. a round-trip property test |
| T3 | `527afc5` | commands; 10 tests, every refusal named |
| T4 | `5665faf` | scene + grid; 10 tests. Fixed: arc bounds were tessellation-approximate |
| T5 | `f9d3a60` | pick/marquee/snap; 13 tests. cam-geom gained public distance queries |
| T6 | `d4edea5` | egui shell; 16 headless frame tests. Fixed: entity id counter ignored loaded ids |
| T7 | `b71fedc` | input wiring; 10 tests with synthetic events. Fixed: status lagged a frame |
| T8 | `3c1fbf1` | eframe binary; built and launched under a compositor, screenshot in `docs/images/` |
| T9 | `dabe910` | this documentation close-out |

## Log

- **L1 (user, verbatim):** "dale con m1" — authorizing this milestone after "mergea primero lo que
  tenemos a master, y de master te sacas la de m1".
- **L2 (deviation from the roadmap crate description):** the roadmap described `cam-view` as the `wgpu`
  renderer. M1 splits it: `cam-view` computes a backend-agnostic `Scene` (testable headless) and the
  shell paints it with `egui`'s painter. Rationale in D-M1-2. The user's stack decision (D1) is not
  contradicted: it stays "Rust + wgpu/egui", and the wgpu backend arrives with the 3D renderer that
  needs it, consuming the same `Scene`.
- **L3 (plan):** this document. Base: `master` at `6bbc52b` (M0 merged), branch `feat/m1-editor`.
- **L4 (execution, T1–T8).** Inline by the parent: the tasks form a strict dependency chain, so no two
  are independent units of behaviour, and the writer subagent was already proven unusable in M0 rather
  than merely unexamined — retrying it was not warranted. Every task landed as its own commit with all
  gates green before the commit (`.scripts/check.sh`: `fmt --check`, `clippy -D warnings`,
  `cargo test`). The suite stands at **208 tests across 17 suites**, up from 123.
- **L5 (egui 0.37 rework).** The 0.37 line moved panels onto a root `Ui` (`egui::Panel::left(...)`
  plus `ctx.root_ui`, and `eframe::App::ui` instead of `update`). The shell was written against the
  current API rather than pinning an older one, and it exposes both `show(app, ctx)` for hosts that
  only hold a context and `show_in(app, ui)` for hosts that already hold the root `Ui`.
- **L6 (defects found during M1, all fixed at the root).**
  (a) `AppState` started its entity counter at 1 regardless of the document, so drawing into a loaded
  file produced a duplicate `E1`; the counter is now seeded from the document's own ids and re-seeded
  on open.
  (b) `Arc` bounds were obtained by tessellation, so a circle's bounds were short by the chord
  tolerance and the viewport re-tessellated every arc every frame; bounds are now exact (endpoints
  plus the axis extremes the sweep crosses) and a property test pins it.
  (c) The status bar showed the previous frame's cursor, because panels are laid out before the
  central area; pointer tracking now runs first, using the previous frame's canvas rectangle.
  (d) Auto-framing fitted a provisional canvas reported on the first frames, leaving the drawing off
  centre; it now waits for the canvas size to stop changing.
- **L7 (verification honesty).** The window is the one thing automation cannot cover in this
  environment. It was verified by building and by launching it under the running Wayland compositor
  (`timeout` exit 124, i.e. still alive), then screenshotting it: `docs/images/editor-m1.png` shows the
  docks, the grid, the stock outline and the sample geometry, with the circle drawn as a smooth arc.
  That is evidence of rendering, not proof against every window-system variation, and the file records
  it as such.
- **L8 (test-side corrections, not implementation fixes).** Three input tests assumed the camera was
  still at 1 px/mm after auto-framing, so their world coordinates fell outside the canvas; the
  coordinates now sit inside the framed view. And `egui::RawInput` has no `modifiers` field: modifier
  state reaches `InputState` only through `Event::ModifiersChanged`, which the harness now emits the
  way a real backend does.
