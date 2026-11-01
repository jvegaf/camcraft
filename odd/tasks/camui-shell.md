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
