# Debt: the fourteen open review advisories

Feature document for the advisory debt recorded in `odd/tasks/m10-documents.md` L9 and in Engram
observation #165. Read **until `## Log`** before touching code; the Log is the record, not the brief.

## Specs

- **S1 (user, verbatim answer to "¿Por dónde arranco la deuda?"):** "Los 14 advisories".
- **S2 (user, verbatim answer to the store question):** "Sí, solo lectura" — read-only access to the
  review authority store under `.git/gentle-ai` for the findings' text; nothing there is written or
  staged.
- **S3 (consequence of S1, not a user string):** an advisory can only be *closed as resolved* when its
  finding's subject is established. A finding whose reviewer text is unrecoverable and whose anchor no
  longer identifies its subject is **not** closed by a plausible-looking edit: it stays open, named as
  unrecoverable, and is re-filed by a review of the current tree if the owner wants it back.
- **S5 (user, verbatim option label chosen for "¿Qué forma toma el piso de `--step`?"):** "Cap de 4M
  anclas (idioma de M5)". The floor is not a millimetre minimum: it is the lattice's anchor count
  measured against the cap the simulator and the medial axis already use.
- **S4 (repo law that binds every task here):** a fix changes the diagnostic or the wording it was
  filed against, never the acceptance of valid input. The one exception is refused below rather than
  smuggled in (T5's step floor is the owner's decision, not a readability fix).

## Tasks

| ID | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | The quoted-token `end_column` off-by-one: a missing argument after a quoted token reports the column *inside* the token instead of one past it | S1 S3 | parent | `68adb00` |
| T2 | The CSV quote check runs before the field-count check, so a quoted field containing a comma is refused as a wrong field count instead of the promised quoted-field refusal | S1 S4 | parent | `106ab8b` |
| T3 | `const HELP`'s scripted `toolpath` and `nest` lines claim to take "every other" flag while the script refuses `--out` and a positional document | S1 S4 | parent | `68adb00` |
| T4 | The `script_panel` test fixture joins a caller-supplied name onto `/var/tmp` and `remove_dir_all`s the result, so a name with a separator escapes the scratch directory | S1 S3 | parent | `3d6589f` |
| T5 | The search step's lattice: a step whose anchor count over the board exceeds 4 000 000 is refused with the step, the columns, the rows and the cap, in the simulator's cap dialect | S5 | parent | `c6eb633` + `b06c004` |

## Log

