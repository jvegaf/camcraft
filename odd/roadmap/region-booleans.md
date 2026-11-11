### 4.7 Region booleans and pocket islands delivered (evidence)

M4's decision D-M4-2 left pockets island-free "until the kernel has boolean subtraction". That kernel
work exists now: the `boolean ops` item of §3's `cam-geom` description is delivered as region
**subtraction**, expressed as even-odd parity plus the arrangement and the distance proof that keep
parity honest. A union is not part of it and is not claimed. The feature document is
`odd/tasks/007-region-booleans.md`; the contract and the user-facing behaviour are in
`docs/old/region-booleans.md`. The workspace stands at **707 tests**, green under `./scripts/check.sh`.

| Deliverable | Where | Proof |
| --- | --- | --- |
| Exact winding number, three-way membership (`Outside`, `Inside`, `OnBoundary`) and the contour relation (`Disjoint`, `FirstInside`, `SecondInside`, `Crossing`, `Touching`), arc-native and never decided from bounds alone | `crates/cam-geom::winding` | 9 + 11 tests; a single-point intersection counts as a crossing only away from both segments' ends, so a shared corner is `Touching`; a property test compares `classify` against a fine tessellation over three shapes, another asserts the relation is antisymmetric |
| `Region` as a flat boundary list under even-odd parity, with normalization (split at crossings, classify by parity, orient material-on-the-left, walk the arrangement) and subtraction as append-and-normalize | `crates/cam-geom::region`, `crates/cam-geom::boolean` | 15 tests; two overlapping squares normalize to two 75 mm² L-shapes — their symmetrical difference, not their 175 mm² union (D-RB-8); touching squares become one 200 mm² contour of 60 mm perimeter, so the shared edge is not doubled; a square plus an edge-crossing r4 circle gives 74.867 + 25.133 mm² with every surviving arc still radius 4 about its original centre; the walk is vertex-for-vertex deterministic; a property test checks that a normalized region's `contains` agrees with the original's |
| `Region::offset` — signs from each boundary's nesting depth, so a wall shrinks and an island grows — with the distance proof moved out of the pocket and into the kernel, and `Region::raster_spans` for even-odd row clipping | `crates/cam-geom::region_offset` | 17 tests; a 100 mm square inset by 10 gives `(10,10)–(90,90)` with area 6400 and every vertex 10.0 mm from its wall; an r10 island at an 8 mm distance gives an outer ring 84 mm wide and an r18 island ring (1017.876 mm²); an odd-depth hole grows (314.159 → 706.858 mm²); square/hole/boss gives 90, 70 and 10 mm widths; an over-shrink is refused where a naive `offset_contour` returns a well-formed gouging ring; a row crossing the island gives spans at `50 ± √91`, a tangent row gives `None` |
| Five whole-program goldens through `cam-cli emit` and the real post: offset island, its save/load round trip, raster island, the `@1` round trip, and the nested boss | `crates/cam-cli` | 45, 45, 45, identical-bytes and 43 lines; the programs were derived from the geometry and read line by line before being pinned — wall squares 3..97 / 11..89 / 19..81, island rings `r13`/`r21`/`r29` as `G03` half turns, raster gaps at `30 ± √(13² − (y−30)²)`, and the boss's rings shrunk to `r9`/`r2` because a nested island is material again and is cut |
| The exact minimum distance between two segments — line/line, line/arc and arc/arc in closed form — and a distance proof that is one comparison per pair with **no resolution knob** | `crates/cam-geom::distance` | 13 tests; hand-computed closed forms for every pair type, including a nearest point that is an endpoint and one that is interior; K5's chord (ends 23 mm, middle 17 mm) is dropped; the 4 mm fixture's analytic 14 stages are visible without a tool; property tests cross-check the answer against a sampled lower bound and against `intersect` |
| `Toolpath.island_ids`, its validation, and `c2d/document@2` | `crates/cam-model`, `crates/cam-store` | 11 model tests plus 1 store test; nesting is legal — a boss strictly inside a hole validates, and a refactor that refuses it again fails `an_island_nested_inside_another_is_a_boss_and_is_accepted` — while crossing and touching are refused; the field round-trips and is omitted when empty; an island outside the boundary, one in a C-shaped pocket's notch, two overlapping islands, a repeated island, an open island and islands on a profile toolpath are each refused by name; a `@1` document loads, keeps its toolpaths byte-identical and saves as `@2`; `@3` is refused naming both tags |
| `--islands` on `cam-cli toolpath --kind pocket`, and the island count `inspect` prints | `crates/cam-cli` | 3 binary tests: a pocket records its islands and reloads with them; an island outside the boundary exits `2` with the file byte-identical; `--islands` on a profile exits `2` naming the flag and the operation it belongs to. `inspect` appends `, N island(s)` and the island-free line is M4's |
| Offset pocket over a region with islands: rings from `Region::offset(inset + k·stepover)`, the strategy's own distance proof deleted | `crates/cam-toolpath::pocket` | 8 tests (7 in `tests/pocket_islands.rs`, one through the CLI asserting what the island adds as arc blocks — 0 without it, 14 with it — rather than as a line count); the 4 mm fixture cuts the analytic **14** rings and the closest approach over the emitted moves is 3.0000 mm; an 8 mm stepover gives 6 rings over three stages; a region that leaves nothing and a 3 mm pocket under a 4 mm tool are both `OffsetCollapsed` carrying the kernel's reason; a narrow neck reports `flattened`; two runs are identical. **All 12 M4 pocket tests passed with their expectations untouched** |
| Raster pocket over a region with islands: rows clipped by `Region::raster_spans`, so a row crossing an island is two spans | `crates/cam-toolpath::pocket` | 5 tests; 100 mm square plus centred r10 island at a 3 mm stepover gives **33 rows and 41 spans** — 8 rows cut two spans ending at `50 ± √(13² − (y−50)²)`, 25 cut one span `3…97` (the finishing row at `y = 97` is 47 mm from the island's centre, outside its 13 mm reach), and the row at `y = 63` is exactly tangent, so the kernel answers `None` and the retry resolves it just outside; a full-width island band gives 25 rows and 25 spans; a narrow strip at 7 mm gives 15 rows and 25 spans with no unpaired span; every cutting move stays ≥ `3 − 1e-6` mm from the island. **`tests/pocket.rs` needed no edit** (12/12) and a differential over eight island-free raster fixtures came back as 875 identical output lines |

Defects found and fixed at the root, in the order the tasks surfaced them: the arc's contribution to the
winding number was computed at its tangency points and its branch chosen from the *sign* of an `atan2`
result, so a bulge dipping below its own chord read as outside — it is now cut where it crosses the
horizontal line through the query point and the branch is decided by distance (`crates/cam-geom::winding`);
A2's rule that any single-point intersection is a crossing made two squares sharing a corner overlap, so a
meeting point now only counts when it is interior to both segments (L5); A4 named `cam_geom::chain` for
chaining the arrangement, and `chain`'s greedy endpoint match rejoins a piece to its own continuation and
hands the input back with its crossings, so the walk turns at every crossing and `chain` is untouched (L7);
the island containment check asked `relation(boundary, island)` and compared it to `FirstInside`, so every
valid island was refused — `relation` is directional and the island is the first argument (L6); the offset
kernel's vertex-only distance proof was unsound for arcs, and the 4 mm fixture carried two lens-shaped
pieces whose only vertices cleared the distance while their arcs bulged 6 mm into the wall (L9); and the
straight-piece form of the same defect — a chord 17 mm from a boundary whose ends are 23 mm away — is what
made a closed-form segment distance the answer rather than a finer subdivision (L10, D-RB-11).

Recorded deviations and limits, each attributed: normalization is **parity-preserving, not a union**
(D-RB-8), so two overlapping boundaries describe their symmetrical difference and a union remains a
separate operation; a boundary that crosses **itself** is passed through unresolved, because A4 only asks
about crossings between boundaries (L7); an exact tangency in the arrangement is **refused by name** rather
than resolved, as at exactly 20 mm in the 100 mm square plus r10 island fixture, where the grown island is
tangent to all four wall insets; `intersect` at tolerance `0.0` can miss a real crossing, which is why the
distance proof asks for the unsnapped minimum and the cross-check runs at the kernel's nanometre; and a
piece that fails the proof is still dropped **whole** (D-RB-9), which costs the material it covered but can
never gouge. The two earlier forms of the proof — vertices only (D-RB-9) and arc stretches against a
sagitta budget (D-RB-10) — are gone rather than softened, and D-RB-11 records why the exact pair distance
is the form that has nothing left to tune.

Four spec wordings were corrected by implementing, all in the feature document's log. A2's crossing rule
was wrong about shared corners and edges (L5); A4's chaining primitive was `chain`, which cannot resolve a
four-way crossing (L7); A4's area expectations described a union where parity gives the symmetrical
difference, so the tests pin 75 mm² L-shapes and 100 mm² for a square plus an edge-crossing circle (L7);
and D-RB-10 as first written budgeted the subdivision by the piece's sagitta, which does not justify a
Lipschitz step hundreds of times larger, so the budget became the stretch's arc length and then, with
D-RB-11, nothing at all (L10). Two corrections came from the strategies rather than the kernel: a raster
row is emitted at the height the kernel actually clipped at, not the nominal grid height, because emitting
the nominal one moves a tangent row's ends off the curve by about 1e-5 mm (L11); and a CLI assertion that
counted program lines was replaced, since an island ring is two arcs where a wall is four straight moves
— what the island adds is arc blocks (L10).

One task of this feature closed its own defect. **P5**, the golden G-code for a pocket with an island in
both modes, the same program after a save/load round trip, and an island nested in an island (a boss)
cut as material again (A14), landed in `075f8da`: five whole programs, and the boss golden found P1's
rule refusing every nested pair, so the fix and its proof arrived together. The island-free half of A15
is covered by the two differentials above — M4's pocket tests pass unchanged and the island-free
fixtures emit identical programs.

