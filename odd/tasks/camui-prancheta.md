# camui-prancheta — the shell redrawn from full-resolution reference frames

Evidence: `design/prancheta-reference/` (15 frames, annotated). This file is the requirement. Branch
`feat/camui-prancheta` from `master` @ `a7a701e`. No subagents are configured in this repository, so every task
routes inline.

## Specs

**S1 — The shell frame follows the reference layout.** Five zones and nothing else: a title bar carrying the
document name, a left rail, a canvas with a ruler on its top and left edges, a right panel, and a status bar.
Evidence: every frame in `design/prancheta-reference/`.

**S2 — There is no stage bar.** The reference has no `Design → Edit → Toolpaths → Simulate → G-code` strip; the
workflow is carried by the rail and by the state of the task open in the right panel. The existing `stage_bar` is
removed. Owner's exact words: *"Sacarla, modelo Prancheta puro"*.

**S3 — All UI copy is English.** Owner's exact words: *"Inglés (mantener lo actual)"*. The reference is pt-BR;
what we take is structure, behaviour and layout, never its strings. `Usinagem` → `Machining`, `Percursos` →
`Toolpaths`, `Camadas` → `Layers`, `Encaixes` → `Snaps`, `Escolher ferramenta` → `Choose tool`.

**S4 — Left rail: five labelled collapsible groups.** `FILE` (two rows: new/open/save/save-as/export/settings,
then fit/import/import-height/export-height/undo/redo), `DRAW`, `EDIT`, `ORGANISE`, `SHOW AND SELECT`, each glyph
carrying its single-key shortcut letter where one exists. Below the groups, the brand block: the product name, the
crate version, and **the document's own last-write stamp, labelled `Saved …`, or `Unsaved changes` while the
document is dirty**. It is not a build stamp (L7).

**S5 — Two rulers in mm**, labelled along the top and left edges, the top one naming the current unit, with an
amber marker on both edges tracking the cursor. The empty document centres the origin in the stock
(`Screenshot 2026-11-03 031736.png`), so the rulers read negative to the left of and below it.

**S6 — Right panel: a ribbon of icon categories, then a task editor.** Categories in order `TOOLPATHS`,
`RELIEF AND 3D PART`, `MACHINE AND OUTPUT`, glyph only, one row per category. Under them the task editor: it
replaces the panel's content while a task is open, its header carries the task name with a `✓` and a `✕`, and
below it a `Layers` block that collapses on its own.

**S7 — Status bar in three zones**: live `X … Y …`; the snap chips (`Snaps`, `Point`, `Mid`, `Centre`,
`Grid 10`) where a chip both arms the snap and shows its value; and a contextual message on the right saying what
just happened (`New drawing`, `Material updated`, `Imported: 14 vector(s)`, `Tool: T22 …`, `Simulation finished`).

**S8 — `2D | 3D` is a segmented toggle at the top right of the canvas.** 3D is a mode: it splits the canvas into
a 2D half and a 3D half, and the 3D half carries an overlay row of `Material` `Vectors` `Toolpaths` `Tool`
checkboxes plus `Centre` and a `?`, a home button, an axis triad labelled `TOP` / `FRONT` / `RIGHT`, and a
playback bar (`▶`, speed, `To the end`, scrubber, `0:05 / 0:05  T22 · Drilling 1 · Z 5.000`).

**S9 — The tool library is a master-detail modal.** A search field; a tree grouped by tool family with a count and
one row per tool (mini tool glyph, name, dim second line `Ø6 · 2 cut(s)`); a detail form whose fields change with
the family, grouped under `GEOMETRY` / `PASSES` / `SPEEDS` / `MACHINE`; the tool drawn to scale with dimension
callouts; a footer of library actions above `Cancel` / `Use this tool`.

**S10 — A strategy's editor is the same surface for creating and editing**, prefixed `New toolpath · <name>` or
`Edit · <name>`. It holds the name field, the current-tool card with `Choose another tool…`, the collapsible
`Quick parameter edit`, then its sections, then the note `The toolpath is calculated when you click "Generate
toolpath".` and `Generate toolpath` / `Cancel`. A choice that needs a picture uses image cards
(`Screenshot 2026-11-03 034023.png`).

**S11 — The 15 reference frames live in `design/prancheta-reference/` with an annotated README.** Owner's exact
words: *"design/prancheta-reference/ + README"*.

## Tasks

