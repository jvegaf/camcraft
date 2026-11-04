# M9 — Nesting: sheets, parts and cost

Feature document for M9 of `odd/roadmap.md`. Read **until `## Log`** before touching code; the Log is
the record, not the brief.

## Specs

- **S1 (user, verbatim answer to "¿Qué es una pieza: una instancia en un documento maestro, o un
  documento?"):** "Un documento maestro con N hojas y N instancias".
- **S2 (user, verbatim answer to "¿Dónde vive el resultado del nesting?"):** "Guardar los placements en
  el documento".
- **S3 (user, verbatim answer to "¿La utilización y el desperdicio son reporte de salida o dato del
  documento?"):** "Los dos: reporte y campo".
- **S4 (user, verbatim answer to "¿Las hojas (sheets) persisten en una biblioteca o viven en el
  documento?"):** "No: las hojas se autoran y viven en el documento".
- **S5 (user, verbatim, roadmap D4):** "Implementar nesting heurístico propio en v1".
- **S6 (roadmap M9 row, not a user string):** "Sheets + parts model, rectangle maxrects+guillotine,
  free-profile NFP, cost/summary report — Own heuristic engine as decided (D4); depends on stable
  geometry and toolpaths".
- **S7 (roadmap risk register, not a user string):** "Ship rectangle nesting first (covers cabinetry
  parts), free profiles second."

## Invariants every task must honour

These are the repo's own conventions, not new requirements; a task that breaks one is refused:

1. **No schema bump.** `DOCUMENT_SCHEMA` stays `c2d/document@2` (`crates/cam-model/src/lib.rs:26`).
   The nesting section is an **optional top-level** field with `#[serde(default, skip_serializing_if =
   "Option::is_none")]`, the `tools_ref`/`postp` precedent (`lib.rs:896-903`): a document written
   before M9 loads and saves **byte-identically**.
2. **Nothing is derived twice.** If a quantity can be computed (utilisation, waste), compute it in one
   function and have the report and the stored field read that function — never two arithmetics that
   can disagree.
3. **Refusals name the value.** A non-positive sheet dimension, a part that names no entity, a part
   larger than every sheet: each is refused at load with its own message and its own value, never a
   default and never silence (M8's rule, `docs/m8-moulding.md`).
4. **Determinism is a test, not a hope.** Every command that prints or writes runs twice and both runs
   are compared byte for byte.
5. **No new dependency** unless a task proves it necessary and records the decision. `Cargo.lock` is a
   reviewable surface.
6. **Every new command, flag or message updates the `const HELP` text** (`crates/cam-cli/src/main.rs:27`)
   and the docs that describe it. A test asserts the help mentions the flags, as `tests/cli.rs:552` does.
7. **Environment:** every cargo invocation needs `RUSTUP_HOME=/home/th3g3ntl3man/.rustup
   CARGO_HOME=/home/th3g3ntl3man/.cargo`; a commit needs `-c user.name="Jose Vega" -c
   user.email="josevega234@gmail.com"`. `gentle-ai` is not on `PATH`
   (`/home/th3g3ntl3man/.local/bin/gentle-ai`). Workers never commit.

## Tasks

| ID | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | The `nesting` section in `c2d/document@2`: sheets, parts, placements and the cost record, all optional, byte-identical when absent, refusals at load | S1 S2 S3 S4 S5 | worker | — |
| T1b | The cost record's own refusals: `sheets_used` must agree with the sheets the placements name, and utilisation and waste must be finite fractions of the used area that sum to one | S3 | worker | — |
| T1c | One arithmetic for the fractions: `Document` derives utilisation and waste from its own placements and sheets, the stored record must equal what that function says, and the CLI's report in T4 reads the same function instead of computing its own | S3 | worker | — |
| T2 | Rectangle nesting in `cam-geom`: maxrects plus a guillotine pass, deterministic order, 90° rotation where the sheet allows it, the authored gap, and a placement certificate (no overlap, inside the sheet) | S5 S6 S7 | worker | — |
| T2b | Make the two algorithms S6 names reachable and pinned: `GeomError` carries the module's refusals instead of a second error enum, and guillotine has a fixture it actually wins | S6 | worker | — |
| T3 | Free-profile nesting: the NFP heuristic for contours that are not rectangles, reusing `Region` and the offset kernel rather than a second intersector | S6 S7 | worker | — |
| T4 | `cam-cli nest`: the flags, the refusals, the report on stdout (utilisation, waste, sheets used, exits non-zero when a part does not fit) and the placements plus cost written into the document — the record is written **from `Document::nesting_utilisation`**, never from `cam_geom::nest::NestPlan::utilisation`, which is the nester's own view and would reopen the trap T1c closed | S2 S3 S6 | worker | — |
| T5 | Goldens for the report and for the authored document, determinism, and a differential against the milestone's cut point over the pre-existing command surface | S3 S6 | worker | — |
| T6 | Docs: `docs/m9-nesting.md`, `odd/roadmap.md` §4.12, README, and this document's evidence | S1-S7 | parent | — |

Order is the dependency order; each task is one work unit and one commit, and a unit that lands a
high-risk item gets its own commit so native review can take it alone.

## Log

- **L1 (user, verbatim):** "1. cierralos en commit unico. 2. vamos" — where "1." is the three
  readability advisories of `review-2c344585` and "2." is M9, the milestone the roadmap named as next.
- **L2 (user decisions, verbatim option labels):** "Guardar los placements en el documento"; "Los dos:
  reporte y campo"; "No: las hojas se autoran y viven en el documento"; "Un documento maestro con N
  hojas y N instancias". Recorded as S1-S4 above.
- **L3 (exploration, delegated and read-only):** no sheet library, no parts list and no instance
  concept exist today — one `Job` per document (`crates/cam-model/src/lib.rs:128-142`), whose fields
  carry no `serde(default)`, so **adding a field to `Job` would not be backwards compatible**; the
  optional-top-level-section precedent is `tools_ref`/`postp`; `cam-geom` is the lowest crate that can
  host a nester (it depends on `cam-units` only) and `cam-toolpath` the next level; `toolpath` is the
  closest structural precedent for the command (`main.rs:204`, write-back at `main.rs:1858-1864`);
  goldens are inline consts in `crates/cam-cli/tests/cli.rs` and the determinism pattern is "run
  twice, compare bytes" (`tests/cli.rs:344`).
- **L4 (T1 delivered, delegated writer).** The optional top-level `nesting` section landed with
  `Nesting`, `NestingSheet`, `NestingPart`, `NestingPlacement`, `NestingCost` and nine `ModelError`
  variants, `Document::validate` refusing each bad value by name, and 16 new tests. Test-first held: 13
  refusal tests failed first at the `refusal` helper, then a **real defect the tests caught** — a quarter
  turn about the part's own origin put the footprint at negative X, so the anchor moved to the placed
  footprint's corner. `cargo test -p cam-model` 62 → 78 passed, `cam-store` 25 passed, clippy clean at
  `-D warnings`, `Cargo.lock` untouched. The byte-identical test is the one that protects every document
  written before M9: a pre-M9 JSON literal loads, validates and re-saves byte for byte with `nesting`
  absent. Decisions the brief left open, now recorded: **D-M9-1** a placement's `position` is the sheet
  point its **placed** footprint corner occupies and `rotation` turns the part before anchoring (the
  negative-X defect is why); **D-M9-2** a placement's footprint is the part's turned axis-aligned
  bounding box, so load-time validation is conservative for a free-profile part and the exact shape is
  the nester's business (T3 may revisit); **D-M9-3** overlap reuses `cam_geom::winding::relation` rather
  than a second intersector — Crossing and containment overlap, Touching does not, except equal bounds
  (two copies on one spot), which is why that one case is refused explicitly; **D-M9-4** duplicate
  sheet and part ids are refused through `DuplicateId`, one rule beyond the brief because a lookup by id
  must be unambiguous; **D-M9-5** `cost` is absent until a nesting run writes it and carries
  utilisation/waste as fractions of the used area summing to one; **D-M9-6** a part with `instances: 0`
  is accepted (nothing to nest is not an error) and a zero-area selection is skipped by the inside-sheet
  and overlap rules. Two gaps the writer declared and this document now owns instead of hiding: the
  stored **cost record is not validated** (T1b), and "a part larger than every sheet" is the nester's
  refusal, not the loader's (T2/T4).
- **L5 (T2 delivered, delegated writer).** `cam_geom::nest` landed with `NestOptions`, `NestRotation`,
  `NestPlacement`, `NestPlan` (ten public methods), the `certifies` predicate as the module's own
  guarantee, and one arithmetic for utilisation. Test-first held: RED #1 was an unresolved import for all
  ten tests, RED #2 the stubbed packer (nine of ten, each at its own assertion), and one **real** RED
  survived GREEN #1 — `the_certificate_fails_when_a_plan_breaks_it` guessed the shrink (500 mm) instead of
  deriving it from the plan's own furthest footprint, which is the difference between testing the
  certificate and testing a constant. `cargo test -p cam-geom` 274 → **284 passed**, clippy clean at `-D
  warnings`, `Cargo.lock` untouched, and the only existing line touched is `pub mod nest;`. The
  cabinetry fixture (2440 x 1220, gap 8) places **5 parts on 1 sheet, 0 unplaced, utilisation
  `0.574039237`** — 1 708 800 of 2 976 800 mm² — and the two-sheet fixture gives **2 sheets, 8 placed, 0
  unplaced**, identical on both runs. Decisions recorded: **D-M9-7** the gap is measured between parts
  and never insets the sheet's own edges, so a full-sheet part sits flush (documented and pinned by
  `the_certificate_holds_at_the_sheets_own_edges`); **D-M9-8** rotation is a **fallback**, used only when
  the part does not fit upright, which is why the cabinetry plan is all `None`. Two gaps this document
  owns rather than hides: the module introduced its own `NestError` where the crate's convention is one
  `GeomError` per crate (**T2b**), and the maxrects-versus-guillotine rule — fewer unplaced, then fewer
  sheets, then maxrects — has **no fixture where guillotine wins**, so half of what S6 names is
  unexercised (**T2b** too).
- **L6 (T1b delivered, delegated writer).** The stored cost record is now validated, so a record that contradicts its own placements cannot load. Three `ModelError` variants — `NestingCostSheetCountMismatch`, `NestingCostFractionOutOfRange`, `NestingCostFractionsDoNotSumToOne` — evaluated **last** in `check_nesting`, after the instance-count rule, so they only ever see placements already known good. The tolerance is `cam_geom::DEFAULT_TOLERANCE` (1e-6) reused through one private const, and the writer's reason for rejecting the crate's other constant is the one worth keeping: `DEFAULT_CHORD_TOLERANCE` is a tessellation chord, not a comparison epsilon. The third message names **four** numbers — recorded utilisation, recorded waste, implied waste and the tolerance — because a refusal that does not say what it expected makes the caller guess. The floating-point boundary was chosen honestly and stated in the test: `0.5 / 0.5+1e-6` sits at `1.0000000000000002e-6`, above the tolerance, so the accepted side is `0.75 / 0.25+1e-6` (sum exactly `1.000001`). Six new tests; `cargo test -p cam-model` 78 → **84 passed**, `cam-store` 25, clippy clean, `Cargo.lock` unchanged, and the diff to `tests/document.rs` is additions only, so T1's 16 nesting tests and the pre-M9 byte-identical test were not touched. Gap the writer declared and this document owns instead of hiding: rule 3 checks the record's **internal consistency only**, so a record that is consistent but wrong about the area still loads — **T1c**.
- **L7 (T2b delivered, delegated writer).** `NestError` is gone: the three refusals moved into `GeomError` with their names, values and messages byte-unchanged (`GapNotMeasurable`, `SheetNotMeasurable`, `PartNotMeasurable`), and the RED was the fold itself — seven `E0599`s until the variants existed. Both heuristics S6 names are now reachable by name: `NestAlgorithm { Auto, MaxRects, Guillotine }` on `NestOptions`, `NestPlan::algorithm()` naming what actually produced the plan (never `Auto`), and `Auto` keeping today's order — fewer unplaced, then fewer sheets, tie keeps maxrects — so nothing existing moved. **Guillotine has a fixture it wins**: a 234 x 173 mm sheet at gap 8 with parts `110x60`, `111x87`, `38x112` and rotation off gives maxrects **2 sheets** (utilisation `0.25335951781038485`) against guillotine **1 sheet** (`0.50671903562076970`), both certified, with the pinned origins. The reason is the algorithm and not the tie-break: maxrects splits around the 111 x 87 part into a 123 mm right strip and an 86 mm top strip, so `38x112` no longer finds 112 mm of height, while the guillotine's full-length cut leaves a 242 x 86 strip above the first part for `110x60` and the full-height right strip for `38x112`. The auto rule now demonstrably picks guillotine here and maxrects on the cabinetry fixture, by name. Four new tests; `cargo test -p cam-geom` 284 → **288 passed**; clippy clean; `Cargo.lock` untouched; the L5 cabinetry numbers are unchanged (`0.57403923676431068`, 1 sheet, 5 placed) against pre-change plan dumps for six fixtures. Decision recorded: **D-M9-9** `NestPlan` equality includes the algorithm, so two plans with identical placements from different heuristics are unequal — deliberate, but a caller comparing plans has to know it.
- **L8 (T1c delivered, and the fixture it indicted).** The fractions now have **one** arithmetic: `Document::nesting_utilisation` — placed footprint area over the area of the sheets **the placements name** (never every sheet in the section), waste its complement, `None` when the document cannot say. Rule 3 was replaced by the strong form: both stored fractions are compared with that derivation, and `NestingCostFractionsDoNotSumToOne` was **deleted**, because the sum is now implied by comparing each fraction with the derivation and its complement. The arithmetic is pinned with exact numbers, including the denominator trap: 5 placements of a 100 x 100 part over a 500 x 400 sheet is 50 000 / 200 000 = **0.25**, with a second 1000 x 1000 sheet authored but named by no placement deliberately **excluded** from the denominator; the T1 fixture pins 30 000 / 240 000 = **0.125**. Then the strong rule did what it was for: it indicted **T1b's own fixture**, which recorded `0.25` / `0.75` — sums to one, wrong about the area — and six tests went red for that one reason. The parent's call was that the fixture is wrong and the rule is right, so `nesting_document()` now carries the **derived** pair with a comment citing `Document::nesting_utilisation` and the two areas, and says why: a hand-invented pair that sums to one is exactly the record the rule refuses. Two T1b rule-3 tests were re-pinned rather than merged into the new ones — the waste mismatch now names `waste`, `0.5` and `0.875`, and the tolerance boundary **moved to the 0.25-derivation fixture**, because on 0.125 the distance at `+1e-6` computes `1.000000000001e-06` and the inclusive check refuses that side: a boundary test seated on the wrong fixture tests the wrong side. One T1c test had been taking its invented record **from the fixture**, so mending the fixture left it unable to refuse anything; it now writes `0.25`/`0.75` explicitly instead of depending on an accident, with no assertion weakened. `cargo test -p cam-model` 84 → **90 passed, 0 failed**; `cam-store` 25; clippy clean; `Cargo.lock` unchanged. Decision recorded: **D-M9-10** the sum rule is implied rather than checked, and a `pub` error variant nothing can produce is dead contract, so it was deleted rather than kept for its message. Carried forward, and now written into T4's own row: `cam_geom::nest::NestPlan::utilisation` is the **nester's** view, so the CLI writes the record from `Document::nesting_utilisation` or two arithmetics reopen the trap from the other side.
