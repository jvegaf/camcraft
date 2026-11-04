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
| T6 | ✅ delivered — `2D \| 3D` as a mode that splits the canvas, with the overlay row of four checkboxes plus `Centre` and `?`, the home button, the `TOP`/`FRONT`/`RIGHT` triad and the playback bar. Merged into `master` as `c176c62`; the refuter's two defects corrected in `ad0eaaa` (L23) | S8 | inline | `2630feb` |
| T7 | ✅ delivered — the tool library as a master-detail modal: search, family tree, a detail form grouped by family, the tool drawn to scale with callouts, and the footer actions. Merged as `c5f4d78`; the units finding corrected in `620330c` (L25) | S9 | inline | `7df91c9` |
| T8 | ✅ delivered — one strategy surface for creating and editing, with the name field, the current-tool card, `Quick parameter edit`, the image cards and the verbatim note. Merged as `42edaab`; two CRITICAL findings corrected in `a282a50` (L27) | S10 | inline | `5140c13` |
| T8b | ✅ delivered — the toolpath rename the editor was missing: `Command::SetToolpathName`, its refusal, and the name field live in both modes (L28). Merged as `223e876`, reviewed and approved on the first pass (L29) | S10 | worker | `884732e` |
| T9 | ✅ delivered — `design/system.md` rewritten as the authority (21 roles × 2 palettes, eight styles, the metrics that build the frame, nine zones with the sources' own strings), `system.html` re-rendered from it, the four artboards re-synced to the delivered shell, and `design.json` saying what they now draw. Five slices: `00e3e45` (system) `89fa0c1` (01) `4765acd` (02) `dc46eaf` (04) `7c54886` (03 + manifest), all on `feat/camui-prancheta-t9`, **not merged and not pushed** (L31) | S1 | inline | `00e3e45..7c54886` |

## Backlog

Debt this feature knowingly leaves open. None of it blocks delivery; all of it is recorded so it is not
rediscovered. The advisory details live in the `Advisories` paragraph of each review entry in `## Log`, not here.

| Item | What | Where it is recorded |
| --- | --- | --- |
| `chrome.rs` frame diagram | the module doc still draws `│ 52 │` for the rail while `palette_width` is 200 and `chrome.rs:414` applies it — a comment that disagrees with the code it heads, outside a design task's surfaces | `chrome.rs:12`, `theme.rs:373` |
| Advisories ×44 | `informational` findings from this feature's review receipts, none of them blocking. The count is the one the last review states it is owed | the `Advisories` paragraph of each review entry in `## Log` |
| `PASSES` unit bug | the readout prints `Length` in mm whatever `rate_units` says — a true sentence about the wrong unit | L25 |
| Dead `segmented` wrapper | left behind by `ad0eaaa`; no live caller | the entry that records `ad0eaaa` |

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
- **L23 (the T6 review: the refuter earned the whole review).** Lineage `review-243a60aa7122cd9b`, tier **high**,
  **three lenses**, 5 paths, 2 469 changed lines, correction budget 200 of which **156 were spent**. Three
  reviewer runs (137 kB of prompt each) produced a `provider_refuter` request, the refuter returned
  **`correction_required`** with three candidate-caused findings, and after the correction a **targeted
  validator** approved the corrected tree (`sha256:371dfd03…`); authority **burned**
  (`gentle-ai.review-acknowledged/v1`). Delivery stays ordinary repository policy.
  **What the refuter found, and why the tests did not:**
  1. `R2-001` (CRITICAL) and `R3-001` (BLOCKER) are one defect: the playback bar was painted through
     `ui.painter().with_clip_rect(layout.picture)` while `half_layout` puts the bar **below** the picture, so
     every rectangle of the bar, its chips, the scrubber and the readout were clipped away — **invisible, with
     live hit areas**. The fix gives the bar its own painter clipped to its own strip.
  2. `R2-002` (CRITICAL): the canvas `2D | 3D` widget was laid out and painted *before* `ruler::paint`, and egui
     paints in order, so the band and its numbers covered the control that enters 3D — in 2D mode, where it is
     the only way in. The fix splits `chrome::segmented` into `segmented_cells` (allocate, click, report) and
     `paint_segmented` (paint), so the toggle is laid out early — which keeps L13's rule that the press frame is
     the split frame — and painted **once**, after the band.
  **The lesson this review paid for:** a headless test reads *shapes*, and a clipped shape is still a shape. The
  43 view3d tests were green while the bar was invisible, because `bar_rects` located the bar by sharing the
  surface's clip rect — it had encoded the clipping bug as its own locator. The correction had to change that
  helper too, and it is recorded here so the next person does not read the helper's clip filter as a rule.
  **Process, three failures worth not repeating:** the START was first rejected `lenses-invalid` (same input,
  fresh idempotency key, accepted); the targeted-validator binding was rejected twice — once because it was
  reconstructed **without** `validationRequest`, and `gentle_review_capture` refuses to run from `codemode` —
  and only the provider's **whole** object, passed as a direct call, was accepted. The candidate itself was
  committed as `2630feb` + `86ebe33` and corrected by `ad0eaaa`; the workspace has **1 256 tests, exit 0** after
  the correction.
  **Two advisories left after the validator, both `informational`:** `R2-003` (`view3d.rs:1118-1119`) and
  `R2-004` (`view3d.rs:664-667`). They join the twenty-seven this feature already owes.
- **L24 (T7: the tool library, and the formatting debt paid before it).** Two commits, in this order and on
  purpose: `aa6393a` formats the crate, `7df91c9` is T7. **L22 assumed the formatting debt was one file; it was
  four** — `ruler.rs`, `view3d.rs`, `tests/shell.rs` and `tests/view3d.rs` were all dirty against the
  repository's own `rustfmt.toml` (`tab_spaces = 2`), verified by reformatting each `master` version and
  comparing byte for byte. The sweep is exactly that reformatting, nothing else, and `cargo fmt --check` now
  exits 0. It is its own commit so that the T7 candidate is T7 and not 279 lines of somebody else's whitespace.
  **The modal** is `crates/cam-ui/src/tool_library.rs` (1 181 lines): a search field, a scrollable tree grouped
  by family with a count (`Flat end mill (2)`) and one row per tool (`Icon::ToolLibrary` mini glyph, name, dim
  `Ø6 · 2 cut(s)`), a detail column whose groups change with the family, the tool drawn on one shared scale with
  its callouts, and a footer above `Cancel` / `Use this tool`. Seventeen tests, none of the 228 existing ones
  touched: the card's new opener reports in `tool_card_hits`, so `the_card_names_the_tool_the_toolpath_cuts_with`
  still reads its three lines.
  **Three decisions the spec did not make, each following a rule this feature already learned:**
  1. **The family is derived from the geometry the record proves**, never from `tool_type`: an included angle
     makes it a V-bit, `tip_radius == Ø/2` a ball nose, any other tip radius a corner-radius cutter, and no tip
     radius at all a flat end mill. The raw type code is documented in the library's own header as unmapped, so
     a heading over it would be a number nobody can read (L6's rule again).
  2. **`PASSES` and `SPEEDS` are drawn only when the cutting data holds one of their numbers**; `GEOMETRY` and
     `MACHINE` always, because every record carries the units its rates were written in. A group heading over
     no fields is a wall with nothing behind it.
  3. **The footer offers no `New` / `Duplicate` / `Delete` / `Import…` / `Export…`.** This shell holds a
     read-only snapshot (`set_tools`), `cam-store` owns the only writer, and `cam-cli tools add` is the route
     that exists: five buttons that emitted nothing would be exactly the CRITICAL finding L14 paid for. The
     footer instead names the five actions and the reason, and a test pins **both** halves — the sentence and
     the absence of the buttons.
  **What it refused, and the rule it followed:** no invented tool length (the drawing's vertical extent is the
  frame's edge, with no callout, exactly as `view3d::paint_tool` refuses to draw a spindle to a guessed length);
  the flute length is drawn only when the record holds one; no new glyph; and no `Use this tool` when the target
  cannot be honoured — `a_tool_with_no_number_…`, `an_empty_library_…` and `a_library_opened_without_a_toolpath_…`
  each pin a different refusal.
  **L23's lesson applied before a reviewer had to find it:** the modal reports only points the frame actually
  showed (`Spot` + `pressable` filtered by clip), so a row the scroll area hides is not reported as pressable —
  the mirror image of the playback bar that was clickable while invisible.
  **Honest note on the RED:** the worker implemented first and produced each rule's RED by reverting it and
  capturing the failure — not the test-first order this document prefers. Two of those mutations found real
  bugs in its first cut (the modal reported the *previous* selection for one frame, which is L13's defect one
  layer down), which is why the tree is drawn first and the detail resolved between the columns.
  **Workspace: 1 273 tests, exit 0** (baseline 1 256; cam-ui 228 → 245). **One loose end, not T7's:** `segmented`
  is now dead code — `ad0eaaa` split it into `segmented_cells`/`paint_segmented` and left the one-shot wrapper
  without a caller, so the crate carries a `never used` warning that pre-dates this task. It gets its own commit,
  outside any review candidate, rather than riding in T7's diff.
- **L25 (the T7 review: one finding, and it was about units).** Lineage `review-e4268ddae85fe9c0`, tier
  **high**, **three lenses**, 5 paths, 2 509 changed lines, correction budget 200 of which **47 were spent**.
  Three reviewer runs (≈120 kB of prompt each) produced a `provider_refuter` request; the refuter accepted
  **`R3-001` (CRITICAL)** and the correction route opened. The targeted validator then approved the corrected
  tree (`sha256:565ce156…`), and authority was **burned** (`gentle-ai.review-acknowledged/v1`). **Workspace:
  1 274 tests, exit 0.**
  **The finding:** `form_fields` wrote every feed and plunge rate as `mm/s` whatever `cutting.rate_units` said,
  so an inch-library tool read `Feed 100.00 mm/s` in `SPEEDS` while `MACHINE` read `Rates in/s` — one number
  carrying two units, which is the one thing a rate cannot do.
  **It was not a label bug, and treating it as one would have been worse.** `cam-store` documents the field as
  *"Units the rates were authored in. Stored rates are millimetres per second."* Renaming the suffix would have
  left a millimetre-per-second number wearing an inch label. `620330c` therefore **converts** the stored value
  into the authored unit (`/ MM_PER_INCH`) and labels it with that unit, so the two rows agree and the number is
  the one the operator typed. `a_rate_is_shown_in_the_units_the_library_authored_it_in` pins the inch path
  (`3.94 in/s`, `Rates in/s`) **and** the millimetre path (`100.00 mm/s`) so the fix cannot move a number a
  metric library reads.
  **What the correction deliberately did not fix:** `PASSES` prints every `Length` in `mm` whatever the authored
  units are — the same root cause, one expression away, and **not corroborated by the refuter**. A bounded
  correction fixes what was corroborated; the lengths join the advisory backlog instead of riding along
  unannounced.
  **Seven advisories, all `informational`:** `R2-001` (`tool_library.rs:310`), `R2-002` (`:38-40`), `R2-003`
  (`:129`), `R2-004` (`tests/tool_library.rs:884-889`), `R3-002` (`:216`), `R3-003` (`:1046`) and `R3-004`
  (`:1067-1073`). They join the twenty-nine this feature already owes.
  **One process note, applied rather than re-learned:** the candidate was `aa6393a..HEAD`, so the formatting
  sweep stayed out of the reviewers' diff, and the targeted-validator binding went in **whole** — with its
  `validationRequest` — on the first attempt, which is L23's lesson used instead of paid for again.
- **L26 (T8: one surface, a draft, and the one place the spec could not be followed).** `5140c13` (code) on top
  of `d142371` (the formatting debt my own L25 correction left in `tests/tool_library.rs` — one long
  `assert_eq!`, paid in a commit of its own so it is not in the candidate). `crates/cam-ui/src/strategy.rs`
  (~1 000 lines) makes the TOOLPATHS body the strategy editor: the **document decides the subject**, so no
  toolpath opens `New toolpath · <name>` and one opens `Edit · <name>`, both on the same surface with the name,
  the current-tool card and its `Choose another tool…` (S9's row, reached through a new
  `Shell::tool_card_for`), the collapsible `Quick parameter edit`, a `CUT` section whose kind is chosen on image
  cards, the note verbatim, and `Generate toolpath` / `Cancel`.
  **What `Generate toolpath` actually does:** it emits `UiAction::AddToolpath` while creating and
  `UiAction::SetToolpathOperation` while editing, both routed through the existing `apply_command`, so the
  kernel still validates and refuses. **It is not drawn when it cannot act** — no tool, no region, no name, or
  an operation this shell does not draft — and the footer prints the sentence that says which. `Cancel` (and the
  header's `✕`) discards the draft and leaves the document, the tool and the name untouched. The draft exists
  for a reason the tests pin: the tool library modal gained a **draft target**, so `Use this tool` writes the
  draft while creating and the document while editing — one modal, one commit, two subjects.
  **The one deviation from S10, and it is the model's, not the UI's:** `cam-app` has `AddToolpath` and
  `SetToolpathOperation` and **no rename command**, so the name is an editable field only while creating; while
  editing it is drawn as the document's own reading with `The name is fixed while editing: …`. A field that
  silently dropped what was typed would be L6's dead control, so the surface says what it can do. Renaming
  needs a command in `cam-app`, which is outside this task's surfaces — it is recorded here and left as the
  owner's decision instead of faked.
  **The image cards carry a picture.** Each card is painted from the operation's own semantics — a contour with
  the bit on it for `Profile`, a region of rings or raster rows for `Pocket` — and each reports how many strokes
  it painted, so a card with no picture fails a test. **No screenshot of the reference app is used as product
  art**, and the module doc says what a real thumbnail would need: a rendered preview of the toolpath, which does
  not exist before `Generate toolpath` and which this shell has no renderer for (the 3D half renders a
  *program*).
  **Three existing tests changed, each with a reason the spec forced:** two header expectations
  (`task_editor` was the word `TOOLPATHS`, and S10 makes it `New toolpath · Profile 1` / `Edit · <name>`), and
  `tall_input` from 2 000 to 2 600 px so its own promise — "tall enough for every button to be pressable" — stays
  true now that the editor occupies the top of the body and the authoring commands start below the fold. That
  placement is worth a reviewer's eye; `the_tools_tab_offers_every_authoring_command` proves the commands are
  still reachable by scrolling.
  **A layout trap paid for and written into the code:** three fixed-width field boxes in a
  `horizontal_wrapped` row measured wider than the 288 px panel on an un-laid-out pass, dragged the inspector
  off the window and made its own buttons unclickable. The quick block draws the rates as rows instead, and the
  comment in `quick_block` says why.
  **L23's rule, this time written first:** `a_control_the_scroll_area_clips_is_not_reported_as_pressable` failed
  before the `ui.clip_rect()` retain existed (it reported `Generate toolpath` and `Cancel` as pressable at
  800 px), then went green. **Workspace: 1 287 tests, exit 0** (baseline 1 274; cam-ui 246 → 259).
  **The other REDs are T7's honest shape:** the implementation came first and each rule's RED was produced by
  reverting it — recorded as such rather than dressed up as test-first.
- **L27 (the T8 review: two findings with one root, and a probe that exposed my own test).** Lineage
  `review-2828bd1487e94795`, tier **high**, **three lenses**, 7 paths, 2 392 changed lines, correction budget
  200 of which **92 were spent**. Three reviewer runs (≈119 kB of prompt each) produced a refuter request; the
  refuter accepted **two CRITICAL findings**, both introduced, and both children of one unconditional method.
  The targeted validator approved the corrected tree (`sha256:174b27d2…`), authority **burned**
  (`gentle-ai.review-acknowledged/v1`). **Workspace: 1 289 tests, exit 0.**
  **`R3-001`:** pressing the kind card the strategy *already was* re-defaulted the operation, silently discarding
  every number typed into `CUT` and the quick block, and the pocket's clearing pattern — then `Generate` wrote
  those defaults into the document. **`R2-001`:** while editing, pressing `Pocket` rewrote the draft's name to
  `Pocket 1` even though `Generate` emits only `SetToolpathOperation` and cannot rename, so the header read
  `Edit · Pocket 1` over a document still called `Profile 1`. **`a282a50` (92 diff lines)** makes `choose_kind`
  return when the kind is unchanged, and leave the name alone while editing — the name is the document's reading
  and nothing on that surface can change it.
  **The lesson is about my own test, and it is worth more than the fix.** I wrote the R2-001 test, reversed the
  editing guard, and **the test stayed green**: the fixture is called `Perfil 01`, which is no kind's default, so
  the rename branch never ran and the test passed for the wrong reason. Rewritten against a document named
  `Profile 1` — the case the finding names — it fails without the guard. **A revert-probe only proves something
  when the fixture reaches the branch it reverts**; the second test (the already-chosen card) bit on the first
  try, which is exactly what made the first one's silence suspicious. Both are now RED-verified.
  **Four advisories, all `informational`:** `R2-002` (`strategy.rs:181`), `R2-003` (`:183`), `R2-004`
  (`:802-840`) and `R3-002` (`:225-226`). They join the thirty-six this feature already owes.
- **L28 (the rename command the editor was missing).** `884732e` on `feat/camui-toolpath-rename`, cut from
  `42edaab`: `cam_app::Command::SetToolpathName { toolpath_id, name }` with its own label, its dispatch through
  `toolpath_index`, and a new `CommandError::EmptyToolpathName`. The refusal needed its own variant rather than
  `EmptyText`, which is about drawn text: reusing it would have been a true sentence about the wrong thing.
  The assignment happens **after** the checks, so a refusal writes nothing. `cam_ui::UiAction::SetToolpathName`
  carries it through `apply_command` exactly as `SetToolpathOperation` travels.
  **The editor's name is one `TextEdit` in both modes** — the control the creating mode already had, not a second
  vocabulary — and the sentence and constant that explained a name nobody could change are **deleted**: a
  sentence that explains a limitation has to go when the limitation goes. `generate` emits the rename **only when
  the trimmed field differs from the document's name**, because an undo step that undid nothing is a lie about
  what happened.
  **The ordering is the parent's, and it is the one decision worth reading twice.** The worker pushed the rename
  first and the operation second, and flagged in its own report that the pair was not atomic: a kernel refusal
  of the operation would already have written the new name, with a failure message that could not explain it.
  Inverted to operation-first, the only reachable failure changes nothing — and a rename that failed *after* the
  operation cannot happen in one frame, because `refusal` already refuses a blank name before either action is
  built. The test that pinned the order was updated with that reason rather than to accommodate the code.
  **Six new tests:** three in `cam-app` (it renames; a blank name is refused and writes nothing; an absent
  toolpath is refused) plus the label case, and four in `cam-ui` (the field is typeable while editing; `Generate`
  renames the document; an untouched name emits no rename; `Cancel` puts a typed name back), with a
  `type_into_name` helper that presses the field, selects all and types. `creating_and_editing_are_one_surface…`
  lost its `retain("Name")` fudge and now asserts that **both** modes draw `Name` and neither paints the
  fixed-name sentence, keeping its note, card and order claims untouched.
  **Workspace: 1 295 tests, exit 0** (baseline 1 289; +6). `cargo fmt --check` clean. The public
  `cam_ui::strategy::RENAME` constant is gone; nothing else in the workspace referenced it.
- **L29 (the rename review: approved on the first pass, and what the four advisories say).** Lineage
  `review-463dc34b6b39af90`, tier **high**, **three lenses**, 6 paths, 555 changed lines, correction budget 200 —
  **unused, nothing to correct**. Three reviewer runs (≈49.6 kB of prompt each, the cheapest of the feature
  because the candidate was one work unit), **approved on the last admitted event**, authority **burned**
  (`gentle-ai.review-acknowledged/v1`, revision `sha256:64a679c2…`). No refuter was requested: after two
  consecutive corrections on T6 and T8 this is the first candidate since T5 that a reviewer read and accepted
  as it stood — the operation-first ordering written into `generate` (L28) is the reason the pair's failure mode
  was already right before anyone looked at it.
  **Four advisories, all `informational`:** `R2-001` (`strategy.rs:457-458`), `R3-001` (`:483-490`), `R3-002`
  (`:479-482`) and `R3-003` (`tests/strategy.rs:625-628`). They join the forty this feature already owes. None
  of them is a reason to re-review this candidate; its receipt stands.
- **L30 (the ODD record brought back in line with `master`, on the owner's word).** Owner: "sanea primero".
  The task table had stopped telling the truth: **T6, T7 and T8 were merged** (`c176c62`, `c5f4d78`, `42edaab`)
  and their rows still read a bare description with no tick and no commit — the shape of a task nobody
  delivered. Three rows now carry what each one actually landed, its merge commit and its **feat** commit — the
  column's existing convention, which `482625b` on T4 settles: the commit that landed the behaviour, not the
  branch tip and not the entry that documents it — plus the commit that corrected it. A **fourth delivered task
  was missing from the table entirely**: the rename (`884732e`, merged `223e876`) exists in the Log as L28/L29
  but was never a row, so it is now T8b under S10 — the spec that asked for the name field in the first place,
  and L28 already says a worker delivered it, which is why its `Route` cell can be filled and its older
  siblings' cannot. The debt this feature owes was also only legible by
  reading twelve review entries in order; it is now a `## Backlog` table that names the four items and points
  at the entry that holds each one, so nothing is restated (a restated inventory goes stale, and that is the
  defect this entry repairs). **No code touched, no test changed**: the workspace is `master` @ `223e876`,
  1 295 tests green, tree clean. **One cell left as it was, because I could not verify it rather than guess:**
  T6, T7 and T8 read `inline` in the `Route` column while L19 says every task from T5 on routes to a worker, and
  T5's own row says `worker`. The commit history cannot tell an inline delivery from a delegated one, so the
  contradiction is recorded here instead of being papered over.
- **L31 (T9: the design artefacts re-synced to the delivered shell, and the four lies my own brief carried).**
  Branch `feat/camui-prancheta-t9` cut from `master` @ `2e52952`. Five commits, one per argument, because the
  whole task is 2 283 changed lines and a single commit would be unreviewable: **`00e3e45`** the authority
  (`design/system.md` + `system.html`), **`89fa0c1`** the 2D shell, **`4765acd`** the workflow board,
  **`dc46eaf`** the token board, **`7c54886`** the icon board and the manifest. **Not merged, not pushed** —
  delivery is the owner's decision, and the slice boundaries are those five commits, in that order.
  **What was wrong, measured against the code rather than against the old board.** `system.md` documented a shell
  from before this feature: a stage bar, an `Inspector` with `Selection`/`Toolpath`/`Material` tabs, a "52 px rail"
  it said the shell had abandoned, and the four group words `DRAW`/`TRANSFORM`/`VECTOR EDIT`/`VIEW`. It also
  rendered only 16 of the 21 roles (`system.html`) and left four undeclared anywhere. `01-shell-2d.html` drew the
  stage bar S2 removed, the tabs S6 replaced, and Portuguese content (`FARMÁCIA`, `SÃO JOÃO`,
  `letreiro-farmacia.c2d`) in a shell whose copy is English by S3. `02-stages.html` argued five stages with gates
  and claimed the shell *today* is one flat column of `ui.label` — true before `camui-shell`, false since.
  `03-iconography.html` counted 70 glyphs where `icons.rs` asserts 71. `04-tokens.html` drew a tabbed inspector
  and a `2D | 3D | snap 1 mm` pill row that do not exist.
  **Three premises in my own brief were false, and the writers found them by reading the source instead of
  obeying me.** (1) I wrote `v4.5.1` for the crate, which is the tool's version, not this project's: `Cargo.toml`
  says `0.0.1`. (2) I wrote "chip = glyph + label"; `palette_panel` calls `icon_button(..., 20.0)` with
  `palette_tooltip(command)` and its own comment says the palette is "the part of the frame that carries no text
  of its own" — the words are the group headings, a chip is a glyph and nothing else. (3) I asked for a greyed
  chip "with the reason"; the delivered rail passes `enabled=true` for all twenty-eight, so a greyed chip would
  have been a false claim about the present. The real dead control is the object menu's `Trim`, dark until two
  vectors are selected. A fourth correction was arithmetic: `theme.rs` hands egui **premultiplied** channels, so
  `rgba()` copied straight from the source darkens the wash twice; de-premultiplied, the washes land on the
  signals (`rgba(105,202,225,.13)`, `rgba(240,165,30,.13)`).
  **The independent audit then found six more, and one of them was the authority's fault.** `03` painted its page
  in `#0a0c0f`, a colour no role owns, and declared its washes as `#6fd3e822`/`#f5a62322`; six places painted
  **prose** in `TextOff` at 11–13 px, roughly 3:1 and outside the twelve pairs `theme.rs` proves; `01`'s rail chip
  was 32 × 32 while `system.md` claimed a 24 px control — and there the file was wrong, not the artboard, because
  `icon_button` really does allocate 32 and a hit area is not a fourth control height. `01` also had no `Name`
  field, which S10 requires and `strategy.rs` draws. All six closed, plus the three the fix worker reported but
  was not authorised to touch: `03`'s `h2 .hint` prose, `02`'s 24 px chip, and `03`'s prose still claiming the
  rail renders at 18 px (18 is the app bar's; the rail takes 20).
  **Two rules the audit earned are now in the authority, and they are the part worth keeping:** `TextOff` never
  carries a sentence — it is the ink of a heading, a count and half a row, and prose takes `TextDim`, the pair
  proved at 4.5:1; and a chip's label is its tooltip, so a pointer-less reader has only `aria-label` and the two
  must stay in sync.
  **The icon count is 71, and the missing glyph was `shortcuts`** — `icons.rs:1237`, drawn in the rail's SHOW AND
  SELECT row since the app bar existed, absent from the board. Added by transliterating the runtime's own three
  rounded rectangles (`SHORTCUTS`), so the board and the code now draw the same shape; `view/document/display`
  goes 16 → 17 and the prose and `design.json` say 71.
  **Verification, and why it is not a test.** These are static artefacts: nothing in the repo reads them
  (`scripts/check.sh` is fmt/clippy/cargo test over `crates/`), so there is no RED to observe and saying otherwise
  would be theatre. What ran instead: each writer's self-containment and colour audit; one independent verifier
  over nine checks (self-containment, colour truth against the 21 + 21 roles and four named washes, fonts, the rail's
  28 chips with no label and no letter and none disabled, no present-tense stage bar or tabs, counts,
  version, manifest, `TextOff` contrast); and the parent's own pass over the five files — one `<style>` each, zero
  `<script>`, zero external URL, **zero hex outside the roles**, only the four washes, 28 chips, `Name` present,
  71 cells, the reduced-motion rule in all five. Canvas: `camui-shell` opened and refreshed.
  **Two honest limits.** The 01 frame is exact at 1440 × 900 because its CSS fixes it; 02, 03 and 04 are
  auto-height boards, and their `design.json` heights stay declared viewports rather than measurements — there is
  a `firefox` on this machine but no scripting harness (playwright, selenium, marionette) to read a document
  height, so I did not invent numbers. And **`chrome.rs:12` is left alone**: its module diagram still draws
  `│ 52 │` for a rail that `palette_width` makes 200. It is a code comment, not a design artefact, so it is
  recorded in `## Backlog` instead of being fixed inside T9's surfaces.
- **L32 (the delivery shape: five chained review candidates, because one could not be reviewed at all).** The whole
  branch as a single candidate was **refused by the provider**, not by me: `lens_context_budget_exceeded` — 8 paths
  and ~2 300 lines of HTML exceed the reviewer's evidence budget, `mutation_outcome: none`, no authority created,
  nothing burned, and the cause says retrying that exact candidate cannot succeed. The fix the cause names is a
  chained sequence of smaller candidates, so the branch was re-cut as six branches off `2e52952`, each containing
  only its own slice's paths (`review/t9-1` … `review/t9-6`, kept as the exact reviewed trees).
  **Five candidates now carry a burned receipt, all approved, none corrected:**
  `review-5b02c33628496345` (`system.md` + `system.html`, tier medium, one `review-reliability` run,
  **4 advisories**: `system.html:274`, `:290`, `:354-356`, `system.md:66`);
  `review-2781d3ca2a33d57f` (`01-shell-2d.html`, medium, one run, **1 advisory**: `:21-24`);
  `review-e3a48f441b77f133` (`02-stages.html`, medium, one run, **no findings**);
  `review-8f14cca9c417e30a` (`04-tokens.html`, medium, one run, **2 advisories**: `04-tokens.html:413` and this
  document at `:88`);
  `review-a19d6e0dc7a8590a` (`03-iconography.html` + `design.json`, medium, one run, **no findings**).
  **The sixth candidate needs none**: the provider answered `stop` with `acknowledged_predecessor_passive_delta`
  for the record itself, which is a passive delta on a path whose predecessor was already acknowledged.
  **One reviewer run was wasted and it is worth recording why:** the first `review-reliability` artifact for the
  `system.md` candidate was refused at admission (`evidence_path_out_of_scope`: the reviewer cited a path outside
  the frozen repository). Admission consumed nothing; STATUS reoffered the same slot, the second run satisfied it,
  and the refused bytes were never resubmitted.
  **Every tier came back `medium` for a reason the provider states and I did not anticipate:** an `.html` artefact
  counts as an *executable change* (`risk_reasons: executable_change`), so the design boards are reviewed as
  software, not as prose. That is why each slice cost one real reviewer run instead of closing instantly like the
  documentation-only candidate in L30 did.
  **Delivery state, and it is the owner's:** `feat/camui-prancheta-t9` @ `3a14cc7` holds the seven commits, all five
  artefact slices carry burned receipts, the branch is **not merged into `master` and not pushed**, and `master`
  stayed at `2e52952` throughout. The review lifecycle stops at acknowledgement; commit, push and merge follow
  ordinary repository policy.
