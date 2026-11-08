# Campilot design system

The tokens the shell draws with. Every value here is read from its source — nothing is invented, and a value
that appears in only one of the two places is a bug in one of them.

**Sources**

| Source | What it owns |
| --- | --- |
| `crates/cam-ui/src/theme.rs` | Every colour, type size, spacing step, radius and control height, plus the `Role` enum widgets ask by |
| `crates/cam-ui/src/icons.rs` | The icon set as geometry — 71 glyphs on one 24 × 24 box |
| `design/camui-shell/04-tokens.html` | The design authority: why these values, and the component states they produce |
| `design/camui-shell/03-iconography.html` | The icon language: 24 × 24 box, 1.6 stroke, dashed = virtual, 22 % wash = kept |

The Rust module is the runtime truth; the artboards are the argument for it. If they disagree, the artboard is
the intent and `theme.rs` is the defect — except for contrast, which `theme.rs` proves by test.

**What this system now describes.** The shell these tokens dress is the one `camui-prancheta` delivered: no stage
bar (S2), English only (S3), a 200 px rail of five labelled groups plus a brand block (S4), two 22 px rulers (S5),
a right panel that is a ribbon and a task editor rather than tabs (S6), a status bar in three zones (S7), 3D as a
mode that splits the canvas (S8), a master-detail tool library (S9) and one surface for creating and editing a
strategy (S10). Anything in an artboard that still draws a five-stage strip or an `Inspector` with
`Selection`/`Toolpath`/`Material` tabs is describing a shell that no longer exists.

## Colour

Twenty-one roles, two sets. A widget never names a colour: it asks `Palette::get(Role::…)`, so both sets
re-resolve. `Role::ALL` is the list `theme.rs` walks in its contrast tests, and a role that is not in it is not
painted.

### Dark (default) — `theme.rs::Palette::dark`

| Role | Value | Where the shell spends it |
| --- | --- | --- |
| `App` | `#12151a` | the window behind every panel |
| `Canvas` | `#0d1014` | the drawing area and the 3D half |
| `Panel` | `#171b21` | app bar, rail, inspector, status bar, ruler bands |
| `Raised` | `#1e242b` | chip chips, ribbon hover, fields |
| `Overlay` | `#232a32` | the tool library modal and the 3D overlay row |
| `Hairline` | `#262d36` | the 1 px line between docked surfaces |
| `Border` | `#35404c` | a control's own edge, where a hairline is too quiet |
| `TextHi` | `#e7ecf3` | the document name, a task's name, the product |
| `Text` | `#c3ccd8` | command labels, field values |
| `TextDim` | `#8b95a3` | ruler numerals, second lines, tooltips |
| `TextOff` | `#5c6673` | section headings, the dim half of a count — never body text |
| `Sel` | `#6fd3e8` | selection: picked vectors, the active toggle, the `TOP` axis |
| `SelSoft` | `rgba_premultiplied(0x0e,0x1b,0x1e,0x22)` | the wash under a selected chip or row |
| `Cut` | `#f5a623` | cutting: the ruler marker, the toolpath preview, the primary action |
| `CutSoft` | `rgba_premultiplied(0x20,0x16,0x04,0x22)` | the wash under the primary action and the cut chip |
| `Ok` | `#5ec27f` | a pass, a resolved tool, the `RIGHT` axis |
| `Warn` | `#f0b429` | a number the shell will not trust |
| `Danger` | `#f2635f` | a refusal, and the `FRONT` axis |
| `StockTop` | `#705c42` | the lit face of the material — wood tan, 34° |
| `StockBottom` | `#453726` | the shade under it, the same wood 1.80:1 deeper |
| `StockEdge` | `#ddc9a3` | the cut edge of the sheet — lighter than the face here, see below |

### Light — `theme.rs::Palette::light`

The same twenty-one roles on paper. Signals are darkened until they clear AA against a light canvas, which is
why `Sel` is `#0d7c92` here and `Cut` is `#8b5100`: a cyan that reads on `#0d1014` disappears on `#f7f9fb`, and an
amber that clears 3:1 on a white panel at `#a86200` still disappears into the shaded half of the material.

