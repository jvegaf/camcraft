# Documents & scripting (M10)

M10 is the milestone that adds no machining. It answers a different question — how a person *drives*
the document — and it answers it four times, once per owner decision, each quoted verbatim:

- **S1**, "DSL de comandos, sin motor", so the scripting surface is a **command language** and not an
  engine: no Lua, no Rhai, no embedding, and no new dependency to carry one.
- **S2**, "Lectura y mutación del documento", so a script reads one document, mutates that document and
  writes nothing else: no ambient file, no environment, no network, no process.
- **S3**, "Import y export", so the CSV of parts goes both ways — parts **in**, the cut list **out** —
  and they are two shapes rather than one schema forced to lie.
- **S4**, "CLI más panel mínimo", so the surface is the command line first and one small panel second,
  and the panel is a **caller** of the same code, never a second implementation.

What it cost is worth stating in M9's terms. It changes **no post contract** and emits no move. It bumps
**no schema**: `git diff --name-only 955fc54..HEAD` names no file under `crates/cam-model` or
`crates/cam-store`, so `DOCUMENT_SCHEMA` stays `c2d/document@2` and a document written before M10 is
read and written by the same code — the differential's six documents hash identically on both sides of
the milestone. It adds **no external dependency**: `Cargo.lock` holds **482** packages before and after,
and its only movement is one line (L6).

The milestone is **eight commits, eighteen files, 8 657 insertions and 3 037 deletions** from `955fc54`
to `ba91f5e`, and the deletions are the milestone's most interesting number: 3 294 lines left
`crates/cam-cli/src/main.rs` for `crates/cam-cli/src/lib.rs` in `6cc3b2d` and were not rewritten, only
moved (§"D-M10-1"). Of the insertions, 117 are the feature document and 749/362/623 are the command
language, the CSV half and the print sheet.

The command, over a document that authors one closed 120 × 80 mm panel and has never been nested — a
real run, not an illustration:

```console
$ cat nest.cam
# two 120 x 80 panels per board, 5 mm apart
open panel.c2d
sheet 400 300
part 2 E1
nest --gap 5
cutlist
save nested.c2d
$ cam-cli script nest.cam
nest: 1 sheet(s) of 400.000 x 300.000 mm, 1 part(s), 2 instance(s)
options: gap 5.000 mm, rotation off
sheet S1: used
part: P1 rectangle, 2 instance(s)
placement: P1 on S1 at (0.000, 0.000), rotation none, 120.000 x 80.000 mm
placement: P1 on S1 at (0.000, 85.000), rotation none, 120.000 x 80.000 mm
algorithm: rectangle, maxrects
sheets used: 1
utilisation: 0.160
waste: 0.840
unplaced: none
part,sheet,x,y,rotation,width,height
P1,S1,0.000,0.000,0.000,120.000,80.000
P1,S1,0.000,85.000,0.000,120.000,80.000
wrote 1 layer(s), 1 entity(ies) to nested.c2d
```

Everything above the cut-list header is `nest`'s report, the seven-column block is `cutlist`'s, and the
`wrote …` line is the **notice** `save` sends to stderr — the binary's own stdout/stderr split, kept
because a command line expects a progress line on stderr (§"The sink"). `0.160` is not M10 arithmetic:
it is `Document::nesting_utilisation`'s answer, 2 × 120 × 80 = 19 200 mm² over the 400 × 300 mm² board
the placements name, and M9's one derivation read through a script.

| Piece | Where |
| --- | --- |
| The language: the eleven-word dispatch table, the lexer, and the located diagnostics | `cam_cli::script` (`COMMANDS`, `Session::mutate`, `lex`) |
| The five operations a script and a subcommand share | `cam_cli` (`validate_report`, `nest_args`/`nest_document`, `toolpath_args`/`append_toolpath`, `emit_args`/`emit_document`, `simulate_args`/`simulate_document`) |
| The part list's dialect: four columns, a fixed order and its refusals | `cam_import::csv` (`COLUMNS`, `import`, `PartRow`) |
| A part list authored into a document, and the cut list out | `cam_cli::pieces` (`author_parts`, `cut_list_document`, `save_report`) |
| The HTML print sheet, escaped by hand | `cam_cli::printsheet` (`print_sheet`, `escape_html`) |
| The minimal panel and its adapter boundary | `cam_ui::script_panel` (`ScriptPanel::run`, `ScriptPanel::ui`) |
| The library target the shell calls (D-M10-1) | `crates/cam-cli/src/lib.rs`, and the 16-line `src/main.rs` |
| The record | this document, and `odd/tasks/m10-documents.md` |

