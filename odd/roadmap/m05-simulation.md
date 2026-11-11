# M5 — simulation

[Back to index](index.md)

### 4.8 M5 delivered (evidence)

M5 is the acceptance test the earlier milestones were written against: a green suite says a toolpath
satisfies its rules, not that the part comes out right. The new `cam-sim` crate takes the `MoveStream`
`cam-cli emit` would hand the post-processor (D-M5-1), sweeps it over a heightfield of the stock, derives
the depth each cell was **authorised** to lose from the document alone, and reports gouge and uncut
material as one subtraction with two signs (D-M5-2). `cam-cli simulate` is that report, and the shell
shows the same field as a depth-shaded 3D surface it can play. The feature document is
`odd/tasks/006-m5-simulation.md`; the model, the report's exact shape, the exit codes and every
recorded limit are in [docs/old/m5-simulation.md](../../docs/old/m5-simulation.md). The workspace stands at **795
tests**, green under `./scripts/check.sh` (`fmt --check`, `clippy -D warnings`, the whole suite).

| Deliverable | Where | Proof |
| --- | --- | --- |
| A grid that covers the job in whole cells with a hard cap, and the stock as one surface height per cell | `crates/cam-sim::grid`, `crates/cam-sim::heightfield` | 11 tests; the default 1219 x 2438 mm sheet is 2,971,922 cells, under the 4,000,000 cap, and a 0.1 mm request names `12190 x 24380`, the product and the cap in one message; a 10.2 mm wide job gets 11 columns whose last centre lies outside the nominal width, which is the coverage bound stated rather than hidden; a zero-sized job and a zero cell are `Degenerate`, not a panic; the top is `0` and the bottom `-thickness`, a cell never rises, a cell clamped at the bottom counts as through, a whole field taken there reports 500 mm³ removed, 0 remaining and 100 through cells, and a property test asserts `removed + remaining` equals the grid volume over random cut sequences |
| Swept removal over a program: feeds and arcs cut, rapids never do, and a rapid below the surface is a crash | `crates/cam-sim::sweep` | 6 tests; a straight feed's cut cells agree cell for cell with a brute-forced exact point-to-segment capsule; a full-circle arc leaves its own centre cell standing, so the arc is swept as an arc; a rapid at clearance cuts nothing and is no crash; a rapid to `-3` is exactly one crash carrying `pocket 1` and move index 1, with the field untouched; two runs are field-identical. `cam_move::Segment` gained `tool_diameter`, which the emitter ignores |
| The authorised target surface and the two deviations | `crates/cam-sim::target` | 15 tests, then 20 after D-M5-5's amendment; a pocket region with a centred r10 island authorises the square minus the disc; `Outside`, `Inside` and `On` profiles authorise the band each tool actually sweeps; a tab keeps its height back; a drilling toolpath authorises each hole and nothing else; two pockets authorise the deeper, never the sum; a cell nobody claims is zero; a field cut exactly to the target deviates zero, and one difference read with two signs is a gouge and then an uncut of the same volume |
| `cam-cli simulate`: the report, the exit code and the tolerance | `crates/cam-cli` | 9 binary tests, `cam-cli` at 53 (55 after S5 and S6); whole six-line reports pinned byte for byte for an offset pocket, a raster pocket, a drilling toolpath and a too-shallow pocket; at a finer resolution the coverage error falls from 505.0 to 126.5 mm³ on a 120.4 x 90.4 mm pocket; a too-shallow pocket exits `6` with uncut and a drill wider than its hole exits `6` with gouge, each stating its own cubic millimetres; a crash exits `6` naming the toolpath and move index on the first crash line; two runs print identical bytes; `--resolution 0.01` exits `2` naming the cell, the grid and the cap; `--help` lists the command, its flags and its exit code; the document is byte-identical afterwards |
| The oracle for every strategy M4 ships, in both signs | `crates/cam-sim/tests/oracle.rs`, `crates/cam-cli` | 6 kernel tests (`cam-sim` at 43) plus 2 added and 1 strengthened through the binary; an offset pocket removes 59976 of 60000 mm³ analytic, raster agrees with offset within 600 mm³, three r3 holes remove 576 mm³ against 508.938, and a circular `Outside` profile is exact; refinement at 2, 1 and 0.5 mm never grows the error (115.062, 67.062, 4.938 mm³); one region cut 1 mm shallow reports 96 mm³ uncut and 1 mm deep 96 mm³ gouge, so the test cannot pass by reporting the wrong sign |
| A 3D camera and the stock surface as a mesh, both pure data | `crates/cam-view::camera3d`, `crates/cam-view::mesh` | 13 tests (`cam-view` at 36); the target projects to the viewport's centre at four pitches and three yaws; yawing equals rotating the point about the vertical; behind the eye is `None`; the pitch clamp holds at both ends; zoom clamps at 1 mm and 100 m and ignores a zero, a negative, a NaN and an infinity; a 10 x 8 field gives 80 triangles and 126 vertices at stride 1, 12 and 12 at stride 4, 4 and 2 at stride 100; the depth ramp is exactly 0.0 at the top and 1.0 at the bottom; the stock box is 12 axis-aligned edges; two builds are identical vertex for vertex, with the triangles ordered back to front |
| The 3D panel, its gestures and the painter's mesh | `crates/cam-ui::view3d` | 9 tests (`cam-ui` at 61); an open panel paints triangles and exactly 16 3D lines (12 box edges + the program's 4); a drag orbits and the wheel zooms about the target; a camera the projection rejects paints nothing instead of panicking; a closed panel paints no 3D at all; the surface reaches the painter as one `egui::Mesh` of three vertices per painted triangle (A11); the 2D canvas still paints its 4 items and its grid, and a closed panel leaves the counts equal to a shell that holds no preview at all — the 52 pre-existing shell tests untouched |
| Playback that is a pure function of an injected delta | `crates/cam-ui::view3d` | 10 tests (`cam-ui` at 71, `cam-desktop` at 2); six deltas at speed 1 walk the cursor 0→1→2→2→2→4 with fractions 0.5/0.25/0.2/0.6/0.9/1.0; the field's volumes equal the closed form of the same prefix; a partly consumed move cuts nothing; a zero delta and a paused frame change nothing; `step` advances exactly one move; `reset` restores the stock cell for cell; the final field equals `sweep::apply` over the whole stream both from one 10 s call and from 40 x 0.03 s frames; playback stops at the end and never loops; and `DrawnState::preview_progress` is 0.0 paused, strictly between 0 and 1 after one playing frame, and exactly 1.0 at the end |
| No new dependency, and the 2D editor untouched | workspace | the one new `[[package]]` in `Cargo.lock` is the workspace's own `cam-sim`: the 3D view is `egui::Mesh` on the existing painter (D-M5-7), and the editor's suites keep passing unchanged (A12) |