| Role | Value | | Role | Value |
| --- | --- | --- | --- | --- |
| `App` | `#eef1f5` | | `Sel` | `#0d7c92` |
| `Canvas` | `#f7f9fb` | | `SelSoft` | `rgba_premultiplied(0x02,0x0d,0x0f,0x18)` |
| `Panel` | `#ffffff` | | `Cut` | `#8b5100` |
| `Raised` | `#f2f5f8` | | `CutSoft` | `rgba_premultiplied(0x11,0x0a,0x00,0x14)` |
| `Overlay` | `#ffffff` | | `Ok` | `#1f7a43` |
| `Hairline` | `#dde3ea` | | `Warn` | `#96610a` |
| `Border` | `#c3ccd6` | | `Danger` | `#b3342f` |
| `TextHi` | `#12181f` | | `StockTop` | `#e8cb9d` |
| `Text` | `#2b3440` | | `StockBottom` | `#cfad7b` |
| `TextDim` | `#5f6b7a` | | `StockEdge` | `#664d2e` |
| `TextOff` | `#8b95a3` | | | |

The two `-soft` roles are the signal **premultiplied** — the channel values `theme.rs` hands egui already carry the
alpha, which is why `SelSoft` reads `0x0e,0x1b,0x1e` at `0x22` and not `0x6f,0xd3,0xe8`. Divide the channels by the
alpha before writing a CSS `rgba()`, or the wash gets darkened twice and a 13 % tint looks like a 1.7 % one. Undone,
they land on the signal itself: `SelSoft` ≈ `rgb(105 202 225 / 13%)`, `CutSoft` ≈ `rgb(240 165 30 / 13%)` in dark,
and `rgb(21 138 159 / 9%)` / `rgb(217 128 0 / 8%)` in light.

### The one gradient — `theme.rs::Palette::stock_shade`

There is exactly one interpolation in the shell and it is the material: `stock_shade(t)` mixes `StockTop` into
`StockBottom` with `t` clamped to `0..=1` (`-1` and `2` return the ends, `NaN` returns the top). Both ends are the
same wood — 34° and 33° in the dark set, 37° and 36° on paper — so depth darkens the material without sliding its
hue, and the two ends sit 28 and 25 codes apart in the ramp's smallest channel: a step the compositor that turned
T9's 8-code difference into 2–5 codes still draws.

The three values are resolved, not picked, and what resolves them is the amber. `Cut` is painted **on** the
material — the toolpath, the tool marker, the pocket — so the lit face has to stay far enough from `Cut` for a
1.6 px signal to read (3:1, measured **3.14:1** dark and **4.11:1** on paper), and — on paper, where the material is
the *darker* of the two — far enough from the canvas to read as a solid (1.45:1, measured **2.99:1** dark and
**1.48:1** on paper, against a `#dfcfb7` face that measures 1.45:1 and is the one that breaks it). The dark face is
a mid-value walnut for that first reason — a face light enough to look like pale wood is also the face the amber
disappears into, and `#75603f` is the value that measures 2.96:1 — which is how this change closes the limit T9
recorded, where the dark amber on the bright stock top measured 1.37:1.

The paper face was resolved inside a 0.024-wide window: the `#a86200` amber of the time needed 3:1 on it, ruling out
a face as dark as `#e6c99f` (2.996:1), while the solid floor ruled out one as light as `#dfcfb7`, and `#e8cb9d` is
the wheat between those two. The amber has since moved to `#8b5100` (see below), which widened that window to
0.4412–0.6361 of luminance and left the face where it was: what holds it now is the solid floor from above and,
from below, its own floor — `#cfad7b`, the same wood 1.36:1 deeper, 25 codes away in the ramp's smallest channel.

`StockEdge` is a role of its own rather than a darker mix, held to 3:1 against both the canvas and the stock top. In
the dark set it is **lighter** than its face — `#ddc9a3`, **11.77:1** off the canvas and **3.93:1** off the face —
because a face dark enough to carry the amber leaves nothing darker that still clears 3:1 against it: the silhouette
is a lit cut edge, not a drawn one. On paper it stays darker than both, `#664d2e` at **7.46:1** and **5.05:1**, and
1.45:1 away from `TextDim`'s rapid ink so the picture's two kinds of 1 px line are not the same colour.

