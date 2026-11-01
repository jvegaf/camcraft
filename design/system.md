# Campilot design system

The tokens the shell draws with. Every value here is read from its source — nothing is invented, and a value
that appears in only one of the two places is a bug in one of them.

**Sources**

| Source | What it owns |
| --- | --- |
| `crates/cam-ui/src/theme.rs` | Every colour, type size, spacing step, radius and control height, plus the `Role` enum widgets ask by |
| `design/camui-shell/04-tokens.html` | The design authority: why these values, and the component states they produce |
| `design/camui-shell/03-iconography.html` | The icon language: 24 × 24 box, 1.6 stroke, dashed = virtual, 22 % wash = kept |
| `crates/cam-ui/src/icons.rs` | The icon set as geometry, transliterated from that artboard |

The Rust module is the runtime truth; the artboards are the argument for it. If they disagree, the artboard is
the intent and `theme.rs` is the defect — except for contrast, which `theme.rs` proves by test.

## Colour

Two sets, one design. A widget never names a colour: it asks `Palette::get(Role::…)`, so both sets re-resolve.
Values are `sRGB` hex; `-soft` entries are premultiplied RGBA washes used as fills behind their signal.

### Dark (default) — `theme.rs::Palette::dark`

| Token | Value | Usage |
| --- | --- | --- |
| `app` | `#12151a` | Window frame, app bar |
| `canvas` | `#0d1014` | Drawing surface — the darkest surface in the set |
| `panel` | `#171b21` | Rail, dock, status bar |
| `raised` | `#1e242b` | Inputs, hovered rows |
| `overlay` | `#232a32` | Menus, tooltips, popovers (the only surfaces allowed a shadow) |
| `hairline` | `#262d36` | One-pixel divisions |
| `border` | `#35404c` | Input and control borders |
| `text-hi` | `#e7ecf3` | Titles, values, labels |
| `text` | `#c3ccd8` | Body, list rows |
| `text-dim` | `#8b95a3` | Help, units, secondary information |
| `text-off` | `#5c6673` | Disabled only — deliberately below AA |
| `sel` | `#6fd3e8` | The selection signal |
| `sel-soft` | `#6fd3e822` | Selection fill |
| `cut` | `#f5a623` | The cut signal: toolpaths, depth, the one primary action |
| `cut-soft` | `#f5a62322` | Cut fill |
| `ok` | `#5ec27f` | Verdict: verifies |
| `warn` | `#f0b429` | Verdict: legal but unwise |
| `danger` | `#f2635f` | Verdict: gouge, crash, refusal |

### Light — `theme.rs::Palette::light`

| Token | Value | Usage |
| --- | --- | --- |
| `app` | `#eef1f5` | Window frame, app bar |
| `canvas` | `#f7f9fb` | Drawing surface |
| `panel` | `#ffffff` | Rail, dock, status bar |
| `raised` | `#f2f5f8` | Inputs, hovered rows |
| `overlay` | `#ffffff` | Menus — a shadow, not a fill, separates them |
| `hairline` | `#dde3ea` | One-pixel divisions |
| `border` | `#c3ccd6` | Control borders |
| `text-hi` | `#12181f` | Titles, values |
| `text` | `#2b3440` | Body |
| `text-dim` | `#5f6b7a` | Help, units |
| `text-off` | `#8b95a3` | Disabled only |
| `sel` | `#0d7c92` | Selection signal, darkened to pass AA on white |
| `sel-soft` | `#0d7c9218` | Selection fill |
| `cut` | `#a86200` | Cut signal |
| `cut-soft` | `#a8620014` | Cut fill |
| `ok` | `#1f7a43` | Verdict: verifies |
| `warn` | `#96610a` | Verdict: warns |
| `danger` | `#b3342f` | Verdict: stops |

### Contrast, proven not asserted

`theme.rs` computes WCAG ratios and fails the build if a pair the shell writes on drops below **4.5:1** for text
(`text-hi`, `text`, `text-dim` on `app`, `panel`, `canvas`) or below **3:1** for a signal drawn as a stroke
(`sel`, `cut`, `ok`, `warn` on `panel` and `canvas`). `text-off` is excluded because it is disabled-only.
Two further invariants are tests, not conventions: the canvas sits at the end of the luminance ladder, and the two
signals are at least 60° apart in hue.

## Typography

Font stack: `Inter, "Inter UI", ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif`. Nothing is
fetched at run time.

| Style | Size | Weight | Usage |
| --- | --- | --- | --- |
| `hero()` | 28 | 600 | Empty states, one line, centred |
| `dialog()` | 20 | 600 | Dialog titles |
| `title()` | 15 | 600 | Dock sections |
| `TextStyle::Body` | 13 | 400 | Everything readable |
| `TextStyle::Button` | 13 | 500 | Button labels |
| `TextStyle::Small` / `caption()` | 12 / 11 | 400 | Secondary rows / status bar and units |
| `TextStyle::Monospace` | 12 | 400 | Every number that may be compared |

Numbers use tabular figures and live in columns; a unit appears once per column, never per field.

## Geometry — `theme.rs::Metrics::shell`

| Token | Value | Usage |
| --- | --- | --- |
| `spacing` | `2, 4, 6, 8, 12, 16, 24, 32` | The only gaps allowed |
| `radius_control` | `4` | Buttons, fields, chips |
| `radius_panel` | `6` | Docked surfaces, cards |
| `radius_dialog` | `10` | Floating windows |
| `control_height` | `24` | Field, button, chip |
| `row_height` | `26` | Layer and inspector rows |
| `app_bar_height` | `40` | App bar |
| `status_height` | `26` | Status bar |
| `rail_width` | `52` | Left tool rail |
| `dock_width` | `288` | Right inspector |
| `hairline` | `1` | The only border weight |
| `icon_size` / `icon_stroke` | `24` / `1.6` | The icon authoring box and its single weight |

Elevation: **no shadow on docked surfaces** — hierarchy is a hairline. Exactly two shadows exist, for floating
menus (`0 8px 24px`) and dialogs (`0 24px 64px`).

## Component rules

- **Rail button** — 32 px target; active is `sel-soft` fill plus a 1 px `sel` border; the glyph keeps its 1.6
  weight, so selection never changes the drawing. Every icon-only control carries a tooltip with its name and key.
- **Primary action** — the `cut` colour, one per screen (`Generate G-code`). Destructive or expensive actions are
  never primary-coloured.
- **Field** — right-aligned value on tabular figures, unit in a fixed column, `control_height`.
- **Tab / chip** — pill radius; active is `sel-soft` fill plus `sel` border and `sel` text.
- **List row** — `row_height`; hover `raised`; active `sel-soft` with an inset `sel` ring.
- **Signal meaning** — never colour alone: `cut` paths are dashed and 2 px, geometry is solid and 1.4 px, so a
  monochrome screenshot still separates them.
- **Focus** — every interactive element has a 2 px `sel` ring at 1 px offset, on both sets.
- **Motion** — at most 140 ms, and `prefers-reduced-motion` disables it.

## What this system forbids

A second accent hue; a filled variant beside the line icon set; more than three control heights in one column;
shadows on docked surfaces; colour as the only meaning; a numeric field without a unit; a value that a token does
not name.
