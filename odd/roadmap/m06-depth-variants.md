# M6 — depth variants

[Back to index](index.md)

### 4.9 M6 delivered (evidence)

M6 is the milestone whose plan was wrong in an instructive way. The roadmap row called the six strategies
"profile/pocket variants with a different Z law — cheap once M4 and M5 exist"; two facts found before the
first write said otherwise, and they are why the milestone has nine task rows rather than six (L3).
**(1) No tool geometry reached the toolpath layer.** `ToolSpec` carried a number, a diameter, a feed rate,
a plunge rate and a spindle speed, while `included_angle`, `flat_diameter` and `tip_radius` had sat unused
in the tool database since migration 1, and `cam-sim` swept a constant-radius cylinder per segment — so a
cone had no carrier at all, and a chamfer swept as a cylinder would have reported a *correct* bevel as a
gouge (D-M6-3). **(2) `Move::Arc` is constant-Z by contract** and the post fixture's arc blocks carry
`[X][Y][I][J]` and no `[Z]`, so thread milling could not ride on the existing move or block: M6 added
`Move::Helix`, four helical block kinds, and a refusal when a post defines none (D-M6-8). Neither was
absorbed silently, and both are recorded as the milestone's own cost.

The feature document is `odd/tasks/008-m6-depth-variants.md`; the strategies, their Z laws, their
refusals, the simulation's claim for each, the differential and the one defect M6 recorded without fixing
are in [docs/old/m6-toolpaths.md](../../docs/old/m6-toolpaths.md). The workspace stands at **975 tests across 70 test
targets** — 180 more than the 795 the branch cut from — green under `./scripts/check.sh` (`fmt --check`,
`clippy -D warnings`, the whole suite). In the four crates M6 touches that is cam-model 47, cam-toolpath
123, cam-sim 99 and cam-cli 104; `cam-sim/tests/target.rs` holds 43 tests, 23 of them M6's, and
`cam-sim/tests/oracle.rs` holds 23, of which 17 are the six strategies' (the other six are M5's).