What the material could not fix, and what did: on paper the amber over the *deepest* stop measured **2.25:1** —
under the 3:1 stroke floor and unchanged from T9's 2.23:1 — so the fix belonged to the amber and not to these three
roles, exactly as T4 recorded. T10 moved it, and it is the one value a light-mode picture cannot be re-derived
without: `#8b5100` measures **3.03:1** on `#cfad7b` and **3.55:1** at half depth, where `#a86200` measured
2.42:1–2.85:1 across that shaded half, so the defect was the whole lower half of the sheet and not just its end.

### Contrast, proven not asserted

`theme.rs` proves these and fails the build on any of them:

- **Twelve pairs the shell actually writes on** clear `AA_BODY` (4.5) or `AA_LARGE` (3.0) in **both** sets:
  `TextHi` on `App`/`Panel`, `Text` on `Panel`/`Canvas`, `TextDim` on `App`/`Panel` are body pairs; `Sel` and
  `Cut` on `Panel`/`Canvas`, `Ok` and `Warn` on `Panel` are large pairs.
- **The luminance ladder holds.** Dark: `Canvas < App < Panel < Raised < Overlay`. Light: `App < Raised < Canvas <
  Panel`. A surface that outranks its parent stops reading as one.
- **The two signals never share a hue**: `Sel` and `Cut` sit at least 60° apart, so an amber `Cut` can never be
  mistaken for a selected thing.
- **The stock reads as a solid against the canvas**: both stops clear the 1.45:1 floor — measured 2.99:1 and 1.66:1
  dark, 1.48:1 and 2.01:1 on paper — with `StockBottom` always darker than `StockTop` and never the canvas itself.
- **The stock is wood, and its shade is the same wood**: the lit face sits inside the 20°–45° band (34° dark, 37° on
  paper; the blue grey it replaced was 218°), the ramp's two ends are 1° apart in hue, and they are 28 / 25 codes
  apart in the smallest channel — under 20 and depth stops reading as depth.
- **The cut signal reads on every stop of the stock**, at five depths through `stock_shade`. Dark: **3.14:1** on the
  lit face, the binding stop there, up to 5.67:1 on the deepest — a face as light as `#75603f` measures 2.96:1. On
  paper the amber is the darker of the two, so the *deepest* stop binds: **4.11:1** at the top, **3.03:1** on
  `#cfad7b`, with `#8c5200` — the next step up the same ray — measuring 2.99:1 and the amber this replaces,
  `#a86200`, 2.25:1.

`TextOff` is deliberately outside the twelve. It is the colour of a heading and of half a count, never of a
sentence anyone has to read.

## Typography

One family, nothing fetched at runtime: `Inter, "Inter UI", ui-sans-serif, system-ui, -apple-system, "Segoe UI",
Roboto, sans-serif`. Numbers are tabular and set in the same face; lengths are written with the unit once per
column, never per row (`0:05 / 0:05`, `Ø6 · 2 cut(s)`).

| Style | Size / weight | Where it is |
| --- | --- | --- |
| `Hero` | 28 / 600 | an artboard's own headline; no widget uses it |
| `Dialog` | 20 / 600 | the tool library's title, `Choose tool` |
| `Title` | 15 / 600 | the product name, the document name, a task's name |
| `Body` | 13 / 400 | command labels, field values, sentences |
| `Button` | 13 / 500 | buttons and chips |
| `Small` | 12 / 400 | second lines, tooltips, the ruler numerals |
| `Caption` | 11 / 400 | the crate version, a section's heading letter-spacing |
| `Monospace` | 12 / 400 | the UI-mono stack, for a number that must not shift: coordinates, `T22`, `Z 5.000` |

`Monospace` is `ui-monospace, SFMono-Regular, Menlo, monospace`. It is the only second family in the system and
it never carries a sentence.

## Geometry — `theme.rs::Metrics::shell`

