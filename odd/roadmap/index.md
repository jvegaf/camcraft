# Roadmap index

Two layers live here. The **live plan is the camcraft pivot** — phases F0–F8 defined in
[docs/ROADMAP_CAMCRAFT.md](../../docs/ROADMAP_CAMCRAFT.md), diagnosed in
[docs/GAPS_CAMCRAFT.md](../../docs/GAPS_CAMCRAFT.md) and designed in
[docs/SYSTEM_DESIGN_CAMCRAFT.md](../../docs/SYSTEM_DESIGN_CAMCRAFT.md) — and its verified state is §1
below. The **pre-pivot general-machining project (M0–M15)** is history: it shipped, and §2 keeps its
record. Milestones are sequential; each was its own ODD feature document created when it started, and
every one ended in an artefact demonstrable through `cam-cli`, not a slide.

## 1. Camcraft roadmap state (F0–F8)

Audited **2026-11-12** read-only against the worktree, not against these documents: every row cites
`path:line` evidence that exists today, and a feature is only `✅` when its code is there. The test
suite was **not** run for this audit, so `✅` means *implemented and covered by named tests*, not
*green right now*.

Legend: `✅` done · `🟡` partial (code exists, the exit criterion does not hold yet) · `⬜` absent
(verified by `grep`/`find` with zero matches).

### F0 — Foundations

