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
carrying its single-key shortcut letter. Below the groups, the brand block and an `Updated on …` line.

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
| T2 | Left rail: five collapsible groups with shortcut letters, brand block and `Updated on …` | S4 | inline | — |
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
