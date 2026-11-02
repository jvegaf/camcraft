# raster-far-edge-pass — the raster lattice reaches the far edge, and the M4 contract says so

The raster strategy stops one stepover short of the region's far edge whenever the stepover does not
divide the region's height, and the strip it leaves is the strip the tool's own disc could have cut.
D-M5-12 recorded that and left it to the owner because finishing the edge changes M4's documented
raster contract. The owner has now taken that decision.

## Specs

- **S1 — the decision.** The owner's words, in order: *"si, acepto"*, answering the parent's proposal
  verbatim: *"D-M5-12 (raster de simulación, 984 mm³ sin cortar) … necesita tu decisión porque cambia
  el contrato de M4"*. The requirement itself is the record it accepted, from
  `odd/tasks/m5-simulation.md:178-188`, verbatim: *"The fix is a boundary pass — one row at the
  offset region's far edge — and it is deliberately **not** taken here: M4's raster contract says
  'rows on the `min.y + k·stepover` grid' and pins '32 rows at y = 3, 6, … 96', so adding a finishing
  row changes a documented contract and belongs to a decision the owner takes"*. That decision is
  taken: **the contract changes**.
- **S2 — the rule.** When the grid `bounds.min.y + k·stepover` stops short of the region's far edge
  `bounds.max.y`, the raster emits **one additional row at exactly `bounds.max.y`**, after the last
  grid row. When a grid row already lands exactly on `bounds.max.y` — the strict `y > bounds.max.y`
  break makes that row real today — **no additional row is emitted**. One rule, no shape special cases.
- **S3 — what must not change.** A row that lands *on* a boundary keeps being retried by `row_spans`
  on both sides (`ROW_RETRY_NUDGE`), so the near edge and an already-reached far edge are untouched;
  island clipping and even-odd pairing, the zigzag direction, span order, the offset and profile
  strategies, and every non-raster emitted program stay byte-identical. The raster programs whose
  region's far edge is already a grid row — the island raster golden and the clean raster report — are
  byte-identical too, which is what says the change lands only where the lattice fell short.
- **S4 — the tangent row is recorded, not special-cased.** A region whose far edge is a single point
  (a circular pocket) gains a tangent row that cuts a chord of ~0.037 mm at radius 17. It is emitted
  because the rule has no shape predicate; making the rule ask whether the far edge is flat would trade
  one honest row for a shape special case. Recorded with its measurement.
