# M10 — Documents & scripting: the command language, the cut list and the print sheet

Feature document for M10 of `odd/roadmap.md`. Read **until `## Log`** before touching code; the Log is
the record, not the brief.

## Specs

- **S1 (user, verbatim answer to "¿Con qué se ejecuta la superficie de scripting?"):** "DSL de comandos,
  sin motor".
- **S2 (user, verbatim answer to "¿Qué puede tocar un script?"):** "Lectura y mutación del documento".
- **S3 (user, verbatim answer to "El CSV de piezas, ¿entra, sale, o los dos?"):** "Import y export".
- **S4 (user, verbatim answer to "¿M10 toca la UI?"):** "CLI más panel mínimo".
- **S5 (roadmap M10 row, not a user string):** "Documents & scripting — HTML print sheets, CSV import for
  part lists, scripting surface (Rhai/Lua) with a UI panel — Reporting and extension points; the
  original's gadget platform is the model". S1 supersedes its "Rhai/Lua": there is **no engine**.
- **S6 (roadmap non-goal, verbatim from §1):** "the vendor Lua gadget compatibility (a scripting surface is
  planned, not their dialect)."

## Invariants every task must honour

These are the repo's conventions plus the two laws this milestone's own decisions create; a task that
breaks one is refused:

1. **One implementation per command, and the script is a caller, not a second parser.** Every command a
   script can execute must dispatch to the same function the corresponding `cam-cli` subcommand calls.
   A script that could drift from the CLI — a second geometry parser, a second nesting call with its own
   defaults — is the defect this law exists to prevent. If a command's logic today lives inside an
   argument-parsing arm, **extract it** rather than duplicating it.
2. **Located diagnostics.** A script that fails says **line and column** and the offending token, the way
   the post-processor parser does. "the script failed" is not an error message.
3. **The document is the only state.** A script mutates a `Document` and nothing else: no ambient files,
   no environment, no network, no process spawn. A script cannot import a path the user did not name on
   the command line or in the script itself.
4. **Every mutation goes through `Document::validate`.** A script cannot write a document that would not
   load; the refusal names the offending script line, not only the model's message.
5. **Determinism is a test.** Two runs of the same script on the same document produce identical bytes,
   including the generated HTML and CSV.
6. **No new external dependency.** S1 chose no engine; a template engine for the HTML sheet is refused for
   the same reason. `Cargo.lock` is a reviewable surface, and the invariant that matters is the
   **package count**: it does not move (482). A **workspace-internal** edge may record one line in it —
   the lock writes a crate's dependency list, so any edge between two workspace crates adds its name —
   and refusing that would have made the panel impossible, which is why the wording changed (L6).
7. **Every new command, flag or message updates `const HELP`** (`crates/cam-cli/src/main.rs:27`) and the
   docs that describe it; a test asserts the help mentions the flags, as `tests/cli.rs:552` does.
8. **Environment:** every cargo command needs `RUSTUP_HOME=/home/th3g3ntl3man/.rustup
   CARGO_HOME=/home/th3g3ntl3man/.cargo`; a commit needs `-c user.name="Jose Vega" -c
   user.email="josevega234@gmail.com"`. Workers never commit. Never write scratch under `/tmp` (full
   tmpfs); use `/var/tmp` and clean it up.

## Tasks

| ID | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | The command language: a line-oriented grammar, a parser with located diagnostics, and a dispatch table whose every entry calls the function the CLI subcommand already calls | S1 S2 | worker | — |
| T2 | CSV: parts **in** (name, width, height, quantity) as nesting parts, and the cut list **out** from M9's placements, both with line-located refusals | S3 | worker | — |
| T3 | The HTML print sheet: a fixed renderer, deterministic bytes, one command that writes it | S5 | worker | — |
| T3b | Make `cam-cli` a **library plus a thin binary**: every operation and the command language become `pub` in the crate's lib target, and `main.rs` keeps only argument parsing and printing. Then the shell calls the same functions the binary calls, instead of a second implementation or a process boundary | S1 S2 S4 | worker | — |
| T4 | The minimal panel: run a script from the shell behind `cam-ui`'s adapter, show its output and its located errors — over `cam-script`, never by spawning the CLI | S4 | worker | — |
| T5 | Goldens for the script run, the cut list and the sheet; determinism twice over; the milestone differential against its cut point | S1-S5 | worker | — |
| T6 | Docs: `docs/m10-documents.md`, `odd/roadmap.md` §4.13, README, `docs/format.md` if the document moves, and this document's evidence | S1-S6 | parent | — |