## What S1–S4 decided

The four owner answers are the whole specification M10 had. The decisions below are its reading, with
what pins each one.

| Decision | What it means | Where it is pinned |
| --- | --- | --- |
| The surface is a **command language**, not an engine (S1) | One command per line, a word and its arguments; `#` comments, blank lines ignored, `"…"` for an argument with a space — and no variables, no conditionals, no loops. A template engine for the sheet and a script engine for the language were refused for the same reason: the dependency would be a new reviewable surface | `COMMANDS` is the whole language; `the_help_lists_the_script_command_and_every_command_the_language_knows` |
| A script **reads and mutates one document** (S2) | `open` reads it, every command changes it, `save` writes it. A script cannot name a second document, cannot reach a path the script or the command line did not name, and cannot write a document that would not load | `a_path_comes_from_the_script_or_the_command_line_and_nowhere_else`, `a_mutation_that_would_not_validate_is_refused_and_leaves_the_file_alone` |
| The CSV is **both directions** (S3) | Parts in: one row, one part, no position. Cut list out: one row, one placed instance, no quantity | `csv`'s 13 tests, `pieces`' 10 |
| The surface is **CLI plus a minimal panel** (S4) | The command line is the contract; the panel runs one script and shows one area, through the library, never through a process | `cam-ui`'s four frame tests, and `D-M10-1` |

Two rules complete it. A word the table does not hold is refused **with the table in the message**
("expected one of open, sheet, part, nest, toolpath, validate, emit, simulate, save, parts, cutlist"),
so the vocabulary cannot be documented one way and dispatched another; and every mutation passes one
gate before it is kept (§"Located diagnostics").

## The command language, and its grammar

The whole grammar is six lines. There is no expression, no operator and no nesting because S2 asked for
"read and mutate a document" and never needed one.

```text
script    ::= line*
line      ::= blank | comment | command
command   ::= word argument*
argument  ::= bare | quoted
comment   ::= '#' to end of line            (only where a token would start)
quoted    ::= '"' … '"'                     (no escapes; ends at the next '"')
```

Whitespace separates tokens; a `#` at the start of a **token** begins a comment, which is why `part#1` is
a legal part name and a trailing comment is not a thing; a token that begins with `"` runs to the next
`"`, so a path with a space in it can be named; an unterminated quote is refused at the column it
opened. Columns are **1-based byte** columns of the token's first byte, which is what makes a diagnostic
point at the argument and not at the line.

Eleven words, each one the subcommand of the same name:

| Word | Line | What it runs |
| --- | --- | --- |
| `open` | `open <document>` | `cam_cli`'s own loader, the one every subcommand reads with — and only once, because a script mutates one document |
| `sheet` | `sheet <width> <height>` | Authors a `NestingSheet` with `fresh_sheet_id`'s scan, the rule `nest` uses for a board it opens |
| `part` | `part <instances> <entity>[,<entity>…]` | Authors a `NestingPart` with `fresh_part_id`'s scan |
| `nest` | `nest [--sheets <WxH>] [--gap <mm>] [--rotation on\|off] [--algorithm <mode>] [--step <mm>]` | `nest_args` + `nest_document` |
| `toolpath` | `toolpath --kind <kind> …` | `toolpath_args` + `append_toolpath` |
| `validate` | `validate` | `Document::validate` + `validate_report` |
| `emit` | `emit --post <file> [--library <path>]` | `emit_args` + `emit_document` |
| `simulate` | `simulate [--library <path>] [--resolution <mm>]` | `simulate_args` + `simulate_document` |
| `save` | `save <document>` | `cam_store::document::save_document`, which validates as it writes |
| `parts` | `parts --csv <file> [--layer <name>]` | `parts_args` + `author_parts` |
| `cutlist` | `cutlist [--out <path>]` | `cutlist_args` + `cut_list_document` |