| Deliverable | Where | Proof |
| --- | --- | --- |
| Six `Operation` variants and their parameter records inside `c2d/document@2`: `Chamfer`, `Engrave`, `Fluting`, `Texture`, `Inlay`, `Thread`, with `ChamferParams::depth` and `TextureParams::grooves` the only derivations | `crates/cam-model` | 47 tests in `tests/document.rs`; every kind round-trips losslessly, a `fluting` document without `ramp` and a `texture` without `spacing`/`angle` load as the pattern they describe (the fields default), an unknown `kind` is still a hard load error, and each refusal names its own value — a zero chamfer width or an angle outside `(0, 180)` would otherwise emit an infinite Z, and a negative ramp, a non-positive spacing, a degenerate inlay stepover, a thread with no pitch and an inlay record naming both halves are refused at load. An `inlay` pair is two toolpaths sharing one operation, and its defaults *are* the pocket's, taken from `PocketParams::default` rather than restated |
| The tool's shape on the move vocabulary, and a sweep that grows with depth | `crates/cam-move::ToolShape`, `crates/cam-sim::sweep`, `crates/cam-sim::target` | `ToolShape::{Cylinder, VBit}` replaces `Segment.tool_diameter`, with `radius_at(depth)` and `rise_at_radius(radius)`; the replacement was behaviour-neutral before any cone test existed (**795 tests**, the number the branch cut from). A 90° V-bit's floor is graded at 3 and 5 mm from the line and reaches 6 mm, a 2 mm flat has a flat bottom and a 7 mm reach, and the D-M5-5 agreement test with a cone reports **gouge exactly zero at zero tolerance** — the sweep's sample can only be further from the path than the path is, so a cone cut is never reported deep |
| Chamfer: a bevel whose depth is derived once from the bit's angle, in one pass with the tip on the vector | `crates/cam-toolpath::chamfer`, `crates/cam-cli` | 7 strategy tests, 3 oracle tests and its binary cases; `6 mm` on a 60° bit is **10.392 mm** deep and `3 mm` on a 90° bit is **3.000**, derived in `cam-model` so program and claim cannot disagree; the oracle asserts **both flanks** (`L·w·d`, not the one flank `L·w·d/2` A9 first named — L9) around a 40 mm ring within `4πR·cell/2·d` and deviates by nothing; a separate oracle covers the one-sided case on the stock's edge; a flat tip is refused at **all three** boundaries that can see it — before authoring, after the document exists, and in the segment's own shape (L11) |
| Quick engrave: one constant-depth pass on the contour, open or closed | `crates/cam-toolpath::engrave` | 6 strategy tests, 3 oracle tests and its binary cases; the closed form is `L·d²·tan(θ/2)` — **3325.6 mm³** for a 6 mm pass over 160 mm with a 60° bit — and an end mill removes a uniform band; the claim carries the reach test a cone self-clamps and a cylinder does not (L12); `cut_depth` at or below `start_depth` is refused naming both |
| Fluting: one pass per vector with a real ramped entry and exit, and the ramp rides an arc as a **helix** | `crates/cam-toolpath::fluting`, `crates/cam-move::Move::Helix` | 10 strategy tests, 3 oracle tests and its binary cases; `2·R·d·(L − ramp) = 5400 mm³` analytic against **5548.5** measured, the 148.5 being mostly `π·R²·d = 169.6 mm³` of toes a real tool leaves where each ramp meets the surface — four samplings of one geometry inside the published `960 mm³` bound (the rejected staircase measured 5549.2); the claim is the **envelope** the law allows anywhere the footprint could have reached, because a tip-only claim reports a clean flute as `gouge 149.2 mm³` (L13); T7 lifted T4's `RampOnArc` refusal and the fixture that chorded at `ramp²/(8r)` now simulates with **gouge 0.000 / uncut 0.000** |
| Texture: parallel grooves at a spacing and an angle, derived once, never a clearing pass | `crates/cam-toolpath::texture`, `cam-model::TextureParams::grooves` | 8 strategy tests, 2 oracle tests and its binary cases; the pattern is clipped in the groove frame by the kernel's own `Region::raster_spans`, so arcs and islands come free and no second intersector exists; a 40 mm square at an 8 mm spacing is five rows and `2·R·d·ΣL = 7200 mm³`, measured **exactly** 7200; the 45° case asserts the emitted diagonals and the claim geometry, because a diagonal groove leaves a two-cell rind the 1 mm grid does not sample (`uncut 1.000 mm³ over 2 cell(s)`, M5's published bound — L15); the command says in its help, and in its refusal of `--stepover`, `--allowance`, `--side`, `--pass-depth`, `--tab`, `--lead-*` and `--ramp`, that the material between grooves stands |
| The inlay pair: one command, two toolpaths, fixed order, the allowance as the clearance | `crates/cam-toolpath::inlay`, `crates/cam-cli` | 9 strategy tests, 2 oracle tests and its binary cases; the female clears the region grown by `allowance` with the pocket's offset rings and the male clears `Job::outline` with the region standing as an island in raster rows; the halves are told apart by the ids they carry (D-M6-13), and a record naming both is refused rather than guessed at; the ring pattern was rejected on a **measurement** — 2300 mm² of an 8556 mm² frame never cut at a 2 mm stepover, against 80 mm² for the rows; what remains is the pocket's own coverage bound, pinned in the oracle as **`uncut 480.000 mm³ over 80 cells`**, with the female's four unreachable corner cells at `24.000 mm³ over 4` |
| Thread milling: a helical descent, a helical move, and a spiral claim | `crates/cam-toolpath::thread`, `crates/cam-move::Move::Helix`, `crates/cam-post` | 9 strategy tests, 4 oracle tests, its binary cases and the emitter's helical block test; the orbit is `(D∓t)/2`, the descent `cut_depth/pitch` revolutions as one full helical move each plus the fraction, and a 3 mm thread at a 1.5 mm pitch emits two `G03` blocks carrying `[Z]`; a post with no helical block **refuses, exit 4**, naming the four kinds instead of emitting a flat circle at the wrong depth. The first claim (the whole annulus at `cut_depth`) failed every real thread — `uncut 106.452 mm³ over 151 cells` — and the tip-only spiral reported a correct thread as `gouge 8.697 mm³`; the claim is now the spiral, taking the maximum over the footprint's angular window (D-M6-14). On the oracle's fixture: analytic `603.186 mm³`, authorised **542.288**, removed **541.000**, tip-only `527.788` — the 62.186 to the analytic is the **crest**, which is the thread |
| Determinism and the old bytes | workspace | every new subcommand run twice prints and emits identical bytes (inlay pair, thread and the rest, asserted from the document on disk through the program); and the pre-existing command surface was run against `master`'s tip `8a953ba` in an identical sandbox — 29 invocations over `validate`, `inspect`, `toolpath` (profile, pocket offset, pocket raster with an island, drilling with a bank), `emit`, `check`, `postp validate`, `tools list`, `tools add`, `import` (report and `--out`), `simulate` at three resolutions, the error paths, an unknown command and an unknown kind — comparing exit code, stdout, stderr and **every produced file** (documents as their JSON payloads, programs byte for byte, the imported document, the library). Exactly two differ, both by design: `--help` and the unknown-`--kind` message. Everything else, including the `--islands` refusals and a four-toolpath `simulate` report, is byte-identical. A third, deliberate difference sits outside that sandbox because the CLI cannot write the document that shows it: `validate` on a document carrying islands on a profile toolpath now says "takes islands only for a pocket or an inlay" — the same exit code, the sentence reworded because an inlay male takes islands (T8) |
| No new dependency, and the editor unchanged in what it authors | workspace, `crates/cam-ui` | `Cargo.lock` is unchanged between `8a953ba` and this branch — the same 470 `[[package]]` entries. `cam-ui` gained one thing and it is not an authoring surface: the 3D preview's segment carries the tool's **shape**, so the field it cuts is the field the program's tool would leave (its own 72 tests, the pre-M6 shell expectations among them); no toolpath form, no new command, and the 2D canvas untouched |