| Token | Value | Note |
| --- | --- | --- |
| `spacing` | `2, 4, 6, 8, 12, 16, 24, 32` | eight steps, every one on the 2 px grid, strictly increasing |
| `radius_control` | 4 | buttons, fields, chips |
| `radius_panel` | 6 | a card, the rail's rows block, the modal's columns |
| `radius_dialog` | 10 | the tool library only |
| `control_height` | 24 | one row a control sits in |
| `row_height` | 26 | one row of a list |
| `app_bar_height` | 40 | |
| `status_height` | 26 | declared; the status panel is laid out by its content, so treat 26 as the promise, not the measurement |
| `palette_width` | 200 | the left rail — **not** 52; the ASCII frame in `chrome.rs`'s module doc still draws 52 and is stale |
| `ruler_thickness` | 22 | each band, so 22 px top and 22 px left |
| `dock_width` | 288 | the inspector |
| `hairline` | 1 | every separator, and the ruler's division marks |
| `icon_size` | 24 | `ICON_SIZE` — the box every glyph is drawn in |
| `icon_stroke` | 1.6 | `ICON_STROKE`, constant in every state, hover included |

Glyphs are drawn *smaller* than the box they are authored in: 18 px in the app bar, 20 px in the rail and in the
ribbon. The 24 is the grid, never the render size.

Declared per module rather than in `Metrics`, and therefore part of the frame an artboard must respect: brand block
**62** (`chrome.rs`), collapsed inspector strip **12**, the 3D split at exactly **half** the drawing area, playback
bar **46**, overlay row line **18**, the triad's corner **86 × 68** with arms of **20** and labels padded **10**,
the home button **22**, the library's columns **296 / 460 / 344 / 186** with a **40 mm** drawing span.

## Component rules

**App bar.** 40 px, `Panel`, one hairline under it. Left: the mark, `campilot` in `Title`, a separator, the
document name, then ` · modified` while the document is dirty. Right, from the right edge inward: `Generate
G-code` (disabled while the code cannot be written from here), the help glyph, the theme glyph, a separator, and
the working unit `mm` in `TextDim`. No command lives in the bar that the rail also offers — one act, one place.

**The rail.** 200 px, `Panel`, scrolling chips over a fixed foot. Five groups in this order: `FILE`, `DRAW`,
`EDIT`, `ORGANISE`, `SHOW AND SELECT`. **The words are the group headings; a chip is a glyph and nothing else** —
20 px at 1.6 stroke inside the 32 × 32 target `icon_button` allocates (glyph plus 12, a hit area rather than a
fourth control height: the three heights govern text controls), its command's label carried in the tooltip, which
is why `chrome.rs` says the palette is "the part of the frame that carries no text of its own". A chip that has something specific to say replaces the label
in the tooltip (`Grid and snapping`, `Layers — the inspector, collapse or expand`, `Help — F1`, `Shortcuts — the
keys the shell answers to`). No chip is ever drawn disabled: the rail offers twenty-eight chips and every one of
them is pressable, because what a command can do is decided when it runs and reported in the status bar. Each
heading is a row the operator folds; folding hides the chips and leaves the heading. **No letter is drawn next to
a glyph**, because the shell gives no rail command a single-key shortcut; the only keys it answers are `F1`, `F2`,
`Esc`, `⌘Z`, `⇧⌘Z`, `Delete` and `F`. The foot is the brand block, 62 px, behind a hairline: `campilot`, the crate
version (`v0.0.1` today), and the document's own stamp — `Saved 2026-11-03 19:30 UTC`, or `Unsaved changes` while it
is dirty. It is a document stamp, never a build stamp.

**Canvas and rulers.** Two 22 px bands, `Panel` with `TextDim` ink, a hairline division every step. The top band
names the unit (`mm`) at its left end; the left band starts below it and turns its numerals −90°. While the
pointer is over the drawing, an amber (`Cut`) marker runs the full length of both bands. On an empty document the
origin is the stock's bottom-left corner, so the rulers read negative to the left of it and below it.

