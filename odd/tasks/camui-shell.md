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
