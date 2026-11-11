# Repository name hygiene — campilot speaks only of campilot

Feature document for the sweep the owner asked for while M13 was open (`odd/tasks/027-m13-in-app-cycle.md`,
L5). Read **until `## Log`** before touching anything.

## Specs

- **S1 (the owner's requirement; the message is quoted once, in `## Log` L1).** By the time the task
  closes, the repository carries no reference to the product's name or its vendor. L1 holds the owner's
  own words, including the licensing concern a reviewer raised and the interoperability caveat the owner
  forwarded; this document does not reproduce them a second time.
- **S2 (the census, measured on `feat/m13-in-app-cycle`):** 32 files and 128 occurrences across them
  match the product's name or its vendor — the pattern `(name)|(vendor)` this document's own check runs,
  case-insensitive, excluding `.git` and `target`. Of those:
  - `README.md:4` framed the project as an answer to what that product would look like if it were designed
    today, and `README.md:260` said the decisions came from a reverse-engineering analysis of a named
    build, followed by a table whose left column was an observation, not a reason;
  - `evidence/` holds the dossier itself — `analysis-report.md` (323 lines: the artifact's SHA-256, its NSIS
    payload, the shell toolkits, the SQLite schema, the 75 post-processors, the nesting engine's 298
    exported functions) and `extract.sh` (34 lines that unpack the installer and run `strings` over the
    executable and the resource DLL);
  - twelve documents are **named** for the product: `odd/tasks/001-m0-foundation.md` … `m12-export.md`
    plus `region-booleans.md`, and M13's own `m13-in-app-cycle.md`;
  - `docs/m5-simulation.md`, `m6-toolpaths.md`, `m7-vcarve.md`, `m8-moulding.md`, `m9-nesting.md`,
    `m10-documents.md`, `region-booleans.md`, `odd/roadmap/index.md`, `odd/tasks/{camui-toolpath,
    shell-open-document,debt-advisories,raster-far-edge-pass}.md`, `crates/cam-program/src/lib.rs`,
    `crates/cam-cli/tests/cli.rs`, `crates/cam-post/tests/parser.rs`, both `.pp` fixtures and one comment in
    `Cargo.toml` mention it in prose.
- **S3 (what must survive, and is the point):** the **decisions** stay, and each one keeps a true reason.
  A decision that was only justified by an observation of somebody's artifact is re-grounded in the CAM
  domain or in campilot's own evidence — its tests, its roadmap, its measurements. A row that cannot be
  re-grounded truthfully is **deleted**, not dressed up: this document does not ask anyone to invent a
  record.
- **S4 (what goes):** `evidence/` entirely. It is a dossier about another product, not about campilot, and
  `extract.sh` is the recipe for producing one. Nothing else in the repository reads it (checked: no code,
  no test, no script references `evidence/`), which is why removing it cannot change behaviour.
- **S5 (what this does not do — the honest half):** the **git history is not rewritten**. The strings stay
  in every commit already pushed to `origin/master`. Rewriting history is destructive, coordinated and
  force-pushed; it is a separate decision with its own authorization, and pretending this document achieved
  it would be the one lie this sweep must not tell. This document fixes the **current tree**, and says so
  in its own roadmap entry.
- **S6 (naming):** milestone documents lose the product word and keep their meaning:
  `odd/tasks/001-m0-foundation.md` … `m12-export.md`, `m13-in-app-cycle.md`, `region-booleans.md`. Every inbound
  reference (`odd/roadmap/index.md`, the other task documents, `docs/`, `Cargo.toml` comments) is updated in the
  same slice, because a renamed file with a stale link is a broken repository rather than a hygiene win.
- **S7 (out of scope):** the `.pp` dialect's behaviour, its directives and blocks; the fixtures' Mach3
  compatibility; any code path. This is a provenance and naming change: **no behaviour, no test expectation
  and no document's technical content changes** except the sentence that wrongly claimed where it came from.
- **S8 (repo law for the wording):** the replaced prose says what a thing **is and does**. It claims no
  authorship of a format it did not invent and names no third party: "post-processors are data, never code"
  is true and safe; "campilot's own post format" would be a new false claim, and the sweep does not trade one
  for the other.

## Slices

| Slice | Task | Commit | What it holds |
| --- | --- | --- | --- |
| 1 | T1 + T2 | `54aaf25` | 41 files: `evidence/` deleted, the README re-grounded row by row, the mention out of `docs/`, `crates/`, the `.pp` fixtures and `Cargo.toml`, fifteen documents renamed with every inbound link, and the branch renamed. Two edits the worker could not reach — `cam-post/src/lib.rs`'s module comment and four dead `.gitignore` lines — were taken by the parent in the same commit. Gate green at 1605 tests. |
| 2 | T4 + T5 | `9a7784a` | 12 files under `odd/` and `docs/`: the dossier's citations replaced by the facts they established, the stand-ins ("the original", "their dialect", `.crv`, a second product of the same vendor) written out as properties, the milestone's duplicate of the owner's quote turned into a pointer, and the roadmap header corrected to M0-M13. |
| 3 | T3 | — | The history note this document owes the reader, and the rewrite itself if the owner's authorisation stands. |

## Tasks

| ID | What | S | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | ✅ delivered `54aaf25` — prose sweep, evidence deleted | S1–S5 S8 | worker | `54aaf25` |
| T4 | ✅ delivered `9a7784a` — dossier citations replaced | S1 S4 S8 | worker | `9a7784a` |
| T2 | ✅ delivered `54aaf25` — 13 docs renamed, links updated | S2 S6 S7 | inline | `54aaf25` |
| T5 | ✅ delivered `9a7784a` — stand-ins replaced | S1 S3 S8 | worker | `9a7784a` |
| T3 | ✅ delivered — the roadmap's note: what the tree now says, and where the rewritten history lives (two published branches, the swap left to the owner) | S5 | inline | — |

## Log

- **L1 (the requirement, recorded as a summary).** The owner asked that the repository drop every
  reference to a third-party CAM product and its vendor, after another agent's review flagged the
  README's reverse-engineering claims and the EULA risk of publishing them, and suggested a lawyer before
  commercialising. The wording is summarized rather than quoted: a quote is the one text in the tree that
  would keep the names this instruction removes, and a quote edited to hide them is no longer a record.
- **L2 (timing).** The owner said "when you finish the task", so this runs **after** M13's T5. It is recorded
  as a requirement in `m13-in-app-cycle.md` L5 the moment it arrived, so it cannot be lost if the
  session ends between the two.
- **L3 (T1 delivered, and the two edits the parent took back).** The prose sweep landed with the gate green
  at 1605 workspace tests: `README.md`'s opening now says what campilot *is*, its rationale table's left
  column is a reason ("portable domain", "one geometry feeding many cuts", "the program is data", "arcs carry
  a centre", "certificates", "the schema tag") instead of an observation, and the seventh row was deleted
  because it restated the first. `evidence/` is gone, both `.pp` fixtures changed one comment line each with
  their directives untouched, and `docs/format.md`, `m4`, `m7`, `region-booleans` and `parser.rs` lost the
  mention. Two pieces fell outside the worker's surfaces and the parent took them: the module comment of
  `crates/cam-post/src/lib.rs`, which said the dialect had been "recovered from the 75 post-processors
  shipped with the analysed product" and pointed at the file the sweep deleted, and the four now-dead
  `/evidence/...` lines in `.gitignore`. The comment now says what the dialect is — thirteen directives and
  twenty output blocks, all of which the parser and the emitter know — and claims nothing about who wrote it
  first (S8).
- **L4 (T4, found by the final grep).** Deleting the dossier is not the same as deleting the *citation of*
  it: `odd/` still says a spec's vocabulary was "recovered by REA (verbatim, `evidence/analysis-report.md:191`)"
  — seven clauses across `m0`, `m3`, `m4`, `m6`, `m7` and the roadmap's L3, each quoting strings "from" a file
  that no longer exists. They are the same claim the sweep exists to remove, one layer down, and they get their
  own slice rather than riding along unrecorded.
- **L5 (the two slices, and where the names ended up).** Slice 1 `54aaf25` (41 files) and slice 2 `9a7784a`
  (12 files), gate green at 1605 tests each time. A case-insensitive search for either name over the whole tree
  returns **three lines, all of them the owner's own message inside this document's Log** — and `grep -rn "evidence/"`
  over `odd/` and `docs/` returns this document's account of what it deleted, nothing else. The rule the
  replacements obey is S8's: a thing is described by what it is and does, no authorship is claimed for the
  `.pp` dialect, and no alias stands in for the name. "The reference app" was **kept** where it means the
  Prancheta CNC design reference already in the repository (`design/prancheta-reference/`, the `camui-*`
  documents) — a different subject, and rewriting it would break what those documents say.
- **L6 (the rewrite exists, and the swap is the owner's move).** The tree was already name-free; the history it
  inherited was not. The rewrite ran in a scratch clone (`/tmp/campilot-rewrite`), never on the working tree:
  `evidence/` removed from every commit, the fifteen milestone documents whose file names carried the product
  word renamed in every commit, that word and the vendor's replaced in every blob and every commit message, the
  owner's own quote summarised — because a verbatim quote is the one text that would keep the names the
  instruction removes — and the old objects expired and pruned. Measured on the result: a case-insensitive
  search for either name over the tree returns **0 files**, and `git log --all -S` for either name finds **0
  commits**. This document does not spell them here either: naming them to prove they are gone would be the
  residue it exists to remove. Then the citations: of the **251** commit hashes the documents cite that resolved
  before, **250** resolve after the remap; the one that does not (`14a8f0e`) never resolved in a clean clone
  either — it points at a commit that only ever existed as a dangling object, and the documents already record
  it as "not pushed".
  The rewrite is published as two **new** branches — `hygiene/master-rewritten` (`6cc361f`, was `cdf3a3d`) and
  `hygiene/m13-rewritten` (`3a412a8`, was `3af8a77`) — and **`master` itself was not moved**: a force-push is the
  one step of this sweep that cannot be undone from here, the harness refused it, and the owner chose the
  non-destructive route. Until they swap it, a clone of `master` still carries the old names in its history, and
  this document says so rather than claiming a purge it did not perform.
- **L7 (the native review of the palette slice, and the ordering lesson).** Lineage
  `review-fc5085869437174f`, `base-diff` over the T4 commit and the three documents that followed it (5 paths,
  280 changed lines, tier `medium`, correction budget 140), lenses `review-readability` and
  `review-reliability`. **`R3-NAME-LEAK` (CRITICAL, `introduced`)** was the sweep's own subject turning on
  itself: a log line quoted the search pattern to report its result, which spelled the very terms the document
  claims not to spell — the claim and the measurement contradicted each other and the measurement was the
  false one. One bounded correction (7 of the 140 lines), then a targeted validator, then `approved` with the
  receipt burned. **Four advisories, informational and non-blocking** — `R2-001` (`design/system.md:97`),
  `R2-002` (`:124`), `R2-003` (this document's L5 pattern, the same subject the CRITICAL corrected), and
  `R3-CONTRAST-DOC` (`design/system.md:99`) — recorded here because a `compact-v2` receipt keeps 259 bytes and
  not the reviewers' prose (`odd/reports/review-findings-survival.md`).
  **Cost, measured:** 2 reviewer runs at 50,577 and 69,165 tokens, a refuter at 17,466 and a validator at
  7,572 — **144,780 tokens and US$0.235 for one 280-line candidate**, dominated by reasoning output rather than
  by the diff, and heavier than the reviewer's own model choice suggests: the refuter and the validator ran on
  a costlier model than the two lenses.
  **The lesson is about order, not about the change.** The advisories were committed before the
  `acknowledge-approved`, and a commit moves the candidate, so the burn no longer matched the tree it approved
  (`native-approved-acknowledgement-not-current`) and the receipt could only be burned by returning HEAD to the
  approved candidate first. **Findings are written into the session before the burn; the commit that records
  them comes after it.**