Order is the dependency order; each task is one work unit and one commit. T1 defines the dispatch table,
so it goes first and alone.

## Log

- **L1 (user, verbatim):** "dale" — authorising M10, the milestone `odd/roadmap.md` names as next.
- **L2 (user decisions, verbatim option labels):** "DSL de comandos, sin motor"; "Lectura y mutación del
  documento"; "Import y export"; "CLI más panel mínimo". Recorded as S1-S4 above.
- **L3 (exploration, before the specs):** no CSV support exists anywhere in the workspace
  (`grep -rln csv crates/` is empty, so the `cam-import` row in §3 names an intention, not a feature);
  `Cargo.lock` holds **482** packages and is treated as a reviewable surface; the roadmap's own M10 row
  leaves the engine undecided ("Rhai/Lua"), which is why S1 had to decide it and why S5 is marked as the
  plan rather than as an owner string.
- **L4 (T4 is not reachable as written, found before it started).** `crates/cam-cli` has **no `[lib]`
  target** — it is `main.rs` plus modules — so `cam-ui` cannot depend on it, and a panel that runs a
  script cannot use the engine where T1 put it. Spawning the CLI as a subprocess would be the
  alternative, and it is refused: it duplicates path and environment handling and puts a process
  boundary between the shell and its own document. So **T3b** was opened to move the command language
  into a `cam-script` library that both `cam-cli` and `cam-ui` call, which is invariant 1 taken
  literally — the engine has one home and both callers are callers. T4's row now depends on it, and
  this is the second requirement-level finding of the milestone: the first was that no CSV support
  existed at all (L3). `cam-ui` already depends on `cam-store`, `cam-program`, `cam-sim`, `cam-import`
  and `cam-model`, so the new crate's edges are the toolpath and post crates it does not have yet.
