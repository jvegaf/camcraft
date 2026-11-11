# M10 — documents

[Back to index](index.md)

### 4.13 M10 delivered (evidence)

M10 adds no machining at all: it is the milestone that answers how a person *drives* the document, and
the owner answered it four times (S1–S4, verbatim): "DSL de comandos, sin motor"; "Lectura y mutación
del documento"; "Import y export"; "CLI más panel mínimo". So the scripting surface is a **command
language** and not an engine — no Lua, no Rhai, no embedding — a script **reads and mutates one
document** and nothing else, the CSV of parts goes **both ways**, and the shell's part is one panel that
calls the same code rather than a second implementation.

What it cost is small in everything except lines moved. It changes **no post contract** and emits no
move. It bumps **no schema**: no file under `crates/cam-model` or `crates/cam-store` differs across
`681b722..152edd1`, so `DOCUMENT_SCHEMA` stays `c2d/document@2` and a document written before M10 is
read and written by the same code — the differential's six documents hash identically on both sides. It
adds **no external dependency**: `Cargo.lock` holds **482** packages before and after, and its only
movement is one line, `+ "cam-cli"` in `cam-ui`'s dependency list, because the shell now calls the
library. The milestone is **eight commits, eighteen files, 8 657 insertions and 3 037 deletions**, and
the deletions are the point: 3 294 lines left `crates/cam-cli/src/main.rs` for `src/lib.rs` in `3297f4d`
and were **not rewritten**. Normalising the new `lib.rs` — strip the `pub ` prefixes, drop `fn main` and
its one import — gives a file **byte-identical** to the old `main.rs`: 3 294 lines against 3 294, an
empty diff. That is **D-M10-1**: the crate named `cam-cli` is also the home of the operations the shell
calls, which reads oddly and is deliberate, because the alternative was moving thousands of reviewed
lines between crates to earn a better name.

The law the milestone is built on is that **the script is a caller, not a second parser**, and the parser
was its cheap half. Every subcommand's logic lived inside its argument-parsing arm, so it was extracted
first — `validate_report`, `nest_args` + `nest_document`, `toolpath_args` + `append_toolpath`, `emit_args`
+ `emit_document`, `simulate_args` + `simulate_document` — and the subcommand's arm and the script's
`Command` entry now hold the same two names. A flag therefore means one thing in both places, and the
parity is measured in bytes rather than promised: a scripted `nest` writes the document `cam-cli nest`
writes, a scripted `toolpath` appends the same toolpath and ids, a scripted `emit` the same program, a
scripted `simulate` the same report and exit, and a script's captured buffer is byte-identical to the
binary's stdout for the same script — asserted for a run that ends well and for one that ends in a
finding, with non-vacuity needles so an empty buffer cannot pass. `parts` produces 63 bytes of stdout and
a 534-byte document through either route, `cutlist` 321 bytes.