- **L1 (user, verbatim):** "si, la deuda".
- **L2 (store reconnaissance, delegated; the findings' provenance).** The review authority store at
  `.git/gentle-ai` holds **three** complete lineages (`review-61e0b405dc7f498b`,
  `review-305eaa8746f90d29`, `review-3592c554bf15c02f`) and only *terminal-consumption* records for
  `bd99706d`, `12219c3f` and `1ab52143` — id, lineage and three hashes, no findings. So **10 of the 14
  advisories exist only as metadata** (lens, severity, path:line) in `docs/m10-documents.md:355-370`,
  `odd/tasks/m10-documents.md:160`, `docs/m9-nesting.md:392-396`, `odd/roadmap.md:813,899`.
  The reviewer's own words survive for the four below, through `review-61e0b405dc7f498b`, documented as
  `bd99706d`'s superseded predecessor over the same code with shifted lines:
  1. *readability WARNING* — "`end_column` is computed as `token.column + token.text.len()`, but for a
     quoted token `Token::column` points at the opening quote while `text` excludes the quotes… reports
     a column one short". Verified live: `crates/cam-cli/src/script.rs:491-493`, lexer sets
     `column: start + 1` at the `"` (`script.rs:452-486`). **T1.**
  2. *readability SUGGESTION* — "The new script help says `toolpath` takes 'every other toolpath flag',
     but the scripted command deliberately refuses `--out` (and a positional document path) with
     `NO_OUT`/`NO_POSITIONAL`. The help therefore misstates the accepted options". Verified live:
     `crates/cam-cli/src/lib.rs:172`; `NO_OUT`/`NO_POSITIONAL` at `script.rs:42,45`; `nest`'s help line
     carries the identical false claim. **T3.**
  3. *reliability WARNING* — "The field-count check runs before the quote check, so a quoted field
     containing a comma, a common spreadsheet export, is refused as a field-count error instead of the
     promised quoted-field refusal". Verified live: `crates/cam-import/src/csv.rs:175-183` (count)
     precedes `:185-196` (quote scan). **T2.**
  4. *risk WARNING* — `crates/cam-ui/tests/script_panel.rs:29-31`, the `scratch` fixture: a hard-coded
     `/var/tmp` joined with a caller-supplied `name`, then `remove_dir_all`. Text unrecovered; the
     subject is the code's own shape, so T4 fixes the escape and does **not** claim the finding by name.
- **L3 (why the other ten stay open).** `bd99706d/R2-001`, `R2-003`, `R2-005`, `R2-006`, `R3-002`,
  `R3-003`, `12219c3f/R1-001` (identity, once T4's subject is read off the code), `1ab52143/R1-STEP-DOS`,
  `R2-002`, `R2-003`, `R3-002`: without the reviewer's text their authors' subject cannot be
  established, and three anchors are worse than stale — `crates/cam-cli/src/main.rs` was split by
  `6cc3b2d`, so `main.rs:162-163`, `:860` and `:3003` name a file that is now 16 lines long. Two M9
  anchors (`cam-geom/src/nest.rs:348`, `nest_profile.rs:241-246`) land on **doc comments**, so the
  finding is a doc-versus-behaviour claim nobody can now read. Guessing a fix for those would manufacture
  a resolution the record cannot support (S3).
- **L4 (deferred to the owner, not fixed here).** The one M9 advisory whose subject *is* inferable from
  the code: `crates/cam-geom/src/nest_profile.rs:171-175` refuses a step that is not finite or not
  positive, and sets no floor, so `--step 0.001` pays an unbounded lattice (`lattice_steps` at `:537`,
  used at `:521-522`). A floor **refuses input that used to be accepted**, which is a contract change —
  the owner's call, not a readability fix (S4).
- **L5 (T1-T4, evidence).** **T1**: `lex` now records `end_column` as each token is pushed — the quoted branch adds its two quotes — instead of deriving it from the last token's `column + text.len()`. RED first: `sheet "100 200"` reported `line 2:14` (inside the token) where the law says `line 2:16`; after the fix `crates/cam-cli/tests/script.rs` is **21 passed**, including the pre-existing unquoted case (`sheet 100` → `line 2:10`), so the behaviour the new test does not cover is pinned and unchanged. **T2**: the quote scan moved ahead of the field-count check, with a line-level fallback when the quote lands past the last column the format has (`COLUMNS[index]` would otherwise index out of bounds on the row it is refusing). RED first: `"post, left",120,80,4` answered `expected 4 fields (found 5)` where the format's own message is the quoting refusal; after, `crates/cam-import` is **30 passed**, and the two quoting tests that predate the change (`"bracket"`, `brack"et`) still print their original message. **T3**: `const HELP` now says what the scripted `toolpath` and `nest` refuse and why, and `the_script_help_says_which_destinations_a_scripted_command_refuses` pins both claims against a whitespace-collapsed help, so the wording is asserted without coupling the test to `HELP`'s indentation. **T4**: `scratch` refuses a name that is not one path segment (`/`, `\\` or `..`), so the fixture's own `remove_dir_all` cannot be aimed outside its directory; `cam-ui`'s `script_panel` is **6 passed**.
- **L6 (the differential, because `split` is shared code).** `cam-cli parts --csv` is a command this request never named and it reads part lists through the same `split` a scripted `parts` uses, so the previous code (`077f48e`, built from a scratch worktree) and this change both ran it over five fixtures, comparing stdout, stderr, exit code and the document each run wrote. `plain` (part authored, exit 0), `count` (3 fields, exit 2) and `quoted` (a quoted field with no separator, exit 2, field-level message) are **byte-identical**. `quoted-comma` and `past-columns` differ only in the message the fix names — the count error became the quoting refusal, exit code `2` in both. `VERDICT=no regression in a command the request did not name`. The harness lives in `/var/tmp/campilot-differential.sh`, outside the repository, and is not committed.
- **L7 (the gate, and why nothing is committed).** `./scripts/check.sh` ran `fmt --check`,
  `clippy -D warnings` and the whole suite: **all gates green, `CHECK_EXIT=0`**. `cargo test --workspace
  -- --list` counts **1511 tests over 110 result lines**, up from M10's floor of 1508/110 — exactly the
  three tests this change adds (T1's quoted column, T2's quoting refusal, T3's help claim). The four
  fixes and this document sit **uncommitted** in the working tree: no commit was authorized, and the
  record of which advisories are closable belongs with them. `assess` could not measure the candidate —
  `nativeRisk: unassessable`, `changedPaths: 0` — because this document is untracked and native review
  demands an explicit untracked declaration, which is the trap the M10 record's L9 already names. Its
  plan is the writer's self-verification, which is the gate above: no independent verifier was required
  and none ran.

- **L8 (committed, owner-authorised).** Three work-unit commits, tests alongside the behaviour:
  **`68adb00`** (T1 + T3, the cam-cli script: the quoted-token column and the help's claim),
  **`106ab8b`** (T2, the CSV quoting refusal) and **`3d6589f`** (T4, the fixture's path segment), with
  this document in `docs(odd):` on top of them. Local only — **not pushed**, and pushing stays the
  owner's decision. L7's "uncommitted" describes the moment it was written, not the tree now.
- **L9 (the review, and the same blindness a second time).** The candidate — all four commits, 7 paths,
  212 changed lines — was reviewed as lineage **`review-b5fe4921af681292`** at tier `medium`, with the
  provider's own lens selection: **`review-reliability`**, one host-relay model run, with the cost
  forecast given before it ran. Closure was **`approved`** on the first admitted event with **no
  correction**, and the receipt was burned (`gentle-ai.review-acknowledged/v1`, consumed revision
  `sha256:c6f0aa64ca3876e06c1900d5d6ddabec1464b810db221840647e053376679d0e`). Its one finding is
  **`R3-001`** — reliability, WARNING, *informational* — at `crates/cam-import/src/csv.rs:180-193`,
  which is inside the very block T2 added: this fix has a successor advisory of its own. **And that
  finding's wording is already gone.** The lineage never gets a state directory —
  `review-transactions/v2/` holds only `review-61e0b405dc7f498b`, `review-305eaa8746f90d29` and
  `review-3592c554bf15c02f` — and its only record is
  `review-transactions/terminal-consumption/v1/67cbe40925a63e44cce8de98e72de753881bcb2f6ebbd4050975c962ff6e5a07.json`,
  which carries the schema, the repository, the target and the lineage and nothing else. So the
  blindness L2 found in M9's and M10's receipts is not an old accident: it is what the store does to
  **every** receipt, including one burned minutes ago. The consequence for method: a finding's words
  have to be written down where the next session can read them at the moment they arrive — inside the
  same session that received them — or they do not survive.
- **L10 (the ninth lens attempt).** `start` with an explicit lens selection
  (`["reliability","readability"]` plus a reason) was refused `native-start-input-invalid` /
  `lenses-invalid`, `lineage_created: false`, nothing burned; the same `start` without a selection was
  accepted and the provider then chose `review-reliability` itself, while reporting
  `lenses_required: true`. Agent lens selection is therefore inert on this contract and the tier's
  default is what actually runs.
- **L11 (T5, the deferred item decided).** L4's live advisory is closed by the owner's choice of S5: the
  step keeps no millimetre floor, and the cost it can buy is bounded instead. `cam-geom` gained
  `MAX_SEARCH_ANCHORS = 4_000_000` — the ceiling `cam-sim`'s `MAX_CELLS` and `cam-geom`'s
  `MAX_LATTICE_SAMPLES` already use — and `ProfileNestPlan::new` refuses a step whose lattice over the
  **board** exceeds it, before any search runs. The lattice is
  `(floor(width/step)+1) * (floor(height/step)+1)`, so the check is arithmetic and board-proportional:
  a 1219 x 2438 sheet at 0.5 mm asks `2439 x 4877 = 11895003` anchors and is refused, at 2 mm it asks
  `744200` and is still the caller's to choose; a 1000 x 600 board at 0.2 mm asks `5001 x 3001 =
  15008001` and is refused at the command's own door with exit `2` and the document untouched. The
  counts saturate, so a step fine enough to overflow `usize` is refused instead of panicking. New error
  `GeomError::SearchLatticeTooFine` speaks M5's dialect — *"… needs N anchors, over the C anchor cap;
  use a coarser --step"*. RED first: the 0.5 mm plan **succeeded** while the test demanded the refusal;
  after the check `cargo test -p cam-geom --test nest_profile` is **18 passed**, which includes the
  fixtures that author 20 mm, 5 mm and 200 mm — a floor in millimetres would have had to argue with
  those, and a cap does not. `const HELP` and `docs/m9-nesting.md` (the "the step is the caller's"
  invariant and the refusal list) now state the cap and the two counts.
- **L12 (the step cap reviewed: approved, and two findings against it).** Lineage
  **`review-2b7f8544a4b4dbd6`**: tier `high` — a `process_boundary` signal, not a judgement about the
  change — 159 changed lines, correction budget 80, **four** host-relay reviewer runs (risk, resilience,
  readability, reliability), closed **`approved`** on the first admitted event with no correction, receipt
  burned (`sha256:59108c52…`). Its two advisories are defects `c6eb633` **introduced**, both
  `causal_disposition: introduced`, and **this time their words survived**: the lineage kept a state
  directory, and reading it *before* the acknowledgement is what kept them — the discipline L9 asked for,
  paying for itself one review later. **`R2-001`** (readability, `crates/cam-cli/tests/cli.rs:7678`): *"The
  under-cap branch only asserts `outcome.code != 2`; a failure with any other nonzero code (including a
  panic) would satisfy it, so the test does not prove the stated accepted behavior and can hide
  regressions in the new cap boundary."* **`R3-001`** (reliability,
  `crates/cam-geom/src/nest_profile.rs:284`): *"The new anchor-cap check runs before the existing
  part-closed loop, so a document that is both over the anchor cap and contains an open part now reports
  SearchLatticeTooFine instead of PartNotClosed; the step error masks the geometry error that identifies
  the offending part."* Both fixed in **`b06c004`**: the cap check moved after the part loop (RED first —
  an open-part document at an over-cap step answered `SearchLatticeTooFine` where `PartNotClosed { part: 1
  }` is the answer), and the accepted-step branch now asserts the run ends in `0` or `7` **and** that the
  report names the step it searched with, instead of "not 2". `cam-geom`'s nest_profile target: **19
  passed**; workspace **1514 tests over 110 result lines**; whole gate green.
- **L13 (the fixes reviewed: approved, and the cap's boundary was not pinned).** Lineage
  **`review-85153ac6e3a443c8`** — the whole step-cap feature including its two corrections, tier `high`,
  206 changed lines, four host-relay runs — closed **`approved`** on the first admitted event, receipt
  burned (`sha256:57192970…`). One advisory, severity **SUGGESTION**: **`R3-CAP-BOUNDARY`**
  (reliability, `crates/cam-geom/src/nest_profile.rs:292`): *"The new cap's inclusive boundary is not
  pinned by a test: `anchors > MAX_SEARCH_ANCHORS` allows exactly 4,000,000 anchors, but the added tests
  exercise only 11,895,003 (refused) and 744,200 (accepted), so a mutation to `>=` would not be caught.
  Cheap cases are a 1999x1999 board at step 1 (2000x2000 = 4,000,000 anchors) and a 2000x2000 board at
  step 1 (2001x2001 = 4,004,001)."* Taken in the reviewer's own two cases:
  `the_anchor_cap_is_inclusive_at_exactly_the_cap` asserts 4 000 000 accepted and 4 004 001 refused —
  and, because it passed on the first attempt, it was proven by **installing the mutation**: with `>=`
  in place the test **fails**, restored it passes, and the source was restored byte-identical. The
  lesson repeats with a sharper edge: the counts the earlier tests pinned were large and specific, and
  the **comparison** was still nobody's test.
- **L14 (the ledger reconciled, and the debt cannot be closed by editing).** No task in this document
  is left, so the table above is the delivery record and this entry is the subject's state. Of the
  **fourteen** advisories L2 counted, **six** are closed and **nine** are not, and L3's list is stale
  in one place it never revisited: it still names `1ab52143/R1-STEP-DOS` as open after L11 closed it
  under the owner's S5 (the cap, not a millimetre floor) — superseded here, not rewritten. The four
  whose text survived are closed by T1-T4, and `12219c3f/R1-001` is closed by **`3d6589f`** with its
  anchor verified rather than assumed: `crates/cam-ui/tests/script_panel.rs:29-31` is still the
  `join` onto `/var/tmp`, the `remove_dir_all` and the `create_dir_all` the finding names, and T4's
  guard sits three lines above them, so the subject and its resolution are the same three lines. What
  is **not** closable is the other **nine**: `bd99706d`'s remaining readability and reliability
  advisories, `1ab52143`'s three, and the successor below — none has reviewer text that survives, and
  three anchors name `crates/cam-cli/src/main.rs`, a file `6cc3b2d` split down to 16 lines, so they are
  worse than stale. **`b5fe4921/R3-001`** (reliability, WARNING, `crates/cam-import/src/csv.rs:180-193`)
  is the fifteenth and was recorded nowhere in this ledger before this line: L9 named it as the
  successor advisory inside the very block T2 added, its wording went with its receipt, and its anchor
  is live and precise — the `split` function's quote-before-count branch. Reading that branch does not
  establish its subject, so it is filed **open and named unrecoverable** rather than guessed at (S3).
  The consequence, stated so the next session does not re-derive it: this debt **cannot be closed by
  editing code**. S3's one sanctioned route is a fresh review of the current tree, which re-files these
  findings with new words and new identities; re-file or accept is the owner's decision, and it is a
  review-mode decision, not a task.
- **L15 (the cc2d609 review's six advisories, captured before the burn — and the method that is now proven
  twice).** Lineage **`review-39a6d85c490202c2`** closed `approved` (see
  `odd/tasks/m9-nesting.md` L13); its receipt is burned, so these words exist only because the
  lineage kept a state directory and it was read **before** the acknowledgement. They are informational
  and none of them reopens anything. Tagged by lineage, as the register requires — `39a6d85c/…`:
  1. *risk WARNING* (`cam-model/src/lib.rs:1763-1783`, `R1-nesting-footprint-bypass`) — "`check_nesting`
     treats a `None` footprint as a zero-area placement and skips the off-sheet and overlap checks.
     `placement_footprint` returns `None` not only for degenerate parts but also when any referenced
     entity is not a contour (`NestingPart::bounds` uses `as_contour()?`) or when the transform fails,
     and the earlier part check only verifies that the entity id exists. A document can therefore
     reference a non-contour entity, place that part anywhere, including outside the sheet or over
     another placement, record a 0/1 cost, and still pass `Document::validate`, so downstream code that
     trusts the load-time certificate may act on an unchecked placement." — the corrected half of this is
     closed by `14a8f0e`; the "non-contour entity" half is **unconstructible** (`EntityKind` has one
     variant) and is kept as the record of a claim that named a path the type system closes.
  2. *risk WARNING* (`cam-model/src/lib.rs:1786-1805`, `R1-quadratic-overlap-validation`) — "The new
     overlap validation nests two loops over every placement and only filters by sheet inside the loop, so
     it performs O(n^2) iterations regardless of sheet distribution. A large or crafted nesting document
     therefore makes `Document::validate` consume quadratic CPU and block the CLI; there is no spatial
     index or placement-count cap bounding the work." **Open, and the most valuable of the six.**
  3. *resilience WARNING* (`cam-model/src/lib.rs:1786-1797`, `R4-002`) — the same double loop measured in
     pairs: "calls `cam_geom::winding::relation` for each same-sheet pair. For N placements this is
     N(N-1)/2 pair visits, **49,995,000 for 10,000 placements**, with no per-sheet grouping or spatial
     index, so a large authored nesting makes document load/validation quadratic in placement count and
     can exhaust CPU/time on the normal load path." **Open.**
  4. *resilience WARNING* (`cam-model/src/lib.rs:1879-1881`, `R4-003`) — "When `nesting_utilisation()`
     returns None because there are no sheets or no placements, `check_nesting_cost` returns Ok after only
     the sheet-count and range checks. A cost record with arbitrary in-range values therefore survives
     validation with no placements to support it; deleting all placements while leaving the cost field
     yields a document that still loads and can report the stale utilisation/waste." **Open.**
  5. *readability WARNING* (`cam-model/src/lib.rs:979-987`, `R2-001`) — "`NestingPart::bounds` documents
     `None` as coming only from an empty selection or an unknown entity, but the implementation also
     returns `None` through `as_contour()?` for any entity that is not a contour. That hidden third case
     flows into `Document::placement_footprint` … Either the loader should refuse non-contour entities by
     name or the doc and handling should make this case explicit." The handling is now explicit at the
     refusal site; **the `bounds` doc sentence is still the one the finding quotes**, and the validator's
     own follow-up says the `placement_footprint` enumeration omits the non-contour case. **Open, cheap.**
  6. *readability SUGGESTION* (`cam-model/src/lib.rs:1830`, `R2-002`) — "The constant is named
     `NESTING_COST_SUM_TOLERANCE`, but T1c deleted the sum rule; it is now only the absolute tolerance for
     comparing each recorded fraction with the derived utilisation or its complement. The misleading name
     keeps the old mental model alive … Rename it to a fraction/tolerance name and align the surrounding
     doc." **Open, one-line plus its doc.**
  The method conclusion, now paid for twice: reading the state directory in the same session that receives
  the findings is the **only** thing that keeps them (L9 lost `R3-001` this way, L12 kept two, this review
  kept six). It is not optional ceremony — for this store it is the whole record.
- **L16 (the overlap sweep reviewed: approved first pass, one BLOCKER refuted, three follow-ups worth having).**
  Lineage **`review-94cfd23437f72a47`** over the sweep — tier `high`, 5 paths, 305 changed lines, budget
  153, four host-relay reviewer runs plus a refuter, closure **`approved`** (`sha256:b44d3526…`) with the
  receipt burned and **no correction required**. It **closes** `39a6d85c/R1-quadratic-overlap-validation`
  and `39a6d85c/R4-002`: the pair loop no longer visits N(N-1)/2 pairs. The message it produces does not
  move, and the proof is `the_pruned_overlap_check_names_the_same_pair_a_naive_scan_would` — a seeded
  scan whose reference walks pairs in document order, which caught the first version naming `P3/P7` where
  the document order says `P1/P3`, because a sweep in x meets pairs in a different order. Its six
  findings, tagged by lineage as `94cfd234/…`, captured from the store before the acknowledgement:
  1. *readability BLOCKER* (`lib.rs:1869`, `R2-001`) — **refuted, correctly.** The claim was that
     `sheet_id` is `&&str` (iterating `&BTreeMap<&str, _>`) while `first_offence` is
     `Option<(usize, usize, &str)>`, "and Rust does not apply deref coercion to tuple-literal elements",
     so the crate fails to type-check. The refuter threw it out and the build proves it: a tuple
     expression is a coercion site, so `&&str` coerces to `&str` there — the whole workspace compiles and
     its 1 529 tests pass.
  2. *risk WARNING* (`lib.rs:1869`, `R1-001`) and 3. *resilience WARNING* (`lib.rs:1844-1880`,
     `R4-001`) — **one finding, real, accepted with its trade named**: "a large invalid document with an
     early overlap and many later placements whose x ranges meet now performs the full candidate scan
     before refusing, whereas the baseline returned immediately on the first pair." That is exactly the
     price of preserving the message, and it is the *failure* path: the sweep is strictly cheaper on a
     valid document, which must visit every pair, and only the invalid-early-overlap case pays. A real
     early exit needs a spatial index, not an accumulation — **open, and the largest of the six**.
  4. *reliability WARNING* (`lib.rs:1850-1854`, `R3-PRUNE-TOLERANCE`) — "the soundness of the
     optimization rests on that prune threshold matching the separation tolerance used inside
     `footprints_overlap`/`winding::relation` … it never exercises the tolerance term and never exercises
     a pair where the AABB predicate and the winding predicate could disagree." **Open, cheap, and the
     one to do first**: a sub-tolerance fixture is a few lines.
  5. *reliability SUGGESTION* (`lib.rs:1830-1834`, `R3-MULTISHEET-PRUNE`) — "every added test constructs
     exactly one sheet. The baseline cross-sheet behaviours are therefore not asserted: two placements on
     different sheets that coincide in coordinates must remain accepted, and when multiple sheets each
     contain an overlap the named pair must be the globally earliest one. A mis-keyed bucket or a
     per-bucket (rather than global) minimum would pass the added tests." Same shape: cheap, real.
  6. *readability SUGGESTION* (`tests/document.rs:1751`, `R2-002`) — the reference's justification says
     strict inequalities are exact "because coordinates are whole millimetres", but the real reason a
     shared edge is not an overlap is `footprints_overlap`'s `Touching` rule (*equal* bounds only). The
     code is right and the reason is the wrong one — the comment should say which rule it relies on.
- **L17 (the sweep's seven, and a sixteenth advisory that the sweep found).** Feature document
  `odd/tasks/advisory-debt-close.md` (S1-S6, T1-T8, L1-L9). Branch `feat/advisory-debt-close`, three
  work-unit commits: **`c16afd6`** (the contract change alone, because it refuses input that used to
  load), **`5190f3e`** (three wording advisories and the two missing fixtures) and **`6e0b085`** (the
  index and the early exit, 61.99 s → 0.05 s on 5 000 stacked copies). **Closed by this sweep**:
  `94cfd234/R2-002` and `R3-PRUNE-TOLERANCE` and `R3-MULTISHEET-PRUNE`, `39a6d85c/R2-001`, `R2-002`
  and `R4-003`, and `94cfd234/R1-001` + `R4-001`. The nine L14 called unclosable are untouched and
  still wait on S4's fresh review.
  **The sixteenth, and it is not one of the fourteen: `relation` calls overlapping material
  `Touching` when two edges are collinear.** Measured, not inferred: `cam_geom::winding::relation` over
  `[0,0]-[100,100]` and `[50,0]-[150,100]` answers `Relation::Touching`, though the two rectangles
  share 5 000 mm² of interior. The same pair with the edges *not* collinear
  (`[0,0]-[100,100]` against `[80,80]-[130,280]`) answers `Crossing`, which is why the workspace never
  noticed. Cause read off `crates/cam-geom/src/winding.rs:245-280`: a coincident edge sets `touching`
  and no segment pair contributes an interior-to-interior intersection, so the function returns
  `Touching` before containment is ever probed — the classification never asks whether the interiors
  overlap, which its own doc claims it is deciding ("They meet without crossing").
  **What it costs**: `footprints_overlap` accepts `Touching` unless the two bounds are *equal*
  (`crates/cam-model/src/lib.rs`), so two nested placements that overlap in area while sharing a
  horizontal or vertical edge load without a refusal — the rule M8 and M9 exist to enforce;
  `cam-geom/src/nest_profile.rs:657` asks the same predicate for `!= Disjoint`, so the profile nester's
  own collision test inherits it. This sweep's T6 fixture was rewritten twice around it: the first
  pair it tried, `[0..100]` and `[50..150]` on one sheet, was exactly this shape and validated `Ok`.
  **Not fixed here, and deliberately not smuggled into a debt sweep**: the fix changes shared geometry
  with four consumers (`cam-model`'s nesting, `nest_profile`, `region_offset`, `cam-toolpath`'s inlay)
  and needs its own document and review. Flagged for the owner in the same turn.

## Successor advisories: the sweep's review, re-filed by its own candidate

Filed here as the register requires, because their words exist only in Engram observation #193 and in
`odd/tasks/advisory-debt-close.md` L9. Lineage **`review-ca5c7eda65951600`**, closure `approved`, receipt
burned. The provider marked all five informational against that candidate; three are open work.

| ID | Lens | Severity | Subject | Status |
| --- | --- | --- | --- | --- |
| `ca5c7eda/R2-001` | readability | WARNING | `crates/cam-model/tests/document.rs:1891-1898` — the two prune-tolerance tests assert only that `validate` succeeds, so a prune that wrongly skipped the pair would leave both green; the first name says "less than" while the loop includes `gap == tolerance` | **open** |
| `ca5c7eda/R3-002` | reliability | WARNING | `crates/cam-model/tests/document.rs:1913-1915` — no case asserts a **refusal** for a pair that truly overlaps by less than the tolerance, so an unsound prune is invisible (use shapes without collinear edges, or the sixteenth advisory masks it) | **open** |
| `ca5c7eda/R2-002` | readability | SUGGESTION | the index was four parallel arrays with `0`/`""` sentinels and nothing enforcing one assignment per footprint | **closed** by `52e7f11` — per-sheet `SheetGrid`, `sheet_of` derived by an iterator |
| `ca5c7eda/R1-001` | risk | WARNING | the load regression, from the risk lens | **answered** by `54a0ef1` |
| `ca5c7eda/R3-001` | reliability | WARNING | the same, from the reliability lens | **answered** by `54a0ef1` |

**The sixteenth is closed** (`odd/tasks/winding-collinear-overlap.md`, S2 = "Ruta B: arreglar en la
raíz"). `winding::relation` now escalates a shared span to `Crossing` when the two interiors lie on the
same side of it and neither contour encloses the other, deciding the side from each segment's direction
and each contour's own `signed_area` sign rather than assuming counter-clockwise. A bare coincident
edge, a tangency, identical contours and a containment at a tangency stay `Touching`, and the fixtures
that pin them are untouched. The witness is **two-sided** — a vertex of each strictly outside the other
— because the one-sided reading escalated the flush-part fixtures, and the part flush against its board
is exactly the shape that proves it. Consequence, measured: `crates/cam-model`'s own load-regression
tripwire had been built on 20 000 ten-millimetre squares 4.5 mm apart, i.e. on copies that overlapped
each other and validated only because of this defect; the fixture is now spaced 11 mm and the document
is valid for the right reason.