| ID | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T0 | ✅ delivered — `design/prancheta-reference/`: 15 frames moved out of `Screenshots/` and annotated per frame | S11 | inline | `a265633` |
| T1 | ✅ delivered — `Stage`, `Readiness`, `counted`, `Shell::stage`, `Shell::set_stage`, `stage_bar` and `DrawnState::stage` all removed; the rail is the whole command table, and the two sentences worth keeping became `next_step(app)` on the Job tab. `palette.rs` pins one palette instead of five | S1 S2 | inline | `c9f3c59` |
| T2a | ✅ delivered — `Group` renamed to the reference's five (`File`, `Draw`, `Edit`, `Organise`, `ShowAndSelect`) with its words, and every command reassigned. FILE is deliberately not drawn yet: every file command still lives in the app bar, and a heading over no chips is a lie | S4 | inline | `fa179ab` |
| T2b-i | ✅ delivered — `Open`/`Save`/`SaveAs`/`Import`/`FitJob`/`Undo`/`Redo` moved into the rail's FILE group and out of the app bar, `Command::ALL` reordered so each group is contiguous in S4's order, and every heading folds its group on click | S4 | inline | `c22f57a` |
| T2b-ii | ✅ delivered — the brand block at the foot of the rail: the product name, the crate version, and the document's own `Saved …` stamp or `Unsaved changes` (L7, L10) | S4 | inline | `8a386f2` |
| T3 | Right panel: the ribbon of three categories, the task-editor container with `✓`/`✕`, and `Layers` as its own collapsible block | S6 | inline | — |
| T4 | Status bar in three zones, with the contextual message | S7 | inline | — |
| T5 | The two mm rulers with the amber cursor marker | S5 | inline | — |
| T6 | `2D \| 3D` toggle, split canvas, overlay row, axis triad and playback bar | S8 | inline | — |
| T7 | The tool library as a master-detail modal | S9 | inline | — |
| T8 | Strategy editors: current-tool card, quick parameter edit, image cards | S10 | inline | — |
| T9 | `design/system.md` and the artboards re-synced to the delivered shell | S1 | inline | — |

## Log

- **L1 (the request, verbatim).** "analiza las capturas que hice de la app prancheta, porque tenemos que
  implementar asi la ui"
- **L2.** Owner's three decisions, verbatim: *"Inglés (mantener lo actual)"*, *"Sacarla, modelo Prancheta puro"*,
  *"design/prancheta-reference/ + README"*. Recorded as S3, S2 and S11.
- **L3 (T1: the stage bar and the stage are gone).** `c9f3c59`, 1210 tests pass. What went: the `Stage` enum,
  `Readiness`, `counted`, `Stage::{ALL,label,note,readiness,count,hover,runs}`, `Shell::stage`,
  `Shell::set_stage`, `Shell::stage_bar`, `DrawnState::stage`. What replaced the one thing the strip did that
  was worth keeping: `next_step(app)` — the same two facts (no vectors on the sheet, no toolpaths in the
  document) asked of the *document* instead of the mode, and drawn on the Job tab. The rail now derives from
  `Command::ALL` alone, so `palette_runs` lost its filter and `palette.rs` pins one golden instead of five.
  `shell.rs` lost four tests that existed only to describe the strip and gained
  `the_frame_draws_no_stage_bar_and_the_rail_offers_every_group_at_once` and
  `the_job_tab_names_the_step_the_document_is_missing`.
  **On the slice size:** the commit is 727 deletions across six files, well past the ~400-line guidance, and
  it stays one commit on purpose — every prefix of this removal fails to compile, so a smaller slice would
  have to be a slice that does not build. The 1023-line count the runtime reports also carries T0's README.
- **L4 (delegation is unavailable, not skipped).** `subagent_list_agents` returns an empty list for this
  repository, so the evidence-budget rule, the writer rule and the context backstop have no configured target.
  Every task below routes inline and the fallback is recorded here rather than silently taken.
- **L5 (the first review: approved, and four advisories that are all about the evidence).** Candidate
  `a7a701e..30e130d` (T0+T1, 22 paths, 1115 lines), lineage `review-d5ba49d2146ee31a`, tier **high**, three
  lenses (`risk`, `readability`, `reliability`), authority **burned** (`gentle-ai.review-acknowledged/v1`).
  Cost: 3 model runs, ~99.7 kB of prompt each. **Approved with no blocking finding.**
  The tier was high for two reasons the provider named itself, and one of them is a defect I introduced:
  `design/prancheta-reference/Screenshot 2026-11-03 031627.png` landed with mode `100755`. A screenshot is not
  executable. **Fixed after the review** (`chmod 644`), because the review's frozen candidate cannot be edited
  and a permission bit is not a reason to reopen it — it is a reason to make the next commit clean.
  **Advisories, all non-blocking and all about the evidence rather than the change:** `R2-001` (readability,
  `shell.rs:361` — the new "no stage bar" test asserts the *absence of words*, which is the weakest thing a
  headless test can say), `R2-002` (readability, `shell.rs:481`), `R3-001` (reliability, `shell.rs:481-502` —
  the same test), and `R2-003` (readability, `README.md:100`).
  **Taken:** `R2-003`, because it was simply false — the README still said "Nothing yet" after T1 had already
  taken the correction. **Recorded and deliberately not taken:** `R2-001`, `R2-002` and `R3-001`, which are
  about how `the_job_tab_names_the_step_the_document_is_missing` and its neighbour are written, not about what
  the shell does. They are the follow-up this task leaves behind, and they must not be a reason to re-review
  this candidate: its receipt stands.