Diagnostics are located in `cam-post`'s shape, `line {line}:{column}: {message}`, the column being the
1-based byte column of the offending token, and the message is the **subcommand's own**:
`line 4:6: --gap must be a non-negative distance in mm, got -1` is `nest`'s sentence at `--gap`'s column,
because a flag-taking command hands its tokens to the shared parser and then locates the answer by the
token the message quotes or the flag it names. Every mutation passes **one** gate — the command runs on a
clone, `Document::validate` is asked, and only an accepted clone replaces the document — so a refusal
carries the script line *and* the model's words (`line 4:1: nesting part "P1" selects entity "E1", which
is an open contour…`) and the file on disk is untouched until `save`, which validates again as it writes.

| Deliverable | Where | Proof |
| --- | --- | --- |
| The command language: a six-line grammar, eleven words, located diagnostics and one mutation gate | `crates/cam-cli/src/script.rs` (`COMMANDS`, `lex`, `Session::mutate`) | 19 tests in `tests/script.rs`; a bad word is refused **with the vocabulary in the message** and at `line 2:1`, a bad argument count at the column past the last token, a bad value at the offending token's column (`line 4:6` for `--gap -1`), a shared parser's refusal located by its quoted token or named flag, a model refusal carrying the script line, a document that would not validate leaving the file byte-identical, comments and blank lines changing nothing, a path coming from the script or the command line and nowhere else, and `--help` listing the command and all eleven words |
| The extraction that makes a script a caller: five operations, two callers each, parity in bytes | `crates/cam-cli/src/lib.rs`, `tests/script.rs`, `tests/pieces.rs` | a scripted `nest`/`toolpath`/`emit`/`simulate` produces the subcommand's document, report, program and exit code byte for byte; the captured sink equals the binary's stdout for a successful and a failing script; the 157 pre-existing `cli.rs` tests were untouched and green through the extraction, and a differential against the base commit over `validate`, `inspect`, `check`, `tools add`, `tools list`, `toolpath`, `nest`, `emit` and `simulate` shows identical stdout, stderr, exit codes and byte-identical documents and library — the only differences being the new `SCRIPTS` help text and `script`'s own "no such file" where the base said `unknown command` |
| CSV parts **in**: four columns, a required header in a fixed order, and refusals that name the line | `crates/cam-import/src/csv.rs` (`COLUMNS`, `import`, `PartRow`) | 13 tests in `tests/csv.rs`; the header must name `name,width,height,quantity` in that order, so a **swapped width and height refuses at its own line and field instead of rotating every part**; quoting is refused by name; a duplicate name is refused naming **both** lines rather than summed, because `quantity` is already a column; blank lines and `#` comments are skipped anywhere while `part#1` stays a legal name; a row with the wrong field count, a dimension that is not positive and finite, a quantity that is not a positive integer, an empty name, an empty list and a non-UTF-8 byte each name their line, field, column and value; a BOM and CRLF endings are tolerated |
| Parts authored into the document and the cut list **out**, two formats and not one schema (S3) | `crates/cam-cli/src/pieces.rs` (`author_parts`, `cut_list_document`, `save_report`) | 10 tests in `tests/pieces.rs`; a part list authors one closed rectangle and one `NestingPart` per row, its `name` becoming the entity id because that is the only part name the model can hold, in one new layer named `--layer` or `Parts`, with `instances` from `quantity`; the cut list is `part,sheet,x,y,rotation,width,height` at **three decimals**, rotation in counter-clockwise degrees, one header row, one row per placed copy and one trailing newline; a duplicate name and a name the document already holds are each refused **at the row's line**; an import that would not validate is refused with the model's message *and* the CSV line; a document with no nesting section and one with parts but no placements are refused by the state they are in rather than written empty; both commands are reachable through the script and write the subcommand's exact bytes, and two runs of the same import and of the same cut list are byte-identical |
| The HTML print sheet, rendered by hand (S5) | `crates/cam-cli/src/printsheet.rs` (`print_sheet`, `escape_html`) | 8 tests in `tests/printsheet.rs`; the **whole sheet is an inline golden**, two runs are byte-identical, `--out` and stdout carry the same bytes, a hostile layer name and entity id (`<b>x</b>`, `</script>`) cannot inject markup because `&`, `<`, `>`, `"` and `'` are all escaped, a document with no toolpath still renders and says so, the nesting section prints the utilisation `Document::nesting_utilisation` derives, a document that does not validate is refused by name before a byte is rendered, and `--help` lists the command. Measured on a real document: **59 lines, 2 349 bytes**, the same sha256 on two runs |
| The library plus a 16-line binary, so the shell calls the same code (S4, D-M10-1) | `crates/cam-cli/src/lib.rs`, `src/main.rs`, `Cargo.toml` | the normalised `lib.rs` is **byte-identical** to the pre-split `main.rs` (3 294 lines against 3 294; the only deltas are 145 `pub` insertions, three `mod` → `pub mod`, one dropped import and the moved `fn main`); `Cargo.toml` did not change because cargo detects both targets; `cargo test -p cam-cli` **192** stayed 192 across the move, the binary still builds, `--help` exits 0, `postp validate` exits 0, and an unknown command still exits 2 |
| One exit for a report — a sink — and the minimal panel on top of it | `crates/cam-cli/src/script.rs` (`run_script_to`), `crates/cam-ui/src/script_panel.rs` | the engine has one exit, `run_script_to(&Path, &mut dyn Write, &mut dyn Write)`: reports to the first sink, the `save` notice to the second, every report through `crate::write_report`, and `cam-cli script` passing stdout and stderr so the binary's own split is preserved; two bypasses found before the panel was written are closed — a scripted `cutlist` without `--out` went straight to stdout and `save` used `eprintln!`, now one `pieces::save_report` shared with `parts_command`. `ScriptPanel::run` touches no egui and calls the library (never a process), `ui()` is the only function needing a frame, and four headless frame tests pin the four observable rules: the report painted, the located `line L:C` error painted, a failed run keeping the previous result, and the cut-list-plus-`wrote …` capture |
| No schema bump, no new external dependency | `Cargo.lock`, `Cargo.toml`, `crates/cam-model`, `crates/cam-store` | `git diff --name-only 681b722..HEAD` names no file under `crates/cam-model` or `crates/cam-store`, so the document format is untouched and `DOCUMENT_SCHEMA` stays `c2d/document@2`; `Cargo.lock` holds **482** packages before and after and moves **one line** (`+ "cam-cli"` in `cam-ui`'s list); no crate was added. The milestone's own invariant 6 was **wrong** as first written — "the lock must not move" is unsatisfiable for an edge between two workspace crates, because the lock records exactly that list — so it was amended to "no new external dependency, and the package count does not move", which is what it always meant |
| The differential against the milestone's cut point (T5) | `odd/tasks/018-m10-documents.md` L7 | **31 invocations per side** from a `git archive 681b722` copy built with its own `CARGO_TARGET_DIR`, the current binary first run twice in two directories so the harness was proven deterministic before it judged anything (both `diff -r` clean over all 31 steps), comparing **323 comparison points per side** — exit code, stdout, stderr and the sha256 of every file left behind, six documents taken twice as zip bytes and as `document.json`, the programs and the library. **The only difference is `--help`**: 108 lines → 152, **44 added and none removed** (`parts`, `cutlist`, `report`, `script` and the `SCRIPTS:` block); everything else is identical, exit codes included (21×0, 4×2, 1×4, 4×6, 1×7 on each side) and **no file hash differs anywhere** |
| The totals, measured | workspace | **1 506 tests, 0 failed**, in 110 `test result` lines (96 test binaries plus 14 doc-test suites, which hold none) against 1 452 at M9: `cam-cli` **194** (was 157), `cam-import` **136** (was 123), `cam-ui` **268** (was 264), and 54 of the new tests in the five new binaries — `script` 19, `pieces` 10, `printsheet` 8, `csv` 13, `script_panel` 4 |

**The review record is uneven, and it is recorded that way.** One lineage covers the language and the
CSV: **`review-bd99706da5d0c7f1`** took **3 800 lines**, tier high, three lenses (risk, readability,
reliability), correction budget 200, and ended **approved** with the receipt **burned** — it covers T1,
T2a, T2b and the `6471d5f` correction, and opened no correction of its own. Its predecessor
**`review-61e0b405dc7f498b`** was **superseded**, not approved: its refuter returned three findings that
were one defect (`R1-001` risk, `R2-001` readability, `R3-001` reliability) — `command_nest` parsed a
positional document and handed `None` to `refuse_document_destination`, so `open a` + `nest b` nested
**a** and a following `save b` overwrote **b** with a's document — fixed in `6471d5f` (**36** of the 200
authorised lines, one line of code plus the test) and then superseded because T2b was committed while it
was open: the frozen scope grew from **7 paths to 9**, the provider returned `recover` /
`scope_changed`, and the **owner authorised** the recover. The approved lineage emitted **nine advisory
findings**, all informational and none corrected (`bd99706d/R2-001` … `R3-003`, listed with their lenses,
severities and line locations in [docs/old/m10-documents.md](../../docs/old/m10-documents.md#the-review-record-and-what-it-does-not-cover)).

**What is not reviewed is stated as measured.** T3 (the print sheet), T3b (the crate split) and T4a (the
sink) were **not** reviewed: `assess` on `6471d5f..HEAD` measured `reviewDue: false` with
`reviewDueReason: under_budget` at **7 774 changed lines**, and again after T4a at **8 015**, both at risk
`medium` — that measurement is the record, and no disposition was invented on top of it. The print sheet
therefore rests on its 8 tests and its whole-sheet golden, the split on the mechanical byte-identical
proof and the 194 `cam-cli` tests the move did not touch, and the sink on its byte-parity tests and a
differential against the base that reports zero differences in stdout, stderr, exit code and produced
files. T4b was cut as its own candidate for the same reason M9 cut its geometry slice: the accumulated
range measures `reviewDue: true` (`high_risk`, `process_boundary` on `crates/cam-ui/src/lib.rs`) at
**8 770 lines over 14 paths**, past the reviewer budget, while T4b alone is **781 lines** on
`bbd2fe5..152edd1` — that lineage, `review-12219c3f04a57822`, tier high with the risk and reliability
lenses, returned `correction_required` on one **CRITICAL** finding: `ScriptPanel::run` decided by
`!text.is_empty()` instead of by the result, so a successful script that printed nothing left the
previous run's text on screen and a failure that had written part of a report overwrote it. The four
frame tests could not see it because they covered a non-empty success and an empty failure and nothing
between; the correction decides by the result, and its receipt was not burned when this record was
written.

Recorded deviations, each a decision in the feature document: **D-M10-1** makes the crate named
`cam-cli` the operations home the shell calls, rather than moving reviewed lines into a better-named
`cam-script` crate; **invariant 6** was amended from "`Cargo.lock` must not move" to "no new external
dependency and a package count that does not move" once a workspace edge made the original wording
unsatisfiable — the invariant was wrong, not the change (L6); the language is **eleven words and not
every subcommand**, so `inspect`, `check`, `import`, `postp validate`, `tools` and `report` are refused
by the vocabulary itself; a scripted command takes **no document path and no `--out`**, both refused by
name because a destination that was accepted and ignored still looks applied; `sheet` and `part` have
**no CLI counterpart**, so the model's `validate` is their only gate; a located error from a shared
parser is reconstructed from its message (quoted token, then named flag, then the command word) and
falls back to the word when a message names neither; a **refused write is now a refusal and not a
panic** — `write_report` maps a failed `write_all` to a named failure where `print!` used to panic, so a
report into a broken pipe exits `2` instead of aborting, and no test provokes `EPIPE`; the panel was a
**standalone module** with no shell tab in M10 and **is mounted now** — `Command::Script` opens it as a
modal over the canvas (`odd/tasks/022-script-panel-mount.md`) — and it runs **synchronously** on the frame
that pressed `Run`; the sheet **omits** the
program, the simulation and the file name and says so under its own `Omitted` heading; the two CSV
shapes are **two formats, not one schema**, so `cam_import::csv::import` deliberately does not read a
cut list back; and **`docs/old/format.md`'s command-line table is stale** from M9 onwards — it lists neither
`nest` nor M10's four commands and its exit table stops at `6` while `nest` exits `7` — recorded rather
than patched, because M10 changed no document format and the format document's own rows were not part
of this milestone's contribution. **Patched after M12**, when the audit went to fix it and found the
staleness itself stale: the table already carried all fifteen commands and the whole exit set, because
the milestones that added them filled their own rows. What was genuinely missing is what the patch
adds — `import`'s six trace and tolerance flags, and `tools add`'s `--name-format`.

