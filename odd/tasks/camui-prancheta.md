# camui-prancheta — the shell redrawn from full-resolution reference frames

Evidence: `design/prancheta-reference/` (15 frames, annotated). This file is the requirement. T0-T4 landed on
`feat/camui-prancheta` from `master` @ `a7a701e`, merged into `master` when T4 closed; **from T5 on, each task
gets its own branch from `master`** (L18). T0-T4 routed inline because no subagents were configured here; that
premise expired (L19), so T5 on routes to a configured worker.

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
| T3 | ✅ delivered — the ribbon of three glyph categories, the task-editor container with its name and painted `✓`/`✕`, and `LAYERS` as its own folding block; the selection's numbers became the panel's idle state | S6 | inline | `38ea6dd` |
| T4 | ✅ delivered — the bar's three zones: `X … Y …` with the zoom and the undo/redo steps, the five snap chips where `Grid` names its step, and the last action's sentence at the right end | S7 | inline | `482625b` |
| T5 | ✅ delivered — two 22 px bands (`ruler.rs`) numbering the world coordinates in mm with `mm` on the top one, an amber `Role::Cut` marker on both edges while the pointer is on the sheet, and the drawing area inset so a press on a band is chrome (L20) | S5 | worker | `7cb26ae` |
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
- **L12 (the second review: approved, authority burned, and the sandbox that nearly stopped it).** Candidate
  `a7a701e..e305ce5d` (25 paths, 2 154 lines, T0 through T2b-ii), lineage `review-c38fa3367409c3bb`, tier
  **high** for `process_boundary` (`crates/cam-ui/src/lib.rs` starts subprocesses), three lenses (`risk`,
  `readability`, `reliability`), correction budget 200 — **unused, no correction was required**. Three reviewer
  runs over the pi host relay (169.7 kB of prompt each), **approved on the last admitted event**, authority
  **burned** (`gentle-ai.review-acknowledged/v1`, revision `sha256:27875908…`). Delivery stays ordinary
  repository policy: nothing pushed, nothing merged.
  **Eight advisory findings, all `informational`, none blocking**, so this receipt stands and none of them is a
  reason to re-review: `R2-001` (`chrome.rs:1537-1540`), `R2-002` (`tests/shell.rs:3542`), `R2-003`
  (`design/prancheta-reference/README.md:111`), `R2-004` (`stamp.rs:72-77`), `R3-001` (`help.rs:419-420`),
  `R3-002` (`chrome.rs:1540-1547`), `R3-003` (`help.rs:417`) and `R3-004` (`lib.rs:1161`). They are the
  follow-up this feature leaves behind, together with L5's three.
  **Two START attempts failed first, and the cause was the harness, not the candidate.** Both died in the
  pre-native phase (`git_command_failed`) with `fatal: grep: failed to create thread: Resource temporarily
  unavailable`; `mutation_performed: false` both times and `repair.counts.lineages = 0`, so no authority was
  created and nothing was burned. The root cause of L9 is now known and was the same thing: the gentle-shell
  service cgroup ran with **`pids.max = 64`** and 47 PIDs in use (Firefox 129 threads, Hyprland 70, Android
  Studio 110, Ghidra 54, opencode 54), so any child that wanted a thread pool — `lld` linking a test binary,
  the native `git grep` — died on `pthread_create`. With the owner's explicit authorisation the limit was raised
  to 512 for that one cgroup, and the same START then succeeded on the first attempt. **The linker workaround in
  L9 is therefore a symptom of the same limit, not a property of the toolchain**, and a session that starts with
  `pids.max` back at 64 will see both failures again.