- **L6 (T2a: the five groups, and three things I refused to invent).** `Group` is now `File`, `Draw`, `Edit`,
  `Organise`, `ShowAndSelect` with the reference's own words (`FILE`, `DRAW`, `EDIT`, `ORGANISE`,
  `SHOW AND SELECT`), and `Command::group` reassigns every command: `Select` stays in DRAW because the reference
  draws the arrow as the first chip under DESENHAR, `FitJob` moves to FILE because S4 puts fit in the
  reference's FILE row, and `History` stops being a group because the reference keeps undo and redo in FILE.
  1210 tests pass.
  **Three things this task deliberately did not do, each for its own reason:**
  1. **FILE is not drawn in the rail yet.** Moving `Open`/`Save`/`SaveAs`/`Import`/`FitJob`/`Undo`/`Redo` into
     `in_palette` would have made the rail and the app bar offer the same seven commands twice, which is a
     defect, not a step. The move and the app bar's corresponding removal are T2b, together, in one commit.
  2. **No single-key letters.** S4 says every glyph carries its shortcut letter, and the reference does. This
     shell has exactly four bindings — `Cmd+Z`, `Shift+Cmd+Z`, `Delete`/`Backspace` and `F` — so there are four
     letters to draw and no others. A letter under a chip that no key actually runs would be the same lie as a
     count nobody measured. The full single-key map changes the canvas input model, not chrome, and it is its
     own decision.
  3. **The `Updated on …` line is not drawn.** In the reference it sits in the brand block beside an info icon,
     on a fresh untitled document, which makes it a *build* stamp rather than a document stamp. Producing one
     needs a build script and a timestamp in the binary — a reproducibility question about delivery (Task Size
     risk item 5), not a rail question. Until the owner says which stamp that line carries, the brand block
     carries the name and the crate version only.
  The group words reached the F1 help panel for free: it already walks `Group::ALL`, so FILE now heads its
  commands there without a second list.
- **L7 (the `Updated on …` line stamps the document, not the build).** The owner's decision, verbatim:
  *"documento"*. The reference's line is ambiguous — on a fresh untitled document, beside an info icon, it could
  be the build, the document or the tool library — and copying an ambiguity is not fidelity. A build stamp would
  need a `build.rs` and a timestamp in the binary (delivery risk item 5, and it breaks reproducible builds), and
  it would be strictly worse than the version string already in the block, which is comparable where a timestamp
  is not. So the line says `Saved <date> <time>` and `Unsaved changes` while the document is dirty, and **the
  label changes from the reference's `Updated on` to `Saved` so it says what it stamps**.
  **Cost, named before it is paid:** formatting `SystemTime` as `2026-11-03 19:30` without `chrono` or `time`
  means civil-from-days by hand — roughly thirty lines, testable, no new dependency. If that turns out to be a
  bad trade during T2b, the fallback is the brand block with the name and the version only, and no line.