| ID | State | Evidence | What is missing |
| --- | --- | --- | --- |
| E0.1 Set engine | ✅ | `cam-geom/src/boolean.rs:754` `BooleanOp`, `:805` `Combined`, `:835` `combine`; 5 properties in `cam-geom/tests/boolean_properties.rs:147` (commutativity, idempotence, `A\A=∅`, inclusion–exclusion) incl. a two-arc disc generator; task [034](../tasks/034-c1-booleans.md) delivered `aee3e5d` | — (known limit: a disc that *touches* a region's boundary from inside is still refused at 1e-9) |
| E0.2 Input robustness | 🟡 | `cam-geom/src/region.rs:148` resolves self-crossing contours in `normalize`; recalculation in `boolean.rs:153` | Degenerate/unresolvable input is still refused (`region.rs:82`, `boolean.rs:75`, `:94`); the exit criterion is «the corpus produces no normalization errors» and there is no corpus |
| E0.3 One tolerance policy | ⬜ | Constants remain: `cam-geom/src/lib.rs:52` (`DEFAULT_TOLERANCE=1e-6`), `offset.rs:49` (`0.001`), `region_offset.rs:41`, `:54` | No tolerance object at all: `grep -rn "struct Tolerance" crates` → 0 matches |
| E0.4 Geometric parity harness | ⬜ | `find . -type d -name '*parity*'` → 0; only `.pp` fixtures exist (`cam-post/tests/fixtures/`) | No polygon/area/point-count comparison against reference geometry |
| E0.5 Board corpus | ⬜ | `find . -path '*corpus*'` → 0 | No KiCad/EasyEDA/Eagle/Altium boards with `top/bottom/mask/silk/outline/drill` |
| E0.6 FlatCAM baseline | ⬜ | No baseline table or script under `scripts/`, `odd/reports/`, `crates/` | Nothing measured to compare against |
| E0.7 Benchmarks | ⬜ | No `benches/` anywhere; `grep -rn criterion Cargo.toml crates/*/Cargo.toml` → 0; `justfile` has no `bench` recipe | `criterion` harness and the CI regression job |
| E0.8 CI | 🟡 | Local gate `scripts/check.sh:6` = `check-docs` + `cargo fmt --check` + `clippy -D warnings` + `cargo test --workspace`, wired at `justfile:10` | No `.github/workflows`, so nothing runs on a PR; no parity/bench job |

### F1 — Board model and importers

| ID | State | Evidence | What is missing |
| --- | --- | --- | --- |
| E1.1 Board model | 🟡 | `cam-model/src/lib.rs:27` `DOCUMENT_SCHEMA = "camcraft/document@1"`; `LayerRole` at `:197` (copper top/bottom, mask, silk, outline, drill, keepout); `Layer { role, … }` at `:237` | **Divergence from the plan**: there is no `c2d/document@3` and no `@2→@3` migration — the schema was *renamed* and legacy `c2d/*` is **rejected** (`cam-model/tests/document.rs:150`, `cam-store/tests/document.rs:139`, `:209`). Per-entity origin metadata is not there. Task [035](../tasks/035-c3-model.md) is still `in progress` (M1 tag/role/reject done) |
| E1.2 PCB operations in the model | ✅ | `cam-model/src/lib.rs:432` `Operation::{Isolation, NonCopperClear, Cutout, Paint, Panelize}`; params `:289`–`:429`; validation `:1612`–`:1734`; round-trip `cam-model/tests/document.rs:369` | Behaviour beyond parameters, which is intentional at this stage |
| E1.3 Fabrication-folder importer | ⬜ | `cam-cli/src/lib.rs:453` — `import takes exactly one input file` | No grouping of a folder into a board by role |
| E1.4 Gerber parser | 🟡 | `crates/cam-gerber/src/lib.rs:14` `parse`; format/units/polarity `:79`; `image(tolerance)` evaluates the layer `:193` | Still rejected by name with the command index: a `D01` trace with an aperture outside a region, apertures `O`/`RoundRect`, macro-named apertures, and a second `D02` inside `G36/G37`. Task [036](../tasks/036-gerber-geometry.md) delivered `fc90dda`; the limits are listed in its doc |
| E1.5 Polarity and nesting | 🟡 | Dark/clear through `combine` in `cam-gerber/src/lib.rs:193`; tested `cam-gerber/tests/gerber.rs:96` | **Step & repeat absent** — no `SR` handling |
| E1.6 Excellon parser | 🟡 | `crates/cam-excellon/src/lib.rs:6` units, `:25` tool table, `:46` `Feature::{Hole, Slot}`, `:225` `G90/G91` | Omitted-leading/trailing zeros and the wider dialect are not corpus-proven |
| E1.7 Import report for Gerber/Excellon | ⬜ | `grep -rn ImportReport crates/cam-gerber crates/cam-excellon` → 0 | Both crates surface their own errors instead of `ImportReport`/`Skipped { construct, reason, location }` (`cam-import/src/report.rs`) |
| E1.8 PCB tool library | 🟡 | `cam-store/src/tools.rs:174` geometry carries `included_angle`, `flat_diameter`, `tip_radius`, `ToolKind::{VBit, Drill, …}` | `cam-cli tools add` writes the basics only (`cam-cli/src/lib.rs:598`); no isolation-cutter type and no PCB workflow |

### F2 — Drilling and cutout

| ID | State | Evidence | What is missing |
| --- | --- | --- | --- |
| E2.1 Drilling from an Excellon table | ⬜ | `cam-ops/src/peck.rs:31` takes geometric `DrillHole`s; no path from `cam-excellon` to a program | Grouping by diameter, tool selection, slots as holes or as paths |
| E2.2 Deterministic ordering | 🟡 | `cam-ops/src/peck.rs:179` nearest-neighbour tour with a deterministic seed | The 2-opt pass under an **iteration** budget |
| E2.3 Cutout with tabs | 🟡 | Tabs validated in `cam-ops/src/follow.rs:24`; `CutoutParams` tabs/tab kind at `cam-model/src/lib.rs:315`, validated `:1679` | No outline-driven cutout program, no gaps/mouse-bites |
| E2.4 Penetration predicate | ⬜ | `grep -rn "overlaps_copper" crates` → 0 | The pure function that F3 accepts against |
| E2.5 Full drills + cutout program by CLI | ⬜ | `cam-cli/src/lib.rs:260`–`:360` lists `validate inspect import export check parts cutlist report tools postp script` | No generation command at all |

### F3 — Isolation

| ID | State | Evidence | What is missing |
| --- | --- | --- | --- |
| E3.1 Copper union before isolation | ⬜ | No consumer of `combine` outside `cam-gerber` | The layer union step |
| E3.2 Multipass accumulated offset | 🟡 | Ring machinery `cam-ops/src/rings.rs`, `cam-geom/src/region_offset.rs:109`; arc-preserving self-intersection cleanup in task [037](../tasks/037-offset-exact.md) (`79a07c9`) | The `dia·((2i+1)/2) − i·overlap·dia` pass law and the direction by milling type; `Region::offset` can still refuse a traversal that crosses the shape |
| E3.3 V-bit depth | ⬜ | `IsolationParams` carries `cut_depth` only (`cam-model/src/lib.rs:329`) | Depth derived from included angle and tip |
| E3.4 Anti-gouge guard | ⬜ | `grep -rn "gouge guard\|anti-gouge" crates` → 0 | Repair-or-fail with an actionable diagnosis |
| E3.5 `iso_type` + keep pours | 🟡 | `IsolationKind` and `combine_passes` exist as parameters (`cam-model/src/lib.rs:301`) | Nothing selects or executes them |
| E3.6 `cam-cli isolate` | ⬜ | `grep -rn '"isolate"' crates/cam-cli/src` → 0 | The command and its post-processor path |

### F4 — Non-copper clear

| ID | State | Evidence | What is missing |
| --- | --- | --- | --- |
| E4.1–E4.7 | ⬜ | Only the model: `NonCopperClearParams` (`cam-model/src/lib.rs:357`) and `FillMethod::{Lines, Grid, Rings}` (`:289`) as data; `grep -rn rayon Cargo.toml crates` → 0, `grep -rn "rstar\|kdtree"` → 0, no `rest machining` | Envelope, concentric fill, spatial index, parallel fill, rest machining, seed/lines methods, and the published FlatCAM comparison — the whole phase, and the product's performance thesis with it |

### F5 — PCB G-code

| ID | State | Evidence | What is missing |
| --- | --- | --- | --- |
| E5.1 FlatCAM dialect catalogue | 🟡 | `.pp` parser/emitter with byte goldens: `cam-post/src/lib.rs:27`, `:137`, `:268`; fixtures `cam-post/tests/fixtures/post_inch.pp`, `post_mm.pp` | The GRBL/Marlin/Repetier dialect **data** |
| E5.2 `exc_areas` and travel routing | ⬜ | `grep -rn "exc_areas\|exclusion" crates` → 0 | Model field, routing around it, and its simulation check |
| E5.3 Autolevelling | ⬜ | `grep -rn autolev crates` → 0; `cam-move` still carries fixed Z | Height map plus arc and long-move segmentation as a stream transform |
| E5.4 `toolchange_style` | ⬜ | `grep -rn toolchange_style crates` → 0 | The enum replacing name matching |
| E5.5 Gerber/Excellon writers | ⬜ | `cam-export/src/lib.rs:67`, `:123` — DXF and SVG only | Both writers and their round-trip proof against `cam-gerber`/`cam-excellon` |

### F6 — Board UI, F7 — extension, F8 — hardening

| ID | State | Evidence | What is missing |
| --- | --- | --- | --- |
| E6.1 Board layer identity | ⬜ | `Layer.role` exists in the model (`cam-model/src/lib.rs:237`), the UI still draws generic layers (`cam-view/tests/scene.rs:17` uses `LayerRole::Geometry`) | Colour by role, copper mode |
| E6.2 Project tree | ⬜ | No board→layers→operations tree in `cam-ui` | The tree and its navigation |
| E6.3 Operation forms | ⬜ | `grep -rn "IsolationParams\|NonCopperClearParams\|CutoutParams\|PaintParams" crates/cam-ui/src` → 0 | One form per PCB operation |
| E6.4 Tool panel bound to the library | 🟡 | `cam-ui/src/tool_library/` is a real editor over the SQLite store (tasks [032](../tasks/032-tool-library-app.md), [033](../tasks/033-tool-library-editor.md)) | The binding to PCB operations, which do not exist yet |
| E6.5 Shortcuts and Ctrl+S | 🟡 | Shell chrome and the help/shortcuts map exist (`cam-ui/src/shell/`, task [010](../tasks/010-camui-shell.md)) | Proof of Ctrl+S and a documented PCB-flow map |
| E6.6 Integrated preview + simulation | 🟡 | `cam-ui/src/view3d/` with playback and `cam-sim` behind it (task [027](../tasks/027-m13-in-app-cycle.md)) | Present, but only for the pre-pivot strategies; nothing PCB to preview |
| E7.1–E7.4 | ⬜ | `PaintParams` (`cam-model/src/lib.rs:395`) and `PanelizeParams` (`:414`) are parameters with no executor; `grep -rn thieving crates` → 0; no film/punch | Paint, panelize, film/punch, copper thieving. Note the plan's warning: nesting ≠ panelize, and **`nest`/`nest_profile` no longer exist** (task [039](../tasks/039-wood-out-sweep.md) removed them) |
| E8.1–E8.5 | ⬜ / 🟡 | No corpus, no benches, no installer (`grep -rn "installer\|packaging"` → 0 relevant). Docs do exist: `README.md`, `docs/ROADMAP_CAMCRAFT.md`, `docs/GAPS_CAMCRAFT.md`, `docs/SYSTEM_DESIGN_CAMCRAFT.md` | 20+ board regression corpus, published benchmarks, signed installers, a migration/CLI guide for a new user, and the alpha |

### Where the code actually stands

- **The pivot's first real asset is the new `cam-ops` crate** (`cam-ops/src/lib.rs:1`): profile-with-tabs
  (`follow`), drilling/peck/drill-bank (`peck`), pocket/rings (`rings`) — rescued from `cam-toolpath`
  so they evolve independently of the wood domain.
- **The wood domain is out (D4 executed, task [039](../tasks/039-wood-out-sweep.md))** — `cam-toolpath`
  and `cam-program` no longer exist, `cam-geom` lost `text`/`moulding`/`nest`/`nest_profile`, and
  `Operation` now lists PCB operations only.
- **Three crates exist that GAPS_CAMCRAFT.md predates**: `cam-gerber`, `cam-excellon`, `cam-ops`.
- **Phase 0 is the gap that decides the rest**: the engine (E0.1) exists but not one of its
  instruments (E0.3 tolerance policy, E0.4 parity, E0.5 corpus, E0.6 baseline, E0.7 benches, E0.8 CI).
  Every phase below F0 is blocked on being able to *measure* correctness and speed.
- **Only E1.2 is fully done of all F1 items**, and only E0.1 of F0. Everything else is partial or
  absent; F4 — the product's thesis — is 0 %.

### Divergences between the plan and the code (decide, do not drift)

1. **Schema identity.** `docs/ROADMAP_CAMCRAFT.md` §4 plans `c2d/document@3` as an *additive* step with
   an `@2→@3` migration (D3). The code writes `camcraft/document@1` and refuses legacy `c2d/*`. That is
   a deliberate break, not an additive bump; D3 in `docs/GAPS_CAMCRAFT.md` §6 now describes something
   that did not happen. Either D3 gets amended with this decision, or the rejection is a defect.
2. **Removed assets.** GAPS §7 lists `cam-geom`'s `nest`/`nest_profile` as reusable; task 039 removed
   them, and E7.2 explicitly must not reuse nesting anyway — so the row is consistent, but the GAPS
   line is stale.
3. **`EntityKind` is still `Contour`-only** (`cam-model/src/lib.rs:250`), which is what GAPS flagged as
   blocking (G2.1) and what D3 resolved as "grows at most with `Drill` and `Slot`". Not done.

### Waves and who builds them

The scheduling view of all of the above — which items run in parallel, in which worktree, with which
claimed surfaces and which merge order — is [waves.md](waves.md). Its first wave is exactly the
instrument set below.

### Next slice, from here

Instruments before product, as `docs/ROADMAP_CAMCRAFT.md` §15 says — but the cheap half of the
instrument set is still the cheapest thing available:

1. **E0.5 corpus + E0.4 parity harness** — the two that turn every later claim into a test, and the
   only way to close E0.2's exit criterion.
2. **E0.8 CI** — without it the corpus and the parity harness protect nothing on a PR.
3. **E0.3 tolerance policy** — the object E3.2 and E4.2 will need, cheaper to introduce before them.
4. **Close task [035](../tasks/035-c3-model.md)**, which carries the board model, then F2 (drilling,
   cutout) as the first real G-code through `cam-cli`.

E0.7's benchmarks and E0.6's baseline can wait for the first PCB operation that has something to
measure.

## 2. Pre-pivot milestones (M0–M15, history)

Delivered, kept as the record of the general-machining product that the pivot froze (D4). The table
below is not a status list: all fifteen milestones shipped.

| M | Milestone | Deliverable (verifiable) | Why here | Link |
| --- | --- | --- | --- | --- |
| **M0** ✅ | Foundation + vertical slice | **Delivered.** `cam-cli emit part.c2d --post fixture.pp --library tools.sqlite` produces a byte-for-byte tested G-code program for an outside profile, with arcs preserved, tabs, pass chunking and validation on every input | Proves units, geometry, doc format, tool library, post-processor emitter end-to-end before any UI exists | [M0](m00-foundation.md) |
| **M1** ✅ | 2D editor shell | **Delivered.** Viewport with pan/zoom-about-cursor/fit, grid, picking by pixel distance, marquee select, snapping, undo/redo, layer dock, properties and status bar, plus a native window | Geometry only becomes usable with a viewport; all of it rides on `cam-app`, proven headless | [M1](m01-editor.md) |
| **M2** ✅ | Vector authoring & editing | **Delivered.** Line/polyline/arc/circle/ellipse/rectangle/polygon/star, single-stroke text, node editing, join/weld/trim, offset, mirror/rotate/scale, array copy and align — as commands through `cam-app`, in the shell and in the native window | Authoring is half of what a 2.5D CAM application does; all of it rides on `cam-geom`, proven headless | [M2](m02-authoring.md) |
| **M3** ✅ | Import & interchange | **Delivered.** DXF, SVG, PDF, EPS and raster trace import with a full report; `cam-cli import` builds the document, `cam-cli check` reports open, self-intersecting and degenerate vectors, and the shell imports undoably (see §4.5) | Users bring geometry; the target set is the filter list shop drawings actually arrive in | [M3](m03-import.md) |
| **M4** ✅ | Core toolpaths | **Delivered.** Profile (inside/outside/on, tabs, passes, lead-in/out) from M0, plus offset and raster pocket with allowance, stepover and clear stepover, and drilling with peck, both retract modes and drill-bank selection; every strategy is authored through `cam-cli toolpath` and reaches G-code through `cam-cli emit` (see §4.6) | The three strategies that define 2.5D routing; they exercise every post-processor block | [M4](m04-toolpaths.md) |
| **M5** ✅ | Simulation & 3D preview | **Delivered.** `cam-sim` sweeps the emitted `MoveStream` over a heightfield of the stock, derives the depth each cell was authorised to lose from the document alone, and reports removed volume, through-cut cells, gouge, uncut material and every crash; `cam-cli simulate` is the contract, with exit `6` for a program that gouges, leaves material standing or rams the tool; a depth-shaded 3D stock view with an animated playback is in the shell; and every strategy M4 ships has an oracle (see §4.8) | G-code without simulation is unverifiable by the user; also the only honest acceptance test for M4–M7 | [M5](m05-simulation.md) |
| **M6** ✅ | Depth-variant strategies | **Delivered.** Quick Engrave, Chamfer, Fluting, Texture, Thread Milling and the Inlay pair (female cavity + male plug from one command), each authored through `cam-cli toolpath` and each with a simulation oracle in both signs; the V-bit's cone now reaches the move vocabulary and the simulation, and the post gained a helical move so a thread is emitted as a helical arc; `c2d/document@2` unchanged and every pre-existing command byte-identical (see §4.9) | The six strategies that make it a carving machine rather than a router; they cost two prerequisites the row did not name — tool geometry and a helical move — and are cheap only after them | [M6](m06-depth-variants.md) |
| **M7** ✅ | V-Carve | **Delivered.** `cam-cli toolpath --kind vcarve` walks the region's verified medial axis with the tip at `−d/tan(θ/2)` for each tip's own exact distance `d` to the boundary, so the toolpath is a depth **field**: the emitted program is varying-Z linear feeds and needs no post change, `simulate` authorises the distance field from the document alone, and the oracle asserts both signs with `gouge` **exactly zero**; V-carve is proved by two golden programs (a square and a disc, each three ways), the exact certificate every axis vertex carries, and a 35-invocation differential against `e0d39a3` (see §4.10) | Research-grade geometry (medial axis of arcs); deliberately isolated so it cannot delay the rest | [M7](m07-vcarve.md) |
| **M8** ✅ | Moulding / swept profile | **Delivered.** `cam-cli toolpath --kind moulding` sweeps an authored cross-section along a rail at a stepover: the rail is the selected entity, one repeatable `--profile <lateral>:<depth>` per cross-section point and `--stepover` (or the library's), and the program is one lateral pass per stepover offset — the rail walked by arc length at that pass's own Z, one linear feed per rail sample. `simulate` authorises the cross-section swept along the rail from the document alone, and the oracle asserts both signs on a straight rail, a closed frame and a V-bit cone, with `gouge` **exactly zero** for a correct program; moulding is proved by two golden programs (a straight rail and a closed frame, each three ways), the stepover law's coverage certificate, the multi-rail case end to end, and one measured limit recorded rather than hidden (see §4.11) | The row expected M7's distance field to be the cost; it was not — the cross-section is authored data | [M8](m08-moulding.md) |
| **M9** ✅ | Nesting | **Delivered.** `cam-cli nest` packs a document's authored sheets and parts — rectangles through maxrects and a guillotine pass, free profiles through a sound-but-incomplete no-fit heuristic — and stores the placements plus **one** cost record derived by `Document::nesting_utilisation`; a run that leaves an instance unplaced reports it and exits `7` writing nothing. Proved by 78 behaviour tests (31 in `cam-model`, 14 + 17 in `cam-geom`, 16 in `cam-cli`), a byte-for-byte report and document golden set, the fixture where guillotine beats maxrects, the counted cost of the profile heuristic's incompleteness, a 42-invocation differential against the milestone's cut point, and two reviewed candidates with their corrections (see §4.12) | Own heuristic engine as decided (D4); depends on stable geometry and toolpaths | [M9](m09-nesting.md) |
| **M10** ✅ | Documents & scripting | **Delivered.** `cam-cli script` runs a line-oriented command language against one document — eleven words, each dispatching to the function its own subcommand calls, with `line:column` diagnostics and every mutation through `Document::validate`; `cam-cli parts` imports a four-column part list and `cam-cli cutlist` exports the placements M9 stored as seven columns, deliberately two shapes and not one schema; `cam-cli report` writes a self-contained, hand-escaped HTML print sheet; and `cam-ui`'s `ScriptPanel` runs a script through the same library, never a process. No schema bump, no external dependency, and `--help` is the only difference from the pre-M10 surface (see §4.13) | Reporting and extension points: a scripted command surface is the model — with S1's "DSL de comandos, sin motor" deciding the engine the row left open ("Rhai/Lua") | [M10](m10-documents.md) |
| **M11** ✅ | Feeds & speeds | **Delivered.** The library already keys cutting data per material and per machine (`material`, `tool_entity`, `cutting_data_for(geometry, material, machine)`, proved by one tool at 120 and 60 mm/s in two materials); what is missing is the **selector**, so `emit` cuts at the tool's own record and `tools add` writes the same `100 / 30 / 18000` for every material. M11 carries `Job.material` (optional, `#[serde(default, skip_serializing_if)]`, tag stays `c2d/document@2`) plus an `emit --material` override that wins, threads it into the lookup the store already has, and leaves **`None` reproducing today's bytes exactly** — all 17 goldens keep `F100.0`. An unknown material is refused by name, never silently ignored. Deriving a rate from chipload and surface speed is **not** in this row: it would change the one number the goldens pin and put a derivation between the library and the program that nothing tests and the selector is threaded to the store's existing per-material lookup. **Evidence**: 21 golden tests untouched, 1548 workspace tests green, an unknown material refused by name, and a review that caught the flag accepting a missing name and a no-selector change nobody asked for, both corrected in 79 diff lines and validated. Two follow-ups recorded: the shell's estimate cannot honour a material until `cam-store` answers one, and `simulate` has no `--material` to reproduce a one-run override | Every strategy emits at the tool's fixed rate today, which is the most user-visible gap left: the machine is told how fast to cut by a record nobody selected (see §4.14) | [M11](m11-feeds-speeds.md) |
| **M12** ✅ | Export | **Delivered.** The workspace has no writer of any interchange format — `cam-import` says so in its own header, and `cam-ui`'s icon catalogue has declared `export-dxf` and `export-svg` since M1 with no command behind them. M12 adds a `cam-export` crate mirroring the reader's shape: **DXF** keeps layer identity (one `LWPOLYLINE` per contour, each vertex's bulge verbatim, `70 = 1` when closed, `$INSUNITS = 4`, the layer table in document order) and **SVG** writes one `<path>` per contour with `A` commands whose `rx == ry`, the Y reflection the reader inverts, and a `viewBox` in the document's own millimetres. The acceptance test is the **round trip against our own reader**: exported and imported again, the same shape and the same layer order, compared up to the 10 mm placement the reader performs — never byte equality. SVG's documented loss is that it carries no layers. Toolpaths are **not** written: a toolpath is not geometry, and writing one would make "the same thing that went in" unprovable | A drawing tool that can read five formats and write none is a one-way door; the reader's own conventions are what make the writer provable (see §4.15) | [M12](m12-export.md) |
| **M13** ✅ | The cycle closes inside the app | **Delivered**, with one task routed to the owner rather than closed. Two dead ends hid behind finished chrome: the app bar's primary action was `add_enabled(false, …)` with a hover excuse (`cam-ui/src/chrome.rs:323`) and the 3D half printed *"Simulate one with `cam-cli simulate` and hand it in"* (`view3d.rs:1390`), so a window that could import, author and display a program sent the person to a terminal for the one artefact the machine runs. M13 makes the program the app's own: `GenerateGcode` posts the open document through `cam-post` inside `apply_action`, the same place `SaveFile` writes (`S7`, corrected after measurement — it had said the host would do it), `Post…` holds the dialect for the session and refuses a file that is not a post-processor while keeping the one in hand, the 3D half builds the drawing's own program with `program_for_snapshot` over a pristine `Heightfield` behind a `Simulate` chip and a `Simulate again` in the overlay row, and the toolpaths are a list whose selection — not `toolpaths.first()` — is what the card, the editor and `open_tool_library` read, with the last of those a door the first version left aiming at the first toolpath (see §4.16). **T4, decided by the owner and delivered in `18ded99`**: the 3D stock paints through `StockTop`/`StockBottom` via `stock_shade` and the system's own value was a cool `#d6dce6` — the tan is the reference palette, so the change is a palette decision, and the owner took it. Resolving the values found a proof nobody had written: the amber `Cut` is painted **on** the stock, and it measured 1.47:1 there before this change; holding it to 3:1 pins the dark stock to a mid-value walnut and narrows its depth ramp from 6.33:1 to 1.80:1 (see §4.16) | The most visible gap left after twelve milestones was not missing machinery but missing *seams*: every part worked, and the person still could not finish a job without a terminal (see §4.16) | [M13](m13-in-app-cycle.md) |
| **M14** ✅ | The tool library as an editor | **Delivered.** The library modal was a read-only snapshot with one sentence where the reference screen draws six buttons: `New, Duplicate, Delete, Import… and Export… are not here` — true, and a dead end. M14 makes it the screen: a header band with a close that writes nothing, three columns (master 300 / detail 424 / preview 200), a tree grouped by the tool's **kind** with folding headings that carry their count, an editor whose every number is a box with its unit outside it, the tool drawn vertically in a dark card with its length and diameter called out, and the footer sentence above `Cancel` / `Use this tool`. Two things had to become true in the store first: the six kinds are now **stored** in `tool_type` rather than guessed (a drill and a V-bit both carry an included angle, so no rule over the geometry tells a 118° point from a 90° V — zero stays reserved for "the writer did not say", which is what `cam-cli tools add` has always written, so no existing library is relabelled), and `validate_tool` + `save_tool`/`remove_tool`/`replace_tools` exist at all, because `put_geometry` accepted any row it was handed. Editing is a **draft**: nothing is written until `Use this tool`, and the sentence that replaces that button when a draft cannot commit is `validate_tool`'s own, so the two cannot drift. Every library write happens **file first, snapshot second** — a failure mutates nothing and keeps the window open, because a snapshot that changed beside a file that did not is the one outcome nobody can see. Found by the suite's own rewrite and fixed here: `close_tool_library` promised in its doc that the drafts went with it and left one behind, so a cancelled visit's typed diameter came back | A window that can import, author, post and simulate still sent the person to `cam-cli tools add` to fill its own tool library; the reference screen had the button row, and the shell had a paragraph explaining why it did not (see §4.17) | [M14](m14-tool-library.md) |
| **M15** ✅ | The tool library the application owns | **Delivered.** M14 made the screen and left it unreachable: `writes` was `tool_library_path.is_some()`, only an opened `.c2d` naming an existing `tools.sqlite` could set that path, and nothing in the app created the file or wrote `tools_ref` — so the six write actions were permanently dark and the populated screen could only be reached by hand-building a SQLite file and editing a document's JSON. The library is now the **application's** (the owner's decision, recorded verbatim in `odd/tasks/032-tool-library-app.md`, D1): one `tools.sqlite` in the app's data directory (`CAM_PILOT_DATA_DIR` as the only override), opened and seeded with the factory catalogue at launch, and opening a document no longer reads a library at all — `tools_ref` stays in the model and `cam-cli` keeps honouring it, because the field is a CLI contract and the ownership rule is the app's. The factory set went from four tools to the reference's **22 across its six families** (T1–T22), with speeds and feeds derived by one rule rather than hand-tuned per tool: feed 100 mm/min, plunge 30, spindle 18000, stepdown half the diameter capped at 3 mm, stepover 40 % of it. `Import…` now **asks** before it writes — `Replace all`, `Add`, `Cancel` — and `Add` skips a tool the library already holds by id, by trimmed case-insensitive name, or by tool number, so importing the same file twice is a no-op that says so. The dialog itself takes the reference's proportions (columns 700 tall rather than 372, a 330-tall preview card drawn light with a dark tool and blue callouts) so the whole form fits without scrolling, as the reference's does. | The screen was finished and unusable, which is the worst of both: the milestone that delivered §4.17 delivered a modal nobody could fill, because the library it edits is not a drawing's property (see §4.18) | [M15](m15-tool-library-app.md) |

Deferred / conditional, not scheduled: PDF text-to-outline fidelity beyond vector paths, EPS edge
cases, rotary wrapping (`ROTARY_WRAP_X/Y` in the DSL is parsed but wrapping toolpaths are a later
milestone), drill-bank hardware integration, i18n.

## 3. Pre-pivot documents

- [M0 — foundation](m00-foundation.md)
- [M1 — editor](m01-editor.md)
- [M2 — authoring](m02-authoring.md)
- [M3 — import](m03-import.md)
- [M4 — toolpaths](m04-toolpaths.md)
- [M5 — simulation](m05-simulation.md)
- [M6 — depth variants](m06-depth-variants.md)
- [M7 — vcarve](m07-vcarve.md)
- [M8 — moulding](m08-moulding.md)
- [M9 — nesting](m09-nesting.md)
- [M10 — documents](m10-documents.md)
- [M11 — feeds and speeds](m11-feeds-speeds.md)
- [M12 — export](m12-export.md)
- [M13 — in-app cycle](m13-in-app-cycle.md)
- [M14 — tool library](m14-tool-library.md)
- [M15 — tool library app](m15-tool-library-app.md)
- [Region booleans and pocket islands](region-booleans.md)
- [Decision log](decisions.md)
- [Log](log.md)
- [Verification strategy](verification.md)
- [Risks](risks.md)
- [Original preamble](preamble.md)
- [Task index](../tasks/INDEX.md)

## Archive

- [Pre-pivot product, stack and architecture](../archive/pre-pivot.md)
