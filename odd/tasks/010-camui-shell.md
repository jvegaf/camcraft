# camui-shell — a product shell for Campilot, after the Prancheta CNC concept

Rebuild the desktop shell's foundations (visual tokens, icon language, staged layout) in `cam-ui`, keeping
`cam-cli` and the `c2d/document@2` format untouched. Design reference lives in `design/camui-shell/`.

## Specs

- **S1 — the goal.** The owner's words: *"nos vamos a centrar en mejorar la UI porque da asco"*. The shell is the
  product's face and today it is a debug inspector: 25 `ui.label`, 21 `DragValue`, 14 `ui.button`, 18
  `ui.separator`, one `CentralPanel`, no top bar, no rail, no dock, no theme.
- **S2 — the reference and the method.** *"Te he dejado en @\"data/Prancheta CNC.exe\" una app que me gusta su ui.
  Analizala con Rea, y sacas el concepto de como lo quiero"*. The reference is a Rust + egui 0.37 app; the concept
  extracted from it is recorded in `design/camui-shell/02-stages.html` (five stages with gates, stage-scoped rail,
  collapsible inspector, teaching strings) and `design/camui-shell/03-iconography.html` (one line weight, 24 × 24,
  stroke 1.6, dashed = virtual, 22 % wash = kept).
- **S3 — the approved scope.** *"Sí, es eso: seguí con el shell completo"*, with four artboards: `01-shell-2d`
  (rail + collapsible dock + stage bar + status bar), `02-stages`, `03-iconography`, `04-tokens`.
- **S4 — the UI language.** *"Inglés — \"Outside drops the part free\""*: every string the operator reads is
  English, in the plain voice of `02-stages.html` §04 — consequence before mechanics, second person, no CAD jargon,
  and every refusal names a number.
- **S5 — the order.** *"Primero los cimientos: tokens + iconos en egui"*: the tokens and the icon set land, with
  tests, **before** any layout change. While they land the existing shell keeps working: no `UiAction` changes, no
  command changes, no test that exists today may change its behaviour.
- **S6 — no new dependencies.** Prior owner decision, recorded verbatim in Engram #82 (M5 mirror): *"egui mesh, cero
  dependencias nuevas"*. So the icons are geometry drawn with `egui::Painter`, not SVG rasterised by `usvg`/`resvg`.
- **S7 — what the design forbids.** From `04-tokens.html` §06: no second accent hue, no filled icon variant beside
  the line set, no shadow on docked surfaces, no colour-only meaning, no numeric field without a unit.

## Tasks

| ID | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | ✅ delivered — `cam-ui/src/theme.rs`: dark + light token sets (surfaces, text, two signals, spacing, radii, control heights, type scale), `Theme`, `apply(ctx)`, AA contrast pinned by test | S1 S4 S6 S7 | inline | `db92b45` |
| T2 | ✅ delivered — `cam-ui/src/icons.rs`: the glyph set on one 24 × 24 grid as a path DSL, `paint_icon` painter, tint from the theme, 1.6 stroke at 24. Landed at 66 glyphs, later `e48423d` (families), `8b860b7` (keyboard), 71 today | S1 S2 S3 S6 | worker | `db92b45` |
| T3 | ✅ delivered — icon sheet as an `egui::Window` behind `F2`, so the whole set can be inspected without touching the layout | S3 S5 | inline | `db92b45` |
| T4 | ✅ delivered — theme applied at startup in `cam-desktop` and the existing `palette` routed through it: the shell changes colour, the layout does not change | S1 S5 S7 | inline | `db92b45` |
| T5 | ✅ delivered — `design/system.md` + `design/system.html` synced from `theme.rs` once it became the project's token source. Created `a37b5cf`, re-synced in `e72dc57`, `e2845f0` and `72bd2e3` as the token set grew | S3 | inline | `a37b5cf` |

## Log

- **L1 (owner, verbatim).** *"nos vamos a centrar en mejorar la UI porque da asco. Te he dejado en @\"data/Prancheta
  CNC.exe\" una app que me gusta su ui. Analizala con Rea, y sacas el concepto de como lo quiero"*
- **L2 (analysis, evidence).** The reference binary is Rust + `egui 0.37.0` + `eframe` + `winit` + `glow`, built from
  a workspace of `apps\prancheta\src\{cam_dlg,jobdlg,nest,nodes_tool,widgets}.rs` and `crates\{cam,geom}\src\*.rs`.
  Extracted from it: 83 command entries with `d`/`u` help pairs (pt-BR), a 15 kB JSON map of ~100 SVG glyphs
  (24 × 24, stroke-only, `currentColor`, `stroke-dasharray` for virtual geometry), three theme glyphs
  (`tema-claro`/`tema-escuro`/`tema-auto`), Inter Regular + SemiBold embedded, and the shortcut set
  (`F2`/`F3` 2D↔3D, `F4` simulate, `F9` fit, `Ctrl+1…7` fixed 3D views). No vendor metadata and no `*.com.br` URL
  in the binary, and a web pass found no screenshots: the layout below is inferred from strings, not observed.
  Working artifacts: `/tmp/prancheta/help_catalog.json` (83 entries) and `/tmp/prancheta/icons.json` (glyph map).
