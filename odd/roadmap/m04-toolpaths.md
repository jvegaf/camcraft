# M4 — toolpaths

[Back to index](index.md)

### 4.6 M4 delivered (evidence)

| Deliverable | Where | Proof |
| --- | --- | --- |
| Two new `Operation` variants inside `c2d/document@1`: `Pocket(PocketParams)` and `Drilling(DrillParams)`, with `PocketStrategy::{Raster, Offset}` and `RetractMode::{Amount, Full}` | `crates/cam-model` | 6 tests; both kinds round-trip losslessly, `RetractMode::Amount` serialises as `{"mode":"amount","amount":…}`, an absent `bank` is omitted, and an unknown `kind` still fails to load |
| Closed-contour validation generalised from profile to every operation that cuts one | `crates/cam-model` | the existing error variant now names pocket and drilling toolpaths by entity, and every M0–M3 document still loads |
| `circle_centre`, and one shared depth rule extracted from profile | `crates/cam-toolpath` | 8 tests on `circle_centre` (2- and 4-arc circles either winding; lines, mixed shapes, open contours and arcs with different centres refused); `pass_depths` shared by profile, pocket and drilling |
| Offset pocket: rings offset from the original boundary, outermost first, each proving its distance | `crates/cam-toolpath::pocket` | 12 tests; 12 rings at insets 3, 7, … 47 on a 100 mm square with a 6 mm tool at 4 mm stepover; `[5, 10, 12]` pass chunking with every ring cut once per pass; arcs kept as arcs in rings; a wall-gouging ring ends the ringing instead of being emitted |
| Raster pocket: rows on the `min.y + k·stepover` grid, finished with one row on `max.y` when the grid stops short of it, clipped by true line/arc intersections | `crates/cam-toolpath::pocket` | 33 rows on the same square at 3 mm stepover — 3, 6, … 96 on the grid plus the finishing row at 97 — and 50 rows unchanged where the far edge already sits on the grid; analytic chord widths on a circle within 1e-6, and the tangent finishing row of the radius-17 region cuts a chord of about 0.037 mm; a tangent row emits no unpaired span; no arc moves in raster mode |
| Drilling: circle-only holes, nearest-neighbour tour, pecks, both retract modes | `crates/cam-toolpath::drill` | 13 tests; `order = [h1, h2, h3, h4]` and `rapid_length = 110 mm` on a 20 mm square; a 3 mm peck on a 7 mm cut plunges at −3, −6, −7; `Full` reaches safe Z and `Amount(1)` does not |
| Drill bank as selection with head compensation, and no machine command | `crates/cam-toolpath::drill` | 7 tests; `drill_index = [Some(0), Some(0), Some(1), Some(2)]` for holes at x = 0, 10, 40, 100 with 3 drills at 32 mm; `bank_travel = 60 mm` against `rapid_length = 120 mm`; a one-drill bank emits the un-banked program move for move |
| `cam-cli toolpath`, plus `inspect` and `emit` for the new kinds | `crates/cam-cli` | 9 tests; appends and reloads, ids are deterministic, the library's `stepover`/`clear_stepover`/`stepdown` supply the defaults, every refusal exits `2` with the file byte-identical, `--out` leaves the input alone, `inspect` names each kind and strategy |
| Golden G-code for pocket and drilling, and a closed error set | `crates/cam-cli`, `crates/cam-toolpath` | 5 tests; a 42-line program asserted whole three ways (as built, after a c2d round trip, and with both toolpaths authored through `cam-cli toolpath`); an exhaustive `match` over every `ToolpathError` with its exact text, so a stub cannot return as a contract |
| 61 tests over four slices, **610 in the workspace** | `crates/` | `cargo test --workspace` green: 23 + 12 + 13 + 7 + 5 added, one T3 placeholder removed with them |

M4 defects found and fixed at the root: `RetractMode` could not be a tagged newtype over a length, because
a bare number cannot carry the tag, so it is a struct-like variant with a named `amount` field; the first
`circle_centre` tests built a two-half-turn shape from `tan(π/4)` where a quarter turn needs `tan(π/8)`, so
they were measuring themselves; the `toolpath` parser advanced two tokens for a positional argument, so the
document path swallowed its first option; and the offset kernel's own refusals do not end the ringing — a
shrink wider than a feature comes back as a well-formed smaller ring with unchanged winding after which the
rings grow, so a ring now proves every vertex sits the requested distance inside the original boundary
(D-M4-9), without which a 3 mm square pocketed by a 4 mm tool emitted a wall-gouging ring and looped
forever. Three more were fixed after both writers reported them: `OpenContour` said "profile toolpaths
need closed geometry" while pocket and drilling return it too, so the message now names no single
operation; `cam-cli toolpath --kind drilling` accepted a rectangle and left the refusal to `emit`, so the
append now resolves every entity through `circle_centre` and exits `2` before writing; and the golden
program pinned a bare `G00` the fixture post emits between toolpaths as `NEW_SEGMENT`, which is
pre-existing post behaviour now covered by a test rather than left to chance. This narrows §4.2's M0 wording: the kernel refuses an offset that crosses the shape's interior, but
not every over-shrink, and the guard lives in the strategy while the kernel limit stays recorded for
arrangement-based offsetting.

Recorded deviations, each a decision in the M4 feature document: `stepover` is a toolpath parameter and the
library's `stepover`/`clear_stepover` are the CLI's defaults (D-M4-3), because the same tool pockets MDF and
hardwood differently and the library keeps the rate on the tool record; `--clear` reads `clear_stepover` for
one clearing pocket rather than introducing a two-tool toolpath (D-M4-4), with the meaning of its "Clear
stepover" field still unresolved; a pocket was one region per selected contour, with no islands until the
kernel gained boolean subtraction (D-M4-2, retired since — see
§4.7); drilling accepts
circles only (D-M4-5); linking stays conservative, retracting to safe Z between rings, rows, spans and holes
(D-M4-6, D-M4-10); raster rows always run along X (D-M4-7); and M4 ships no toolpath UI, `cam-cli toolpath`
being the authoring surface (D-M4-8). The drill bank is geometry and selection only: no `G` code, `M` code or
toolchange is emitted, because the plan defers drill-bank hardware integration and the move vocabulary has
no bank command, and `rapid_length` (hole spacing) and `bank_travel` (head travel) are reported as the two
different numbers they are.

Six spec wordings were corrected by implementing rather than by reading, and all six are in the feature
document's log. Four came from the pocket slice: the ring stop rules were insufficient, because a shrink
wider than a feature returns a valid smaller ring and the rings then grow (the distance proof of D-M4-9 is
the fix); the raster example said "32 rows at y = 3, 6, … 97", arithmetic that cannot hold on the declared
grid, where 32 rows end at 96; the retry nudge of `1e-6` equalled the kernel tolerance, so a retried row
could land back on the ambiguity band, and it is now `1e-5`; and "a 4 mm tool in a 5 mm pocket is refused"
is false — a 2 mm inset leaves a 1 mm region the tool fits, so the refusal test uses a 3 mm pocket. Two came
from the drill-bank slice: the bank bound could not be `(count-1)*pitch/2` from a hole when the emitted
position is `x − k*pitch` with `k >= 0`, so what is bounded is the head's incremental step, and the T3 test
that asserted a bank is ignored was deleted rather than adapted, because it pinned exactly the behaviour the
bank task exists to reverse. One scope correction was recorded instead of papered over: `drill()` receives
resolved holes rather than contours, so the non-circle refusal is pinned where the resolution happens, in
`tests/drill_targets.rs` and the CLI layer, not in `tests/drill.rs`.