`sheet` and `part` are the two commands with no subcommand behind them, and that is deliberate: a
script authors the records a nesting needs without a file for each. Their only gate is the model's own
`validate`, which is why the refusal tests around them mean something.

## The script is a caller, not a second parser

This is the milestone's law, and the parser was the cheap half of it. Every subcommand's logic lived
**inside its argument-parsing arm**, so a script that dispatched to "the same command" would have
reached either a copy of that logic or the arm's own printing. The extraction therefore came first:

* `validate`'s body became `validate_report(&Document)`;
* `nest`'s became `nest_args(&[String])` + `nest_document(&mut Document, &NestRequest)`;
* `toolpath`'s became `toolpath_args(&[String])` + `append_toolpath(&mut Document, &Path, &ToolpathArgs)`;
* `emit`'s became `emit_args(&[String])` + `emit_document(&Document, &Path, &EmitArgs)`;
* `simulate`'s became `simulate_args(&[String])` + `simulate_document(&Document, &Path, &SimulateArgs)`.

The subcommand's arm and the script's `Command` entry now hold the same two names, and there is no third
path. Two consequences are worth stating plainly. A flag means one thing in both places, because both
places call **the same flag parser** — a scripted `nest --gap -1` is refused by `nest`'s own sentence.
And a script cannot nest with different defaults, emit with a different post or simulate at a different
resolution, because those defaults live in the one parser and the one request struct.

The parity is a test, not a promise, and it is measured in **bytes**:

* `a_script_that_nests_writes_the_document_the_nest_subcommand_writes` — the same nesting through both
  routes produces the same document bytes.
* `a_script_appends_the_toolpath_the_toolpath_subcommand_appends` and
  `a_script_emits_the_program_the_emit_subcommand_emits` — the report and the program are identical,
  including the `TPn` ids the authoring command chose.
* `a_script_simulates_the_document_the_simulate_subcommand_simulates` — same report, same exit.
* `the_script_reaches_both_commands_and_both_write_the_subcommands_bytes` — `parts` produces 63 bytes of
  stdout and a 534-byte document through either route, `cutlist` 321 bytes.
* `a_scripts_report_reaches_a_sink_byte_identical_to_the_binarys_stdout` — a script's captured buffer
  equals the binary's stdout for the same script, for a script that ends well **and** for one that ends
  in a finding, with non-vacuity needles (`format: csv`, `nest: 1 sheet(s)`, `valid: `,
  `added TP2 profile`) so an empty buffer cannot pass.

## Located diagnostics, and the one gate

A script that fails says where, in `cam-post`'s parser shape — `line {line}:{column}: {message}` — and
names the token it is about. Three measured refusals, each from a run:

```console
$ cam-cli script bad.cam
error: line 2:1: unknown script command "shet"; expected one of open, sheet, part, nest, toolpath,
validate, emit, simulate, save, parts, cutlist
$ cam-cli script bad2.cam          # nest --gap -1 on line 4
error: line 4:6: --gap must be a non-negative distance in mm, got -1
$ cam-cli script open.cam          # part 2 E1, where E1 is an open contour, on line 4
error: line 4:1: nesting part "P1" selects entity "E1", which is an open contour: a part's footprint is
the area it encloses, so a part is nested from a closed contour
```

The second is the interesting one: the message is `nest`'s own, and the column is `--gap`'s, because a
flag-taking command hands its tokens to the shared parser and then **locates** the answer
(`ScriptLine::locate`): the column is the first token the message quotes with `"…"`, else the first
token that is a flag the message names, else the command word. It is exact for the shapes those parsers
emit and falls back to the word when a message names neither — declared in the feature document rather
than discovered later.