- **L3 (owner decisions).** Concept accepted (*"Sí, es eso: seguí con el shell completo"*), shell language English
  (*"Inglés — \"Outside drops the part free\""*), foundations first (*"Primero los cimientos: tokens + iconos en
  egui"*).
- **L4 (evidence, T1 and the design slice).** Branch `feat/camui-shell` from `master` `f24cfc2`. The design
  deliverable is one commit, `a37b5cf` (1 701 authored lines, all of them HTML artboards and the design system):
  artboards are read as rendered pages, so the 400-line review slice does not apply to them, and they are the
  specification the code commits answer to. `crates/cam-ui/src/theme.rs` then landed with five tests green —
  `every_pair_the_shell_writes_on_meets_aa`, `the_canvas_sits_at_the_end_of_the_luminance_ladder`,
  `the_two_signals_never_share_a_hue`, `the_metrics_are_ordered_and_land_on_the_grid`,
  `apply_registers_every_named_text_style` — and clippy clean. RED was compile failure (the module did not exist);
  GREEN is the five assertions. One defect was found by review rather than by the first test run: `apply` had
  registered `Small`/`Body`/`Button` as *named* styles while the real `TextStyle::Small/Body/Button` kept egui's
  defaults, so every stock widget would have ignored the scale; the test now asserts the native variants directly.
- **L5 (environment, two repairs worth recording).** This harness redirects `HOME` to
  `<install>/home`, which silently broke three tools: Engram read an empty store, `rustup` reported no toolchain,
  and `git` had no identity. All three are now symlinks from the redirected home to the real one
  (`~/.engram`, `~/.rustup`, `~/.cargo`, `~/.gitconfig`), with the empty stores moved aside rather than deleted.
  Worth knowing: this machine's `rustc` is `1.99.1 (58cf8f9c3)`, the exact toolchain hash embedded in the
  reference binary's panic paths — the reference was built here, with this toolchain.
- **L6 (T2, the 65-vs-66 question, and the slice boundary).** `crates/cam-ui/src/icons.rs` landed: 1 966 lines,
  66 glyphs, six tests green, transliterated from the artboard by a delegated writer and verified two ways — the
  worker rendered every glyph and compared it against the artboard at 3× (`/tmp/camicon/compare.png`), and the
  five structural assertions (geometry present, everything inside the 2…22 safe box, 66 unique names that
  round-trip, one tint per glyph, stroke follows the token) pin it from the other side. The writer raised a real
  defect in **my** numbers: the artboard draws 66 cells while its own heading said 65 and its family hints summed
  to 61. The drawing wins — no glyph is "the extra one" — so the prose was corrected (heading and three family
  hints) instead of the set; `design.json` now says 66 too. Two literature-to-code clamps worth knowing: `polyline`'s
  first node square and `import-image`'s right arrowhead wing both sat outside the safe box in the artboard and were
  pulled to the 2.0 line, 0.2 and 0.8 box units respectively.
- **L7 (T3 + T4, the tokens stop being dead code).** `Shell` gained `theme`, `applied_theme` and `show_icons`;
  `theme::apply` runs when the set changes rather than per frame; `F2` opens a sheet of all 66 glyphs so the
  vocabulary is inspectable from inside the app; the six hand-picked `palette` constants are gone and every canvas
  colour now comes from a `Role` — canvas, hairline grid, body geometry, selection, construction, stock border, snap.
  The old number-picking is replaced by the design's meaning: selection is now cyan (was amber) and the cut signal
  owns amber. `view3d.rs` keeps its own shading constants for now: a 3D surface ramp is not the 2D token set, and
  folding it in is its own task, not a side effect of this one.
- **L8 (gate evidence).** `./scripts/check.sh` — fmt, clippy `-D warnings`, and the workspace suite: **1 067 tests
  passed** across 91 suites (baseline 1 056 at `f24cfc2`, so +11: five for the tokens, six for the glyphs). The
  slice is 2 588 new lines plus ~90 changed in `lib.rs`; it is one commit because the three parts are mutually
  dependent — the tokens have no meaning without the set and the routing that consumes them, and splitting them
  would either leave an undeclared module whose tests never run or a commit that does not compile.
- **L9 (environment, two more instances of the same root cause).** `cargo fmt --check` reported the whole repository
  as unformatted because rustfmt reads `$XDG_CONFIG_HOME/rustfmt/rustfmt.toml` (`tab_spaces = 2`) and the redirected
  `HOME` hid it: the gate has to run as `XDG_CONFIG_HOME=/home/th3g3ntl3man/.config ./scripts/check.sh`. The
  symlink alone is not enough, which is worth knowing before trusting a green `check.sh` in this harness. The first
  gate run also appeared to pass while failing, because `check.sh | tail` swallows the exit status — the gate must
  be run without swallowing it.
## Next phase (delivered in full — see L43)

The foundations are delivered, and so is every slice below. L43 records the two rows this table left stale. What
remains of the concept, in the order the design argues for:

| ID | Task | Specs |
| --- | --- | --- |
| T6 | ✅ delivered `5592544` — the shell frame: app bar, stage bar, the stage-scoped rail (52 px then, the 200 px labelled palette since `e72dc57`), the 288 px collapsible inspector, the status bar. `01-shell-2d.html` implemented. This row used to name `4318205`, a pre-rewrite commit no branch contains | S1 S2 S3 S7 |
| T7 | ✅ delivered `6014611` (T13) — the stage model: five stages with gates, the counts that make a stage done, and the strings that name the next step. L42 closes this row onto T13; the substance is `Readiness` in `chrome.rs:184`, not a separate module | S2 S4 |
| T8.1 | ✅ delivered `c23cfac` — `help.rs`: a command identity that is *not* `UiAction` (an action carries a width), one `d`/`u` pair per command, and the `F1` help panel that renders it | S2 S4 |
| T8.2 | ✅ delivered `c22b7e1` — the palette and the app bar read the command table; `rail_commands` is gone and the two-site *list* construction with it | S2 S4 |
| T8.3 | ✅ delivered `3d8a620` — object context menus on the canvas: the first use of `PointerButton::Secondary` and of `Response::context_menu` here, emitting `UiAction`s only | S2 S4 |
| T9 | ✅ delivered `72bd2e3` — `view3d.rs` paints only through the theme: the five literals and `depth_colour` are gone, replaced by `StockTop`/`StockBottom`/`StockEdge` and `Palette::stock_shade`, with a material-to-canvas invariant the suite did not have before | S1 S7 |

- **L10 (owner, verbatim).** *"Implement the design artboard design/camui-shell/01-shell-2d.html … Implement it
  in the project's real stack. Use the artboard as the visual spec: map its parts to the project's existing
  components and styles, and do not copy the artboard HTML verbatim."*
- **L11 (T6 delivered, the frame).** New module `crates/cam-ui/src/chrome.rs` (≈640 lines) holds the frame and the
  two enums it needs: `Stage` (Design, Edit, Relief, Toolpaths, Prove) and `Tab` (Tools, Selection, Job).
  `Shell::show_in` now composes `app_bar` → `stage_bar` → `view3d` → `status_bar` → `rail` → `inspector` → canvas,
  in that order because egui lays panels out in the order they are added and a central panel must come last.
  `command_bar`, `layer_dock`, `properties` and `authoring` stopped being panels and became inspector sections;
  `layer_dock`/`properties`/`authoring` are otherwise unchanged in content, so every command keeps its own
  implementation and its tests.
  The stage is **view state**: switching it pushes no `UiAction` and never touches the document. Counts are only
  shown where the document can prove them — vectors for Design, `document.toolpaths.len()` for Toolpaths — and the
  rest show a dash while the inspector says in words that those commands live in `cam-cli` today. The rail is
  stage-scoped: the nine canvas gestures on Design, seven immediate commands on Edit, and a one-line explanation
  where a stage has no tools of its own. Collapsing the inspector leaves a 12 px strip that reopens when the
  pointer touches it.
- **L12 (three regressions the tests caught, and one design gap).** Deleting `command_bar` silently removed three
  affordances: the rectangle-by-numbers `Add`, `Delete selection`, and the 2D/3D toggle. The tests failed on each,
  and each was put back where it now belongs — `Add rectangle` beside the drawing tools, `Delete selection` in the
  Selection tab, the `2D | 3D` toggle in the app bar. Two further defects came from the same run: a `RichText`
  style lookup panicked because a `Ui` captures its `Style` when it is created, so publishing the tokens inside the
  frame reaches the *next* frame's widgets; fixed by `Shell::publish_theme` (called before the root `Ui` by
  `Shell::show`, and by `cam-desktop` at creation) plus explicit `.size()` on the frame's own text. And the 12 px
  strip's hover target rounded to nothing inside a panel's margin, so it reads the pointer from the frame input
  instead of a widget's hover state, which egui resolves from the previous pass. The four control heights, the two
  signals and the type scale come from `theme.rs`; nothing in `chrome.rs` names a colour or a pixel size.
- **L13 (evidence, and what is not verified).** `./scripts/check.sh` green: fmt, clippy `-D warnings`, **1 071 tests
  across 91 suites**. Four new tests pin the frame as behaviour rather than pixels: the stage bar's real counts vs
  its dashes, the rail changing with the stage, an empty stage naming where its work lives, and the collapse/reopen
  round trip (`DrawnState` gained `stage` and `rail` — the rail carries no text, so this is the only way a headless
  test can see it). The input harness now asserts every computed click lands on the canvas, which is what caught
  the coordinate assumptions breaking. Visual check: the shell was run on the machine's X display and captured
  (`/tmp/shell3.png`); it matches the artboard's structure. **Not** verified: pixel-level comparison at 1440 × 900,
  because the window manager maximises the frame to the full display — and the sample document opens the M5 3D
  preview, which the artboard's 2D shell does not show.
- **L14 (debt this slice adds, stated rather than hidden).** `Shell::rail_commands` builds the seven Edit commands
  from the same fields the Tools tab's buttons read: two construction sites for one command. The fix is a command
  module both call, which is a refactor with no visible effect and therefore its own slice. `view3d.rs` still holds
  its own shading constants (T9). Object context menus and the per-command help pairs (T8) are untouched.

- **L15 (03-iconography implemented, and the drift closed).** The glyph set itself landed in `db92b45`; this pass
  audited it against the artboard and found the only real drift: the code holds **70** glyphs and the artboard drew
  **66**, the difference being the four the app bar needs (`undo`, `redo`, `theme-light`, `theme-dark`). No glyph in
  the artboard was missing from the code — the code is an exact superset. The drawing was the authority last time,
  so the sheet gained the four, with a line saying where they came from, and the record now reads 70 across six
  families (11 + 12 + 13 + 10 + 16 + 8). `Icon::family()` groups the set for the in-app sheet the same way the
  artboard does, keyed on position in `Icon::ALL` with a test that a family is one contiguous run — a reordering
  that moved a glyph between families fails instead of mislabelling the sheet silently. Sheet renders at 760 px with
  `min_col_width(84)`, so `toolpath-profile` no longer prints over its neighbour.
- **L16 (incident, self-inflicted, recorded).** To photograph the sheet I flipped `show_icons` to `true` in the
  default and "restored" it with a `str.replace` that did not match, so the sheet stayed open by default and the
  input tests started failing because the window took the clicks meant for the canvas. The revert pattern asserting
  its own success is what would have caught it; the lesson is the same one as the gate swallowing its exit code —
  a cleanup that cannot fail is not a cleanup.

- **L17 (owner proposal, implemented: the two views are exclusive).** *"la vista 2d y la 3d creo que deberian de
  mostrarse solas, o la 2d o la 3d, no crees?"* — agreed, and it was a defect of this shell rather than of the
  artboard: the 3D preview was a `Panel::top` that took a slice of the frame and left the 2D canvas the rest, so both
  views were on screen at half size and neither was usable. The 3D view is now the **central area** when
  `Shell::view_3d()` is true, and the canvas is not built at all when it is not, so the toggle in the app bar is a
  mode. Two consequences worth naming: the hidden view costs nothing (no playback clock, no mesh, no region — the
  A12 policy survives, it just applies to the mode instead of the panel), and `view3d::PREVIEW_HEIGHT` is gone
  because the surface now fills whatever the frame gives it, which retires the "fixed height so the camera framing
  does not jump on resize" trade along with the stacking. The test that asserted the old behaviour
  (`the_2d_canvas_still_paints_while_the_panel_is_open`) was rewritten as `the_two_views_are_exclusive`: with 3D
  showing, `painted_items` and `painted_grid_lines` are exactly zero; back on 2D, `painted_triangles` is zero and
  the four edges plus the grid return.
