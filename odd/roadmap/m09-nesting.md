# M9 — nesting

[Back to index](index.md)

### 4.12 M9 delivered (evidence)

M9 is not a machining strategy at all — it emits no move, so **no post contract changes** — and it is
cheap in what else it touches: the `nesting` section is an **optional top-level field**
beside `tools_ref` and `postp`, so **no schema bump** (`DOCUMENT_SCHEMA` stays `c2d/document@2` and a
document written before M9 loads and re-saves byte-identically); and it adds **no crate and no
dependency** (`Cargo.toml`/`Cargo.lock` unchanged from the milestone's cut point `2ad152d`). What it spent
is two packers, a certificate each, and the one arithmetic the owner asked to be **both** report and
field. The milestone is seven commits, ten files, **6 606 insertions and 7 deletions** in total.

The owner's four answers are the whole specification (S1–S4, verbatim): "Un documento maestro con N
hojas y N instancias"; "Guardar los placements en el documento"; "Los dos: reporte y campo"; "No: las
hojas se autoran y viven en el documento". So a part is a selection of the document's own entities plus
an instance count, there is no parts library and no default board, the sheets live in the document, and a
run writes the placements and the cost back into it. Two rules make that safe to store: a placement's
`position` is the **placed** footprint's lower-left corner with the quarter turn applied before anchoring
(D-M9-1, the defect T1's tests caught — turning about the part's own origin put the footprint at negative
X), and an authored section with **no placements** is a request that loads, while from the first
placement on every part's instance count must equal its placements.

Both nesters S6 names are reachable by name and each states its guarantee as **one predicate**. Maxrects
and guillotine run on the same area-descending stable list, and the auto rule prefers fewer unplaced
parts, then fewer sheets, then maxrects; `NestPlan::certifies` is the sentence "inside the sheet, and a
gap apart on it" and every test asserts it **beside** its own clearance measurement rather than instead
of one. The fixture where the two heuristics disagree is the proof that half of S6 is not decoration: a
**234 × 173 mm** board at gap 8 with parts **110 × 60**, **111 × 87** and **38 × 112** needs **2 sheets**
with maxrects (utilisation `0.25335951781038485`) and **1** with guillotine (`0.50671903562076970`),
because maxrects' split around the 111 × 87 part leaves an 86 mm top strip where the 38 × 112 needs 112
and the guillotine's full-length cut leaves a 242 × 86 strip for the 110 × 60. On the cabinetry fixture
that S7 exists for — a **2440 × 1220** board, gap 8, two 720 × 580 sides and three 560 × 520 shelves —
the plan is **5 parts on 1 sheet, 0 unplaced, `0.57403923676431068`** (1 708 800 of 2 976 800 mm²).

Free profiles take `cam-geom::nest_profile`, and the milestone records its limit as a **measurement**
rather than an adjective: the heuristic is **sound** (a placement is accepted only when the candidate's
outline is `Disjoint` from every outline already on the board and from every no-fit region on it) and
**incomplete**, which the counted fixture states — a **100 × 100** board, a 40 × 100 bar, a 50 × 50
square at gap 5 and step **20** puts the square on a **second board** although **`(45, 0)`** is legal,
and step **5** finds it at `(50, 0)`, one step past the no-fit region because contact is refused. There
is no exact NFP to reach for: building the Minkowski difference needs a boolean **union**, and `Region`
offers `subtract` and nothing to build a region *with*. Inside-ness is the board's **material**, not the
absence of a crossing — `FirstInside` outright, `Touching` only when the part inset by **`INSET_STEP`**
(0.01 mm) is itself strictly inside — and that rule is the review's one CRITICAL defect, corrected with
three REDs before any `src/` edit and a differential of the 11 pre-existing profile tests against
`df69091` (11 passed both sides).

Utilisation has **one** arithmetic and the milestone spent a review cycle on it. `Document::nesting_utilisation`
is placed footprint area over the area of the sheets **the placements name**, waste its complement, and
the stored `NestingCost` must equal it; the denominator is pinned with exact numbers — five 100 × 100
placements on a **500 × 400** sheet is **50 000 / 200 000 = 0.25** with a second authored-but-unused
**1000 × 1000** sheet deliberately excluded, and the T1 fixture pins **30 000 / 240 000 = 0.125**. The
strong rule then indicted **T1b's own fixture** (0.25/0.75 sums to one and is wrong about the area) and
six tests went red for it; the fixture was mended to the derived pair and `NestingCostFractionsDoNotSumToOne`
was deleted rather than kept for its message, because the sum is now implied. `cam-cli nest` writes the
record from that function and never from `NestPlan::utilisation`, which divides by every board the plan
opened — the nester's view, and the second arithmetic the rule exists to forbid.

The command is the milestone's user-visible surface: `--sheets` (repeatable, replacing the document's
own list), `--gap`, `--rotation`, `--algorithm`, `--step` and `--out`, a report on stdout, the placements
and cost written into the document, and **exit `7`** for a nesting that could not place every instance —
not `2`, because the document is valid, and not `6`, because no program was simulated: it is a finding
about the document's own request, and **nothing is written**, since `Document::validate` refuses a
placements-versus-instances disagreement and a partial nesting therefore has no honest stored form. A
flag a document's own classes give no meaning to (`--step` with no profile, `--algorithm` with no
rectangle) is refused by name, M8's rule; so are two board sizes, because both nesters pack identical
boards.