The milestone found one real defect outside its own crates, and it is the kind S6 exists to find. **S6:**
`Region::raster_spans` answers `None` for a row lying along a boundary, and `row_spans` retried such a row
at `y + nudge` only — so at a region's near edge the nudge landed inside the material and the row was
cut, while at the far edge it landed outside and the row was dropped. The row the far edge sits on is the
row that clears the strip that edge ends. Measured before the fix on a 100 mm square with a 2 mm tool at a
2 mm stepover: **49** rows instead of 50, 9800 of 10000 cell centres covered, 58800 mm³ removed against
the analytic 60000, and 200 cells / 1200 mm³ reported uncut — the same numbers the M5 oracle had reported
as a simulation failure. The fix is in `row_spans`: the nominal height, then `y + nudge`, then `y - nudge`,
and the first height that clips any material wins. Two nudge directions rather than a tolerance knob, no
"last row" special case, and a fixed candidate order, so the emitted program stays a function of the
region and the row height alone (S9). Two expectations moved and were re-derived rather than fitted: the
raster `simulate` fixture now runs its own 2 mm stepover instead of the 1.58 mm one that had been chosen
so the row lattice missed the far edge, and the island raster golden gained the `y = 57` row (45 to 49
lines, 9 spans to 10). Every non-raster golden is byte-identical, which is what says the defect was where
the oracle measured it and nowhere else.

Recorded deviations, each a decision in the feature document: **D-M5-5 was amended by evidence** — the
profile target is the material the tool *sweeps* (`allowance … 2·radius + allowance`), not the offset of
its centre, because the first form reported the outer half of every clean kerf as gouge (1632.0 mm³ over
272 cells on a perfect 100 mm `Outside` cut), and the `On` side ignores an allowance because the emitter
does; **D-M5-6** gives a found defect its own exit code, `6`, on the precedent `check` set with `5`;
**D-M5-7** paints the surface with `egui::Mesh` while D-M1-2's `wgpu` trigger stays intact, since a wgpu
backend would consume the same mesh; **D-M5-8** ships no `rayon` and no tiling, with the one-second
measurement as its trigger; **D-M5-9** keeps the preview off the document — it moves a cursor over a
program a caller hands in, adds no `cam-app` command and pulls no `UiAction`; **D-M5-10** does not
authorise the wall an allowance keeps, so a correct allowance pocket is not reported as uncut;
**D-M5-11** records the mitered-corner over-authorisation — at most `4(r² − πr²/4)` per right-angled
corner, about `7.73 mm² × depth` at a 6 mm tool, measured as 24 mm³ over 4 cells at a 1 mm cell — with
the circular contour as the case where the statement is exact. Two smaller corrections are recorded rather
than hidden: `set_preview` takes the untouched stock, because `reset` needs a pristine field and nothing
can un-cut one (L8), and a rapid and a retract are timed at the segment's feed rate because `Segment`
carries no rapid rate. Linking stays conservative (D-M4-6, unchanged): M5 makes a gouge-free linking
provable, it does not re-plan a program.