- **L18 (a visual defect the screenshot caught, fixed).** The first run of the new frame showed the inspector's
  *Vectors* and *Nodes* headings printed one under the other with the Offset controls below both: two edits had each
  inserted a heading before the same `horizontal_wrapped`. The groups are now split where the node controls really
  begin. Recorded because it is the second time a screenshot found what 1 071 green tests did not — the tests pin
  behaviour, and grouping is not behaviour.

- **L19 (design comment applied, `bbqxd2m4`).** Owner comment on `03-iconography.html`'s `main`: *"me gusta ese tipo de
  estetica, junto con el design system de la primera, y el layout de la segunda"*. Applied to the artboard it names:
  the sheet now says in its lede that its colours, type, spacing and radii are the tokens in `design/system.md` (the
  same surfaces and two signals as `01-shell-2d`), and its six `<h2>` sections became **six family cards** with a
  heading, the count and one honest line about what the family is for — the layout `02-stages` uses. No glyph, rule
  or anatomy figure changed: the comment asks the sheet to *express* the combination, and the combination is the
  same drawing in the same tokens under the other board's structure.

- **L20 (owner delivered screenshots of the reference; the inference is retired).** *"sigue con la ui, te he
  dejado capturas de la otra para que te guíes. las tienes en design/Screenshots-reference"*. Eight frames from two
  videos about Prancheta CNC, annotated in `design/Screenshots-reference/README.md` with the pattern each one
  settles. Three findings are worth naming because they contradict or correct what the shell had inferred:
  *(1)* the reference's left panel is a **palette of labelled groups** (`CRIAR VETORES`, `TRANSFORMAR OBJETOS`,
  `EDITAR OBJETOS`, `CONTORNAR E POSICIONAR`), not a rail of glyphs — which is exactly the limit this shell hit
  when seven Edit commands had to hide behind unnameable icons; *(2)* its inspector is a stack of **uppercase
  sections with labelled fields two to a row** and paired fields that derive each other, not a list of sentences;
  *(3)* the top bar **changes with the mode**, which corroborates the exclusive 2D/3D decision.
- **L21 (implemented from the screenshots).** The 52 px rail is now a **200 px palette** (`Metrics::palette_width`)
  with the reference's group labels: `DRAW` (the nine gestures plus Add text), `TRANSFORM` and `VECTOR EDIT` on the
  Edit stage, `VIEW` always. Commands carry their group, so `palette_panel` renders runs and needs no second table
  saying which group a command is in. The Job tab became **labelled measurement boxes** in two columns
  (`MATERIAL`: Width X / Height Y / Thickness Z / Grid; `DOCUMENT`: toolpaths, layers) through a `field` widget.
  `DrawnState.rail` became `DrawnState.palette`, and two tests pin the new grammar: the group labels are painted per
  stage, and the Job tab shows its measurements labelled. Gates: fmt, clippy `-D warnings`, **1 073 tests** across
  91 suites. Artboard `01-shell-2d` was updated in the same pass (its rail became the palette with the same ten plus
  three buttons), and `design/system.md` now carries `palette_width` and the palette-group rule instead of the rail's.

- **L22 (the status bar becomes the reference's chip strip, and the shell gets a voice).** Three things the
  screenshots show at the bottom of every frame, implemented: **one chip per snap kind** (`Endpoint`, `Midpoint`,
  `Centre`, `Grid`) plus a `magnet` chip, which is real behaviour and not decoration — `Shell::toggle_snap` arms and
  disarms a kind and refuses to disarm the last one, because a snapping strip with everything off is a magnet with no
  field; **a hint line** at the right end, one sentence naming what the current gesture will do, taken from the
  reference's own `Clique para selecionar · arraste da esquerda p/ direita` and changed with the tool instead of lying
  about it; and **the `2D | 3D` toggle became a segmented control**, which is the shape the reference gives every
  exclusive small choice. Two pieces of copy were wrong and the screenshot caught them: the 3D hint read "the 2D
  drawing is behind the 2D tab", and the sample's status message repeated what the hint now says (a fresh window has
  no last command, so that slot is empty instead of chatty). `DrawnState.hint` reports the sentence for tests.
  Gates: fmt, clippy `-D warnings`, **1 076 tests** across 91 suites.

- **L23 (owner, verbatim).** *"primero mergea la rama a master"* and, in the same breath, *"y de ahi salimos limpio"*.
  `feat/camui-shell` merged into `master` with `--no-ff` at `508d92f`, parents `f24cfc2` + `9f60dea`: 29 files,
  **+6 730 / −431**, the four artboards, `design/system.{md,html}` and this document included. Not pushed. The tree is
  clean, and the two untracked outputs were resolved by understanding them rather than by deleting them: the design
  canvas state (`design/camui-shell/comments.json`, `handoffs.json`) is now committed — it records the owner's one
  design comment and the artboard the implementation was handed, so it belongs beside the artboards — and `data/`,
  the owner's 17 MB reference application, is ignored as a design input rather than committed.

- **L24 (incident, environment).** While a background explorer ran, every attempt to spawn a process failed with
  `EAGAIN` — `bash`, `codemode`, and even `echo` — and the harness reported no tool calls made, so nothing was lost.
  `free` and `uptime` came back normal once the subagent was cancelled (load 0.9, 24 GB available), which makes the
  subagent's own process tree the suspect, not the machine. Recorded because the failure mode is indistinguishable
  from a broken repository: when spawning dies, cancel the running subagent before concluding anything about the work.

- **L25 (owner, verbatim).** *"seguimos con t8"* — T8 authorised on `master` `508d92f`: object context menus and the
  embedded help panel, in the teaching voice, with one `d`/`u` pair per command (S2, S4).

- **L26 (review, blocked twice, and what the attempts taught).** Receipt-driven development flagged the branch as an
  unreviewed candidate (`sha256:80fde7a3…`, 29 paths, **7 178 changed lines**). Three attempts, in order:
  *(1)* `assess` agreed: `reviewDue: true`, native risk high, reason `process_boundary` on `crates/cam-ui/src/lib.rs`.
  *(2)* START on the whole candidate was refused in preflight with **`lens_context_budget_exceeded`** — no authority
  created, nothing to abandon. The rule this states plainly: a review candidate is a work unit, never the accumulated
  feature branch. *(3)* The owner chose to review only the risky part, so a temporary worktree
  (`.worktrees/camui-frame-review`, detached at `5592544`) anchored a candidate for **tokens + frame only**; START with
  two lenses returned `lenses-invalid`, one lens (`reliability`) was accepted, and authority
  `review-3ebc68eeba5006f5` was created (tier high, 4 640 original changed lines, correction budget 200).
  The capture forecast one host-relay model run; the run then failed with **`reviewer-config-invalid`**: *"no model is
  configured for review-reliability; assign it a model in the agent model routing config"*. The lineage stays
  `reviewing` and unburned — a fresh STATUS reoffers the same slot, so the review resumes the moment a model is
  assigned to the lens. Two mechanical lessons: **abbreviated commit ids are rejected** as `base-ref-unresolvable`
  (full 40-character ids only), and `baseRef..HEAD` always sweeps to the branch tip, so a middle commit can only be
  isolated from a worktree parked on it.

- **L27 (environment, and the gate could not run).** The session runs inside
  `gentle-shell-*.service` with **`MemoryMax=3G`** and **`TasksMax=64`**. Every `cargo test` link dies there:
  `rust-lld` / `collect2` abort with `std::system_error: Resource temporarily unavailable`, and bash itself reports
  `fork: retry: Resource temporarily unavailable`, with `memory.current` peaking at 3 026 MB of 3 072 MB. Serialising
  (`CARGO_BUILD_JOBS=1`, `RUST_TEST_THREADS=1`) does not help, and `cargo test -p cam-ui` alone fails the same way.
  What does pass inside the cap: `cargo fmt --check` and `cargo clippy --workspace --all-targets -D warnings`.
  The same cap is what killed the background explorer and several shell commands earlier in the session (L24) — it was
  never the repository. The unblock is a host setting, not a code change:
  `systemctl --user set-property gentle-shell-<id>.service MemoryMax=12G TasksMax=512` (runtime; the unit name carries
  a per-session hash, so a durable fix belongs in the gentle-shell launcher).
  **Resolved:** the owner raised the unit to `MemoryMax=16G` / `TasksMax=512`, and the full gate then ran clean on
  `master` `508d92f` — fmt, clippy `-D warnings`, **1 076 tests across 91 suites**, 0 failed, exit 0, in 1 m 03 s.
  That is the recorded baseline of `e2845f0` reached again after the merge, so the shell on `master` is verified.

