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
| T1 | `cam-ui/src/theme.rs`: dark + light token sets (surfaces, text, two signals, spacing, radii, control heights, type scale), `Theme`, `apply(ctx)`, AA contrast pinned by test | S1 S4 S6 S7 | inline | — |
| T2 | `cam-ui/src/icons.rs`: 65 glyphs on one 24 × 24 grid as a path DSL, `paint_icon` painter, tint from the theme, 1.6 stroke at 24 | S1 S2 S3 S6 | worker | — |
| T3 | Icon sheet as an `egui::Window` behind a keystroke, so all 65 can be inspected without touching the layout | S3 S5 | inline | — |
| T4 | Apply the theme at startup in `cam-desktop` and route the existing `palette` through it: the shell changes colour, the layout does not change | S1 S5 S7 | inline | — |
| T5 | `design/system.md` + `design/system.html` synced from `theme.rs` once it is the project's token source | S3 | inline | — |

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
- **L4 (evidence, T1 and the design slice).** Branch `feat/camui-shell` from `master` `e9dcb01`. The design
  deliverable is one commit, `d8e1ff9` (1 701 authored lines, all of them HTML artboards and the design system):
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
  passed** across 91 suites (baseline 1 056 at `e9dcb01`, so +11: five for the tokens, six for the glyphs). The
  slice is 2 588 new lines plus ~90 changed in `lib.rs`; it is one commit because the three parts are mutually
  dependent — the tokens have no meaning without the set and the routing that consumes them, and splitting them
  would either leave an undeclared module whose tests never run or a commit that does not compile.
- **L9 (environment, two more instances of the same root cause).** `cargo fmt --check` reported the whole repository
  as unformatted because rustfmt reads `$XDG_CONFIG_HOME/rustfmt/rustfmt.toml` (`tab_spaces = 2`) and the redirected
  `HOME` hid it: the gate has to run as `XDG_CONFIG_HOME=/home/th3g3ntl3man/.config ./scripts/check.sh`. The
  symlink alone is not enough, which is worth knowing before trusting a green `check.sh` in this harness. The first
  gate run also appeared to pass while failing, because `check.sh | tail` swallows the exit status — the gate must
  be run without swallowing it.
## Next phase (planned, not started, not authorised)

The foundations are delivered. What remains of the concept, in the order the design argues for:

| ID | Task | Specs |
| --- | --- | --- |
| T6 | ✅ delivered — the shell frame: app bar, stage bar, the 52 px stage-scoped rail, the 288 px collapsible inspector, the status bar. `01-shell-2d.html` implemented; commit `4318205` | S1 S2 S3 S7 |
| T7 | The stage model: five stages with gates, the counts that make a stage done, and the strings that name the next step | S2 S4 |
| T8.1 | 🔨 in progress — `help.rs`: a command identity that is *not* `UiAction` (an action carries a width), one `d`/`u` pair per command, and the `F1` help panel that renders it | S2 S4 |
| T8.2 | The palette reads the command table instead of seven hardcoded labels and three inline group literals, closing the two-site construction recorded in L14 | S2 S4 |
| T8.3 | Object context menus on the canvas: the first use of `PointerButton::Secondary` and of `Response::context_menu` here, emitting `UiAction`s only | S2 S4 |
| T9 | `view3d.rs` shading onto the tokens — the surface ramp is the last place with its own colours | S1 S7 |

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

- **L15 (03-iconography implemented, and the drift closed).** The glyph set itself landed in `3ecf6ac`; this pass
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
  `feat/camui-shell` merged into `master` with `--no-ff` at `c24f690`, parents `e9dcb01` + `46d73f2`: 29 files,
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

- **L25 (owner, verbatim).** *"seguimos con t8"* — T8 authorised on `master` `c24f690`: object context menus and the
  embedded help panel, in the teaching voice, with one `d`/`u` pair per command (S2, S4).

- **L26 (review, blocked twice, and what the attempts taught).** Receipt-driven development flagged the branch as an
  unreviewed candidate (`sha256:80fde7a3…`, 29 paths, **7 178 changed lines**). Three attempts, in order:
  *(1)* `assess` agreed: `reviewDue: true`, native risk high, reason `process_boundary` on `crates/cam-ui/src/lib.rs`.
  *(2)* START on the whole candidate was refused in preflight with **`lens_context_budget_exceeded`** — no authority
  created, nothing to abandon. The rule this states plainly: a review candidate is a work unit, never the accumulated
  feature branch. *(3)* The owner chose to review only the risky part, so a temporary worktree
  (`.worktrees/camui-frame-review`, detached at `7e09653`) anchored a candidate for **tokens + frame only**; START with
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
  `master` `c24f690` — fmt, clippy `-D warnings`, **1 076 tests across 91 suites**, 0 failed, exit 0, in 1 m 03 s.
  That is the recorded baseline of `6e59b91` reached again after the merge, so the shell on `master` is verified.

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
  the same accumulated candidate (29 paths, base `e9dcb01`, now past 8 000 lines), which is the shape the provider
  already refused with `lens_context_budget_exceeded`, and the lens that would have to run it still has no model. The
  owner chose *"Dejá T8 sin review por ahora y seguí"*. So the T8 slices carry the gate — `fmt`, `clippy
  -D warnings`, the full workspace suite — plus the parent reading the change against this document, and no native
  receipt. Two facts are recorded rather than buried: the frame lineage `review-3ebc68eeba5006f5` is still
  `reviewing` and **unburned**, waiting for a model routed to `review-reliability`; and the accumulated candidate
  cannot be started at all, because a review candidate is one work unit and never the branch. When the lenses get
  their model, T8.1 (`f06b43a`, 918 lines) is the natural first candidate — one worktree anchor away.

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
  in a worktree at `f06b43a` and the same probe in the working tree printed `DrawnState.palette` for every stage, and
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