**The inspector.** 288 px, `Panel`. Header `Inspector` with a collapse control. Below it a ribbon: one glyph row
per category, in the order `TOOLPATHS`, `RELIEF AND 3D PART`, `MACHINE AND OUTPUT`, each with a tooltip that says
what opening it gives you; a second press on the open category closes it. While a task is open the ribbon is
followed by the task header — its name, then a painted `✓` (`Done`) and a painted `✕` (`Cancel`) — and by the
task's own body. `LAYERS` is a block of its own below the body and folds independently. With no task open the
panel shows the current tool's card (when a toolpath is selected) and `Properties`.

**The status bar.** Three zones on `Panel`, one hairline above. Left: `X 12.40   Y 88.00` in monospace, the unit,
then the facts (`zoom 0.42 px/mm   undo 12 / redo 3`). Middle: a `Snaps` chip, then `Point`, `Mid`, `Centre`, and
a grid chip that names its step (`Grid 10`, `Grid 2.5`, `Grid —` when it has none). Right: the sentence the last
action produced, or the hint for the tool in hand. A refusal is the same sentence prefixed `refused: `.

**3D is a mode, not a tab.** `2D | 3D` is a segmented control at the top right of the canvas. Pressing `3D` splits
the drawing area in half; the right half carries an overlay row of `Material`, `Vectors`, `Toolpaths`, `Tool`, then
`Centre` and `?`; a 22 px home button at its top centre; a triad labelled `TOP`, `FRONT`, `RIGHT`; and a 46 px
playback bar — `Play`/`Pause`, the speed pill over `1x … 200x`, `To the end`, the scrubber, `Reset`, `Step` — above
the readout `0:05 / 0:05  T22 · Drilling 1 · Z 5.000`.

**The tool library is a modal.** `Choose tool` at `Dialog`, a `Find a tool` field, a tree of four families — `Flat
end mill`, `Ball nose`, `Corner radius`, `V-bit` — each heading carrying its count (`Flat end mill (6)`), each row
a mini tool glyph, `T22 - 6 mm flat` and the dim second line `Ø6 · 2 cut(s)`. The detail column groups its fields
under `GEOMETRY`, `PASSES`, `SPEEDS`, `MACHINE` and draws the tool to scale with callouts (`Ø 6 mm`, `Flute length
18 mm`). The footer is a sentence saying what the library cannot do from here, then `Use this tool` and `Cancel`.

**A strategy's editor is one surface for both modes.** The header reads `New toolpath · Profile 1` or `Edit ·
Profile 1`. The body: the `Name` field, the current-tool card (`TOOL — 6 mm flat`, then `Choose another tool…`),
the collapsible `Quick parameter edit`, then `CUT` with its kind on image cards (`Profile`, `Pocket`) and its
fields, then the note *"The toolpath is calculated when you click "Generate toolpath"."*, then `Generate toolpath`
and `Cancel`. When a button cannot act, the sentence that says why takes its place — never a dead control.

**Signal by shape, not by colour alone.** Selection is a dashed-then-solid `Sel` outline with square handles;
cutting is `Cut` at 1.6 px with a round cap. A monochrome print of the shell still tells the two apart.

**Focus is a 2 px ring** in `Sel`, drawn outside the control so it never moves what it points at. Motion is
`≤ 140 ms`, and `prefers-reduced-motion: reduce` turns every transition and animation off.

**`TextOff` never carries a sentence.** It is the ink of a heading, a count and half a row; a caption, a figure
note, a table caption or a hint that says something is body text and takes `TextDim`, which is the pair
`theme.rs` proves at 4.5:1. `TextOff` measures about 3:1 on either panel and is deliberately outside the twelve
proved pairs — right for furniture, wrong for prose.

## What this system forbids

- A second accent. There is `Sel` and there is `Cut`; a third means the screen is telling two stories.
- A filled icon beside a stroked one. The set is stroke-only at 1.6, and a glyph's mixed parts are dashes and
  washes, never a heavier pen.
- More than three control heights. `24` is the control, `26` the row, and a bar is its own thing.
- A shadow on a docked surface. Only the modal and the overlay cast one.
- Colour as the only carrier of meaning.
- A number without its unit, or a unit on every row of one column.
- An unnamed value. If it is not in this file, it is not a token.
