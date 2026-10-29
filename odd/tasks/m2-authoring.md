# Feature: the reference CAM application-class CAM application — Milestone M2 (vector authoring & editing)

Program plan: `odd/roadmap.md` (M2 = "Vector authoring & editing"). This document is the executable
specification for M2 only. Read this file until `## Log` before implementing any task.
Stack: Rust workspace, `cam-geom` kernel first, then `cam-app` commands, then `cam-ui` wiring.

## Specs

Requirements as stated by the owner (verbatim):

- **S1** — "quiero crear una aplicacion similar a the reference CAM application"
- **S2** — "quiero que sea cross platform Windows/Linux"
- **S3** — Roadmap M2 row (verbatim, `odd/roadmap.md`): "Line/polyline/arc/circle/ellipse/rectangle/polygon/star, text, node editing,
  join/weld/trim, offset, mirror/rotate/scale, array copy, align"

Decisions taken by the owner for this milestone (verbatim option labels):

- **S4** — Decision D-M2-1 (user, verbatim option label): "Trazo simple ahora, TrueType después"
- **S5** — Decision D-M2-2 (user, verbatim option label): "Teselar automáticamente con tolerancia
  declarada"

Owner requirements still in force from earlier milestones:

- **S6** — Decision D1 (user, verbatim option label): "Rust + wgpu/egui (o Rust + Qt bindings)"
- **S7** — (this session's instruction, verbatim): "Arrancar M2 (plan de tareas ODD primero)"

Everything else in this document is a **derived acceptance criterion** for M2 — an engineering
consequence of S1–S7 with no new scope. Each one is falsifiable and is the thing a task is checked
against.

### Derived acceptance criteria (M2)

- **A1** — Every primitive in S3 other than text and ellipse is built by one `cam-geom` builder that
  returns a `Contour` and **pre-tessellates nothing**: line, polyline, rectangle, circle, regular
  polygon, star and arc-through-three-points are exact bulge/arc geometry.
- **A2** — A circle is exactly two half-turn arcs and a regular polygon is exactly N straight
  segments; each builder's analytic area, perimeter and `is_ccw` winding are asserted, not its vertex
  dump.
- **A3** — The ellipse is an arc approximation with a **documented, caller-visible maximum radial
  error**; no builder in the kernel silently loses that error.
- **A4** — `translate`, `rotate` about a point, uniform `scale` about a point and `mirror` about a
  line are **exact on arcs**: centre, radius and angles are transformed, the contour keeps its
  closedness, and a mirrored closed contour comes back counter-clockwise (winding normalised).
- **A5** — A non-uniform scale (factors X ≠ Y) over geometry containing arcs **tessellates** the
  affected contours at the declared tolerance and the result reports **how many entities lost their
  arcs** (S5). Exact cases (X = Y, or no arcs at all) stay exact and report zero.
- **A6** — Node editing operates on a `Contour`: move a vertex, insert a vertex on a segment, delete a
  vertex. Inserting on an arc **splits it exactly**, preserving the arc through the new point, and a
  deletion that would leave fewer than two vertices is refused.
- **A7** — Segment intersections exist for line/line, line/arc and arc/arc, are tolerance-driven, and
  report tangent and overlapping coincident cases explicitly instead of dropping them.
- **A8** — Join/weld merges contours whose endpoints match within a tolerance, reversing as needed,
  and **refuses naming why** when the chain is ambiguous (three or more contours meeting at one
  point) or when the group does not form exactly one chain.
- **A9** — Trim removes the piece of a contour between two intersections with a cutting contour and
  refuses when the cut points are not unambiguous.
- **A10** — Offset is reachable as a document command, keeps arcs as arcs for a closed contour with
  arcs, and propagates `OffsetCollapsed` instead of producing a self-intersecting result.
- **A11** — Array copy produces an explicit R×C grid copy with independent X and Y spacing from a
  fixed origin and deterministic entity ids; align places selected entities against a reference
  left/right/top/bottom/horizontal-centre/vertical-centre with exact arithmetic on the bounds.
- **A12** — Every authoring and editing action in S3 is a `Command` applied through
  `AppState::execute`, so it participates in M1's exact undo/redo and is refused **atomically**: a
  refused command leaves the document and the history depth unchanged.
- **A13** — Every such action is reachable from the shell through one `UiAction` variant, so the shell
  keeps satisfying M1's A8/A9 (testable without a display).
- **A14** — Text becomes contours **at creation time** (S4): the document schema stays
  `c2d/document@1` and no downstream stage (toolpath, emitter, view) is modified for text. Glyphs live
  behind a `GlyphSource` trait so a TrueType source can be added later without touching callers.
- **A15** — All M0 and M1 behaviour is unchanged: the existing suite stays green, the `cam-cli`
  contract is untouched, and open contours remain uncuttable by a profile toolpath.

### Decisions recorded for M2

- **D-M2-1** — Text glyph source: single-stroke table behind `GlyphSource`; TrueType is a later task
  and must not change callers (S4).
- **D-M2-2** — Non-uniform scale tessellates at `cam_geom::APPROXIMATION_TOLERANCE` (0.01 mm) and
  reports the number of entities that lost arcs (S5). Uniform scale and arc-free geometry stay exact.
- **D-M2-3** — Authoring builders live in `cam-geom`, not in `cam-app`: `cam-app` owns validation,
  ids, history and refusal, never geometry. This keeps the kernel usable by M3 importers.

## Tasks

One line per task: ID — acceptance criteria — route — commit.

- **T1** — A1, A2 — inline — `feat(geom): primitive builders for line, polyline, rectangle, circle, polygon and star`
- **T2** — A3 — inline — `feat(geom): ellipse approximated by arcs within a declared tolerance`
- **T3** — A4, A5 — inline — `feat(geom): translate, rotate, scale and mirror on contours`
- **T4** — A7 — inline — `feat(geom): tolerant intersections for line and arc segments`
- **T5** — A6 — inline — `feat(geom): node editing on contours`
- **T6** — A8 — inline — `feat(geom): join contours at matching endpoints`
- **T7** — A9 — inline — `feat(geom): trim a contour against a cutting contour`
- **T8** — A12 — inline — `feat(app): draw commands for every M2 primitive`
- **T9** — A11, A12 — inline — `feat(app): transform, array copy and align commands`
- **T10** — A10, A12 — inline — `feat(app): offset, join, trim and node edit commands`
- **T11** — A14 — inline — `feat(geom): single-stroke text rendered to contours`
- **T12** — A13 — inline — `feat(ui): authoring tools wired through UiAction`
- **T13** — A15 — inline — `docs: m2 authoring quickstart and roadmap status`

Route notes: M2 is a dependency chain (`cam-geom` → `cam-app` → `cam-ui`), so no two tasks are
independent units of behaviour; parallel writers do not apply. Every task is inline, test-first.

### Task detail

**T1 — Primitives.** New `cam-geom/src/primitive.rs`, exported as `cam_geom::primitive`:
`line(from, to)`, `polyline(&[Point2], closed)`, `rectangle(at, width, height)`,
`circle(center, radius)`, `polygon(center, radius, sides, rotation)`, `star(center, outer, inner,
points, rotation)`, `arc_through(a, b, c)`. All return `Result<Contour, GeomError>` except the ones
that cannot fail. `cam-app`'s private `rectangle_contour` and `circle_contour` move here **without
changing behaviour** (the M1 tests are the proof). Tests: analytic area and perimeter per builder;
circle is exactly two arcs each with `|sweep| = PI`; polygon has exactly N straight segments and its
area equals `N/2 * r^2 * sin(2*PI/N)`; star has 2N segments; every builder's result is closed,
counter-clockwise and passes `Contour::new` invariants; a property test asserts every builder
round-trips through `serde_json` and stays equal.

**T2 — Ellipse.** `cam_geom::primitive::ellipse(center, rx, ry, tolerance)` returning the contour plus
the achieved maximum radial error. Implementation: 4-arc minimum, doubling until the sampled radial
error is below `tolerance`; degenerate `rx == ry` returns exactly a circle. Tests: error bound holds at
several aspect ratios including 100:1; a circle-equal case has zero error; `rx` or `ry` ≤ 0 refused;
segment count stays bounded (`MAX_TESSELLATION_STEPS`).

**T3 — Transforms.** New `cam-geom/src/transform.rs`: `translate(&Contour, Vec2)`,
`rotate(&Contour, angle_rad, about)`, `scale(&Contour, sx, sy, about, tolerance) -> ScaleOutcome`,
`mirror(&Contour, line)`. Uniform scale and mirror are exact on arcs; `sx != sy` tessellates contours
that contain arcs (D-M2-2) and `ScaleOutcome` reports `entities_lost_arcs`. Mirror reverses the vertex
order to keep `is_ccw` true. Tests: a circle rotated by an arbitrary angle is the same circle; mirror of
a circle about a vertical line lands exactly on the reflected centre with unchanged radius; a mirrored
closed contour is still counter-clockwise; `scale(2,2)` doubles area and keeps arcs; `scale(2,1)` on a
circle yields a polyline whose bounds match `2r x r` within tolerance and reports one lost arc;
`scale(2,1)` on a rectangle stays exact with four vertices and reports zero.

**T4 — Intersections.** New `cam-geom/src/intersect.rs`: `intersect(Segment, Segment, tolerance) ->
Intersections` where `Intersections` distinguishes none/one/two/tangent/coincident-overlap. Cases
line/line, line/arc, arc/arc, including the tangent case returning one point rather than two nearly
equal ones. Tests: a chord crossing a circle returns two points in deterministic order; an external
tangent returns exactly one; two identical segments return `Coincident`; parallel non-coincident returns
none; every case checked against an analytic expectation, and a property test asserts symmetry
(`a ∩ b == b ∩ a`).

**T5 — Node editing.** `cam-geom/src/edit.rs`: `move_vertex(&Contour, index, Point2)` (bulge
preserved), `insert_vertex(&Contour, segment_index, Point2)` (splitting an arc exactly: the two
resulting arcs pass through the new point and keep the original endpoints; a straight segment stays
straight), `delete_vertex(&Contour, index)` (refused below two vertices). Tests: inserting a point on a
half-turn arc yields two arcs whose sweeps sum to PI and whose midpoint error is zero; inserting on a
line yields collinear vertices; moving a vertex on a closed contour keeps it closed; deleting from a
two-vertex contour is refused.

**T6 — Join/weld.** `cam-geom::join(contours: &[Contour], tolerance) -> JoinOutcome` merging at
matching endpoints with reversal as needed; refuses with a named error when a point is shared by three
or more ends, or when the group forms more than one chain. Tests: two lines sharing an endpoint join
into one open contour of three vertices; reversed orientation is joined by flipping the second; three
contours meeting at one point refused with the count in the message; the result's total length equals
the sum of the parts.

**T7 — Trim.** `cam-geom::trim(contour, cutter, tolerance) -> TrimOutcome` removing the piece between
the two outermost intersections and returning the remaining contour(s); open and closed targets
handled, refusal when there are not exactly two usable cut points. Tests: a line trimmed by a crossing
line loses the piece between the intersections; a closed rectangle trimmed by a crossing line becomes
one open contour whose length equals perimeter minus the removed span; a cutter that misses is refused
naming zero intersections.

**T8 — Draw commands.** `cam-app::Command` gains `AddPolyline`, `AddLine`, `AddArc`, `AddEllipse`,
`AddPolygon`, `AddStar`, with the entity id supplied by the caller as in M1. Every one validates
through `Document::validate` after applying and is refused atomically. Tests: one per command asserting
the stored contour's geometry, the dirty flag, and that a refused command leaves `history_depth`
unchanged; undo after each returns the document byte-identical via `serde_json`.

**T9 — Transform, array, align commands.** `TransformEntities { ids, transform }` where `transform`
covers translate/rotate/scale/mirror; `ArrayCopy { ids, rows, columns, dx, dy }`; `AlignEntities {
ids, mode }`. Non-uniform scale over arcs returns the lost-arc count so the status bar can report it;
ids for copies come from the same counter as drawing. Tests: six translate/rotate/scale/mirror cases;
a 2×3 array from a 2-entity selection yields 12 entities with ids `E` and stable ordering; align
left/top/centre-h each move the selection to the exact target coordinate; one undo restores everything.

**T10 — Offset, join, trim, node-edit commands.** `OffsetContour { id, distance }` (delegating to
`cam_geom::offset::offset_contour`), `JoinEntities { ids }`, `TrimEntity { target, cutter }`,
`EditVertex { id, edit }`. Refusals surface the kernel error's message and leave the document intact.
Tests: offsetting a closed circle by 2 mm yields a circle of radius r+2 that is still two arcs; a
collapsing offset is refused with the kernel's reason and no document change; joining two touching
lines yields one entity and removes the other; trim on a rectangle produces one open entity; moving a
vertex is undoable in one step.

**T11 — Text.** `cam-geom/src/text.rs`: `GlyphSource` trait (`glyph(ch) -> Vec<Contour>`, `advance(ch)
-> f64`), `SingleStrokeFont`, and `text_outlines(text, origin, height, source) -> Vec<Contour>` with
line advance and newline handling. Text becomes contours here; `cam-app` gets `AddText`. Tests:
"AB" produces more than one contour and each is within its advance box; height scales linearly;
an unknown character is refused naming the character; the same input twice is identical (deterministic
ids and vertex order).

**T12 — Shell wiring.** `cam-ui` gains one `UiAction` per T8–T11 command plus an active-tool state, and
canvas drags become draw gestures (press → move → release) previewing the shape and committing one
command. Tests: a synthetic drag with `RawInput` draws a rectangle through one `Command`; the toolbar
button and the keyboard shortcut reach the same `UiAction`; the status bar reports the lost-arc count
after a non-uniform scale; the whole shell still runs headless.

**T13 — Docs.** `docs/m2-authoring.md` quickstart: which primitive is which, the ellipse and
non-uniform-scale tolerances with their numbers, the join/trim refusal messages, and the text glyph
source and its limitation. Roadmap M2 row marked delivered with the evidence table.

## Evidence — commit per task

| Task | Commit | Notes |
| --- | --- | --- |
| T1 | `e59f484` | 20 tests; analytic area/perimeter/winding; cam-app delegates to the kernel; serde_json `float_roundtrip` added (see L5) |
| T2 | `2a0bc5d` | 10 tests; tolerance met at up to 100:1 and re-checked by independent sampling; error handed back when unmet |
| T3 | `HEAD` | 15 tests incl. two property tests; winding normalised; non-uniform scale measures its own sagitta |
| T4 | — | pending |
| T5 | — | pending |
| T6 | — | pending |
| T7 | — | pending |
| T8 | — | pending |
| T9 | — | pending |
| T10 | — | pending |
| T11 | — | pending |
| T12 | — | pending |
| T13 | — | pending |

## Log

- **L1** — Owner request (verbatim, this session): "Arrancar M2 (plan de tareas ODD primero)" —
  selected from the offered option whose description reads: "Leo el roadmap, escribo
  odd/tasks/m2-authoring.md con specs y tareas, y empiezo por T1 en una rama de feature."
- **L2** — Owner decision on text glyph source (verbatim option label): "Trazo simple ahora, TrueType
  después". Recorded as D-M2-1 (S4).
- **L3** — Owner decision on non-uniform scale with arcs (verbatim option label): "Teselar
  automáticamente con tolerancia declarada". Recorded as D-M2-2 (S5).
- **L4** — Plan: 13 tasks, dependency chain, all inline, branch `feat/m2-authoring` off `master`
  (`4578cdc`). Baseline suite: see `scripts/check.sh`.
- **L5** — T1 evidence. `2a11db6` holds the plan (228 lines); `e59f484` holds T1 (530 authored
  lines). Slice boundary recorded per the review-workload rule: the slab is above ~400 lines because
  the plan and the first task are separate units and the task's tests are half of it. A real defect
  surfaced while writing T1's round-trip property test: `serde_json`'s default float parsing returns
  one ULP off, so a stored coordinate did not round-trip exactly. Fixed at the root by enabling
  `float_roundtrip` in the workspace dependency; the whole suite, every M0 golden G-code program
  included, is green with and without it.
