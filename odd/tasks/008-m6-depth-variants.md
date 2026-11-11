# Feature: 2.5D CAM application — Milestone M6 (depth-variant strategies)

Program plan: `odd/roadmap/index.md` (M6 = "Depth-variant strategies"). This document is the executable
specification for M6 only. Read this file until `## Log` before implementing any task.
Stack: the existing Rust workspace (`cam-model`, `cam-toolpath`, `cam-move`, `cam-post`, `cam-sim`,
`cam-cli`, `cam-store`); no new crate and no new dependency. Branch: `feat/m6-depth-variants`,
cut from `master` at `8a953ba`; baseline `./scripts/check.sh` green with **795 tests** before the first
write. `feat/m5-simulation` (merged, `1b42e20`) was deleted as part of the same instruction.

## Specs

Requirements as stated by the owner (verbatim):

- **S1** — This session's instruction, verbatim: "3" — the option chosen, with its exact label and
  description: "Limpieza de rama mergeada y después M6" — "Borrar feat/m5-simulation y arrancar M6
  sobre master limpio." Preceded by the session's first instruction, verbatim: "coge el contexto de
  engram para poder continuar".
- **S2** — Roadmap M6 row (verbatim, `odd/roadmap/index.md:143`): "| **M6** | Depth-variant strategies |
  Quick Engrave, Chamfer, Fluting, Texture, Thread Milling, Inlay (male/female pair) | All are
  profile/pocket variants with a different Z law — cheap once M4 and M5 exist |".
- **S3** — What the README promises for M6 (verbatim, `README.md:185-188`): "M6 is the depth-variant
  strategies — Quick Engrave, Chamfer, Fluting, Texture, Thread Milling and the Inlay pair — profile and
  pocket variants with a different Z law, cheap once M4's strategies and M5's simulation exist."