Every mutation goes through **one** gate, `Session::mutate`: the command runs against a **clone**, the
clone is asked `Document::validate`, and only an accepted clone replaces the open document. So a refusal
carries the script line **and** the model's words, and nothing reaches the disk until `save`, which
validates again as it writes. A script therefore cannot write a document that would not load, and a
refused script leaves the file byte-identical — `a_mutation_that_would_not_validate_is_refused_and_leaves_the_file_alone`
pins it.

## The sink, and why one exit

The engine printed straight to stdout, which made the panel impossible: a shell has somewhere to show a
report and nothing to catch one. The tempting shape was a second entry point beside the first, and two
implementations of one behaviour drift, so the sink became the **only** way out:
`cam_cli::script::run_script_to(&Path, &mut dyn Write, &mut dyn Write)` takes the two writers the caller
chose, `run_script` passes stdout and stderr, the panel passes two handles onto one buffer, and every
report leaves through `crate::write_report` — the single place bytes reach a sink.

The two streams are the binary's own contract: **reports** (`nest: …`, `valid: …`, the cut list, the part
list's report) are a command's data and go to stdout; the one **notice** (`save`'s `wrote …` line) is a
progress line and goes to stderr. `crates/cam-cli/tests/script.rs` pins both, and the differential found
the CLI's own bytes unchanged, which is why the notice stayed on stderr.

Two bypasses were found before the panel was written and closed with it: a scripted `cutlist` without
`--out` printed through `pieces::cut_list_document` directly, and `save` used `eprintln!`. Now the cut
list writes through `write_report` into the session's report sink, and `save`'s sentence comes from one
`pieces::save_report` — shared with `parts_command`, which said the same words — delivered through
`Session::notice`. The proof is a test that runs `open`/`sheet`/`part`/`nest`/`cutlist`/`save` through
`ScriptPanel::run` and asserts both the painted area and the captured text hold the cut-list header, its
row and the `wrote …` line. A panel that shows *most* of a run is worse than one that fails.

## The CSV dialect, and two formats that are not one schema

**In** is `name,width,height,quantity`, one row per part, millimetres, and the parser stops there:
turning a row into a `NestingPart` and a rectangle is `cam-cli`'s task, because that is where the
document, the entity ids and validation live. Each choice in the dialect has a reason that decides it:

* **The header is required, in that fixed order.** A list whose `width` and `height` were swapped is
  refused **at its own line and field** instead of silently rotating every part — which is the mistake a
  hand-written part list actually produces, and the one no downstream check could catch.
* **Quoting is refused by name.** Any `"` in a field is a located error. A dialect that half-supports
  quotes is worse than one that says it does not.
* **A duplicate part name is refused, not summed.** `quantity` is already a column, so summing would
  change the number of copies the list asked for and hide the copy-paste that produced the row.
* **Blank lines and `#` comments are skipped anywhere** — the script language's convention, not a second
  one — and there are no trailing comments, so a part may be named `part#1`.
* **A BOM and CRLF endings are tolerated**, because a part list is authored in a spreadsheet; a byte that
  is not UTF-8 is refused with its line, and a list with no rows is refused as nothing imported rather
  than handed back empty.

Every refusal names the line, and the field with its column and the value read:
`csv at line 2, field 3 ("height"): expected a positive finite number of millimetres, found "-0.5"`.

**Out** is `part,sheet,x,y,rotation,width,height` — one row per **placed instance**, three decimals,
rotation in counter-clockwise degrees, one header row, one trailing newline:

```csv
part,sheet,x,y,rotation,width,height
P1,S1,0.000,0.000,0.000,120.000,80.000
P1,S1,0.000,85.000,0.000,120.000,80.000
```

The two are **not one schema**, and the module comment says why in its own words: a part list **asks
for parts** (no position, no sheet, a `quantity`), a cut list **records placements** (a sheet, a
position, a rotation, one row per copy and no `quantity`). Forcing one shape on both would make one of
them lie. So `cam_import::csv::import` deliberately does not read a cut list back — see
§"Known limits" — and the two commands share the dialect (`\n` line ends, comma-separated fields, a
required header) and nothing else. A row's own `name` becomes the authored entity's id, because it is the
one name a part list carries and `Entity` has no name field; a name the document already holds is
refused **at the row's line** and never renamed.

## The print sheet, escaped by hand

`cam-cli report <document> [--out <path>]` writes one self-contained HTML file a person can print: the
job, the layers with their entity counts, every toolpath with its operation and authored parameters, the
tools it names, and the nesting with the utilisation `Document::nesting_utilisation` derives — M9's one
arithmetic, never a second. It reads no tool library, runs no post-processor and simulates nothing, and
what it therefore cannot say is listed under its own `Omitted` heading rather than left out quietly.

It is rendered **by hand**, one line at a time, for the same reason S1 refused a script engine: a
template engine is a dependency, and `Cargo.lock` is a reviewable surface. Three rules carry it, each a
test:

* **The document is escaped, not trusted.** Every id and name is user text and every one of them reaches
  the sheet through `escape_html`, which covers `&`, `<`, `>`, `"` and `'` — the five characters that can
  open a tag, close one, end an attribute or start an entity reference. A layer named `<b>x</b>` prints
  as those characters.
* **Two runs are the same bytes.** Nothing whose order the document does not fix is iterated (the one
  sorted collection is a `BTreeSet`), nothing is timestamped, and every number goes through `cam-cli`'s
  locale-free `decimal`. Measured on the sheet the script above produces: **59 lines, 2 349 bytes**, and
  two runs hash the same (`9e2a5b6d…`).
* **It refuses rather than lying.** A document that does not validate is refused by `cam-cli`'s own
  loader before a byte is rendered, a document with no toolpath still renders and **says so**
  (`no toolpath: the document holds none, so there is nothing to cut.`), and the `Omitted` section names
  the three things a sheet cannot honestly report — the program, the simulation, and the file name,
  "because the document does not record one, so two copies of one document print one sheet".

## The panel, and its adapter boundary

S4's "panel mínimo" is one module, and it is deliberately not an editor, a console or a debugging
surface. `cam_ui::script_panel::ScriptPanel` is a `Script` heading, a path field (`Script file`), one
`Run` button and an output area painted one line per row, with the located error in the danger colour.

The boundary is the whole design. `ScriptPanel::run` touches **no egui**: it reads the script, calls
`cam_cli::script::run_script_to` and keeps the text, so a caller without a frame — a test, the binary,
another shell — can run the same script through the same engine and read `output()` and `error()`.
`ui()` is the thin surface on top and the only function that needs a frame. No process is spawned: the
panel calls the library, so there is no path handling, no environment and no second document on the far
side of a pipe.

Three behaviours are rules rather than accidents. A failed run **keeps** the previous output visible and
paints the engine's own `line {line}:{column}: {message}` beside it, because a failure that clears the
area reads as "the run produced nothing". A run is **synchronous**, on the frame that pressed `Run`:
one script over one document, no queue, no progress state and no race for a span a frame cannot show. And
the report sink and the notice sink are two `Capture` handles onto **one** buffer, so the operator reads
the whole run in the order the script wrote it. Four headless frame tests pin the four observable
rules: the report painted, the located error painted with its line and column, a failed run keeping the
last result, and the cut-list-plus-`save` capture.

The module **is mounted now**: a palette command (`Command::Script`, in the SHOW AND SELECT group)
opens it as a modal over the canvas, `Esc` closes it, and the frame's canvas does not move — see
`odd/tasks/script-panel-mount.md` and §"Known limits" for what the mount does not solve.

## D-M10-1: the crate named `cam-cli` is also the operations home

The panel was unreachable as the task list first wrote it, and the finding is the milestone's second
requirement-level one (the first was that no CSV support existed anywhere: `grep -rln csv crates/` was
empty before T2a). `crates/cam-cli` had **no `[lib]` target** — it was `main.rs` plus modules — so
`cam-ui` could not depend on it, and spawning the binary instead would duplicate path and environment
handling and put a process boundary between the shell and its own document.

The design that looked obvious was a new `cam-script` crate. Designing it showed the engine is not the
only thing the shell needs: the **operations** it dispatches to (`nest_document`, `append_toolpath`,
`emit_document`, `simulate_document`, `validate_report`) lived in `main.rs` too, so a new crate would have
to absorb them or the shell would reimplement them — the second implementation invariant 1 exists to
prevent. The cheap honest shape is therefore a **library plus a thin binary**: `src/lib.rs` holds
everything the binary used to hold, `src/main.rs` is sixteen lines (collect the arguments, call
`cam_cli::run`, map the result to an exit code), `Cargo.toml` did not change because cargo detects both
targets, and `cam-ui` depends on the library.

It is a **move**, and the proof is mechanical: normalising the new `lib.rs` — strip the `pub ` prefixes,
drop `fn main` and its one import — gives a file byte-identical to the old `main.rs`, **3 294 lines
against 3 294, an empty diff**, with 145 `pub` insertions, three `mod` → `pub mod`, one dropped import and
the moved `main` as the only deltas. So **D-M10-1** is a recording of an oddity that is deliberate: the
crate named `cam-cli` is also the home of the operations the shell calls. The alternative was moving
thousands of reviewed lines between crates to earn a better name, and the name is not worth that.

## The review record, and what it does not cover

Three lineages touch this milestone, and the honest record includes the work none of them covered.

**`review-61e0b405dc7f498b` — superseded, and the recover was the owner's call.** It froze 2 607 lines
across 7 paths, tier high, three lenses, and its refuter returned three findings that were **one defect**:
`R1-001` (risk), `R2-001` (readability) and `R3-001` (reliability) — all the same line, because
`command_nest` parsed a positional document into `NestArgs::path` and then passed `None` to
`refuse_document_destination`, so `open a` followed by `nest b` nested **a**, exited 0, and a following
`save b` overwrote **b**. That is data loss with no message, and it broke both rules the language
documents: a script mutates the one document it opened, and an explicit argument is never silently
dropped. Fixed in `d76b26e` — **36 lines of a 200-line budget**, one line of code plus the test — with the
test copying the named document first and asserting its bytes unchanged afterwards, because a silent
success is exactly what an exit-code-only test would have let through the RED (which only became the bug
once the fixture had a board to nest onto; before that `nest` failed earlier for an unrelated reason).
The lineage was then **superseded**, not corrected to approval: T2b was committed while it was open, the
frozen scope grew from 7 paths to 9, the provider returned `action: recover` with
`action_disposition: scope_changed`, and the **owner authorised** the recover with the option the provider
named (`recover-scope-changed`).

**`review-bd99706da5d0c7f1` — approved, receipt burned.** It covered T1, T2a, T2b and the `d76b26e`
correction: **3 800 lines**, tier high, three lenses (risk, readability, reliability), correction budget
200. The refuter opened **no correction**, so the lineage ended `approved` with the receipt burned
(`native-approved-acknowledgement-completed`, authority `burned`), and it emitted nine **advisory**
findings, informational and later work, tagged by lineage:

| Finding | Lens | Severity | Location in the frozen candidate |
| --- | --- | --- | --- |
| `bd99706d/R2-001` | readability | WARNING | `crates/cam-cli/src/script.rs:202-207` |
| `bd99706d/R2-002` | readability | WARNING | `crates/cam-cli/src/script.rs:436-438` |
| `bd99706d/R2-003` | readability | SUGGESTION | `crates/cam-cli/src/pieces.rs:224-225` |
| `bd99706d/R2-004` | readability | SUGGESTION | `crates/cam-cli/src/main.rs:162-163` |
| `bd99706d/R2-005` | readability | SUGGESTION | `crates/cam-cli/src/script.rs:37` |
| `bd99706d/R2-006` | readability | WARNING | `crates/cam-cli/src/script.rs:129-131` |
| `bd99706d/R3-001` | reliability | WARNING | `crates/cam-import/src/csv.rs:175-193` |
| `bd99706d/R3-002` | reliability | WARNING | `crates/cam-cli/src/main.rs:860` |
| `bd99706d/R3-003` | reliability | WARNING | `crates/cam-cli/src/main.rs:3003` |

Six WARNINGs and three SUGGESTIONs, none blocking, none corrected. The three `main.rs` locations are the
frozen candidate's: `main.rs` became `src/lib.rs` at `6cc3b2d`, so the same code sits in a different file
and at different lines today, and `script.rs`'s own lines moved when T4b added the notice sink.

**`review-12219c3f04a57822` — the T4b slice, and it found a defect the frame tests could not see.** The
accumulated `d76b26e..HEAD` range measures `reviewDue: true` (`high_risk`, `process_boundary` on
`crates/cam-ui/src/lib.rs`) at **8 770 lines across 14 paths** — past the reviewer budget, the same wall
M7 and M9 hit — so T4b was cut as its own candidate: **781 lines** on `ff1b526..ba91f5e`, tier high, two
lenses (risk, reliability), correction budget 200. Its refuter returned `correction_required` on one
**CRITICAL** finding, `R3-001` (reliability): `ScriptPanel::run` decided whether to replace the output by
asking `!text.is_empty()`, which conflates "failed before it wrote anything" with "succeeded and printed
nothing" — so a successful script with no report left the *previous* run's text on screen with the error
cleared, and a failure that had written part of a report overwrote the output the panel's own rule says
it keeps. The four frame tests could not see it because they covered a non-empty success and an empty
failure and nothing between. The correction is one decision moved to the result instead of the buffer: a
success replaces the output even when it printed nothing, a failure keeps the previous output and shows
the diagnostic. Its receipt was **not** burned when this document was written.

**What is not reviewed is stated as measured, not softened into "reviewed".** T3 (the print sheet), T3b
(the crate split) and T4a (the sink) were **not** reviewed. `assess` on `d76b26e..HEAD` measured
`reviewDue: false` with `reviewDueReason: under_budget` at **7 774 changed lines**, and again after T4a at
**8 015 lines**, both at risk `medium`; those measurements are the record, and no disposition was
invented on top of them. So the print sheet rests on its 8 tests and a whole-sheet golden, the crate
split on a mechanical proof (the normalised `lib.rs` is byte-identical to the old `main.rs`) plus 194
`cam-cli` tests untouched by it, and the sink on its byte-parity tests and a differential against the
base commit that reports zero differences in stdout, stderr, exit code and produced files.

## Known limits, each a decision

- **The language has no expressions.** No variables, no conditionals, no loops, no arithmetic — S1 chose
  a command language and S2 named the state as the document, so nothing needed them. The limit is the
  feature: `COMMANDS` is eleven entries and the whole language.
- **The language is not every subcommand.** `inspect`, `check`, `import`, `postp validate`, `tools` and
  `report` are absent from the table, and `print_sheet` is `pub(crate)`, so a script cannot ask for a
  print sheet. Measured by reading `COMMANDS`: eleven words, and `--help` lists the same eleven.
- **A scripted command takes no document path and no `--out`**, both refused by name, because accepting a
  destination and ignoring it would leave a `--out` that was never applied looking applied. The first
  implementation of that rule had the defect above (`bd99706d`'s predecessor, corrected in `d76b26e`).
- **`sheet` and `part` have no CLI counterpart,** so `Document::validate` is their only gate — declared
  when T1 landed, and the reason their refusal tests are about the model's own sentences.
- **A located error from a shared parser is reconstructed from its message.** `ScriptLine::locate` takes
  the first token the message quotes with `"…"`, else the first token that is a flag the message names,
  else the command word. Exact for every shape those parsers emit today; a new message that names
  neither would be located at the word rather than at its argument.
- **A refused write is a refusal, not a panic — a behaviour change.** `write_report` maps a failed
  `write_all` to `Failure::input("cannot write the report: …")`, where `print!` used to panic, so a
  report written to a closed or broken pipe now exits `2` with a message instead of aborting with a
  panic (exit `101`). The change is deliberate — a caller holding a buffer must not inherit `print!`'s
  panic — and no test provokes `EPIPE`, which is the honest state of that claim.
- **The panel is mounted as a modal, and the artboards do not draw it (was: "a standalone module,
  not a shell tab").** The mount is `crates/cam-ui/src/chrome.rs::script_dialog` behind
  `Command::Script`, an `egui::Modal` on S9's precedent; a test pins that `output.canvas` is identical
  with the panel open and closed, so the frame's five zones are untouched. Two residuals stand, named
  rather than hidden: **the design authority still has no zone for it** — `design/system.md:18-22`
  enumerates five zones, `design/camui-shell/02-stages.html:358` says "Five zones and nothing else", and
  `grep -ri script design/` finds nothing, so the surface exists in the shell and not in the boards; and
  **the panel edits a file on disk, not the shell's document** — `run_script_to` takes no `Document`
  (`crates/cam-cli/src/script.rs:164`), so a script's mutations reach the shell only when the file is
  opened again.
  A third residual belongs to the mount rather than to the milestone: its review's refuter filed a
  **BLOCKER** against `egui::Id::unique` — that the constructor does not exist and the crate cannot
  compile — which is false and is contradicted by `egui-0.37.0/src/id.rs:91`, by two sibling call sites
  and by a green build; the spelling it demanded, `egui::Id::new`, is deprecated in 0.37, so the demanded
  correction would have failed `clippy -D warnings`. That lineage is left unburned in
  `correction_required` and closing it is an owner decision (`odd/tasks/script-panel-mount.md`, L6).
- **The panel runs synchronously on the frame that pressed `Run`.** Deliberate, and recorded in the
  module: one script over one document, no worker thread, no queue and no progress state. A script that
  took seconds would block the painter, which is a known cost rather than a surprise.
- **A scripted `nest` gets M9's engine, so the profile nester's search step is still the only cost lever.**
  M10 adds no nester and no step: the measured miss stands — a **100 × 100** board at step **20** puts a
  50 × 50 square on a second board although **`(45, 0)`** is legal, and step **5** finds it at
  `(50, 0)`. A script that wants the tighter answer passes `--step 5` and pays the search.
- **`cam_import::csv::import` deliberately does not read a cut list back.** Its header and its four fields
  are the part list's; a parser that accepted both shapes would have to guess which one it was handed.
  The two formats are documented side by side in `cam_cli::pieces` precisely so the refusal is a decision
  and not an omission.
- **The sheet omits what it cannot honestly report.** The program, the simulation and the file name are
  named in its own `Omitted` section — three sentences in a 59-line sheet — because it runs no
  post-processor, simulates nothing and the document records no file name.
- **`docs/format.md`'s command-line table is stale, and M10 did not close it.** It lists ten commands and
  neither `nest` (M9) nor M10's `parts`, `cutlist`, `report` and `script`, and its exit table stops at `6`
  while `nest` exits `7`. M10 changed **no** document format — no file under `crates/cam-model` or
  `crates/cam-store` differs in the range — so T6's edit surface for that document was the format, and
  nothing there moved; the gap is recorded here instead of being patched with rows this milestone did not
  earn. Its `nesting` row, added by M9's T6, is intact.

## Where the record lives

| Question | Where |
| --- | --- |
| What M10 had to be, and every decision behind it | `odd/tasks/m10-documents.md` — S1 … S6, T1 … T6, L1 … L7 |
| The language's grammar, its diagnostics and its help text | this document, and `--help`'s `SCRIPTS:` block (HELP is `crates/cam-cli/src/lib.rs`) |
| The part list's dialect in full | `crates/cam-import/src/csv.rs`'s module comment |
| The cut list's columns and formatting | `crates/cam-cli/src/pieces.rs`'s module comment |
| What the sheet renders, and what it refuses to | `crates/cam-cli/src/printsheet.rs`'s module comment |
| The panel's boundary and its three rules | `crates/cam-ui/src/script_panel.rs`'s module comment |
| Why `cam-cli` is a library (D-M10-1) | this document, and `odd/tasks/m10-documents.md` L4–L6 |
| The `nesting` section the cut list reads | [m9-nesting.md](m9-nesting.md) |
| How the simulation authorises a cut | [m5-simulation.md](m5-simulation.md#the-target-surface) |
| What M10 delivered, and what it cost | [odd/roadmap.md](../odd/roadmap.md) §4.13 |
| The document and post-processor contracts | [format.md](format.md) — its command-line table predates M10's four commands (see Known limits) |
| The project-level summary | [README.md](../README.md) |