The milestone found one defect it did **not** fix, and it is recorded rather than buried. `cam_sim::grid::grid_for`
builds the field at origin `(0, 0)` from `job.width/height`, i.e. in document coordinates, while programs
are emitted in machine coordinates (`cam_toolpath::machine_point` translates or flips them for `top_left`
and `center`), and the sweep cuts the field with the move coordinates as they are. So at any origin corner
other than `bottom_left`, and **for every strategy since M5**, the field and the program disagree about
where everything is. Measured: a single pocket over an 80 x 30 mm region at `y = 60 … 90` reports
`gouge 14376.000 mm³ over 2396 cells` and `uncut 14400.000 mm³ over 2400 cells` at `top_left`, against
`gouge 0.000` and `24.000 mm³` over 4 cells at `bottom_left`; with `origin: center` a 100 x 80 mm square
pocket's `removed` collapses from 47976.000 to 2394.000 mm³. The emitted G-code is unaffected — it is what
the machine runs — but the report and the 3D preview agree with the document only at bottom-left, and a
`bottom_left`-symmetric fixture hides it, which is why every fixture and the default sheet missed it. It
needs its own slice: a fix moves the grid's origin and the sweep's cell mapping together, and nothing
short of that is honest.

Recorded deviations, each a decision in the feature document: **D-M6-1** delivers the six cheapest first
and the post-contract change last, so the riskiest unit could not block the other five; **D-M6-2** derives
a chamfer's depth once, in the model (amended twice — the bevel is symmetric so there is no side, L7, and
a flat tip is refused by name rather than absorbed, D-M6-10); **D-M6-4** makes an engrave the profile's
`On` side with the pass law deleted, and **D-M6-5** makes a flute's ramp a length along the vector with an
arc carried as a helix; **D-M6-6** makes a texture a pattern and never a pocket, with the pattern derived
once; **D-M6-7** ships the inlay as two pocket toolpaths sharing one depth rather than two extra step
parameters, and **D-M6-13** records that the male cuts rows because the ring lattice left 2300 mm² of the
frame standing; **D-M6-9** consumes the library's `included_angle` and `flat_diameter` and refuses a
missing field by name instead of defaulting 90°; **D-M6-11** leaves the CLI's "a flag no kind uses is
silently dropped" open milestone-wide — closing it changes the contract for callers that pass a superset
today — while every M6 kind refuses by name the flags its own law forbids; and **D-M6-12** amends D-M5-3
so the sweep interpolates Z along a cutting move, which is what lets a ramp be one move and a helix be
swept honestly. Two amendments are recorded because the specification got them wrong: A9's oracle
arithmetic asserted one flank where a symmetric bevel removes two (L9), and A10's oracle for the thread
asserted the annulus where the helix leaves a spiral (L17).