- **S4** — The strategy vocabulary and its parameter names — the superset M6 reads, six of which it
  implements: "`Quick Engrave`, `Inlay` (`Create Inlay
  Toolpath`, `InlayStepDepth`, `InlayStepWidth`, `InlayColour`), `Chamfer Toolpath`, `BevelCarving`,
  `Fluting Toolpath`, `Texture Toolpath`, `Moulding Toolpath`, `Thread Mill` (`ThreadDiameter`,
  `ThreadPitch`, `ThreadDirection`, `ThreadedLength`)". Which of those a given edition exposes is
  decided by gating M6 does not model.
- **S5** — Rule 1 of the architecture, in force since M0 (verbatim, `odd/roadmap/index.md:90`): "**`cam-cli`
  is the contract.** Any behaviour that cannot be produced headless through the CLI is not considered
  testable, and therefore not done."
- **S6** — Rule 2 of the architecture, in force since M0 (verbatim, `odd/roadmap/index.md:93`):
  "**Determinism is a feature.** Identical input must produce byte-identical G-code. No `HashMap`
  iteration may reach an output path; ordering is explicit (`BTreeMap`, sorted IDs, stable sorts)."
- **S7** — Verification rule 5 (verbatim, `odd/roadmap/index.md:411-412`): "**Simulation as oracle** — for
  each strategy, a test asserts the simulated removed volume matches the analytic expectation within
  tolerance; a too-shallow pocket fails the test, not a screenshot."
- **S8** — Decision D7 (verbatim, `odd/roadmap/index.md:294`): "Do not vendor third-party `.pp` files".
- **S9** — The move vocabulary's Z convention and its arc restriction (verbatim,
  `crates/cam-move/src/lib.rs:19-21`): "Z position, in millimetres. Positive is up; cutting depths are
  positive magnitudes." and (verbatim, `:57-58`): "Circular cutting move in the XY plane at constant Z."
- **S10** — How M5 justified carrying the tool on the segment (verbatim, `crates/cam-move/src/lib.rs:93-97`):
  "The emitter ignores it — a post-processor has no block that states a tool's diameter — but the
  simulation needs the footprint a move sweeps, and taking it from the segment keeps the tool joined to
  the moves that use it (M5 S2)."
- **S11** — The tool-geometry columns the library carries, which M6 must start consuming: "`tool_geometry(id,
  name_format, notes, tool_type, units, diameter, included_angle, flat_diameter, num_flutes,
  flute_length, thread_pitch, outline BLOB, tip_radius, laser_watt, custom_attributes, tooth_size,
  tooth_offset, neck_length, tooth_height, threaded_length)`".
- **S12** — Scope boundary D2 (user, verbatim option label): "Todo menos 3D y Laser". M6 adds no 3D
  machining strategy and no laser strategy; `Moulding`/`Swept Profile` stay at M8 and `V-Carve` at M7.
- **S13** — Decision D-M4-8, still in force (verbatim, `odd/tasks/005-m4-toolpaths.md:163-166`):
  "**D-M4-8 — No toolpath UI in M4.** ... `cam-cli toolpath` is how a toolpath is authored." M6 keeps
  that: the window gains no toolpath form for any of the six strategies.
- **S14** — Decision D-M4-6, whose cost M6 does not re-open (verbatim, `odd/tasks/005-m4-toolpaths.md:145-147`):
  "**D-M4-6 — Linking moves stay conservative.** Between rings, rows and holes the tool retracts to
  `safe_z` and rapids." Every new pattern strategy in M6 inherits it.

## Acceptance criteria (derived)

What "M6 delivered" means, testable through `cam-cli` unless the criterion names the window (S5):

- **A1 — Six strategies, one contract.** `cam-cli toolpath` accepts `--kind chamfer|engrave|fluting|
  texture|inlay|thread` in addition to `profile|pocket|drilling`, and each appends one `Operation`
  variant to the document (`inlay` appends two). The schema stays `c2d/document@2`: the variants are
  additive under the existing `kind` tag, an unknown `kind` remains a hard load error, and every
  previously serialised document still loads and re-saves byte-identically.
- **A2 — A chamfer is a V-bit bevel with a derived depth.** `--kind chamfer` takes `--entities`,
  `--tool`, `--chamfer-width` and `--start-depth`; the depth is `start_depth +
  chamfer_width / tan(included_angle / 2)`, derived from the tool and never authored, and there is no
  `--side`: the tip travels on the vector and the bevel is symmetric about it (amended in L7). The
  tip travels on the selected contour, unoffset, in **one** pass (S-D-M6-2 below), and the tool's own emitted path
  is unchanged by the width except through its Z.
- **A3 — A quick engrave is one constant-depth pass on the contour.** `--kind engrave` takes
  `--entities`, `--tool` and `--cut-depth`; it cuts each selected contour at that single depth with no
  lateral offset, no stepdown, no lead-in/out and no tabs.
- **A4 — Fluting is one pass along each vector, optionally ramped.** `--kind fluting` takes
  `--entities` (open or closed), `--tool`, `--cut-depth` and an optional `--ramp`; the tool enters over
  the first `ramp` millimetres of each vector and leaves over the last, so with `ramp = 0` it plunges
  in place, and with `ramp > 0` it never plunges at full depth. A `ramp` longer than half the vector is
  an input error, not a silently shortened ramp.
- **A5 — Texture is a pattern of grooves across a region.** `--kind texture` takes `--entities`
  (closed region), `--tool`, `--cut-depth`, `--spacing` and `--angle`; it cuts parallel grooves across
  the region at that spacing and angle, each at `cut_depth`, retracting to `safe_z` between grooves
  (S14). No clearing: the material between grooves stands, and the command says so in its help.
- **A6 — Inlay is a male/female pair from one command.** `--kind inlay` takes `--entities` (closed
  region), `--tool`, `--cut-depth`, `--inlay-allowance` and the usual pocket stepover fallback; it
  appends **two** toolpaths in fixed order — `inlay female` clears the region offset outward by the
  allowance, `inlay male` clears the job rectangle with the region as an island — so the male plug fits
  the female cavity with the allowance as clearance.
- **A7 — Thread milling is helical, and a post that cannot express it refuses.** `--kind thread` takes
  `--entities` (circles), `--tool`, `--cut-depth`, `--pitch` and `--thread-direction internal|external`;
  the tool descends one pitch per revolution around the circle. The program expresses the helix as a
  helical arc, the emitter has block kinds for it, and a post without those blocks fails loudly with a
  named error instead of emitting a flat arc at the wrong depth (the precedent is M4's arc refusal).
- **A8 — The simulation models the tool's shape, not just its width.** The cone of a V-bit is carried
  from the toolpath to `cam-sim`, which sweeps a footprint whose radius grows with depth below the tip;
  a cylinder tool keeps its existing behaviour cell for cell. A V-bit strategy whose tool has no
  `included_angle` is an input error naming the tool, never a default angle.
- **A9 — Every strategy has an oracle in both signs (S7).** For each of the six, a test asserts the
  analytically derived removed volume within tolerance, and a test asserts that a deliberately wrong
  depth is reported as gouge or uncut — the M5 oracle's own shape (`cam-sim/tests/oracle.rs`). **The
  arithmetic is per side, and T2's review found A9 asserting the wrong one** (L9): a chamfer whose
  vector has material on both sides removes `L·w·d` per unit length; one whose vector is an outside
  edge, with air on the other side, removes `L·w·d/2`. Both cases get an oracle, so neither reading
  can pass as the whole truth. The one-sided case is expressible without inventing a clip: a closed
  rectangle whose cut edge lies on the stock's low edge and whose other three edges lie outside the
  sheet, so the grid clips exactly the flank that would fall on air.
- **A10 — Determinism and the old bytes (S6).** Two runs of every new subcommand print and emit
  identical bytes; the pre-existing subcommand differential still holds against `8a953ba` except for
  `--help`, the unknown-`--kind` sentence, the new commands themselves, and one error sentence an inlay
  made false. (A10 said "twelve"; M5's own count was twelve *invocations* of ten subcommands — the
  wording is corrected here and the differential is reported by invocation, per L18.)
- **A11 — No new dependency, and the shell untouched.** `Cargo.lock` gains no `[[package]]`; the
  existing 795 tests keep passing unmodified except where a criterion above moves an expectation, and
  each such move is re-derived, never fitted.

## Decisions

- **D-M6-1 — Cheapest first, and the contract change last.** The six strategies are delivered in this
  order: chamfer, quick engrave, fluting, texture, inlay, thread milling. Four of them are new Z laws
  over the machinery M4 already has (`pass_depths`, `Region::offset`, `Region::raster_spans`); thread
  milling is last because it is the only one that changes the post contract (A7), and a milestone's
  riskiest unit should not block its other five.
- **D-M6-2 — A chamfer's tip runs on the contour and its depth is derived from the tool's angle.** The
  bevel a V-bit cuts is the cone's flank, so the width and the included angle fix the depth:
  `width / tan(included_angle / 2)`. With the tip on the contour, the flank crosses the surface exactly
  `width` inside the edge and reaches the edge line at depth `depth`, giving a bevel of horizontal width
  `width` whose face makes the half-angle with vertical — for a 90° bit, a 45° chamfer of width
  `width`, which is the case a user checks by eye. One pass, because a second pass at a deeper Z would
  cut a second bevel into the first. **Amended in L7:** a symmetric bevel about the vector has no side,
  so `ChamferParams` carries no `side` and no `cut_depth` — only `chamfer_width`, `included_angle` and
  `start_depth`, with one shared `ChamferParams::depth()` in `cam-model` that `cam-toolpath` and
  `cam-sim` both call, so the two can never derive different depths. **Amended in L9:** the derivation
  assumes a sharp point, so a bit with a flat tip is refused by name (`D-M6-10`) instead of cutting a
  bevel wider than the width the user typed.
- **D-M6-3 — The cone is modelled in `cam-sim`, not approximated by a cylinder.** M5 S2 put the tool's
  diameter on the segment because the simulation needs the swept footprint; a V-bit's footprint is not a
  diameter. `cam_move::Segment` therefore carries a tool shape (`Cylinder { diameter }` or
  `VBit { included_angle, flat_diameter }`) and `cam-sim::sweep` grows the footprint radius with depth.
  The alternative — a cylinder at the bevel's top width — would report a correct chamfer as a gouge,
  which is exactly the failure D-M5-5 was amended to fix.
- **D-M6-4 — Quick Engrave is the profile's `On` side with the pass law removed.** Quick Engrave
  engraves along vectors at one depth; M4's profile already knows how to walk a contour
  and lift for tabs, and a pass law is a liability when the feature is a single V-groove. So engrave
  reuses `walk_contour`'s machinery, refuses `cut_depth <= start_depth`, and takes no stepover.
- **D-M6-5 — Fluting ramps linearly, and the ramp is a property of the vector, not of the depth.**
  "Fluting" in S4's vocabulary is one groove per vector; the ramp exists so a round-nose
  bit does not plunge into the end of a visible flute. A linear ramp over the first and last `ramp`
  millimetres is the honest minimum that satisfies that, and it makes the removed volume a closed form
  (A9): `2r · depth · (length − ramp)`. **Amended in L14:** a ramp cannot run along a contour that
  contains an arc, because a ramped move cannot ride on `Move::Arc` (constant-Z by contract), so a
  ramped arc would be cut as a straight chord — `ramp²/(8r)` inside the vector, half a millimetre for a
  10 mm ramp on a 25 mm circle. `cam-toolpath::fluting` refuses that selection by name until T7's
  `Move::Helix` exists, and `ramp = 0` on the same contour is untouched.
- **D-M6-6 — Texture is parallel grooves at a spacing and angle, not a pocket.** The strategy's value is
  the pattern, not the clearing: the material between grooves is meant to stand, so the command cuts
  rows only, at the given spacing, rotated by the given angle, and says plainly that it does not clear
  (A5). Rows come from the region's own raster spans, so the clipping into the region is M4's
  (`Region::raster_spans`), not a new intersector.
- **D-M6-7 — Inlay from one command, in a fixed order, with the allowance as the clearance.** The
  S4 names the pair and its two parameters — `InlayStepDepth`, `InlayStepWidth`; M6 delivers
  the pair as two pocket toolpaths that share the depth: the female is the region grown by the
  allowance, the male is the job rectangle with the region as an island (S14's conservative linking
  unchanged). `InlayColour` is a preview property, not a toolpath parameter, and is out of scope (S13).
- **D-M6-8 — A helical arc is a new move kind and a new post block, and absence is a refusal.** `Move::Arc`
  is documented as constant-Z (S9) and the fixture post's arc blocks carry `[X][Y][I][J]` and no `[Z]`
  (`crates/cam-post/tests/fixtures/post_mm.pp:64-73`), so a helix cannot ride on them. M6 adds
  `Move::Helix` and the four helical block kinds; the emitter requires a block for every move kind it
  meets, so a post that lacks them refuses with a named error. Additive by construction: no existing
  block kind changes meaning, and no document or post that works today stops working.
- **D-M6-9 — Tool geometry comes from the library, and a missing field is an input error.** The tool
  database has carried `included_angle`, `flat_diameter` and `tip_radius` since migration 1 and nothing
  outside the name formatter consumed them (`crates/cam-store/src/tools.rs:122-148`). M6 consumes the
  angle and the flat diameter through `ToolSpec`; a V-bit strategy given a tool without an angle exits
  as input error naming the tool, because defaulting to 90° would silently cut a different chamfer.
- **D-M6-12 — The sweep interpolates Z along a cutting move, and a ramp is one move.** M5's D-M5-3
  recorded that the sweep removes to a move's *destination* `z` over its whole footprint and "the vertical
  interpolation between the previous `z` and this one is not modelled", which was conservative while every
  cutting move was at a constant depth. A ramp is not: simulated at its destination depth, a linear ramp
  removes full depth along its whole travel. T4's first implementation worked around that by emitting the
  ramp as a staircase of 0.1 mm Z steps, which is the wrong trade — it puts the simulator's limitation into
  the program the machine runs, and it cannot express a helix at all (T7). So the sweep interpolates: a
  cutting move removes, at each sampled point along it, the depth interpolated between where it started and
  where it ends. Fluting's ramp is then one linear move per end, the fluting claim keeps its footprint
  envelope (which is independent of this), and T7's helix can be swept honestly.
- **D-M6-14 — A thread's target is the spiral, not the annulus.** The swept floor of a helix is not a
  flat disc: the last revolution passes each point at that point's own angle, so the material the program
  was entitled to remove at a cell is the deepest pass whose footprint covers it, `pitch·Φ_last/2π` over
  the unwrapped angle `Φ`. Authorising the whole annulus at `cut_depth` instead was T7's first claim, and
  it is an upper bound of up to one pitch — which makes `simulate` exit 6 on **every** correct thread
  whose pitch is coarser than `cell/8`, i.e. on nearly every real thread. A gate that fails every correct
  program is not conservative, it is broken. The claim takes the maximum over the tool's angular window
  `α(r)` rather than at the cell's own angle, because a cell is reached from angles up to `α` later than
  its own and the later pass is the deeper one — the tip-only form reports `gouge 8.697 mm³` on a correct
  thread, the same one-sided error `claim_flute` was amended for.
- **D-M6-13 — The two inlay halves are told apart by the ids they carry, and the male cuts rows.** A6
  says both halves share one `InlayParams`, so the half cannot live in the parameters without adding a
  field the spec does not have; it lives in the vocabulary the document already has. The female names the
  region as `entity_ids` and stands nothing; the male has no boundary entity at all — its boundary is
  `Job::outline()`, which is not an entity — and names the region as `island_ids`. A record that names
  both is refused (`InlayMaleTakesNoEntities`) rather than guessed at, and the toolpath's display name is
  never consulted: it is a user-editable string. The male cuts **raster rows**, not offset rings, and the
  number is the reason: on a 40 mm region in a 100 mm stock with a 6 mm tool at a 2 mm stepover, the ring
  male leaves 2300 mm² of the 8556 mm² frame never cut — the four diagonal corners where a ring lattice
  converges on the frame's medial axis and stops, i.e. a plug still attached at four points — while the
  row male leaves 80 mm². What remains is the pocket's own coverage bound, which T6 records rather than
  fixes.
- **D-M6-11 — The CLI ignores a flag its kind does not use, and M6 does not close that milestone-wide.**
  `cam-cli toolpath` parses every option and only the strategies that consume one validate it, so
  `--allowance` reaches a drilling toolpath and is dropped. Chamfer and engrave refuse the flags their own
  law forbids (a chamfer's `--side`, an engrave's `--pass-depth`, `--tab`, `--lead-in`, `--lead-out`), which
  keeps their contract honest where M6 touched it. Closing it for every existing kind changes the CLI's
  contract for callers that pass a superset today, so it is recorded as its own slice and not folded into a
  strategy task.
- **D-M6-10 — A chamfer needs a sharp V-bit, and the library refuses a degenerate one.** With a flat tip
  of diameter `f` the bevel reaches the surface at `f/2 + d·tan(θ/2)`, so honouring `chamfer_width`
  exactly would need `d = (w − f/2)/tan(θ/2)` — a derivation that reads the tool, while `ChamferParams`
  deliberately does not (L7). M6 refuses a flat-tipped bit for `chamfer` by name rather than cutting a
  bevel wider than the number the user asked for; a tapered footprint with a flat is a later strategy's
  business. For the same reason `cam-toolpath::chamfer` refuses an angle that is not strictly inside
  `(0, π)` at the library boundary and not only in the model's validation: a caller that bypasses the
  CLI must not receive an infinite Z. **Amended in L11:** the gate has to exist at every boundary that can see the
  flat, and they see different things — `cam-cli toolpath` sees the library before a toolpath is stored,
  `cam-cli emit` sees the library *after* the document exists (the case that matters: a document authored
  against a sharp bit and emitted against a flat one), and `cam-sim::target` sees the `ToolShape::VBit` the
  segment carries. One boundary cannot cover another, which is why all three refuse.

## Tasks

| ID | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | Tool shape on the move vocabulary: `cam_move::ToolShape`, `Segment.tool`, `cam-sim::sweep` cone footprint, `cam-sim::target` cone-aware claim | A8, S9, S10, D-M6-3 | inline, test-first | `ae44528` |
| T2 | Chamfer: `Operation::Chamfer`, `ToolSpec.included_angle`, `cam-toolpath::chamfer`, CLI `--kind chamfer --chamfer-width`, sim arm, oracle, golden | A2, A8, A9, D-M6-2, D-M6-9, D-M6-10 | delegated (context backstop), test-first | `93795ee` |
| T3 | Quick Engrave: `Operation::Engrave`, `cam-toolpath::engrave`, CLI `--kind engrave`, sim arm, oracle, golden | A3, A9, D-M6-4, D-M6-11 | delegated, test-first | `63694c0` |
| T4 | Fluting: `Operation::Fluting`, `cam-toolpath::fluting` with the linear ramp, CLI, sim arm, oracle, golden | A4, A9, D-M6-5 | delegated, test-first | `93935c3` |
| T4b | The sweep interpolates Z along a cutting move, so a ramp is one linear move and not a staircase (amends D-M5-3) | A2, A8, D-M6-12 | delegated, test-first | `93935c3` |
| T5 | Texture: `Operation::Texture`, `cam-toolpath::texture` (spacing + angle over a region), CLI, sim arm, oracle, golden | A5, A9, D-M6-6 | delegated, test-first | `8487140` |
| T6 | Inlay pair: `Operation::Inlay`, two pocket toolpaths, CLI, sim arm, oracle, golden | A6, A9, D-M6-7, D-M6-13 | delegated, test-first | `330309d` |
| T7 | Thread milling: `Move::Helix`, post helical block kinds + refusal, `cam-toolpath::thread`, CLI, helix sweep, oracle, golden. **Also lifts T4's `RampOnArc` refusal** | A7, A9, D-M6-8, D-M6-5 | delegated, test-first, own commit (Risk item 3) | `7063fb3` |
| T8 | Docs and the closing gate: `docs/m6-toolpaths.md`, README status, roadmap §4.9 + M6 ✅, `docs/format.md`, `./scripts/check.sh` | A1–A11 | delegated | `b7153fb` |

## Recorded out of scope

- `BevelCarving` appears in S4's vocabulary but not in the roadmap's M6 row (S2):
  recorded here as named-but-unscheduled, not silently dropped.
- `InlayColour`, `InlayStepDepth`/`InlayStepWidth` as *separate* step parameters: M6 derives the step
  from the tool library's `stepdown`/`stepover` the way profile and pocket already do (D-M6-9), so the
  pair ships with one depth and one stepover rather than two extra flags.
- No toolpath form in the window (S13), no 3D or laser strategy (S12), no new crate or dependency (A11).

## Log

- **L1 (owner, verbatim, in order).** "coge el contexto de engram para poder continuar", then "3",
  choosing: "Limpieza de rama mergeada y después M6" — "Borrar feat/m5-simulation y arrancar M6
  sobre master limpio."
- **L2 (execution, pre-flight, not a task).** The merged branch `feat/m5-simulation` was deleted
  (`git branch -d`, was `1b42e20`); the repository was clean at `master` `8a953ba`, in sync with
  `origin/master`, and M5's handoff said "nothing pushed" — the merge and the push had happened after
  that summary was written, so the summary's next step 1 was already done. Recorded because a stale
  handoff is exactly what a resume must not trust blind.
- **L3 (M6 pre-flight, the two prerequisites the roadmap did not anticipate).** The roadmap's M6 row
  calls these "profile/pocket variants with a different Z law — cheap once M4 and M5 exist" (S2). Two
  facts found before any write say otherwise, and they are the reason T1 and T7 exist: (1) no V-bit
  geometry reaches the toolpath layer — `ToolSpec` carries `number, diameter, feed_rate, plunge_rate,
  spindle_speed` and nothing else (`crates/cam-toolpath/src/lib.rs:24-38`), while `included_angle`,
  `flat_diameter` and `tip_radius` sit unused in the tool database; and `cam-sim` sweeps a
  constant-radius cylinder per segment, so a cone has no carrier at all. (2) `Move::Arc` is documented
  constant-Z and the post fixture's arc blocks carry no `[Z]`, so thread milling cannot ride on the
  existing move or block. Both are recorded as decisions (D-M6-3, D-M6-8) rather than absorbed
  silently.
- **L4 (execution, T1).** `cam_move::ToolShape` replaced the bare `tool_diameter` on `Segment`:
  `Cylinder { diameter }` and `VBit { included_angle, flat_diameter }`, with `radius_at(depth)` and
  `rise_at_radius(radius)`. The refactor itself was behaviour-neutral and was checked as such before any
  cone test existed: **795 tests** green, the same number the branch cut from `8a953ba`. The emitter still
  reads no geometry, `cam-cli` builds every segment as `ToolShape::cylinder` from the library's diameter,
  and `cam-ui`'s preview carries the shape so its own field is cut by the tool the program names.
- **L5 (execution, T1, and a sign error caught before it became a test that agreed with it).** The first
  form of the cone law was `removed = (x − flat/2) / tan(α)`, which is wrong: that is how far the cone's
  surface sits *above* its tip at distance `x`, not how deep it cuts. A cone with its tip `d` below the
  surface removes `d − (x − flat/2)/tan(α)` at distance `x` from the vector and nothing past
  `radius_at(d)`. The method was renamed `rise_at_radius` so the name cannot be read as a depth again,
  and the mistake is recorded because the original reading would have produced a groove that widens with
  depth — the exact inverse of a V-groove — with a test written to match it.
- **L6 (execution, T1, evidence).** Five tests, and their RED was obtained by mutation rather than by
  the order they were written in, which is stated plainly: the law had to be fixed before a test could
  be written against it, so the tests were written after the implementation and then shown to fail —
  neutralising `rise_at_radius` in both `sweep` and `target` failed all five, restoring it passed them.
  They pin: a 90° V-bit's floor at 3 and 5 mm from the line (graded, not flat) and its 6 mm reach; a
  2 mm flat's flat bottom and its 7 mm reach; the target's authorised depths 5.5 / 2.5 / 0 at 0.5, 3.5 and
  6.5 mm from the vector; the named refusal of a cone on one side of a vector; and the D-M5-5 agreement
  test with a cone — a clean cone cut reports **gouge exactly zero** at zero tolerance and no uncut
  beyond a quarter cell, because the sweep walks the path every half cell and its sample can only be
  further from the path than the path itself. That one-signedness is a property worth stating: a cone
  cut can be reported shallow by the sampling and never deep, so the sampling can never hide a gouge.
- **L7 (design, T2 pre-flight, an amendment to A2 and D-M6-2).** Writing the task made a hole in A2
  visible: a V-bit with its tip on the vector cuts symmetrically about it, so there is no side to choose.
  `ChamferParams` therefore carries `chamfer_width`, `included_angle` and `start_depth` and **not** a
  derived `cut_depth`; the derivation lives once, in `cam-model`, because a stored derived value and a
  recomputed one would eventually disagree in a hand-edited document.
- **L8 (process, slice size).** T1 landed as one commit of 819 authored lines, past the ~400-line slice
  the harness asks for. The boundary is deliberate and cannot be moved: `Segment.tool_diameter` became
  `Segment.tool`, and a half-applied shape change leaves the workspace uncompilable, while every test in
  the tree constructs a segment. The next commits are the smaller ones the plan already had.
- **L9 (review, T2, three defects found by the writer against its own spec).** Recording what the
  specification got wrong, because each one would have shipped as a number nobody questioned.
  *(1) The oracle's arithmetic.* A9 asserted `L·w·d/2`; that is one flank. The cone is symmetric about
  the vector, so a bevel with material on both sides removes `w·d` per unit length, and the writer's
  circular-edge fixture measured `L·w·d`. Asserting `/2` is impossible against a correct implementation,
  so A9 now names both cases and both get an oracle. *(2) A flat tip widens the bevel.* `depth()` omits
  the tip's flat, so a bit with `flat_diameter = f` cuts a bevel `w + f/2` wide — the target and the
  sweep agree (same shape) but the geometry is not what the user asked for. Refused by name (D-M6-10)
  rather than absorbed. *(3) The library boundary had no gate.* `cam-toolpath/tests/errors.rs` was
  outside T2's edit surface, so `chamfer()` first shipped with no error variant of its own and a direct
  caller could pass an angle of zero for an infinite Z; the surface was widened and the refusal added.
  One tolerance moved and was re-derived, not fitted: the chamfer oracle judges at `rise(cell/4)`
  instead of `cell/4`, because D-M5-3's bound is a distance bound and a graded cut pays `cot(θ/2)` of it
  in depth — the mutation still fails at the wider tolerance, so the test did not go blind.
- **L10 (process, T2's slice).** T2 lands as one commit for the same reason T1 did, one degree stronger:
  a new `Operation` variant breaks exhaustive matches in `cam-model`, `cam-sim` and `cam-cli`, so any
  split by crate or by layer leaves the workspace uncompilable, and splitting code from tests would
  separate a behaviour from the only thing that proves it. The boundary is recorded here rather than the
  guideline being quietly exceeded; the next strategies are additive and can be sliced smaller.
- **L11 (review, T2 close, the gate that was still missing).** The first round put the flat-tip refusal at
  authoring and in the simulation, and the writer flagged the asymmetry instead of shipping it: `cam-cli emit`
  still accepted a document whose library had since been re-pointed at a flat-tipped bit, and wrote a program
  for a bevel wider than its own stored width. The decision was to refuse at all three boundaries, and the
  reason is worth keeping: each boundary sees a different object — the library before authoring, the library
  after the document exists, and the `ToolShape` the segment carries — so no one of them can cover the others.
  Three tests, one per gate, and the emission one re-points an existing library tool by id so it proves the
  library can change under a valid document rather than constructing a doctored fixture. T2 closes at **224
  tests** in the four crates it touched (cam-model 25, cam-toolpath 81, cam-sim 54, cam-cli 64) with the whole
  gate green.
- **L12 (review, T3, four things the specification under-specified).** *(1) The claim needs the reach
  test.* A3 said every cell in the window is authorised `cut_depth − rise(distance)`; with `rise = 0` for a
  cylinder that authorises the whole rectangle, corners included, which is the opposite of the band the same
  paragraph demands. The writer kept the window as the outer loop and added `distance > reach`, which is the
  law `sweep_disc` already applies; a sharp V-bit self-clamps, a cylinder does not, and the uniform-band test
  is what caught it. *(2) The oracle's end caps.* A straight vector of finite length also sweeps two end
  caps, so `L·d²·tan(θ/2)` is the interior alone; the fixture runs the vector past both sheet edges, T2's L9
  device, instead of widening a tolerance over the caps. *(3) `--start-depth` is taken* although A3 names
  only `--cut-depth`, because the `cut_depth > start_depth` rule is otherwise unreachable through the CLI;
  the refusal list is exactly the flags D-M6-4 names. *(4) A flat tip is not refused for engrave*, and that
  is deliberate: an engrave's depth is authored rather than derived, so a flat widens the groove by `f/2`
  and `rise_at_radius` accounts for it in both the sweep and the claim. D-M6-10 stays chamfer-only. A fifth
  finding is a gap T3 found and did not widen its surface to fix: `crates/cam-toolpath/tests/errors.rs`
  matches every `ToolpathError` exhaustively, so a new variant is unrepresentable to a task that cannot edit
  it — that file is now part of every later strategy's surface, and T3 reuses `OpenContour` for its empty
  guard rather than shipping an unrepresentable variant.
- **L13 (review, T4, two findings and one of them rejected the implementation).** *(1) The claim is an
  envelope, and the writer is right.* A tip-only claim — the law at the cell's own arc-length position —
  reports a **clean** flute as `gouge 149.2 mm³ over 120 cells`, because the tool's footprint extends *along*
  the path: a 6 mm end mill 3 mm further up the ramp is deeper there, and its flat bottom really does remove
  the cell. The claim computes the maximum the law allows anywhere the tool could have removed the cell, in
  closed form rather than by sampling, and a clean flute then deviates by nothing. *(2) The staircase is
  rejected.* Faced with the same finding from the other side — the sweep removes a move's depth over its
  whole travel, so one linear ramp move simulates as a full-depth plunge — the writer emitted the ramp as
  0.1 mm Z steps. That puts the simulator's limitation into the program the machine runs and cannot express
  a helix, so D-M6-12 fixes the sweep instead and T4b follows. The writer's own numbers are kept as the
  evidence for both: 5549.2 mm³ measured against D-M6-5's 5400 analytic, the 149.2 mm³ the envelope removes,
  and `gouge: 5549.2 over 960 cells` for a correct flute under the naive law. Two smaller notes: a negative
  ramp is refused by `cam-toolpath` but not at document load (`crates/cam-model/tests/document.rs` was
  outside the surface), and `RAMP_STEP`'s coupling to cell size disappears with T4b.
- **L14 (review, T4, four rounds and three implementations rejected).** T4 took four rounds, and each
  rejection was for the same reason: the program must not be bent to the simulator's convenience.
  *(a) The staircase.* The first implementation emitted a ramped entry as 0.1 mm Z steps because M5's sweep
  removes a move's destination depth over its whole travel. Rejected; D-M6-12 makes the sweep interpolate.
  *(b) The claim envelope.* Accepted, and the writer was right: a tip-only claim reports a clean flute as
  `gouge 149.2 mm³`, because a 6 mm end mill 3 mm further along the ramp is deeper there and its flat bottom
  removes the cell. The claim is the maximum the law allows anywhere the tool could have removed the cell, in
  closed form. *(c) A document-global tolerance.* Fixing the sampler's residual by widening `simulate`'s
  judgement to the document's steepest grade would license 3 mm of deviation anywhere in a document that
  contains one steep ramp. Rejected; the sampler now takes `max(2·travel/cell, 16·|Δz|/cell)` steps so the
  residual is half the judgement by construction, and the constant-Z count is bit-identical (33,600 of
  33,600 cell values against `9205816`). *(d) The chorded arc ramp.* A ramp across a contour arc was emitted
  as a straight chord: `gouge 33.228 mm³ over 22 cells` at r = 20. Rejected; refused by name instead, with
  `ramp = 0` on the same contour still emitting arcs, and T7 lifting the refusal when the helix exists. The
  claim's vertex gap the writer found on its own (`gouge 5.750 mm³ over 6 cells` for a correct flute, because
  `±sqrt(reach² − d²)` is exact only while the nearest point is interior) is fixed with the reachable stretch
  solved in closed form for both segment kinds. T4 closes at **283 tests** in the four crates (cam-model 29,
  cam-toolpath 97, cam-sim 79, cam-cli 78), gate green. Two gaps recorded, not closed: `Document::validate`
  does not refuse a ramped flute on an arc (strategy-level rule, like the depth range), and a cone's interior
  candidate in `claim_flute` is exact only while the nearest point is a perpendicular foot — measured clean on
  every fixture, and a ternary search would remove the assumption at a real per-cell cost.
- **L15 (review, T5, three notes and one bound worth naming).** The pattern is derived once, in
  `TextureParams::grooves`, and both `cam-toolpath` and `cam-sim` call it — D-M6-2's rule applied to
  something much easier to get subtly wrong twice than a depth. Two behaviours are recorded because a
  reader will meet them: a **45° texture does not pass `simulate`'s zero-defect gate at 1 mm cells**
  (`uncut 1.000 mm³ over 2 cell(s)`), which is M5's documented one-sided coverage bound on a rind the
  grid does not sample, so the 45° case asserts the emitted diagonals and the claim geometry directly
  instead of widening a tolerance (L14(c)'s rejected trade); and the lattice's phase — half a spacing
  inside the region's lower extent in the groove frame — is a decision the spec did not make, recorded
  in the cam-model test with its own numbers, centred only when the extent is a whole number of
  spacings. `--spacing`/`--angle` on a kind that does not use them now parse and are ignored rather
  than refused as unknown, which is D-M6-11's position and a real change for those two names.
- **L16 (review, T6, one gap T8 must close and one decision recorded).** The writer found that the two
  halves cannot be told apart from `InlayParams` alone and encoded the half in the ids a toolpath carries
  (D-M6-13), refusing a record that names both rather than guessing — the alternative, reading the display
  name, is a user-editable string and was rejected on sight. It also chose the male's raster over rings on
  a measurement (2300 mm² of frame left standing against 80 mm²), which is the kind of choice that should
  never be made by taste. Two follow-ups: `ModelError::IslandsRequirePocket`'s sentence now says islands
  are "only for a pocket" although an inlay male takes them, and the text is asserted in
  `docs/region-booleans.md` — T8 rewords both; and the male leaves 480 mm³ standing on the oracle fixture,
  which is the pocket's published coverage bound (`crates/cam-toolpath/src/pocket.rs`), pinned in the
  oracle with its arithmetic rather than absorbed into a tolerance. T6 closes at **341 tests** in the four
  crates (cam-model 43, cam-toolpath 114, cam-sim 90, cam-cli 94).
- **L17 (review, T7, the milestone's riskiest unit and what it found).** T7 changed the post contract —
  `Move::Helix`, four helical block kinds carrying `[Z]`, and a post without them refusing by name — and it
  lifted T4's `RampOnArc` refusal, so a ramped flute now follows an arc helically instead of chording it:
  the fixture that gouged 33.228 mm³ simulates with **gouge 0.000 / uncut 0.000**. Two rounds went into the
  claim. The first shipped the annulus at full depth; the acceptance criterion (a 6 mm deep, 1.5 mm pitch
  thread reporting clean at `cell/8`) failed with `uncut: 106.452 mm³ over 151 cells`, which is what a gate
  that fails every real thread looks like, and D-M6-14 replaces it with the spiral. The second round also
  found that the tip-only spiral is too tight — a correct thread reports `gouge 8.697 mm³`, because a cell
  is reached from angles up to `α(r)` later than its own — so the claim takes the maximum over the
  footprint's angular window, exactly as `claim_flute` takes the maximum over the reachable stretch.
  Numbers worth keeping: on the oracle's fixture the claim authorises 542.288 mm³, the program removes
  541.000, the tip-only spiral would authorise 527.788, and the analytic `π·t·(D − t)·d` is 603.186 — the
  gap to the analytic is the crest, which is the thread, not a defect. T7 also **discovered a pre-existing
  defect it did not fix**: `grid_for` builds the field from `job.width/height` at origin `(0, 0)` while
  programs are emitted in machine coordinates, so for any origin corner other than bottom-left the field
  and the program disagree about *where* everything is — for every strategy since M5, not for threads. It
  is recorded here, with its severity, and it needs its own slice; every fixture and the default sheet are
  bottom-left, which is why nothing caught it.
- **L18 (execution, T8, the closing evidence).** `docs/m6-toolpaths.md` is 655 lines in the shape
  `docs/m5-simulation.md` has, and every console block and number in it was produced by running the
  shipped binary rather than transcribed: the six authoring lines, a seven-toolpath simulate report, the
  helical `G03` pair, a post refusal at exit 4, and a 60° bit's square chamfer at 0/0. README's status
  and "Next" move to M7 with M6 reported as shipped; `odd/roadmap/index.md` gains §4.9 with the two
  prerequisites the milestone's plan did not anticipate, a ten-row deliverable/where/proof table, the
  origin-corner defect as a defect rather than a paragraph, and D-M6-12 / D-M6-14 in §7.
  `ModelError::IslandsRequirePocket` became `IslandsRequirePocketOrInlay` with its sentence and
  `docs/region-booleans.md` updated, because an inlay male carries islands and the old message said only
  a pocket could. The closing differential against `8a953ba` ran 29 invocations over ten subcommands in an
  identical sandbox, comparing exit code, stdout, stderr and every produced file: **two** deliberate
  differences (`--help` listing nine kinds and the new flags, and the unknown-`--kind` sentence) plus one
  outside that sandbox — the islands refusal, which the CLI cannot author and which this task made false.
  No regression. The gate is `./scripts/check.sh` green (`fmt --check`, `clippy -D warnings`, the whole
  suite) at **975 tests**, `cam-model` 47, `cam-toolpath` 123, `cam-sim` 99, `cam-cli` 104.
  The origin-corner defect was reproduced through the binary before it was written down: an 80 x 30 mm
  region at `y = 60..90` in a 200 x 100 sheet reports `gouge 0.000 / uncut 24.000 mm³` at `bottom_left`
  and `gouge 14376.000 over 2396 cells / uncut 14400.000` at `top_left`; `origin: center` collapses
  `removed` from 47976 to 2394. It is recorded in §6 of the roadmap with a fix sketch and left unfixed,
  because changing the field's frame touches every strategy's numbers and needs its own slice and its own
  tests — one per origin corner.
- **L19 (T8, recorded corrections to this document).** Two statements here were superseded by later work
  and are noted rather than rewritten: L13 says a negative ramp was not refused at document load, and it
  now is (`ModelError::FlutingRampNegative`); and A10's "twelve subcommands" is twelve invocations of ten
  subcommands, as L18's differential reports. Neither changes a decision.
- **L20 (delivery, the native review this milestone could not have).** The owner-owned review switch is
  on, so the candidate was submitted: `inspect` reported the whole branch ready (46 paths, `workspace`
  against `master` `8a953ba`), and `start` with all four lenses was refused with
  `lens_context_budget_exceeded` — no lineage was created and nothing was mutated, so there was nothing to
  abandon or repair, and the provider states that retrying that exact candidate cannot succeed. The
  candidate is ~16.3k authored lines; the per-commit sizes are 819 (T1), 1777 (T2), 1119 (T3), 3269
  (T4+T4b), 1710 (T5), 2541 (T6), 3163 (T7) and 868 (T8). The three large ones are large for the reason
  L8 and L10 record: a new `Operation` variant breaks the exhaustive matches of four crates and
  `Move::Helix` breaks the workspace's, so slicing them by layer would leave the tree uncompilable. The
  owner's decision was to merge M6 on its own evidence — **975 tests**, the gate green, and a
  differential of 29 invocations against `8a953ba` identical in exit code, stdout, stderr and every
  produced file but three deliberate differences — and to carry the native review into M7, whose slices
  are additive over what this milestone landed. Recorded because "merged without a review" and "merged
  after a review the tooling refused" are not the same fact, and the next reader deserves the second one.