- **L5 (T3b's shape, cheaper than L4 first said).** L4 proposed moving the engine into a new
  `cam-script` crate. Designing it showed that the engine is not the only thing the shell needs: the
  **operations** it dispatches to (`nest_document`, `append_toolpath`, `emit_document`,
  `simulate_document`, `validate_report`) live in `main.rs` too, so a new crate would have to absorb
  them or the shell would reimplement them — the second implementation invariant 1 exists to prevent.
  The cheap honest shape is therefore to make `cam-cli` a **library plus a thin binary**: the
  operations and the command language become `pub` in the lib target, `main.rs` keeps argument parsing
  and printing, and `cam-ui` depends on the library. Zero code moves between crates, zero behaviour
  change, and one implementation with two callers. Recorded as **D-M10-1**: the crate named `cam-cli`
  is also the home of the operations the shell calls, which reads oddly and is deliberate — the
  alternative was moving thousands of reviewed lines to earn a better name.
- **L6 (T4b delivered as `partial`, and the invariant it broke).** The two bypasses T4a declared are
  **closed**: `pieces::cut_list_document` now takes a sink and writes through `crate::write_report`, and
  `save`'s sentence comes from one new `pieces::save_report` — shared with `parts_command`, which said
  the same words — delivered to a second session writer through `Session::notice`. So the panel's
  capture holds the cut list **and** the `wrote …` line, proven by a test that runs
  open/sheet/part/nest/cutlist/save through `ScriptPanel::run` and asserts both the painted area and the
  captured text. The CLI's own bytes did not move: a byte-level differential against a copy of the base
  commit shows identical stdout, stderr and exit code, because the `save` notice stays on stderr where an
  existing cam-cli test pins it. The panel itself is `cam-ui/src/script_panel.rs`: a heading, a path
  field, a `Run` button and an output area painted one line per row, with the adapter boundary between
  `run()` — which touches no egui and calls `cam_cli::script::run_script_to` as a library, never a process
  — and `ui()`. Four headless frame tests: the report painted, the located `line L:C` error painted, a
  failed run **keeping** the previous output, and the cut-list/save capture. `cargo test -p cam-ui` 264 →
  **268**, `cam-cli` **194**, workspace **1506**. Two disclosures were escalated instead of absorbed.
  First: `cam-cli.workspace = true` was unavailable because `cam-cli` had no entry in the root
  `[workspace.dependencies]`, so the writer used the path form; the parent added the entry and switched to
  the workspace form. Second, and the reason this document changed: **`Cargo.lock` moved one line**
  (`+ "cam-cli"` in cam-ui's dependency list). The writer reported it rather than hiding it, and the
  finding is right: the original invariant 6 said the lock "must not move", which no edge between two
  workspace crates can satisfy, because the lock records that list. The invariant was therefore **wrong,
  not the change**: it now reads "no new external dependency, and the package count does not move" — 482
  before and after — which is what it always meant.
- **L7 (the milestone's differential, run against M10's cut point).** The cut point is `955fc54`
  ("docs(odd): close M9 and record what it leaves open"), the last commit before M10's first,
  `b3d1170` ("feat(cam-import): read a part list from CSV, refusing each bad row by line");
  `git log --oneline 955fc54..HEAD` lists those eight commits and nothing else, so the archive holds
  M9's binary — the surface this milestone promised not to move. `git archive 955fc54 | tar -x -C
  /var/tmp/camm10diff.*/base`, built there with a `CARGO_TARGET_DIR` of its own; the current binary
  is `ba91f5e`'s, built the same way. The base binary and the current one then each ran **31
  invocations** in a fresh directory, and the current binary ran them **twice, in two directories
  (`cur`, `cur2`), to prove the harness deterministic before it judged anything**: those two runs are
  `diff -r` clean over all 31 steps' exit codes, stdout, stderr and file inventories. Fixtures: one
  small SVG imported by each side (report, then `--out`) and a tool library each side authored with
  two `tools add` calls — both sides author them, so their bytes are part of the comparison — plus
  one **shared** nesting document authored by the harness with the current binary's `script`
  (`open`/`sheet`/`part`/`part`/`nest`/`toolpath`/`save`, kept beside it as `nest.cam`) and copied
  byte for byte into both directories, because authoring a nesting part needs `parts` or `script` and
  neither exists at the cut point: a fixture the base binary could not read would have compared
  nothing. Over that surface — `--help`; `import` report and `--out`; `validate`; `inspect`; `check`;
  `tools add` twice and `tools list`; `toolpath` profile, pocket and engrave; `emit` twice to `--out`,
  once to stdout, and once on a document with no toolpath (exit 4); `simulate` at 1 mm and 0.5 mm on
  the profile, at 1 mm on the pocket and on the nesting; `nest` in place, re-sheeted onto a fresh
  board, and onto a board too small (exit 7); and the four error paths, unknown command, unknown kind,
  missing document and missing input — each invocation's exit code, stdout, stderr and the sha256 of
  **every file it left behind** were recorded after every step: **323 comparison points per side**,
  the 10 files the run produces (plus the fixture it is given) the 6 documents taken twice, as zip
  bytes and as their `document.json` payload, the two programs and the library byte for byte. **The
  only difference is `--help`**: 108 lines became **152**, **44 added and none removed** — `parts`,
  `cutlist`, `report` and `script` with their flags and the `SCRIPTS:` block that documents the
  language. Everything else is identical, exit codes included (21×0, 4×2, 1×4, 4×6, 1×7 on each
  side), and **no file hash differs anywhere**: `part.json` `80f7678…`, `tp-profile.json`
  `866eb8c…`, `nested.json` `7a76c77…`, `renest.json` `9682154…`, `profile.nc` and `nested.nc`
  `e4cc3d9…`, `engrave.nc` `6ec9830…`, `lib.sqlite` `eeb0b88…` on the cut point and on `HEAD` alike.
  The in-place `nest` of an already-nested document left it byte-identical on both sides as well, so
  M10 moved nothing on the pre-existing command surface — the `nesting`-reaching commands it added
  are additions, not changes, and the exit-7 board, the error paths and `--help`'s older entries all
  answer exactly as M9 did. Cleanup was refused, not skipped: `rm -rf` of the sandbox, the second
  `CARGO_TARGET_DIR`, the harness and the help diff in `/var/tmp` was blocked by the session's own
  command guard ("child safety blocked a recognized destructive command"), so what remains and
  where is `/var/tmp/camm10diff.1793835085` (the archived cut point with its own `CARGO_TARGET_DIR`
  and the three run directories, 427 MB), `/var/tmp/camm10cur-target` (433 MB), `/var/tmp/camm10-harness.sh`,
  `/var/tmp/.help.diff` and `/var/tmp/.camm10diff-sb`, all for the parent to delete; nothing of the
  harness is inside the repository, and `git status --short` shows this document and nothing else.