- **L28 (map, and the four constraints T8 inherits).** A read-only pass over the shell fixed what T8 can build on.
  *(1)* **There is no command table.** `UiAction` (`lib.rs:392`) is 17 parameterised variants — `AddRectangle{width,
  height}`, not `AddRectangle` — and every user-visible label is a literal at its call site (`chrome.rs:497-548`).
  `rail_commands` returns `(&'static str, Icon, &'static str, UiAction)` with the group as a bare string, and
  `UiAction::label()` (`lib.rs:472`) only words the status bar. So a `d`/`u` pair cannot hang off `UiAction`: the unit
  that owns help is the *command*, which has no parameters. T8.1 introduces that identity.
  *(2)* **Selection is not shell state.** It lives on `AppState` (`Selection { ids: BTreeSet<String> }`,
  `cam-app/src/lib.rs:34`, read through `selection()`), and `handle_action` (`lib.rs:1987`) remains the only mutator —
  so a context menu scopes itself by asking `AppState` and emits `UiAction`s, never touching the document.
  *(3)* **Right-click is unused.** `PointerButton::Secondary` appears nowhere in `crates/`; the canvas senses
  `click_and_drag()` (`lib.rs:1451`) and that must stay. egui is **0.37**, whose `Response::context_menu` is the API,
  and no `menu`/`popup`/`context_menu` call exists yet: T8.3 is the shell's first.
  *(4)* **Two overlay precedents exist**: the `F2` icon sheet (`Shell::icon_sheet`, `lib.rs:871`) and the Shortcuts
  window (`chrome.rs:754`), whose app-bar tooltip already reads *"Shortcuts and help"*. The design draws a per-command
  card bound to `F1`, so the help panel is its own window and the shortcuts list keeps its own.
  The panel's shape and the three voice rules are already drawn in `02-stages.html` §04 — consequence before
  mechanics, second person without jargon, every refusal naming a number; the `d` is the first rule's one sentence and
  the `u` is the body that follows it.