- **L13 (T3: the ribbon, the task editor, and four things the panel had to decide).** `38ea6dd`, 110 shell tests
  and the whole workspace green. The tab row is gone: `TOOLPATHS`, `RELIEF AND 3D PART` and `MACHINE AND OUTPUT`
  are glyph rows (the profile toolpath, the relief, the post-processor — three glyphs the set already had, and no
  new symbol drawn for a row nobody has seen), and opening one puts a task editor over the body with a header
  carrying the task's own name and a `✓` / `✕`. `LAYERS` moved out of the panel's furniture and became its own
  folding block below the editor, so it survives every task opening and closing above it.
  **Four decisions this task had to make, none of them in S6:**
  1. **`Selection` is not a category.** The reference draws three, and the shell had four tabs. The selection's
     numbers are not a task anybody opens — they are what the panel says while nothing is open — so they became
     the panel's **idle state**, drawn when `Shell::task` is `None`. Nothing the three tabs reached was lost:
     `TOOLPATHS` holds the authoring body, `MACHINE AND OUTPUT` the job's, and the idle panel the readout.
  2. **`RELIEF AND 3D PART` opens a state, not a form.** The reference opens a 3D machining strategy there; this
     shell authors no relief, and the 3D half plays a program `cam-cli` produced. So the task says exactly that,
     names the split's state and whether a program is loaded, and toggles the split. A form over a capability
     that does not exist would be the same lie as a count nobody measured (L6). The overlay row, the triad and
     the playback bar are T6's, and the task says so rather than pre-empting them.
  3. **`✓` and `✕` differ in exactly one thing.** Every number in the authoring body applies as it is typed, so
     `Done` has nothing to commit; the one draft the panel holds is the material's (`material_size`, written only
     by the dialog's own apply), so `Cancel` puts it back to the document's stock and `Done` keeps it. The
     tooltips say that, because two glyphs that did the same thing would be a control that lies.
  4. **`Tab` survives as the body selector.** `Shell::set_tab` now opens the tab's category and `tab()` derives
     from what is open, so a host keeps one call and the panel keeps one answer. The enum was not deleted
     mid-feature: `design/camui-shell/01-shell-2d.html` still draws tabs, and re-drawing that artboard is T9 —
     deleting the type now would leave the artboard describing something the code cannot express.
  The two glyphs are **painted**, not typeset, for the same reason the rail's caret is (L8): `✓` and `✕` are a
  font bet and an affordance that renders as a box is worse than a drawn mark.
  **A reporting defect the tests caught and this task fixed:** the panel reported its state *before* handling the
  click, so the frame that folded `LAYERS` still drew its rows and the frame that closed a task still named it.
  Both answers are now read after the click, which is what makes "a folded block draws no row" true in the frame
  it happens rather than one frame later.
  **Two PRESERVE tests changed shape, not meaning:** `the_tools_tab_offers_every_authoring_command` and
  `the_palette_and_the_tools_tab_build_the_same_seven_actions` now open the `TOOLPATHS` task first, because the
  authoring body is one click further from the idle panel than it was. Their assertions are untouched.
- **L14 (the first correction: the `Import` chip was a command the rail could not run).** The review of the T3
  candidate rejected it: `R3-001` (CRITICAL, reliability, `help.rs:419`) — T2b let `Import` into the rail because
  S4 lists it in FILE, but this shell has **no import route**. `ImportFile` is what a host applies after its own
  dialog, and `Command::action` answers `None` for `Import`, so the chip was reachable and emitted nothing. The
  refuter accepted it as candidate-caused and the correction is one line of `in_palette` plus the golden that
  pinned the chip: `Import` stays out of the rail until a host route exists, and the refusal is written down in
  `in_palette`'s own doc comment so the next person does not re-add it from the frame. **The lesson is L6's,
  learned twice:** an entry copied from the reference's list is a claim that the command works, and a command
  with no action is the same lie as a count nobody measured.
- **L15 (the T3 review: one correction, then approved and burned).** Candidate `ad387109…` on lineage
  `review-5b477569a0cfb177`, tier **high**, three lenses, three reviewer runs (204 kB of prompt each). The
  refuter was **not** a formality: it accepted `R3-001` as candidate-caused and **CRITICAL**, which put the
  review in `correction_required` and spent part of the bounded correction budget (22 of 200 diff lines, the
  one correction this authority allows — L14). After the correction a targeted validator ran, and the review
  came back **approved** (revision `sha256:944295df…`), authority **burned**
  (`gentle-ai.review-acknowledged/v1`), candidate `61ba0757…`.
  **Nine more advisories, all `informational`, none blocking** — the receipt stands and none of them is a reason
  to re-review this candidate: `R2-001` (`chrome.rs:1823`), `R2-002` (`chrome.rs:1836`), `R2-003`
  (`tests/shell.rs:3551`), `R2-004` (`tests/shell.rs:4343`), `R2-005`
  (`design/prancheta-reference/README.md:110`), `R2-006` (`chrome.rs:1490`), `R2-007` (`lib.rs:1183`), `R3-002`
  (`chrome.rs:692`) and `R3-003` (`chrome.rs:190`). They join the eleven this feature already owes.
  **One process note for the next session:** the whole-branch projection means every review re-reads T0
  onwards, and a 3-lens pass over 2 839 lines costs roughly 600 kB of prompt. The slices in L11 are the answer;
  until delivery splits them, each new work unit pays that bill.