The feature document is `odd/tasks/020-m9-nesting.md`; the two nesters and their laws, the two
certificates, the inside-ness rule and its `INSET_STEP`, the one arithmetic, the CLI surface with exit
`7`, the refusals, both review lineages and the known limits are in [docs/old/m9-nesting.md](../../docs/old/m9-nesting.md).

The workspace stands at **103 test binaries** green under `cargo test --workspace`, and in the crates M9
touches that is cam-geom **302**, cam-model **93** and cam-cli **157** — 28 more in `cam-geom` than the 274
M8 left, 31 more in `cam-model` than 62, and 17 more in `cam-cli` than 140.

| Deliverable | Where | Proof |
| --- | --- | --- |
| The `nesting` section in `c2d/document@2`: sheets, parts, placements, cost and their refusals | `crates/cam-model` (`Nesting`, `check_nesting`) | 31 tests in `tests/document.rs`; a pre-M9 JSON literal loads, validates and re-saves **byte for byte** with `nesting` absent and the schema tag does not move, the section round-trips losslessly, and a non-positive sheet dimension, a duplicate sheet or part id, a part with no entity, a part naming a missing entity, a placement naming a missing part or sheet, a negative position and a footprint that leaves its sheet are each refused **naming the value**; two placements that overlap are refused while two that share an edge are accepted; a part whose instance count disagrees with its placements is refused while an authored section with **no** placements loads |
| One arithmetic for utilisation and waste, and a cost record that must equal it (S3) | `Document::nesting_utilisation`, `placement_footprint`, `NestingCost` | five 100 × 100 placements on a **500 × 400** sheet pin **50 000 / 200 000 = 0.25**, with a second authored 1000 × 1000 sheet that no placement names **excluded** from the denominator, and the T1 fixture pins **30 000 / 240 000 = 0.125**; a record that agrees loads while a wrong sheet count, a fraction outside `0..=1`, a utilisation that is not the derivation (`0.25` recorded against `0.125` derived) and a waste that is not the complement are each refused by name, and the tolerant boundary is `0.25 + 1e-6` accepted and `+2e-6` refused. The strong rule indicted T1b's own hand-invented 0.25/0.75 record and six tests went red for it |
| Rectangle nesting: maxrects, a guillotine pass, the auto rule, and one certificate | `crates/cam-geom::nest::{NestOptions, NestAlgorithm, NestPlan}` | 14 tests in `tests/nest.rs`; the cabinetry fixture (2440 × 1220, gap 8) packs **5 parts on 1 sheet, 0 unplaced, `0.57403923676431068`**, and the contested fixture (234 × 173, gap 8) needs **2 sheets with maxrects (`0.25335951781038485`) and 1 with guillotine (`0.50671903562076970`)**, both certified, with the auto rule picking each by name; the gap is a measured distance between neighbours, a part is turned only when it does not fit upright, a part larger than the board is unplaced rather than an error, equal areas keep the caller's order, the certificate holds at the sheet's own edges and fails when a plan breaks it, and two runs are the same plan |
| Free-profile nesting: a sound, incomplete no-fit heuristic (S6, S7) | `crates/cam-geom::nest_profile` | 17 tests in `tests/nest_profile.rs`; a circle and a rectangle pack on one board, two contours that would overlap never do, the authored gap is measured between two placed contours, a concave part sits in a notch, a part is turned only when no upright placement exists, an oversized part and a part that only **touches** a circular board from outside are unplaced, a flush part is placed and certified on a rectangle and on an L's arm while one `INSET_STEP` past the edge is refused, a second board opens only when the first cannot hold the part, the same input twice is the same plan, and the certificate fails when a plan breaks it; the incompleteness is the counted fixture — **step 20 costs a second board for a 50 × 50 square although `(45, 0)` is legal, and step 5 finds it at `(50, 0)`** |
| `cam-cli nest`: the flags, the refusals, the report, exit `7` and the write-back | `crates/cam-cli` | 16 tests, five of them whole-report goldens: a 600 × 400 panel on a 1000 × 600 board prints `utilisation: 0.400` / `waste: 0.600` and stores the record the document derives; two instances that no longer share a board open and author `S2`; a 1200 × 500 part that fits no board prints `sheets used: 0`, `utilisation: none`, `unplaced: P1, 1 of 1 instance(s)` and exits **7** with the file byte-identical; an L bracket goes through the no-fit heuristic and the report names it with its step (`0.200`, conservative by D-M9-2 where its own area would be `0.100`); a 500 × 900 part enters a 1000 × 600 board only turned, as 900 × 500 at `0.750`; `--out` writes elsewhere and leaves the input alone; `--step` on rectangles and `--algorithm` on profiles are refused by name with the file unchanged, as are `--gap -1`, `--sheets 1000`, `--sheets 0x600`, `--step 0`, `--rotation sideways`, `--algorithm triangle`, `--cut-depth`, two board sizes, a part that is not one closed contour, a document with no parts and one with no boards; and `--help` lists the command, its six flags and `7` |
| Determinism, and no new dependency | `tests/cli.rs`, `tests/nest.rs`, `tests/nest_profile.rs` | two `nest` runs print **and** write identical bytes on a mixed fixture that runs both nesters with its boards authored on the command line, `the_same_input_twice_is_the_same_plan` pins the rectangle packer, and the profile packer's determinism is a stable sort plus `Vec` order with nothing iterating a hash map; `git diff 2ad152d -- Cargo.toml Cargo.lock crates/*/Cargo.toml` is empty, so no crate and no dependency was added |
| The differential against the milestone's cut point (T5) | `odd/tasks/020-m9-nesting.md` L11 | **42 invocations per side** run from a `git archive 898cab6` copy built with its own target directory — two `tools add` and `tools list`, three `import` forms, `validate`/`inspect`/`check`, four `toolpath` kinds, five `emit` runs and their four programs, `simulate` at 2, 3 and 5 mm, a document carrying a `nesting` section, two `nest` runs, the error paths and `--help` — compared by exit code, stdout, stderr and **21** produced files' sha256: **39 of 42 invocations and 19 of 21 files byte-identical**, and the three differences are M9's own (`nest` did not exist — exit `2` `unknown command "nest"` against exit `0` with an 11-line report at `utilisation: 0.080`; `--help` **90 → 108 lines**, 18 added and 0 removed; the `nesting` section preserved where the base drops it on a re-save, and deleting the key makes the payloads identical). The current binary run twice gave 42/42 identical result triples and 21/21 identical hashes, so the comparison is not measuring drift |