- **L8 (T2b split in two, and the half that landed: FILE in the rail, and foldable groups).** `Command::ALL`
  now keeps every group contiguous and arrives in `Group::ALL`'s order, so the rail's sections are `FILE`,
  `DRAW`, `EDIT`, `ORGANISE`, `SHOW AND SELECT` — before this they came out as DRAW, ORGANISE, EDIT, VIEW,
  because the table's File commands sat in the middle and `in_palette` filtered them all out. `in_palette`
  lets the whole FILE group in, and the app bar **lost every command of its own**: the undo/redo pair, the
  Open/Save/Save As run and Fit the job are gone from the bar, which now holds the document's name, the
  units, the theme, the help panel and the `2D | 3D` switch. `DrawnState::app_bar` is still there and is
  asserted **empty**, so a duplicate button cannot come back without failing a test.
  **A heading folds its group.** `Shell::group_open` / `Shell::toggle_group` hold the fold; `DrawnState::
  palette_headings` reports where each heading was drawn, because the heading is a control and a test should
  click `FILE`, not set a field the operator would have set by clicking. The caret is painted as a triangle
  rather than written as `▾`: a group whose affordance renders as a tofu box is worse than no affordance.
  **Tests:** the palette golden is re-measured with FILE first; `the_rail_headings_are_the_references_five_in_its_order`,
  `the_app_bar_offers_no_command_of_its_own` and `folding_a_group_hides_its_chips_and_keeps_the_rest` are
  new; the four app-bar click tests became rail click tests (`the_rails_open_chip_asks_the_host_for_a_file`,
  `the_rails_file_chips_still_emit_their_commands`, `the_rails_save_chips_emit_their_actions`), and
  `the_palette_shows_no_history_run` became `the_palette_shows_all_five_runs` because the run it forbade is
  now the point. 184 cam-ui tests pass.
  **Why the brand block is not in this commit.** L7's stamp needs a fact the shell does not hold yet — when
  the document was last written — and a civil date formatter without `chrono`. Both belong to the rail's
  foot and neither is needed by the move, so they are T2b-ii and this commit is the surface change alone.
- **L9 (an environment incident, so the next session does not read it as a regression).** Linking any test
  binary in this session failed with `ld terminated with signal 6` and `LLVM ... Resource temporarily
  unavailable`, from `lld`'s thread pool, across `-j` 1 and 2 alike while `nproc` reports 1. Every test run
  here was therefore made with `RUSTFLAGS="-C link-arg=-Wl,--threads=1"`, which links and runs normally. It is
  a host/toolchain condition and not a property of this branch: `cargo check --all-targets` never failed.
  The workaround is **not** committed to any config file, because a one-thread linker is not something this
  repository should inherit.
- **L10 (T2b-ii: the brand block, and two things it settled that L7 left open).** The rail's foot now carries
  `campilot`, `v<crate version>` and the document's own stamp: `Saved 2026-11-03 19:30 UTC` while the drawing
  matches its file, `Unsaved changes` while it does not. **The stamp is the file's mtime**, read once per
  action that opens or writes a document — never per frame, and never `SystemTime::now()`, which would say
  "saved" about a file nobody wrote. `Shell::saved_stamp` is the one sentence the panel paints and a test
  reads, so the two cannot drift.
  **The clock is UTC and the line says `UTC`.** L7 asked for a formatter without `chrono` or `time`, and
  local time needs a timezone database or `libc`'s `localtime_r` — a new dependency, which is delivery risk
  item 5. Printing UTC without saying so would be wrong by hours in exactly the case the line exists for, so
  the suffix is part of the string. `crates/cam-ui/src/stamp.rs` is Hinnant's civil-from-days, ~60 lines with
  four tests: the epoch, the 2000 leap day and the day after it, the 2026 example L7 wrote down, and one
  second before the epoch — the case truncating division would print as `1970-01-01 -00:01`.
  **The layout lesson, paid for twice in this session.** The block was first drawn by wrapping the rail in
  `Layout::bottom_up`, which put the brand at the foot and silently broke every click on the rail: inside a
  bottom-up layout the scroll area's content is laid out from the bottom, so the point a frame reported was
  not the point the next frame drew the chip at — three rail click tests and the seven-commands test failed
  with actions arriving from the *neighbouring* chip. It is a bottom `Panel` now, which reserves its height
  before the chips are laid out, and the coordinates hold still. Nothing about this is visible in a
  screenshot, which is why it is written down here rather than in a comment that would be deleted by the next
  refactor.
  **Tests:** 190 in cam-ui. New: `the_rail_foot_names_the_product_and_says_the_work_is_unsaved` and
  `opening_and_saving_stamp_the_brand_block_with_the_files_own_write` (open a real `.c2d`, check the stamp is
  the file's mtime, edit, check `Unsaved changes`, save, check the stamp followed the write).
- **L11 (the slice boundaries, and what delivery is not).** The branch now holds four slices, each one a commit
  boundary and a review unit: `a7a701e..30e130d` (T0+T1, already reviewed and approved — L5, lineage
  `review-d5ba49d2146ee31a`), `fa179ab` (T2a), `c22f57a` (T2b-i) and `8a386f2` (T2b-ii). 2 125 authored lines
  over `origin/master` in eight commits, against the ~400-line guidance per slice: the boundaries above are the
  answer to that, not one diff to review in a sitting, and the delivery shape (chained PRs, one per slice) is
  the owner's decision. Nothing is pushed and nothing is merged from here.