- **L29 (T8.1 delivered: 33 commands find their voice, and `F1` opens it).** `crates/cam-ui/src/help.rs` (496 lines)
  holds the identity T8 needed: `Group` (DRAW, TRANSFORM, VECTOR EDIT, **HISTORY**, VIEW), `Command` (33 variants),
  `Help { what, how, related }`, `label()` reusing the literals the UI already shows, and `action()` for the commands
  that take no parameter. `chrome.rs::help_window` (+80) draws the design's card — grouped rows, the focused command's
  *what* as a strong paragraph, its *how*, then `Related:` — using only `theme::` tokens and the existing helpers.
  `lib.rs` (+40) carries `Shell::show_help`, `Shell::help_focused`, `DrawnState.help`, and the `F1`/`Esc` wiring beside
  the `F2` block. Seven tests (+219 in `tests/shell.rs`; no existing test altered), RED by build — `unresolved import
  cam_ui::help`, `no field show_help`, `no field help` — GREEN at **1 083 tests across 91 suites**, exit 0 (baseline
  1 076; +7).
  The writer's judgements, kept because each is a real decision and not an accident: **a fifth group, HISTORY**, because
  Undo/Redo are app-bar commands and filing them under VIEW would be a lie in the one place the shell is meant to be
  exact (T8.2 either gives the palette that run or folds them back); `Align` is in, since its six modes are live buttons
  in the Tools tab; `Import` is in although it has **no button and no shortcut** today, and its `how` says so plainly
  ("a host opens the file by path"); `Node edit` is one command naming all three of its buttons; `View3d` is labelled
  `3D`, the only word the shell shows for it. Left out deliberately: `SetSelection` (a click's effect, not a command),
  `ToggleLayerVisible` (a per-layer checkbox), the six Align modes (sub-choices of one `Align`), and stage/tab
  switching (view state, pushes no action).
  **Flagged for T8.2:** the palette's `Help` button still opens the Shortcuts window. `Command::Help` is the `F1` panel
  and `Command::Shortcuts` is that window; rewiring needs Shortcuts to keep a mouse path, so it is T8.2's job, not a
  side effect of this one.
  **Incident, and why the scope check matters.** The writer ran `cargo fmt` *without* `XDG_CONFIG_HOME`, the redirected
  `HOME` hid the repo's `tab_spaces = 2`, and the whole workspace was reformatted to four spaces. It re-ran `cargo fmt`
  with the variable and no `git checkout/restore/reset`; the parent then proved `git diff --name-only HEAD` lists only
  the three in-scope files plus this document, with `help.rs` untracked — so the revert is real and not asserted. The
  rule L9 found for the gate applies to the loose invocation too: **`cargo fmt` always needs
  `XDG_CONFIG_HOME=/home/th3g3ntl3man/.config` in this harness.**
  **Parent verification, independent of the report:** the scope check above, the gate re-run by the parent (exit 0,
  1 083 tests, 91 suites), and one factual spot check of the prose — `Trim`'s *how* says "the lower id is the target and
  the other one cuts it", which `lib.rs:1354` states and the arm at `lib.rs:1360` implements by destructuring
  `selected.as_slice()`. Help that describes behaviour the code does not have would be worse than no help.

- **L30 (owner disposition: the T8 slices ship unreviewed, on purpose).** `inspect` offered review.start again for
  the same accumulated candidate (29 paths, base `f24cfc2`, now past 8 000 lines), which is the shape the provider
  already refused with `lens_context_budget_exceeded`, and the lens that would have to run it still has no model. The
  owner chose *"Dejá T8 sin review por ahora y seguí"*. So the T8 slices carry the gate — `fmt`, `clippy
  -D warnings`, the full workspace suite — plus the parent reading the change against this document, and no native
  receipt. Two facts are recorded rather than buried: the frame lineage `review-3ebc68eeba5006f5` is still
  `reviewing` and **unburned**, waiting for a model routed to `review-reliability`; and the accumulated candidate
  cannot be started at all, because a review candidate is one work unit and never the branch. When the lenses get
  their model, T8.1 (`c23cfac`, 918 lines) is the natural first candidate — one worktree anchor away.

- **L31 (T8.2 delivered, and the differential that checked it).** `rail_commands`, `GroupedCommand`, `Stage::tools()`
  and `DESIGN_TOOLS` are gone. The palette derives every run, word, glyph and command from `Command::ALL` with
  `group()`, `label()` and `in_palette()`; the DRAW chips arm `active_tool` through the new `Command::tool()`, which is
  the only command→tool mapping left; the app bar takes Undo/Redo/Fit from their command with `can_undo`/`can_redo`
  still gating them; and both help paths now open the `F1` panel while a new VIEW `Shortcuts` chip keeps the keys sheet
  reachable by mouse. Six tests, plus two report fields (`DrawnState.palette_hits`, `DrawnState.app_bar`) because egui
  offers no widget-by-text lookup — a click test presses a recorded point instead of guessing at a margin. Gate:
  **1 089 tests across 91 suites** (+6), exit 0. Tooltips the rewrite made false were corrected with it: the help
  button no longer says "Shortcuts and help" now that it opens the help panel.
  Because the slice rewrites 443 lines of `chrome.rs`, the claim "same palette" was **measured, not trusted**: a probe
  in a worktree at `c23cfac` and the same probe in the working tree printed `DrawnState.palette` for every stage, and
  the transcripts were diffed. **One difference, on all five stages: `Shortcuts` after `Help`** — the chip the brief
  asked for, so a declared change rather than a regression. The group runs, their order, the headings (`DRAW`,
  `TRANSFORM`, `VECTOR EDIT`, `VIEW`) and the other twenty labels: zero differences, and the whole-frame painted text
  diffed word by word to zero. The verifier also corrected the writer's own wording: `in_palette()` is **not** the old
  palette — it has **four** VIEW chips, not three, because Shortcuts was added.
  That transcript is now a committed golden, `crates/cam-ui/tests/palette.rs` (two tests: the per-stage list in order,
  and no stage offering a history chip). A test that derives its expectation from the table cannot notice the table
  changing; writing the measured chips down can.
  **Residual, stated rather than hidden:** the palette no longer holds a *list*, but the *construction* of the
  parameterised actions — Move, Rotate, Scale, Mirror, Array, Offset, Add text, with their numbers — still exists
  twice: `chrome.rs::palette_action` and the Tools tab's `authoring()`. The brief asked for the list; folding the
  constructions into one module is its own slice. Also flagged: the icon set has no keyboard glyph, so `Shortcuts`
  uses `Icon::Text` with a comment instead of an invented glyph, and the app-bar `?` rewiring carries no mouse-level
  test because its rect is not reported — the palette chip test pins the same one-line toggle.

- **L32 (T8.3 delivered: the canvas grows a second mouse button).** Right-clicking the sheet opens a context menu with
  two scopes. **Object** — Move, Rotate, Scale, Mirror, Array, Offset, Join, Trim, Node edit, Delete selection —
  because every one of those takes its operand from the selection and would only refuse over empty canvas. **Canvas** —
  Add text, Add rectangle, Undo, Redo, 3D, Grid, Layers, Theme, Fit the job. Left out on purpose: `Align`, whose six
  modes *are* what Align means (a menu row that must then ask which one is a submenu nobody asked for), and `Select`
  with the nine gestures, since arming a gesture while opening a sheet over the drawing is two gestures at once.
  `help.rs::MenuScope` is the only list of either scope and it filters `Command::ALL`, so rows inherit the table's
  words and order; a row runs through `run_command`, the same runner the palette chips use. The scope is decided on
  the right-click in `handle_input` (`lib.rs:1865`) and remembered in `Shell::menu_scope` as **view state** — a
  right-click on empty canvas must keep the existing selection and still mean the canvas menu, which recomputing the
  pick inside the popup would break. Pointing at an unselected entity emits exactly one `UiAction::SetSelection`, the
  same effect a plain click has. The report surface is `DrawnState.menu` plus `DrawnState.menu_hits` — the popup only
  exists while the button has opened it and egui offers no widget-by-text lookup from outside a frame, so the tests
  press a recorded point (the same shape as L31's `palette_hits`).
  Nine tests (+275 lines in `tests/input.rs`), RED by build (`no field menu` six times, `menu_hits` once) — with the
  writer's own honesty that two Trim tests had no RED, since they were written after the menu compiled. Gate:
  **1 100 tests across 92 suites** (+9), exit 0. `Sense::click_and_drag()` is untouched, which the parent checked by
  grep: the drawing gesture is the one thing a context menu could silently break.
  **Differential on the shared code**, because this slice renamed `run_palette_command` → `run_command` and added four
  arms: every palette chip on all five stages clicked in a base tree and in the working tree with a shared scratch
  target, comparing emitted actions verbatim (Transform/Array/Offset/AddText payloads and camera centres included),
  armed tool, snapping, inspector, 3D, help panel, theme, status, hint, selection and the whole document JSON —
  **zero differences**; and the Tools tab's Trim button for selections `[E1]`, `[E1,E2]`, `[E2]` — zero differences.
  **Judgements kept:** the canvas menu says **"Fit the job"**, the table's own word, rather than inventing "Zoom to
  fit" (a second name for one command is exactly what the menu rule forbids); `Node edit` is a single row that can only
  mean move-node, so its hover slightly over-promises — flagged rather than papered over, and a future slice either
  turns it into a submenu or splits the three node edits into their own commands; `Trim` is the one dark row, and its
  target/cutter pairing moved into `chrome::trim_action` so the button and the row cannot cut differently.
  **egui 0.37 facts a later slice needs:** `Response::context_menu` opens on `secondary_clicked()` and
  `Sense::click_and_drag()` already satisfies it; a right-click does not make `clicked()` true; the popup is
  `at_pointer_fixed` with `CloseOnClick`, so it does not follow the pointer; its **first frame is a sizing pass**
  (invisible, not interactable), so a headless test needs one quiet frame before reading `menu_hits`; a press on a
  disabled row does not fall through to the canvas underneath; and the open state lives in the context, not in
  `Shell`, so a second right-click re-anchors instead of stacking menus.

- **L33 (the screenshot's defect, fixed, and re-checked by the same eye).** The visual pass on T8.3 found that the new
  `Shortcuts` chip was **indistinguishable from `Add text`**: both drew `Icon::Text` — the same `T` — and its tooltip
  read "The keys the shell answers to", which never names it. The tests passed because they read
  `DrawnState.palette` strings, not pixels: the L18/L22 class again. Fixed in three parts. A keyboard glyph — two
  keycaps over a space bar, `(3,5)+(7,6)`, `(14,5)+(7,6)`, `(6,14)+(12,5)` on the 24 grid — joined the set in the
  `ViewDocument` family right after `Help`, the range widening `46..=62` and the `Icon::ALL` length assert moving
  70 → 71. `palette_tooltip` now builds every entry from `Command::label()`, so no chip can depend on a sentence that
  never says what it is; the RED for that found a **second** unnamed chip (`Layers` said "Inspector — collapse or
  expand"). And two tests pin the pixel fact in the only way a headless test can:
  `shortcuts_has_a_glyph_and_a_word_of_its_own` (the two glyphs must differ) and
  `every_palette_chip_is_named_by_its_tooltip`. Gate: **1 102 tests across 92 suites** (+2).
  Re-checked on screen at the same 2533×1383 window: distinguishable at actual size without hovering, tooltip naming
  it, the sheet reading `71 glyphs` with `View, document, display · 17` and the new stroke matching the `?` beside it.
  Three honest leftovers, none of them fixed here: *(1)* adding the 17th member pushed `toolpath-pocket` onto a third
  row and the sheet now **clips `toolpath-profile`'s label** at its right edge — the same pre-existing column-width
  problem with a new victim; remedy is widening `min_col_width` or shortening the label, and it deserves the test that
  can see it. *(2)* A palette chip's tooltip still draws **above** the `F2` sheet when the pointer rests on a chip as it
  opens (pre-existing z-order). *(3)* `icons.rs` documents `tests::the_families_cover_the_set_in_order`, which **does not
  exist**; family contiguity rests only on `family()` being one ordered `match`, so the doc promises a guard the tree
  does not have. Recorded, not papered over.

- **L34 (owner, verbatim).** *"t9"* — T9 authorised from `8b860b7`: `view3d.rs` shading onto the tokens. The module holds a
  `shading` block with five own literals — `STOCK` `(88,132,178)`, `CUT` `(255,176,59)`, `TRAVEL` `(96,104,116)`,
  `SURFACE_TOP` `(214,220,230)`, `SURFACE_BOTTOM` `(38,74,122)` — plus `depth_colour`, which interpolates between the
  two surface ends by depth. `theme.rs` owns 18 roles and none of them is a ramp, which is exactly the question L7
  left open when it said a 3D surface ramp is not the 2D token set. So the first thing this slice does is **ask the
  tokens artboard**, not invent: `04-tokens.html` is the authority for whether a ramp exists, and if it does not, the
  roles it gains are named for what they mean and documented in `design/system.md` like every other.

- **L35 (T9: the tokens, the ramp, and the regression a screenshot caught before the commit).** `view3d.rs` paints
  every colour through the theme now — **zero `Color32` left in the module**, and the `shading` block plus
  `depth_colour` are gone, replaced by `Role::StockTop`/`Role::StockBottom` and `Palette::stock_shade`. `STOCK` mapped
  to `Border`, `CUT` to `Cut` (L7: the cut signal owns amber), `TRAVEL` to `TextDim` — by meaning, not by rename.
  **`04-tokens.html` specifies no material tokens at all**: its surfaces ladder ends at `bg-canvas`, "drawing surface
  — the darkest", with no step for stock, so the ramp is a **proposal** and `design/system.md` says so in those words.
  Then the screenshot refused to ship it: the proposed light ramp used `#FFFFFF` on a `#F7F9FB` canvas, which is
  **1.06:1 on paper and a 2-to-5 code difference on screen** — the light-theme stock stopped being a solid and became
  a hairline rectangle on a white page, the worst possible misread ("nothing is loaded"). It was a regression T9
  introduced, and 1 106 green tests did not see it because every existing contrast test compares *text to a surface*,
  never *material to the canvas*. The dark theme lost its accent in the same change: `STOCK`→`Border` took the stock's
  rim from 4.9:1 to 1.9:1, because a control hairline is not a solid's silhouette.
  The fix, measured rather than chosen: light `StockTop` `#FFFFFF`→**`#C6D1DD`** (1.06→1.47 token, **1.51 rendered**,
  Δ53 codes on the row the regression was found on), `StockBottom` `#8B95A3`→**`#A6B3C3`**, and a new **`StockEdge`**
  role held to 3:1 against *both* the canvas and the stock's face, because no existing neutral sits in that band in
  either set. The light canvas was measured as an alternative and rejected: moving it lowers every ratio it already
  guards. The ceiling is forced by the **cut**, not by taste — `#A86200` needs ≥3:1 on the fill, which pins
  `L_top ≥ 0.612` and caps the ramp at 1.50:1, so the light ramp narrowed from 3.03:1 to 1.40:1 as the price of a
  visible solid *and* a visible amber. The amber on the deepest stop is 2.23:1 (2.18 rendered), still under the 3:1
  stroke floor and **not fixable inside the tokens**: it needs a lighter paper amber — an artboard change — or a ramp
  so shallow it stops meaning depth.
  A new invariant holds it: `the_stock_reads_as_a_solid_against_the_canvas`, both sets, both ramp ends ≥1.45:1 off the
  canvas and the silhouette ≥3:1 off canvas and face — RED against `#FFFFFF` at 1.06:1 before the values moved.
  One more thing only rendering could teach: the silhouette test passed at token 4.90:1 while the screen delivered
  **2.32:1**, because a 1.0 px stroke is antialiased over two. The stock's edges now draw at **1.6**, the icon set's own
  line weight, so the invariant is also true of the pixels. Verified on screen afterwards: light fill 1.51:1 with a
  rim peak of `124,132,143` and visible thickness on the near edges; dark edges 3.79:1 off the canvas and 2.99:1 off
  the face, **92 % of the pre-T9 blue silhouette**; light 2D differing by 12 565 px of 3.5 M, all of them the cursor
  and the status line.
  **Recorded, not fixed:** the dark theme's amber on the bright stock top measures **1.37:1** — the pocket reads only
  because the raster is dense, and a sparser toolpath would vanish on that face (pre-existing, not caused here); and
  the depth ramp itself is **unobservable in the sample document**, whose pocket is 100 % covered by the cut raster, so
  neither a test nor an eye can see the gradient until a fixture has an uncut region.

- **L36 (the review could not run in this session, and why — the routing is frozen at startup).** Five attempts, one
  error every time: `reviewer-config-invalid`, "no model is configured for review-reliability". The cause is not
  misconfiguration and not the candidate: `session-profile-freeze.ts` states it — a session *"resolves the shared
  defaults … once, at startup, and keeps that snapshot with its origin. Changing a default later only affects new
  sessions."* So no file written while this session lives can reach the lens. Two candidate shapes were measured on the
  way: the accumulated branch is refused by `lens_context_budget_exceeded` (a review candidate is one work unit, never
  the branch), and this build accepts **one** lens per start — two returned `lenses-invalid` twice.
  What is left in place for the next launch: `install/agent/subagents.json` (the agent-home routing map, read by
  `agents-config.ts`) carrying the six review keys — the four lenses plus the refuter and the validator — each pointed at
  `opencode-go/deepseek-v4.1-flash` with thinking `high`, the model the explorer already runs on.
  **Owner disposition:** ship T9 on the gate and the two visual passes, and release the two live lineages
  (`review-89edc321cc30ebdb` for this slice, `review-3ebc68eeba5006f5` for the frame). The natural first candidates for a
  session that *can* review are `c23cfac`, `c22b7e1`, `3d8a620` and this T9 commit — each one work unit under the
  budget.

- **L37 (bookkeeping closed, and the merge).** Owner: *"empieza por el primero"*. Four statements in this document
  were false, all corrected in `1cc10b1`. **T9** read "in progress from `8b860b7`" although `72bd2e3` delivered it
  (L35). **T6** named commit `4318205`, which no branch contains: it is a pre-rewrite copy of `5592544` — same subject,
  1 489 lines, and both `git branch -a --contains 4318205` and `git merge-base --is-ancestor 4318205 HEAD` come back
  empty, so a session following this record would have detached into a commit that is not the frame. The T1–T5 rows
  carried a dash in the Commit column and read as pending, five tasks later; the hashes were already in the log
  (`db92b45` for T1–T4, `a37b5cf` for T5). And the heading claimed "planned, not started, not authorised" over six
  delivered rows.
  Then `d74becc` merged `feat/camui-t8-context-help` into `master` with `--no-ff`. **The merge changed no content**:
  `git diff --stat feat/camui-t8-context-help HEAD` is empty, so `master` holds the branch's own tree and every gate and
  visual pass recorded in L31–L35 applies undiluted rather than approximately.
  Gate on `master`, `XDG_CONFIG_HOME=/home/th3g3ntl3man/.config ./scripts/check.sh` with no pipe: exit 0, **1 107
  tests across 92 suites**, 0 failed, 0 ignored. That is one more than the 1 106 L35 recorded, and the discrepancy is
  itself a correction rather than a new test: the only commit between the two runs changes documentation, so the source
  trees are identical and one of the two counts was wrong. The measured one is 1 107; the earlier arithmetic was off by
  one.
  `master` is **9 commits ahead of `origin/master` and 0 behind**, and **not pushed** — publishing is the owner's
  decision, not this session's. Deliberately untouched: the merged branch `feat/camui-t8-context-help` (merged is not
  deleted), the untracked `.atl/` directory, and the two unburned lineages in L36, which a session that can reach a
  review model should still be the one to burn.

- **L38 (the review of T9 reached the provider and died at the session boundary, measured this time).** Owner:
  *"dale con todo"*. The cycle ran further than L36 did and stopped at the same wall, but the cause is now pinned to a
  timestamp instead of inferred.
  `assess` refused at first because `.atl/` is untracked: that directory is the generated skill-registry cache
  (`gentle-ai skill-registry refresh`), not part of any candidate, so the scope was declared `exclude`. `inspect` then
  offered the **accumulated** range `508d92f..cd94144` — 12 paths, 2 906 lines, T8.1 through T9 — which is the shape
  L36 already recorded as refused by `lens_context_budget_exceeded`. A review candidate is one work unit, never the
  branch, so START was narrowed with `baseRef` at `8b860b7`. That was refused with
  `native-start-retained-selection-candidate-mismatch` and `mutation_performed: false`: the retained intended-untracked
  selection belongs to the candidate it was computed for, so narrowing the range invalidated it. A second `inspect`
  with the narrowed range produced a fresh target (`sha256:615a946f…`) and the right six paths, and START accepted it:
  lineage `review-3592c554bf15c02f`, tier **high**, 6 files, 672 changed lines, one lens (`review-reliability`),
  correction budget 200.
  `collect` offered a single materialize slot; its forecast said one model run over `pi_host_relay`; the acknowledged
  capture returned `reviewer-config-invalid` — *"no model is configured for review-reliability"*.
  **The cause is a session boundary, and it is measurable:** `install/agent/subagents.json` carries all six review keys
  (four lenses, refuter, validator) and `models-store.json` knows `deepseek-v4.1-flash`, which is also this session's
  own `PI_MODEL`. But that file's mtime is **07:32** and this session started at **06:45** (`PI_SESSION_FILE`). A
  session freezes its agent routing at startup, so a file written 47 minutes later cannot reach it — L36's diagnosis was
  right and this is the proof rather than the report of it.
  **No retry.** A configuration failure is not repaired by repeating the capture, and the contract forbids replaying one
  from transcript inference. Three lineages are now stalled and unburned: `review-3ebc68eeba5006f5` (frame),
  `review-89edc321cc30ebdb` (T9's first shape) and `review-3592c554bf15c02f` (T9, this one). The session that burns
  them is one that starts **after** the routing file exists; nothing written while this one lives can reach the lens.
  T9 stays shipped on the gate, the two visual passes and the invariants L35 added — the disposition of L30 and L36
  unchanged, and now with a dated reason.

- **L39 (owner, verbatim).** *"dale con todo"* — the three fronts proposed after the merge: the review of T9, the
  slices the record still owes, and the screenshot patterns. The review is L38. What follows is the rest, split by
  what the code can actually support, measured rather than assumed.

## Slices from the screenshots (T10-T14)

| ID | Task | Specs | Route |
| --- | --- | --- | --- |
| T10 | ✅ delivered `f985255` — params.rs fold | S5 S7 | worker | `f985255` |
| T11 | ✅ delivered `a33e45f` — ProgressBar from view3d clock | S2 S4 S8 | worker | `a33e45f` |
| T12 | ✅ delivered `f4b96b1` — material dialog + SetJob positivity | S2 S7 S8 | worker | `f4b96b1` |
| T13 | ✅ delivered `6014611` — Stage readiness: the rule that makes Design and Toolpaths *done*, and the string naming what is missing. Relief and Prove keep `—`, because no simulation result and no G-code state reach `AppState` | S2 S4 S9 | worker |
| T14 | ✅ left this feature — the current-tool card and the estimated time became `odd/tasks/012-camui-toolpath.md` and landed there: the card `ffe4d68`, the estimate `126febc`, both merged in `master` (`abe2e7b`). The missing layer this row named (no tool library handle on `Shell`, no toolpath authoring) is the debt that feature left open | S2 S8 | — |

- **S8 — the screenshots are the pattern authority.** Eight frames of the reference live in
  `design/Screenshots-reference/`, and `README.md` annotates each one with the pattern it settles. Where a screenshot
  and an inference from the `.exe`'s strings disagree, the screenshot wins.
- **S9 — counted, never invented.** A stage that cannot be measured says so with a dash and a sentence, rather than
  showing a number nothing computes. The rule is already in `chrome.rs:25-28`; T13 is where it starts to bite, because
  two of the five stages become countable and three do not.

- **L40 (the plan after the handoff, and the two places it changed).** A read-only exploration of the shell measured
  what each slice needs, and it moved two of them. (1) **T14 is not implementable now**: the cutting tool lives in
  `cam-store`/`cam-toolpath` and each `Toolpath` carries only a `tool_number` resolved against `Document.tools_ref`,
  none of which `cam-ui` can reach — `cam-ui` has no dependency on either crate and no library handle on `Shell`.
  An estimated time would have to be invented, which S9 forbids. It stays unstarted. (2) **T12 carries a defect it
  found rather than created**: `cam_app::Command::SetJob` assigns width, height and thickness with no positivity
  check (`cam-app/src/command.rs:433-443`) while `Job::outline` panics on non-positive dimensions
  (`cam-model/src/lib.rs:180-185`). A dialog is the first thing that can feed it a zero, so the validation travels
  with the dialog. Two facts keep the other three honest: the only real time computation in the workspace is
  `Preview3d`'s clock (`view3d.rs:106-107,260-291`), which is what T11 must read instead of inventing one; and the
  seven duplicated constructions produce **identical** `UiAction`s today — every difference lives outside the action,
  in widget metadata and in the `array_grid` range that only the Tools tab applies (`lib.rs:1333-1334`), so T10 is a
  pure fold and its risk is a silent behaviour change, not a design question.
  Route: four writers **sequentially**, not in parallel, because all four surfaces include
  `crates/cam-ui/src/lib.rs` and parallel writers on one file are a collision, not a speed-up.

- **L41 (three of the four screenshot slices delivered, each gate re-run by the parent).** `f985255` (T10),
  `a33e45f` (T11), `6014611` (T13). The counts below are the parent's own runs of
  `XDG_CONFIG_HOME=/home/th3g3ntl3man/.config ./scripts/check.sh`, not the writers' claims: 1 109 → 1 116 → 1 123
  tests across 92 suites, exit 0 each time.
  **T10** moved the seven parameterised commands into `params.rs` with the canonical `array_grid_range()` read by both
  the builder and the widget. Its RED is worth keeping: *"the Array chip must run a grid inside the form's range:
  1..=64 / left: (0, 99) / right: (1, 64)"* — and the test keeps the inspector on **Selection** because a visible
  `DragValue` clamps its own field as it is drawn. The runtime exposure was therefore narrower than the brief assumed:
  the disagreement only existed while the form was off screen. The differential still found it, 3 differing lines out
  of 77, all of them the declared clamp. The pinning test has no RED by construction — both routes already agreed on
  the payload; the differential measured that rather than a test asserting it.
  **T11** replaced the text percentage with a real `ProgressBar` fed by the clock `view3d` already owned, with `m:ss of
  m:ss` from two pure functions. `bar_fill` paints empty unless the total is a positive finite number, because
  `progress()` answers 1.0 for a zero-duration program once played out — true about the program, a lie about the time.
  Fill is `Role::Cut`, the same amber the panel paints cutting moves with; corners are square because egui paints the
  fill at least as wide as the radius and a rounded bar would show a bead of progress at 0.0. One of its three
  integration tests finds the bar's two rectangles **by role colour**, so a fill painted with a literal fails the test
  instead of passing a string assertion. Verified on screen at 1:37, 3:57 and 6:31 of 6:31 — and the environment note
  is corrected: `grim` is Wayland-only and needs `XDG_RUNTIME_DIR`/`WAYLAND_DISPLAY`, so `DISPLAY=:0` alone cannot
  capture here (L35 and L33 both said it could; on this session it could not).
  **T13** gave the bar a three-way `Readiness` (`Ready(n)` / `NotReady(sentence)` / `Unmeasured`) and rendered
  `Stage::count` **from** it, so the number and the state cannot diverge. It also fixed the Job section printing
  `Stage::Toolpaths.note()` on all five stages.
  **Three debts, recorded rather than smoothed over.** *(1)* In the light theme the progress bar's **trough** is
  `Role::Canvas` on `Role::Panel` at 1.06:1, so the unfilled half is invisible on paper: the fill and the sentence
  still carry the meaning, but "where does full end" is lost. No role in the set is a field/track surface, and adding
  one is an artboard decision like L35's ramp — not a side effect of a widget. *(2)* T9's `colours_in_the_3d_panel`
  audit matches the surface viewport clip, which excludes the playback control row, so that audit never looked at the
  new bar (its own test does, by role). *(3)* The stage gates in `02-stages.html` are stricter than any fact `AppState`
  holds — "every vector either closed or knowingly open", "every active path has a tool, a depth inside the stock" — so
  `Stage::readiness` implements the artboard's *conclusion* on the one fact the document carries and the module doc
  says plainly that the count is a **lower bound on the gate, not a certificate**. The artboard also labels Relief
  "optional" while its own bar draws a dash; the bar won, because "counts, never adjectives" is its own rule.
  **Method**: four writers, **sequential** by necessity — every one of the four surfaces included
  `crates/cam-ui/src/lib.rs`, and parallel writers on one file are a collision, not a speed-up. The writers found three
  things this record's briefs had wrong (the DragValue clamp, `Preview3d::progress` answering 1.0 at zero time, and the
  artboard gates being unreachable), which is the argument for stating measured facts in a brief instead of conclusions.

- **L42 (T12 delivered by a writer that vanished, and this feature closed).** `f4b96b1`. Gate: exit 0, 92 suites,
  **1 132 tests** (+9) — the parent's run, not a writer's claim, because there was no writer's claim to check.
  The Job section's numbers are editable now: a material dialog with the sheet card and the rotary one over a plan
  drawn live with the theme's painters and annotated as the numbers move, writing through `UiAction::SetJob` into
  `cam_app::Command::SetJob`.
  **The defect it closed is one layer down, and it was found rather than created.** `SetJob` assigned width, height
  and thickness with no positivity check while `Job::outline` panics on a non-positive dimension; `AppState` catches
  a zero only by validating *after* the assignment, and `apply` is public, so a caller handing a document in directly
  got an invalid job and the next `outline` panicked. The refusal now runs before anything is assigned and reuses
  `ModelError::NonPositiveJobDimension` instead of spelling one rule twice. The rotary card is drawn, cannot be
  chosen, and names its own absence — the document has no axis mode and `cam-model` and the format were out of scope,
  so a card that wrote nothing would have been a dead control.
  **The incident, recorded because it changed how this slice was verified.** The writer's task **disappeared from the
  registry without returning a report**, leaving 1 110 lines and no RED, no defended decisions and no gate. What it
  left also failed the gate: 18 tests in `cam-ui --test input`, all of them clicks that stopped reaching the canvas.
  The cause was `Shell::new()` shipping `show_material: true` — a dialog open at birth swallowing the first click,
  which is L108's `show_icons: true` wearing a different hat. Now `false`, with the reason written at the value.
  So nothing here rests on a report: the refusal is asserted to name the field and the number and to leave
  `document.job` unchanged, `outline()` is reachable afterwards without a panic, the rotary card is asserted to write
  nothing, `the_plan_is_a_drawing_and_not_a_caption` counts painted quads so the diagram is checked as a drawing, and
  the diff adds zero `Color32` literals. The 1 110-line single commit is declared in its message rather than hidden:
  the dialog, the plan, the action path and the validation are mutually dependent.
  **Two rows in this document close with it.** **T7** (the stage model) is delivered by **T13**, not abandoned: the
  readiness rule, the gates it can and cannot certify, and the strings that name the next step are exactly what T7
  described. **T14** (the current-tool card and the estimated time) left this feature: it became
  `odd/tasks/012-camui-toolpath.md`, because the exploration showed `cam_app` has no toolpath command at all and the
  shell cannot reach the tool library — a missing layer, not a missing button.
  **What this feature leaves behind, none of it hidden**: the light theme's progress trough at 1.06:1 with no
  field/track role in the token set; a stage readiness that is a lower bound on the artboard's gates rather than a
  certificate for them; a colour audit whose clip excludes the playback row it now misses; and three review lineages
  stalled and unburned (`review-3ebc68eeba5006f5`, `review-89edc321cc30ebdb`, `review-3592c554bf15c02f`) waiting for
  a session that starts after the routing file exists.

- **L43 (owner, verbatim: `ya he mergeado yo todo e hice limpia, por donde seguimos`. Two rows corrected; the feature is
  closed and this entry is the only thing that was open).** Nothing was rebuilt here and no code changed: the tables
  above disagreed with L42, and a stale row is what sends the next session to re-deliver something that already
  exists. **T7 → `6014611`**, verified in the tree rather than in the log: `crates/cam-ui/src/chrome.rs:184` is
  `pub fn readiness(self, app: &AppState) -> Readiness`, the five-stage gate rule and the naming strings L41/L42
  describe; the heading that called it *planned and not authorised* now says delivered. **T14 → left the feature**,
  delivered as `camui-toolpath`'s T1 `ffe4d68` and T2.1 `126febc`, merged into `master` at `abe2e7b`. **What actually
  remains, unchanged by this entry**: the four debts L42 lists (contrast, lower-bound readiness, colour-audit clip,
  three stalled lineages) and `camui-toolpath`'s own hand-off — the shell still cannot open a `.c2d`, so
  `Document.tools_ref` remains a filename only `cam-cli` resolves. `master` is 33 commits ahead of `origin/master` and
  still unpushed; the owner merged and cleaned the branches, not the remote.

- **L44 (the hand-off L43 recorded is now half closed: the shell opens a `.c2d`).** The follow-up feature that L43
  named as `camui-toolpath`'s hand-off landed as `odd/tasks/014-shell-open-document.md` — its S2–S4, not its T4, and
  uncommitted in the working tree when this entry was written. The app bar's Open button emits
  `UiAction::RequestOpenFile` and nothing else (`crates/cam-ui/src/chrome.rs:334-348`); the host answers it with
  `rfd::FileDialog::pick_file` (`crates/cam-desktop/src/main.rs:95-102`), which is the file-dialog dependency
  decision this project had left undecided, taken in the **host** crate only — `cam-ui` gains no dialog crate; and
  `UiAction::OpenFile { path }` loads through `cam_store::document::DocumentFile::load`, the same archive scan,
  `migrate()` and `validate()` `cam-cli`'s own loader performs for every command that reads a document
  (`crates/cam-cli/src/main.rs:218-223`, used by `validate`, `inspect`, `check`, `emit` and `simulate`), and
  replaces the drawing through `replace_document_undoable` (`crates/cam-ui/src/lib.rs:2320-2336`) so the open is one
  undo away. `Shell::document_name` (`:1041-1048`) then names the file where the bar said `untitled.c2d`, and the
  success-only reset (`:1013-1020`) clears `preview`, `tools` and `material_size` and re-frames. Six tests in
  `crates/cam-ui/tests/shell.rs:1781-2012` pin it: the name in the bar, three refusal cases (missing, not an archive,
  invalid) leaving document, history depth, dirty flag and name untouched, the cleared preview/snapshot/drafts, one
  undo back to the `document_json` the window started from, the re-frame, and the Open button emitting the intent
  alone. **What L43's sentence still says, and this entry does not change:** `Document.tools_ref`
  is still a filename only `cam-cli` resolves. Opening *clears* the tool snapshot (`lib.rs:1017`, and
  `cam-desktop/src/main.rs:76` still passes `set_tools(Vec::new())`); filling it from the opened document's
  library is the next slice, because resolving that filename the way `cam-cli` does is a rule nobody has moved yet.
  **No gate was run for this entry**: it is documentation, and the code it describes arrived with its own.

- **L45 (L44's other half: the window writes the `.c2d` it opened, and the archive's other entries survive it).**
  The feature `odd/tasks/015-shell-save-document.md` specifies landed as its S2–S5 and T1–T3; T4's gate, commits and
  independent verification stay the parent's, so this entry cites no commit — the code is uncommitted in the working
  tree. `UiAction` gained `Save`, `SaveFile { path }`
  and `RequestSaveAs` (`crates/cam-ui/src/lib.rs:508-530`); the app bar draws Save and Save as beside Open
  (`crates/cam-ui/src/chrome.rs:350-369`), in the order `undo · redo · open · save · save as · fit the job`
  (`crates/cam-ui/tests/shell.rs:2802-2825`), and the host answers `RequestSaveAs` with `rfd`'s `save_file()`
  seeded with the name the bar shows — `rfd::FileDialog::new().add_filter("campilot document", &["c2d"])
  .set_file_name(...)` (`crates/cam-desktop/src/main.rs:107-114`) — the same one-dialog route Open took, so
  `cam-ui` still names no dialog crate. The write is `cam_store::document::DocumentFile::save`
  (`crates/cam-store/src/document.rs:93-104`), which validates first (`:96-102`) and goes through
  `write_atomically` (`:168-187`): a sibling `.{name}.tmp` (`:190-196`) then a rename, never a truncating write in
  place. It is **not** `save_document` (`:148`), which builds a `DocumentFile` with `extras: BTreeMap::new()` on
  purpose for a CLI document that authors a new file: the shell carries the entries it does not interpret in
  `document_extras` (`lib.rs:844`, `:1085-1087`), an open fills them (`:2439-2456`), and a save writes them back
  under their own names — the half of this that matters, since the alternative is deleting an entry a later build
  understands. A save whose entry point holds no shell refuses instead of writing a document-only archive over the
  file (`NO_SHELL_FOR_SAVE`, `:2316-2318`, reached by the `SaveFile` arm at `:2487-2503`), and `Save` with no
  recorded path refuses in one sentence that names Save As (`NO_RECORDED_PATH`, `:2309-2310`, arm `:2469-2486`).
  Two lies of the open feature are gone with it: a freshly opened document is no longer dirty (`mark_saved()`
  after the undoable replacement, `:2450`, so the bar stops saying `· modified` over a file nobody touched), and
  opening over unsaved work is refused with *"… has unsaved changes — save or undo them before opening …"*
  (`:2424-2434`) instead of discarding the edits with undo as the only way back; the refusal files the failure
  under `refused: …` at `:1069`, as every other refusal does. Eight tests in `crates/cam-ui/tests/shell.rs:2061-2470`
  pin the behaviour: the refusal with no recorded path and the empty directory it leaves, the write plus the
  cleared flag plus the three-entity document read back from disk, Save as becoming the file the bar names and the
  old name leaving it, an `assets/logo.png` entry surviving a load → edit → save, a failed save leaving the flag,
  the recorded path and the previous bytes untouched, the dirty-open refusal with the save that then lets the open
  through, both app-bar buttons emitting exactly their action, and the bare `handle_action` refusing both save
  actions while the same action through the shell writes the entries the shell is holding.
  **What it does not fix, unchanged from L43/L44:** `Document.tools_ref` is serialized in `document.json` and now
  survives a load → save round trip, and the window still does not resolve it — opening keeps clearing the tool
  snapshot, so the tool card still has nothing to resolve against and that stays the next slice. There is also no
  keyboard shortcut for Save: the shell's key handling is `Ctrl+Z`, `Ctrl+Shift+Z`, `Delete`/`Backspace`, `F`, `F1`,
  `F2` and `Esc` (`lib.rs:2097-2118`). The insertions moved L44's citations by roughly forty lines (`lib.rs:1013-1020`
  is now `:1061-1066`); its numbers are left as written, because a period record is not renumbered.
  **No gate was run for this entry**: it is documentation, and the code it describes arrived with its own.

- **L46 (the sentence L40, L44 and L45 shared is now false: an open fills the tool snapshot from the library
  the document names).** The three entries said the same thing in three tenses — `Document.tools_ref` "is still
  a filename only `cam-cli` resolves", an open "keeps clearing the tool snapshot", and "filling it from the
  opened document's library is the next slice, because resolving that filename the way `cam-cli` does is a rule
  nobody has moved yet". That slice is `odd/tasks/016-shell-tools-ref.md`: on a successful open the shell resolves
  the loaded document's `tools_ref` **relative to the document's own directory** through `tools_beside_document`
  (`crates/cam-ui/src/lib.rs:2752-2798`) and hands `ToolLibrary::tools()` to `set_tools` (`:1081-1090`,
  `:1169`), so the snapshot an open leaves behind is the document's library instead of nothing. L40's reason for
  leaving the estimate unstarted — "`cam-ui` has no dependency on either crate and no library handle on `Shell`"
  — is moot for exactly this half: the resolver imports `cam_store::tools::ToolLibrary` itself (`:49`), which is
  the crate `cam-ui` already reached for the load. **The rule the window took is stricter than the CLI's, on
  purpose:** a reference that is absolute or carries a `..` is refused before any filesystem call (`:2773-2781`),
  and existence is checked with `try_exists` before `ToolLibrary::open` (`:2788-2795`) because that `open`
  (`crates/cam-store/src/tools.rs:220-242`) creates the parent directory and the file it is pointed at — so a
  document can neither read outside its own directory nor make the window write anything. That stricter rule is
  the window's alone: `cam-cli` still joins the same reference with no guard at all
  (`crates/cam-cli/src/main.rs:1828-1844` for `emit`, `:1924-1942` for `simulate`, `:1634-1659` for the
  `toolpath` default), so it still accepts an absolute `tools_ref` and one that climbs out through `..`, and
  nothing in the repository pins that either way — the only CLI tests that set a `tools_ref` set an absolute
  path (`crates/cam-cli/tests/cli.rs:49-85`, `:976`, `:1017`). **What the containment still does not buy, stated
  rather than implied:** it is **lexical**. `tools_beside_document` refuses on the reference's own components and
  never canonicalises, so a symlink *inside* the document's directory that points outside is still followed and
  read; it cannot *create* anything out there, because `try_exists` follows the link and a dangling one reads as
  absent, so the write surface is closed and what stays open is reading a file whoever owns that directory put
  there. Canonicalising would close that read and would also refuse a legitimate shared-library symlink, so the
  trade is stated and not taken silently (`shell-tools-ref` L3). Two cases stay deliberately unpinned:
  `sub/../tools.sqlite` is refused although it lands back inside, and an unreadable library is untested — only
  "missing" and "not a library" are. Failure is soft: the drawing opens, the snapshot stays empty, the card keeps
  its own "not in the tool library this window was given" sentence (`crates/cam-ui/src/chrome.rs:822-830`), and
  the open's own sentence gains an appended ` — …` naming the library and the reason (`:1100-1102`), which is
  S9's rule holding rather than bending: a stage that cannot be counted says so. `cam-desktop` still passes
  `set_tools(Vec::new())` at launch (`cam-desktop/src/main.rs:73`) and now records why the host never needs the
  rule. Six tests in `crates/cam-ui/tests/shell.rs:1971-2203` pin the resolution, the two hostile references and
  the document-that-names-nothing case. **The code is uncommitted in the working tree** when this entry was
  written, so it cites no commit. **No gate was run for this entry**: it is documentation.