- **L16 (T4: the three zones, and the two things that left the bar to make room).** `482625b`, **1226 tests
  pass workspace-wide** (114 of them in the shell target). The bar is the reference's three zones: `X … Y …`
  on the left with the zoom and the steps available in each direction, the five chips in the middle, and the
  sentence the last action produced at the right end.
  **The chips are the reference's words, and the grid chip names its step.** `Encaixes | Ponto | Meio |
  Centro | Grid 10` → `Snaps | Point | Mid | Centre | Grid <step>` (S3's mapping, S7's list). `Snaps` is the
  magnet — the strip armed or disarmed at once — and each kind keeps its own chip. The grid chip reads the
  shell's own `grid_spacing`, so the number it names is the number `cam_view::pick` snaps to, formatted by
  `grid_chip` (`Grid 10`, `Grid 2.5`); with no step yet — the first frame, before the canvas has measured the
  camera — it says `Grid —`, because `Grid 0` would name a snap that can never fire.
  **A click is applied in the frame it lands in.** The old bar collected its toggles and applied them after the
  panel closed, which is L13's reporting defect one layer down: the frame that turned a chip on was not the
  frame that could say it was on. Each chip now mutates the shell where it is clicked, and the bar reports
  `DrawnState::status_chips` — word and pressable point — so a test presses what the frame drew instead of
  guessing at the layout, the contract `palette_hits` and `category_hits` already keep.
  **Two things left the bar, each for its own reason:**
  1. `saved` / `modified`. The rail's brand block already names *when* the document was written (`Saved
     2026-11-03 19:30 UTC`, `Unsaved changes` — T2b-ii); the bar printed the same fact a second time, without
     the moment. What stays is the two facts nothing else on the frame reports: the zoom and the steps
     available in each direction. The test that pinned the save state in the bar is now
     `the_status_bar_reports_zoom_and_history`; the save state keeps its own test in the brand block.
  2. The right end stopped belonging to the gesture sentence. S7 gives it to what just happened, so the
     message is painted first and keeps its width, and the hint truncates behind it. The hint was already a
     `truncate()`d label, so the behaviour is not new — but it is now the first thing a narrow window clips,
     and a long refusal in the bar eats most of it. **That is a decision, not a measurement:**
     `galley.text()` returns the source string, not what the eye sees, so a headless test cannot tell a whole
     sentence from a clipped one, and a test claiming to would be `R2-001`'s weakness with a new name. The
     three tests on the sentence's content stay; the truncation is recorded here instead of pretended to be
     tested.
  **Slice:** 253 authored lines across two files, under the ~400-line guidance, and one commit because the two
  report fields, the paint and the tests describe one rule. It is the seventh slice on this branch (T0+T1,
  T2a, T2b-i, T2b-ii, T3, T4); the whole-branch projection still means every review re-reads from `a7a701e`,
  and splitting the delivery into chained PRs remains the owner's decision (L11).
- **L17 (the T4 review: two harness failures, then a narrowed candidate and an approval).** Lineage
  `review-ba4913842ce49c58`, tier **high** (`process_boundary` — `crates/cam-ui/src/lib.rs` starts
  subprocesses), two lenses (`readability`, `reliability`), 3 paths, 341 original lines, correction budget 171
  — **unused, nothing to correct in the frozen candidate**. Two reviewer runs over the pi host relay (33.6 kB
  of prompt each), **approved**, authority **burned** (`gentle-ai.review-acknowledged/v1`, revision
  `sha256:a5854c53…`). Delivery stays ordinary repository policy: nothing pushed, nothing merged.
  **The first START never reached the provider.** It died `pre_native` with `git_command_failed` — `git grep
  -I -l -z -i -E ^#! <tree>` → `fatal: grep: failed to create thread`, exit 128 — because the gentle-shell
  cgroup was back at **`pids.max = 64` with 48 PIDs in use**. `mutation_performed: false`, no lineage, nothing
  burned: the same incident as L12, and L12's prediction that a session starting at 64 would see it again was
  exact. With the owner's authorisation the limit was raised to **512** for that one cgroup, and **restored to
  64 as soon as this review closed**.
  **The second START failed the budget instead.** With the pids limit gone, the whole-branch projection
  (`origin/master..HEAD`: 25 paths, ~3 200 lines) came back `lens_context_budget_exceeded` in **preflight** —
  no authority created, nothing to repair. This is L15's process note turning into a hard stop: past roughly
  this size the candidate is **no longer reviewable at all**, whatever the lens count. The contract's own
  remedy is to reduce the scope, so the third attempt named the candidate explicitly — `baseRef 523edd9`,
  `committedOnly: true`, i.e. **T4 and its doc and nothing else**, the delta no earlier receipt covers (T0+T1 is
  L5, through T2b-ii is L12, T3 is L15). It created the lineage on the first try.
  **Consequence for delivery, and it is no longer advisory:** from here on a receipt is only obtainable per
  narrowed committed range, so the chained slices L11 and L16 asked the owner to decide about are now the only
  shape that can be reviewed. The alternative is a branch that grows without receipts.
  **Four advisories, all `informational`, none blocking:** `R2-001` (`lib.rs:2982`), `R2-002`
  (`lib.rs:2992-2995`), `R2-003` (`lib.rs:1830-1832`) and `R3-001` (`lib.rs:2992-2997`). They join the twenty
  this feature already owes.
  **Two of them were the same defect and it was mine, so it was taken immediately:** inserting `grid_chip`
  anchored on `fn apply_command` split that function's doc comment in two, leaving `apply_command`
  undocumented and the grid chip wearing both contracts. `9d7bc11` moves the doc block back. A function whose
  documentation describes another function is worse than an unreviewed follow-up — but **`9d7bc11` is not
  covered by this receipt**, and it is written down here so the next candidate knows it.
