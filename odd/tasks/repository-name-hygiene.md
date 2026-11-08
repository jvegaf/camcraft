# Repository name hygiene — campilot speaks only of campilot

Feature document for the sweep the owner asked for while M13 was open (`odd/tasks/m13-in-app-cycle.md`,
L5). Read **until `## Log`** before touching anything.

## Specs

- **S1 (the owner, verbatim):** "necesito que cuando finalizes con la tarea, elimines del repo toda
  referencia a the vendor o the reference CAM application, porque otro agente me lo a avisado en una revision. me puso esto: El análisis
  de the vendor. El README de campilot dice que las decisiones salen de un análisis de ingeniería inversa de
  the reference CAM application, y que el dialecto .pp está “recuperado del producto analizado”. Los EULA suelen prohibir eso. En
  la UE hay excepciones de interoperabilidad, pero conviene revisarlo con un abogado antes de publicar o
  comercializar. No soy abogado."
- **S2 (the census, measured on `feat/m13-in-app-cycle`):** 32 files and 128 occurrences match
  `the vendor|the reference CAM application`, case-insensitive, excluding `.git` and `target`. Of those:
  - `README.md:4` frames the project as an answer to *"what would the reference CAM application look like if it were designed
    today"*, and `README.md:260` says the decisions *"come from a REA analysis of the reference CAM application"*, followed by a table whose left column is an observation of that artifact;
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
| T2 | The thirteen documents renamed with every inbound link updated, `grep -ri "the vendor\|the reference CAM application"` empty over the whole tree, gate green | S2 S6 S7 | inline | — |
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