- **S5 — the documents follow the behaviour.** `docs/m4-toolpaths.md` (the "last grid line that
  fits" sentence, and the table row that says 99 mm is past 97 so there is no 33rd row),
  `odd/tasks/m4-toolpaths.md` A5 (superseded by an appended entry, never rewritten),
  `odd/roadmap.md`'s raster row, and D-M5-12 itself (recorded as decided and delivered). A document
  that still promises the old lattice is a defect of this task, not a follow-up.
- **S6 — risk.** Task Size item 3: this changes the G-code that an existing subcommand emits for a
  documented contract. Verification is the full gate plus a differential against the previous code for
  every command the request did not name, and the native review takes the work unit alone.

## Tasks

- T1 — RED: two pins, one for each half of S2 (a stepover that stops short gets the far-edge row; a far
  edge already on the grid gets no duplicate). Route: parent inline. Commit: `e9c003a`.
- T2 — the rule in `raster_rows` (S2) and every pin it legitimately moves, re-derived not fitted (S3, S4).
  Route: parent inline. Commit: `e9c003a`.
- T3 — the observable effect (S2): the strip the grid stopped short of is cut, pinned cell by cell on
  the 1 mm lattice rather than through `simulate`'s volume, because that fixture's `uncut` also carries
  D-M5-11's mitered corners and mixing the two would make the number unfalsifiable. Route: parent inline.
  Commit: `e9c003a`.
- T4 — differential over the commands the request did not name (S3, S6). **Dropped, on purpose**, and not
  to be read as done: the box cannot compile two trees, so the base-vs-new binary differential was never
  measured. What replaces it is a scope proof plus the repository's own goldens — see L5.
- T5 — the documents (S5). Route: parent inline. Commit: the commit that carries this line.
- T6 — the gate, the two commits, and the native review (S6). Route: parent. See L4 and L6.

## Log

- **L1 (the request, verbatim).** *"que queda pendiente"* → *"me pareve bien tu recomendacion"* → the
  parent's offer *"D-M5-12 (raster de simulación, 984 mm³ sin cortar) … necesita tu decisión porque
  cambia el contrato de M4"* → *"si, acepto"*.
- **L2 (the reconnaissance, before any write).** The far edge is already in scope at `pocket.rs:300` as
  `bounds.max.y` over `region_bounds(&offset.region)` — the region the ring path cuts, boundary plus
  islands. The loop breaks on `y > bounds.max.y` (`pocket.rs:305-307`), so a row *on* the far edge is
  already emitted and retried today: 1…99 at a 2 mm stepover, 3…57 at 9 mm, and the inlay male at 97.
  Every one of those would double under an unconditional append. No golden file exists anywhere — every
  golden is an inline `const &str` in `crates/cam-cli/tests/cli.rs` — and `984.000 mm3 over 164 cell(s)`
  is asserted by no test, only written in prose.
- **L3 (the tests to move, and the ones that must not).** Move: `raster_square_cuts_rows_at_stepover_spacing`
  (32 rows at 3+3k, region 3…97), `circle_raster_spans_match_the_analytic_chord` (9 rows, 33…65, region
  33…67), and the three island raster pins whose regions fall short (32, 24, 14 rows). Must not move:
  `a_raster_row_on_the_region_far_edge_is_cut` (50 rows, region 1…99), the island raster golden, and
  `CLEAN_RASTER_REPORT` — these are the no-duplicate evidence, so they are the ones T2 has to keep green.
- **L4 (the gate: two real failures and one the parent inflicted on itself).** `./scripts/check.sh` went
  red three times before green, and the first attempt reported exit 0 only because a `| tail` swallowed
  the status of an `ld` abort — the pipe is not a result. The first failure was real: the new test lines
  exceeded 100 columns and `rustfmt` refused them. The second was real too: `clippy -D warnings` caught a
  manual index loop (`for index in 0..32 { units[index] }`) where the file already writes
  `units.iter().take(32).enumerate()` two tests above. The third was self-inflicted: `CARGO_BUILD_JOBS=2`
  put two `rust-lld` processes against the 64-task cgroup and the linker died spawning a thread, which is
  the same cap that killed the first attempt of this session's other feature — the gate passes with
  `CARGO_BUILD_JOBS=1` and `RUSTFLAGS="-C link-arg=-Wl,--threads=1"`, which is how it had already passed
  twice. Final: **exit 0, 96 suites, 1 177 passed, 0 failed** — the previous 1 176 plus exactly the one
  new test, with the two inline CLI goldens running unchanged inside that count. The Python-era lesson
  repeats: a check whose exit code you did not look at is not a check.
- **L5 (the differential that could not run, and what stands in for it).** A delegated verifier could not
  build the base commit at all: `rustc` dies before codegen with `failed to spawn work thread: Os { code:
  11, kind: WouldBlock }` even with the linker thread cap, `CARGO_BUILD_JOBS=2` and the base tree built
  alone, so the ten-command-family binary differential was **never measured** and the task is dropped
  rather than reported as done. What it did establish: the `cam-toolpath` suite green by executing the
  post-change test images already in `target/` (16 binaries, 140 tests, 0 failed), and a structural scope
  proof — `raster_rows` has exactly one call site (`pocket.rs:126`, inside the `Raster` arm) and `emit_row`
  is called only from `raster_rows`, so the `Offset` arm and `cam-cli` cannot change through the raster
  path. Its two draft probes were authored from the spec and never executed (its circular case ends in the
  word "adjust"), so they were **not** committed: a test nobody ran is a claim, not evidence. The stand-in
  is the repository's own byte-exact goldens, which the gate does run: `ISLAND_RASTER_PROGRAM` and
  `CLEAN_RASTER_REPORT` are unchanged because both regions' far edge is already a grid row (3 + 6·9 = 57
  and 1 + 49·2 = 99). That is a substitute, and it is recorded as one.
- **L6 (the delegate that corrected the parent).** The docs writer re-derived the tangent chord from
  `ROW_RETRY_NUDGE = 1.0e-5` and got ≈0.037 mm, and said so: the parent's handoff had put ≈0.0117 mm,
  which is what a `1.0e-6` nudge would give and is not this code. The parent accepted the correction and
  rewrote its own brief's number out of the record. Everything written into `docs/m4-toolpaths.md`
  (contract sentence, the 33-row table with row 33 on the far edge, the 50-row unchanged case, the chord)
  and into `odd/roadmap.md` (33/41, 25/25, 15/25) was re-derived from the tests, not copied from the
  brief; the two period records got appended entries only, with the owner's `si, acepto` verbatim, and
  `docs/m5-simulation.md` was checked and left alone because its 1…99 region already ends on the grid.
  Two things the writer left deliberately and flagged instead of silently fixing: roadmap §4.7's cell that
  still says `tests/pocket.rs` needed no edit (true of an earlier task, odd in a current-state table) and
  §4.6's period narrative about the old 32 rows. Both are record, not contract, and stay as evidence.