- **L18 (the owner's delivery decision: one branch per task).** The owner's exact words: *"mergea, limpia y
  arrancamos una rama por task"*. That settles the question L11, L16 and L17 kept handing back: the merged unit
  is T0-T4 (18 commits, `a7a701e..c7b30d3`) into `master`, **locally — `origin/master` is untouched, and a push
  stays the owner's decision**; the feature branch is deleted with the merge, because a merged branch is not a
  place to keep working; and from T5 on every task gets its own branch cut from `master`.
  **What this buys, measured rather than hoped:** each task's review candidate becomes `master..HEAD` at the end
  of its own branch — the T4 candidate was 341 lines and cost ~67 kB of reviewer prompt, an order of magnitude
  under the whole-branch pass that L17 found unreviewable. The narrowing argument T4 needed (`baseRef` to a
  commit three slices back) stops being necessary.
- **L19 (L4's premise expired: subagents are configured now).** `subagent_list_agents` returns ten targets
  today, including `gentle-ai-explore`, `gentle-ai-verify` and `gentle-ai-worker`; when L4 was written it
  returned an empty list. The record is corrected rather than quietly rewritten: **T0-T4 did route inline and
  that was true when it was written**, and **T5 on routes to the configured worker**, with the parent keeping
  the feature document, the commits and the review candidates.
- **L20 (T5: the two rulers, and the three things an independent read caught).** `7cb26ae` (code) and this
  slice's correction, workspace green at **1239 tests**. `crates/cam-ui/src/ruler.rs` (new) draws the two 22 px
  bands from three palette roles — `Panel` for the band, `TextDim` for the ink, `Cut` for the marker — stepping
  by `cam_view::nice_spacing` at the grid's own target and numbering the world coordinate the camera puts
  under each pixel, so the stock's centred origin reads negative to its left and below it. `Shell::canvas`
  insets the drawing area by `Metrics::ruler_thickness` before the camera viewport, the click/drag response and
  the painter's clip rect. **The task routed to a configured worker** for the first time (L19), and the
  handoff named the document, the task and S5 rather than restating them.
  **The independent verify could not run a single test.** Its cgroup sat at 63/64 PIDs and 3.0/3.2 GiB, so
  `rustc` died with `install_ctrlc_handler: WouldBlock` before building a crate; every verdict it returned was
  static, and it said so. The executed evidence is the parent's: 1238 tests at `7cb26ae`, 1239 after the
  correction. What the static read was worth: three findings, none cosmetic, all taken.
  1. **One new test could not fail.** `the_drawing_is_still_framed_inside_the_rulers` checked the document's
     corners against a rect derived from `output.canvas`, and the fit's 32 px padding is wider than the 22 px
     band, so a fit measured against the *un-inset* area would have passed it. It now requires the drawing
     area to start exactly where the band ends and the fit's padding to be measured from that area. **Observed
     RED:** dropping the inset from `CanvasRect` fails it with `canvas.y 40 against band (200.0, 40.0, 712.0,
     22.0)`.
  2. **One assertion locked in a framing accident.** `the_ruler_numbers_are_the_world_coordinates_under_them`
     ended with `left.last() == Some(0.0)` — true only because this stock is taller than the frame, and it would
     have punished the negative-below-the-origin behaviour S5 asks for. Replaced by the invariant: the gap
     between the lowest printed number and the band's bottom edge is **less than one step**. **Observed RED:**
     making the band drop a tick that fits fails it with `the band below the last printed number (200) is
     shorter than one 200.000 step down to -131.196`.
  3. **One consequence was left unnamed, and is now a decision with a test.** Narrowing `output.canvas` from
     the central area to the inset one also narrowed the `response.hovered()` gate, so the wheel over a band no
     longer zooms — as it already did not over the rail or the dock — and neither does middle-drag pan nor the
     canvas context menu. That is right: a band is chrome. `the_wheel_zooms_over_the_sheet_but_not_over_a_ruler`
     pins both halves, so the day it changes it changes on purpose.
  **`ShellOutput::canvas` changed meaning** — from the central area to the drawing area. No consumer outside
  `cam-ui` reads it (`cam-cli` and `cam-desktop` do not), so the blast radius is this crate's tests, and the
  doc comment on the field says which rect it is.
- **L21 (the T5 review: approved on the first receipt, after two harness failures).** Lineage
  `review-54750e7c98594128`, tier **high** (`process_boundary`), **three lenses** (`risk`, `readability`,
  `reliability`), 5 paths, 1 080 changed lines, correction budget 200 — **unused, nothing to correct**. Three
  reviewer runs over the pi host relay (62.2 kB of prompt each), **approved**, authority **burned**
  (`gentle-ai.review-acknowledged/v1`, revision `sha256:e40de020…`). Delivery stays ordinary repository policy.
  **The narrowing paid off exactly as L18 predicted:** the candidate was `4c619e1..HEAD` — five paths, this
  branch and nothing else — against the 26-path whole-branch projection that had died on
  `lens_context_budget_exceeded` in T4. One branch per task turned a hard stop into a normal review.
  **Two START attempts died before the provider.** Both `pre_native`, both `git_command_failed`,
  `git grep: failed to create thread`, exit 128, `mutation_performed: false` — no lineage, nothing burned. The
  service cgroup was at **`pids.max = 64` with 45 PIDs in use**; the second attempt also coincided with this
  session's own workspace suite. The owner authorised raising the limit whenever a START needs it, so it went
  to **512** for the run and **back to 64** as soon as the receipt closed. Same defect as L12 and L17, third
  time; the rule is now explicit rather than re-derived per session.
  **Three advisories, all `informational`, none blocking:** `R2-001` (`lib.rs:1938-1948`), `R2-002`
  (`ruler.rs:112-132`, a suggestion), `R3-001` (`lib.rs:1940-1948`). They join the twenty-four this feature
  already owes. Note what the candidate *already* contained when the reviewers saw it: `b349d28`, the two test
  corrections and the wheel test that the independent read demanded. The review judged the corrected tree, not
  the first delivery.
- **L22 (T6: the split, and six existing tests that had to change).** `2D | 3D` moved out of the app bar and
  into the band strip above the drawing area, where a press cannot also pick or draw underneath it, and 3D
  became **a mode that splits** the canvas into two columns: the reference frame splits left to right, and this
  sheet is wider than it is tall, so a horizontal split would take the drawing's long side from it. The 2D half
  *is* the canvas — `ShellOutput::canvas`, the camera viewport, fit, picking, drawing, marquee, pan, zoom, the
  context menu and the rulers all live there — and the 3D half inherits the 22 px band inset from L20 and owns
  the orbit plus S8's chrome: the overlay row (`Material` `Vectors` `Toolpaths` `Tool`, each checkbox hiding its
  own layer, proven by per-layer painted counts including the surface), `Centre` re-framing at the current
  angle, `?` opening the existing F1 panel, a home button whose house is painted from five segments because one
  button does not earn a seventy-ninth glyph, a triad labelled `TOP`/`FRONT`/`RIGHT` drawn through the same
  camera, and the playback bar: painted `▶`/`❚❚`, speed, `To the end` (the whole-program sweep), a scrubber
  that re-cuts from the pristine stock so it can seek backwards, and the readout `0:05 / 0:05  T22 · Drilling 1
  · Z 5.000` built from the one formatter set (`progress_text`, `clock_text`, `elapsed`/`duration`) — no second
  time formula anywhere.
  **One structure decision carries the frame ordering:** `Preview3d::show` was split into `take_gestures` and
  `paint_scene`, and `Preview3d::controls` deleted, so the 3D half can register its chrome *before* the picture
  (so a press on a chip is the chip's) and paint it *after* (so it sits on top). Every press is applied in the
  frame it lands in, which is L13's rule applied to a second surface.
  **Six existing tests changed, each for a reason the spec forced**, and the first one matters because S8
  reverses a claim M5 shipped: `the_two_views_are_exclusive` asserted the 2D scene and grid are *not* painted in
  3D mode, which a split makes false by design, so it became
  `the_split_paints_both_halves_and_closing_it_paints_one` (M5's real claim — closing 3D restores the drawing
  exactly — is kept and now checked against both halves). The others: triangle counts became per-layer
  (12/4/4/2) because the overlay row hides them one at a time; `the_panel_offers_play_step_and_reset` reads
  `▶` from the report because S8 paints a triangle, not the word; the app-bar toggle test became the canvas
  toggle test; four assertions moved from `0:00 of 0:01` to the reference's `0:00 / 0:01` because there is one
  pair formatter and S8 is what writes it; and `bar_rects` now picks the smallest `Role::Canvas` rect containing
  the fill, because the split's panel paints its ground in the same role and "the first match" had become the
  whole canvas. `input.rs`, `palette.rs`, `tool_card.rs` and the 124 shell tests are untouched.
  **What it refused, and why (L6's rule, fifth time):** no `×` on the playback bar although the reference has
  one — the toggle is already the way out, and a second close affordance is a second vocabulary for one act;
  `Tool` is an axis plus a diameter at the playhead, **not a spindle**, because the program carries no tool
  length and a spindle drawn to a guessed length would be a dimension the shell does not have (said in
  `paint_tool`'s own doc); `Vectors` are the document's edges at `z = 0`, read from the same `build_scene` the
  2D canvas paints, so a hidden layer is hidden in both pictures; and **with no program loaded the half draws
  neither the overlay row nor the playback bar**, saying `No program to preview…` instead — two controls that
  govern nothing would be the L6 lie, and `a_split_with_no_program_draws_no_overlay_row_and_says_why` pins it.
  **Slice size, named before anyone has to ask:** 2 178 added and 245 removed across four files, 831 of them
  tests — far past the ~400-line guidance, and one commit because the parts share the same state and the same
  frame ordering. It is the largest candidate this feature has asked a reviewer to read (T5's was 1 080); if
  the provider refuses it on the reviewer budget, the range splits by `baseRef` into the `view3d` model and the
  shell composition, which is the remedy L17 established.
  **A deviation shipped in T5, recorded rather than hidden:** `cargo fmt --check` is **not clean on `master`** —
  `crates/cam-ui/src/ruler.rs` is one of the files rustfmt wants to rewrite, verified against
  `master:crates/cam-ui/src/ruler.rs`. It is not fixed inside this candidate (a formatting sweep would put
  files outside T6's diff into T6's review); it gets its own commit once T6 lands.
