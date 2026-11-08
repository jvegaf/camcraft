# Repository name hygiene — campilot speaks only of campilot

Feature document for the sweep the owner asked for while M13 was open (`odd/tasks/m13-in-app-cycle.md`,
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
  - twelve documents are **named** for the product: `odd/tasks/m0-foundation.md` … `m12-export.md`
    plus `region-booleans.md`, and M13's own `m13-in-app-cycle.md`;
  - `docs/m5-simulation.md`, `m6-toolpaths.md`, `m7-vcarve.md`, `m8-moulding.md`, `m9-nesting.md`,
    `m10-documents.md`, `region-booleans.md`, `odd/roadmap.md`, `odd/tasks/{camui-toolpath,
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
  `odd/tasks/m0-foundation.md` … `m12-export.md`, `m13-in-app-cycle.md`, `region-booleans.md`. Every inbound
  reference (`odd/roadmap.md`, the other task documents, `docs/`, `Cargo.toml` comments) is updated in the
  same slice, because a renamed file with a stale link is a broken repository rather than a hygiene win.
- **S7 (out of scope):** the `.pp` dialect's behaviour, its directives and blocks; the fixtures' Mach3
  compatibility; any code path. This is a provenance and naming change: **no behaviour, no test expectation
  and no document's technical content changes** except the sentence that wrongly claimed where it came from.
- **S8 (repo law for the wording):** the replaced prose says what a thing **is and does**. It claims no
  authorship of a format it did not invent and names no third party: "post-processors are data, never code"
  is true and safe; "campilot's own post format" would be a new false claim, and the sweep does not trade one
  for the other.

## Tasks

| ID | What | S | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | The prose sweep: the README's framing and its rationale table re-grounded row by row, `evidence/` deleted, and the mention removed from `docs/`, `crates/`, the `.pp` fixtures and `Cargo.toml`. Rows that cannot be re-grounded are deleted and named in the log | S1–S5 S8 | worker | — |
| T4 | The **dossier paths** the sweep revealed: `odd/` still cites `evidence/analysis-report.md`, `evidence/derived/exe_utf16_strings.txt` and `evidence/installer-extract/` as where a spec's vocabulary came from (m3 S5, m4 S4/S5, m6, m7, m0 L3, roadmap L3). Each clause either states the fact it was establishing without the citation, or goes | S1 S4 S8 | worker | — |
| T2 | The thirteen documents renamed with every inbound link updated, the pattern `(name)|(vendor)` matching only this document's own Log quote over the whole tree, gate green | S2 S6 S7 | inline | — |
| T5 | The **stand-ins** the names left behind: "the original", "the product", "the dossier", "the observed/recovered `.pp` dialect", the bitmap-relief strategy's product name and the closed project-file extension, each replaced by the CAM fact it stood for, or deleted where no fact survived it | S1 S3 S8 | worker | — |
| T3 | The roadmap's entry: what the tree now says, and the history note S5 owes the reader | S5 | inline | — |

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