Recorded deviations, each a decision in the feature document: **D-M9-2** makes a placement's footprint
the part's turned axis-aligned bounding box, so load-time containment and overlap are conservative for a
free profile — measured `0.200` where the bracket's own area would be `0.100` — and the exact shape is
the nester's business; **D-M9-7** measures the gap between parts only, never insetting a sheet's own
edges, so a full-sheet part sits flush and is certified; **D-M9-8** makes the quarter turn a fallback and
never a tie-break, which is why the cabinetry plan is all `None`; **D-M9-10** deletes the fractions' sum
rule rather than keeping a `pub` variant nothing can produce, because the sum is implied by comparing
each fraction with the derivation; the **free-profile heuristic is incomplete by measurement** and its
strict disjointness costs one search step beyond the gap; there is **no exact NFP** because `Region` has
no union; a run packs **one board size**, and a **mixed document nests each class on boards of its own**,
because neither nester can be handed the other's placements as an occupied board. Two recorded gaps are
not deviations but omissions owned by this document: **no shell drafting form** (S7's assumption, carried
from M4 and M7 — parts and sheets are authored through `cam-cli nest`), and **`docs/old/format.md` carries no
row for the `nesting` section**, which T6's edit surfaces did not include.

**Both of M9's candidates were reviewed, at tier high, and both receipts are burned; the document-format
commit was not.** `review-1ab521437bba8607` covered `a5a82fe..df69091` — 2 801 lines, three lenses,
correction budget 200 — after the **full-range** candidate was refused in preflight with
`lens_context_budget_exceeded` (`mutation_outcome: not_started`, no authority created, nothing abandoned),
the same wall M7 hit; the reduced range was accepted, which is the fact worth keeping: the budget is a
property of the candidate's size and the base ref is the lever. Its refuter returned
`correction_required` with three CRITICAL findings that were **one** defect — inside-ness accepting
`Touching`, with `certifies` repeating it, so a plan could certify a part outside a non-rectangular board
— fixed in `fc73d18` with **161** of the 200 authorised lines, three REDs observed before any `src/` edit,
no existing test changed, and a differential of the 11 pre-existing profile tests against `df69091`
(11 passed both sides). It also produced four advisory findings tagged by lineage
(`1ab52143/R1-STEP-DOS`, `R2-002`, `R2-003`, `R3-002`) and named a gap now closed: every new placement
test asserted a refusal, so the fix's **accept** branch had no positive case — `91794aa` pins a flush part
placed and certified on a rectangle and on an L's arm and one inset step past the edge refused.
`review-33ff9cdba8c8746d` covered `bc22942..53934f1` — 375 lines, lenses risk + reliability, budget 188
of which **12** were used — and its one finding, `R3-001`, was a **BLOCKER whose premise was false and
whose substance was true**: it held that two 600 × 400 instances cannot share a 1000 × 600 sheet so
`nest` must exit `7`, when in fact `nest` opens a second sheet — invisible to the reviewer because
`main.rs` lay outside the candidate's paths and only the tests were in the diff. The correction
(`a57ae9b`, 12 lines) makes the fixture fit one sheet and adds `assert_eq!(after_first.sheets.len(), 1)`,
the assertion that would have caught the ambiguity. The honest gap is `a5a82fe`: the document format, its
refusals and the one utilisation sit at the **base** of the first candidate, which git excludes, and the
full-range candidate that would have included them was the refused one — once HEAD moved past it that
range could not be re-cut. `bc22942`, the `nest` command, lies between the two ranges and is covered by
neither. M9's document contract therefore rests on its 31 model tests, the pre-M9 byte-identical proof
and the differential's finding that the section survives a re-save by a pre-existing command, and the
command on its 16 CLI tests; that is weaker evidence than a lineage on the *format*, and it is recorded
rather than softened — the differential above is what says the rest of the surface did not move.

