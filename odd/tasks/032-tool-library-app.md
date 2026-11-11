# The tool library, finished: one library for the application

Feature document for M15. `odd/tasks/033-tool-library-editor.md` built the screen and pinned it; this document
gives that screen the library it was missing, the reference's catalogue, and a JSON round trip a person can
keep. Read to `## Log` before acting on any task.

## Specs

**S1 — The library belongs to the application, not to any drawing.**
Owner decision, verbatim: `la biblioteca de herramientas pertenece a la aplicacion, no a ningun dibujo.`
The app keeps exactly one library file, `tools.sqlite`, in the application's data directory:

- `$CAM_PILOT_DATA_DIR` when that variable is set (tests use this; it is the only override),
- otherwise the platform's per-user data directory: `$XDG_DATA_HOME/campilot` if `XDG_DATA_HOME` is set,
  else `$HOME/.local/share/campilot` on Unix; `%APPDATA%\campilot` on Windows.

Every drawing the window opens shows that same library, and closing a drawing does not close it. Opening a
`.c2d` no longer resolves `tools_ref`, no longer reads a sibling library and no longer changes what the
dialog lists. `tools_ref` stays in the document model and `cam-cli` keeps honouring it: the change is the
app's ownership rule, not the deletion of a field other tools read.

**S2 — First launch creates the library.** If the file does not exist, the app creates its directory and
writes the factory catalogue (S3) before the window draws, so the dialog is never born empty and no drawing
file is needed to have a library. A failure to create is the status line's message and leaves the six actions
disabled; it is never faked with an in-memory library.

**S3 — The factory catalogue is the reference's 22 tools in its six families.** `design/prancheta-reference/Screenshot 2026-11-03 032331.png` and `034320.png` are the source; the tool numbers are `T1`–`T22`, each
used once, and this table is the whole set (Ø is the tool's diameter, `n` its flute count):

| # | Name | Kind | Ø mm | n | shape |
| --- | --- | --- | --- | --- | --- |
| T1 | Flat end mill · 6 mm | FlatEndMill | 6 | 2 | — |
| T2 | Flat end mill · 3.175 mm (1/8") | FlatEndMill | 3.175 | 2 | — |
| T3 | Ball nose · 6 mm | BallNose | 6 | 2 | tip radius 3 |
| T4 | V-bit · 90° · 12.7 mm | VBit | 12.7 | 2 | 90°, tip width 0.2 |
| T5 | V-bit · 60° · 12.7 mm | VBit | 12.7 | 2 | 60°, tip width 0.2 |
| T6 | Drill · 5 mm | Drill | 5 | 2 | 118°, shank 5, flute 30, overall 60 |
| T7 | Flat end mill · 8 mm | FlatEndMill | 8 | 2 | — |
| T8 | Flat end mill · 12 mm (roughing) | FlatEndMill | 12 | 2 | — |
| T9 | Ball nose · 3.175 mm | BallNose | 3.175 | 2 | tip radius 1.5875 |
| T10 | Ball nose · 1.5 mm (detail) | BallNose | 1.5 | 2 | tip radius 0.75 |
| T11 | V-bit · 30° · 3.175 mm | VBit | 3.175 | 1 | 30°, tip width 0.1, flute 7.7, overall 38 |
| T12 | Drill · 8 mm | Drill | 8 | 2 | 118° |
| T13 | Single flute · 3.175 mm | FlatEndMill | 3.175 | 1 | — |
| T14 | Single flute · 4 mm | FlatEndMill | 4 | 1 | — |
| T15 | Compression · 6 mm | FlatEndMill | 6 | 2 | — |
| T16 | Surfacing · 25 mm | FlatEndMill | 25 | 2 | — |
| T17 | Engraving · 30° · 0.3 mm tip | Engraving | 3.175 | 1 | tip width 0.3 |
| T18 | V-bit · 120° · 32 mm | VBit | 32 | 2 | 120°, tip width 0.2 |
| T19 | Tapered ball · R0.25 · 3.6° per side | CornerRadius | 3.175 | 2 | tip radius 0.25 |
| T20 | Tapered ball · R0.5 · 4.76° per side | CornerRadius | 6 | 2 | tip radius 0.5 |
| T21 | Tapered ball · R0.75 · 6.42° per side | CornerRadius | 6 | 2 | tip radius 0.75 |
| T22 | Flat end mill · 1 mm · 3.175 shank | FlatEndMill | 1 | 1 | shank 3.175, flute 8, overall 38 |

The taper angle of `T19`–`T21` lives in the name only: the store's corner-radius shape has no taper field,
and adding one is not this feature. Every tool is stored with its declared kind (S4 of the editor document),
never with the legacy `0`.

**S4 — Speeds and feeds are derived by one rule, not invented per tool.** Each factory tool carries the
baseline the four existing factory tools already carry — `feed_rate = 100`, `plunge_rate = 30`,
`spindle_speed = 18000` — plus two numbers derived from its own diameter: `stepdown = diameter / 2` capped at
`3 mm`, and `stepover = 40 % of the diameter`, both rounded to two decimals in millimetres. The rule names
the provenance of every number the form will show; nothing is hand-tuned per tool.

**S5 — Import asks, then either replaces or adds without duplicates.** User requirement, verbatim: `a la
hora de importar, hay que preguntarle al usuario si quiere reemplazar todo, o agregar. si dice agregar,
debemos comparar cada herramienta con las de la biblioteca para no tener duplicados`.

Reading the chosen file and validating every tool in it happens first, exactly as today: a file this build
cannot use changes nothing and says why. When the file is valid the app asks, in its own dialog:

- title `Import tool library`;
- sentence `N tool(s) in <file>. Replace the library, or add only the tools it does not already hold?`;
- buttons `Replace all`, `Add`, `Cancel`.

`Replace all` makes the file's tools the library (today's behaviour: file first, snapshot second).
`Add` compares each tool in the file against every tool already in the library and skips it as a duplicate
when it shares that tool's **id**, its **name** (trimmed, case-insensitive), or its **tool number**;
everything else is appended, and the note this window reports afterwards is
`{added} tool(s) added, {skipped} already in the library.` Nothing is written when every tool was skipped:
the question closes and says `Nothing was added: every tool in <file> is already in the library.` `Cancel`
and Escape close the question and write nothing at all, and the tool-library modal behind it is still open
with the library it had.

**S6 — Export is unchanged.** `Export…` writes the `campilot-tool-library` version 1 JSON that
`crates/cam-store/src/tool_file.rs` already defines: one object per tool, lengths in millimetres in fields
named for it, the kind as a slug. A person can keep, diff and mail that file, which is the point of the
button.

**S7 — The dialog takes the reference's proportions.** `COLUMN_HEIGHT` grows from 372 to 700 and
`PREVIEW_HEIGHT` from 172 to 330, the reference's three columns scaled by the same factor its widths already
use (`403 / 589 / 230` → `300 / 424 / 200`). The tallest family's form (the V-bit, eight rows per group) must
fit its column without scrolling: every form row's bottom edge is inside the body. The preview card is
**light**, as the reference draws it (light grey card, dark tool, blue callouts), which needs three palette
roles the chrome does not have yet: `PreviewCard`, `PreviewTool`, `PreviewDimension`. The kind's label stays
as the caption under the card.

**S8 — An empty library says so, and is one click from usable.** With no drawing owning the library the
sentence drops its "window was given" framing: `The library holds no tool, so there is nothing to list or to
use.` `Restore defaults` stays enabled whenever the library resolved, so an emptied library is recovered in
one click. The per-document sentence about writing `tools.sqlite` beside the drawing, and the refusal that
carried it, are removed with the rule that produced them.

**S9 — Nothing about the six actions changes but their gate.** The actions are enabled when the library file
path resolved (S1/S2); `Duplicate` and `Delete` additionally need a selected tool. Every write is still file
first and snapshot second, so a failure mutates nothing and keeps the window open.

**S10 — Pinned by the report, never by pixels.** Every behaviour above is pinned by a test that drives the
shell and reads its report, as the editor document's S14 established. Heights and widths are read from the
report's geometry, not from a captured image.

**S11 — The tree's rows are two clean lines inside a panel.** One row is 46 px tall with a 2 px gap: a 24 px glyph
slot 6 px in from the row's left edge and vertically centred, the title at `row.left + 34`, `row.top + 6`, and its
`Ø6 · 2 cut(s)` subtitle at `row.top + 24`. The title's and subtitle's rectangles never intersect, and neither
does the glyph's. The tree sits inside an inset card — rounded, `Role::Panel`, a hairline `Role::Border`, 4 px of
padding — with a scrollbar when the list overflows. *(M15 follow-up: the first delivery painted the title and the
subtitle on top of each other in a 29 px row with the glyph over the title, and drew no panel at all.)*

**S12 — The form is one column of boxes.** `BOX_WIDTH` is 150 and every row's box shares its left and its right
edge: right edge = the column body's right edge − `UNIT_SLOT`, left edge = that − `BOX_WIDTH`, the unit in the slot
beside it. The value is right-aligned inside its box, while editing included, and a label ellipsizes rather than
pushing the box. The `Type` row is one line, label left and combo right-aligned to the same column. The computed
chipload is emphasis text in that column, with no box stroke and no warning colour: the reference draws a number,
not a field.

A kind whose geometry the reference describes with a tip width draws that row too: `Tip width` in `GEOMETRY`,
in millimetres, after the included angle for a V-bit and after the diameter for an engraving cutter, reading
`flat_diameter`. The reference's V-bit form asks `Diâmetro maior`, `Ângulo incluído`, `Largura da ponta`, and this
build stored the third without ever drawing it.

**S13 — The preview draws the tool the reference draws, with its four dimensions.** The card stays light and
330 tall, with 16 px margins. Inside it, the tool is drawn as the reference draws it, from the top down:
a **shank** (a solid body at a fixed 0.16 × the card's inner width, outlined in `#43474e`, filled
`Role::PreviewTool`); below it a **cutting section** whose width is the shank's scaled by
`diameter / shank_diameter` with a 6 px floor (equal to the shank when the record has no shank diameter), whose
height follows the flute length (or 30 % of the drawn length when the record has none); and an **amber tip** on the
last 6 px of the cutting section, as the reference paints the edge that cuts. The drawing leaves a gutter on its
left and its right for dimensions and a strip under the tip.

Four **dimensions**, every one of them a 1 px line in `Role::PreviewDimension` with an arrowhead at both ends and a
rotated (−90°) label for the vertical ones:

| dimension | where | label |
| --- | --- | --- |
| overall length | vertical, in the left gutter, spanning shank top to cutting bottom | the overall length in mm, bare |
| flute length | vertical, in the right gutter, spanning the cutting section | the flute length in mm, bare, drawn only when the record has one |
| cutting diameter | horizontal, just under the tip | `Ø<diameter>` |
| shank diameter | horizontal, just above the shank's top | `Ø<shank>`, drawn only when the record has one |

A dimension the record does not hold is not drawn — the rule this module has kept since M14. Nothing is painted
outside the card, and the drawing keeps its proportions when the tool is 1 mm in a 3.175 mm shank (a narrow cutting
section on a solid body), which is the case the first delivery got wrong by drawing a hairline.

**A V-bit and a drill are drawn as their cone**, not as a second rectangle: the included angle decides it, so the
section below the shank tapers from the shank's width to the tip's (`flat_diameter`, or a point when it is absent
or zero) over a height that follows `half_width / tan(angle / 2)`, and the drawing carries the angle as a fifth
dimension — `<degrees>°` beside the cone — because the angle is what makes the two kinds different from every other
one. An engraving cutter keeps a flat tip and no angle, since its cone is not what the record stores.

Every dimension's **label is offset 8 px from its own line**, perpendicular to it, and clamped inside the card: a
label never sits on its line, on the tool's body, or on another label. That is the defect the first S13 render
showed — the `Ø` labels lay along their own arrows — and it is asserted, not eyeballed: each reported label
rectangle intersects neither its line, nor the body, nor the other labels.

**S14 — The header and the footer are the reference's.** The title is `Choose tool` and nothing else; the chosen
tool is in the name field below it. The footer row is `Cancel` first and the primary action rightmost — `Use this
tool`, or the refusal sentence in its place.

## Delivery

**The slice strategy for this branch, recorded before the third commit.** The work splits into the five rows
above, one commit each, in this order: `T1` (the catalogue — large by nature, because 22 tools are 22 rows of
shape data, not a design choice), `T2` (the ownership change, whose diff is mostly the deletion of the old
per-document resolver and its 650-line suite), `T3` (the import question), `T4` (the proportions), `T5` (the
record). Nothing is pushed and no pull request is opened until the owner asks for one: the branch is where the
feature is reviewed, and the slice sizes are named here because two of them are measured in table rows and
deleted lines rather than in design.

The follow-up (T6-T14) is a second delivery of the same feature: looking at the render showed the geometry the
tests pinned did not add up to the screen the reference draws, so the screen itself became the requirement. Three
slices carry it — the render harness, the paint fixes, and this record — and the rows above name which. The
first slice took a worker; T9, T12, T13 and T14 were done inline after two worker launches timed out without
writing anything.

Delivered as planned, with one extra commit: `49f99a7` (T1), `0b410ea` (T2), `a975f46` (the document and this
strategy), `6c0a060` (T3), `4a78ec0` (T4), and the record commit that carries this table. 2411 authored changed
lines across the branch, 1679 tests green in the workspace.

## Tasks

| # | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 ✅ | The factory catalogue: 22 tools, six families, one derived speeds rule | S3, S4 | worker (`cam-store`) | `49f99a7` |
| T2 ✅ | The app owns the library: data directory, first-launch creation, no `tools_ref` on open, the actions' gate | S1, S2, S8, S9 | parent inline | `0b410ea` |
| T3 ✅ | Import asks replace or add; add skips duplicates by id, name or number | S5, S6 | worker (`cam-ui`) | `6c0a060` |
| T4 ✅ | The reference's proportions and the light preview card | S7 | worker (`cam-ui`) | `4a78ec0` |
| T5 ✅ | Close: roadmap M15 row and §4.18, the editor document's superseded note, README, whole suite | S10 | parent inline | the record commit |
| T6 ✅ | An offscreen render of the modal, so the screen can be compared with the reference frame and not only measured | E3 | worker (`cam-ui`) | `b2fba96` |
| T7 ✅ | Tree rows 46 px with a glyph slot and no overprinting; an inset panel with a scrollbar | S11 | worker (`cam-ui`) | the follow-up commit |
| T8 ✅ | One column of 150 px boxes, values right-aligned, `Type` on one line, chipload as text | S12 | worker (`cam-ui`) | the follow-up commit |
| T9 ✅ | The preview draws a solid shank, a proportional cutting section, an amber tip and four dimensions | S13 | parent inline | the follow-up commit |
| T10 ✅ | Title `Choose tool`, footer `Cancel` before the primary | S14 | worker (`cam-ui`) | the follow-up commit |
| T12 ✅ | `Tip width` — `flat_diameter` was stored and never drawn | S12 | parent inline | the follow-up commit |
| T13 ✅ | A V-bit and a drill drawn as their cone, with the angle; no label on its own line | S13 | parent inline | the follow-up commit |
| T14 ✅ | T22's flute and overall lengths, the numbers the reference states for it | S3 | parent inline | the follow-up commit |

## Log

**L1** (user, verbatim, with two screenshots): `necesitamos terminar la ui de la libreria de herramientas que
ahora mismo es asi [Image#2] para que quede igual que [Image#3]` — Image#2 is the dialog as the app renders
it today with an empty library; Image#3 is `design/prancheta-reference/Screenshot 2026-11-03 032653.png`,
`Escolher ferramenta` with `T22 Fresa 1,0 mm` chosen.

**E1** (evidence, before the decision): the screen is already built — three columns `300/424/200`
(`crates/cam-ui/src/tool_library/dialog.rs:31-37`), the header band, the kind-grouped tree, the form's four
groups, the preview card and the footer. What Image#2 shows is the empty state, and behind it a real defect:
`writes = self.tool_library_path.is_some()` (`dialog.rs:597`) and `tool_library_path` is set **only** by
`Shell::set_tools_from` (`crates/cam-ui/src/shell/mod.rs:635`) when an opened `.c2d` names a `tools_ref`
whose `tools.sqlite` already exists. Nothing in the app creates that file and nothing writes `tools_ref`, so
the six actions can never enable and the populated screen is unreachable from inside the app.

**L2** (user, verbatim): `ademas de debe de poder importar y exportar la biblioteca en json, para que el
usuario pueda guardarsela. a la hora de importar, hay que preguntarle al usuario si quiere reemplazar todo, o
agregar. si dice agregar, debemos comparar cada herramienta con las de la biblioteca para no tener
duplicados`.

**D1** (owner answer, verbatim): `la biblioteca de herramientas pertenece a la aplicacion, no a ningun
dibujo.` — this supersedes S10 of `odd/tasks/033-tool-library-editor.md` ("write the tool library beside the
document") and the sentence that carried it.

**D2** (owner answer): the factory catalogue is the reference's 22 tools, with speeds and feeds derived from
the four existing ones and accepted as starting values, not as a machine's numbers.

**E2** (delivery). T3 took three rounds for one reason worth recording: its first version proved the question's
buttons by calling the shell's answer methods, which is a test skipping the control it claims to test. Pressing
the reported rectangle exposed the real constraint — an egui modal is placed on its first pass and settled on the
second, so the rectangles reported in the frame the question appears are not the ones a person clicks. The second
round answered that by remembering the first frame's rectangles and resolving clicks against those, which would
have shipped a control that answers where the dialog used to be; the third round reports the frame that was
drawn and lets the test settle one frame before reading it. Two exploration subagents and one writer launch also
timed out before this task began, which is why the evidence gathering for T2 was done inline.

**L3** (user, verbatim, with two screenshots at `/home/th3g3ntl3man/Pictures/Screenshots/Screenshot_2026-11-10
21-08-22.png` — the app as delivered — and `…21-16-43.png` — the reference frame): `todavia falta que lo arregles,
aqui tienes las diferencias … asegurate mediante screenshots`.

**E3** (evidence, and how it was checked). The two frames settled S11-S14: the delivered dialog painted the tree's
title and subtitle into a 29 px row so they overprinted each other and the glyph landed on the title, had no panel
or scrollbar, started every value box wherever its label ended, drew the 1 mm cutter in the preview as a hairline
with its callouts at the card's bottom edge, repeated the tool's name in the title, put the `Type` combo on its own
line, drew the computed chipload as an amber outlined box, and ordered the footer primary-before-`Cancel`.
Verification is a **render**, not a report: `cargo test -p cam-ui --test ui_snapshot -- --ignored --nocapture`
writes `target/ui-snapshots/tool-library-{t22,vbit,import-question}.png` at 1240×1000 through
`egui_kittest`/wgpu, which is the same size the reference frame was captured at and the only way this work was
compared to it. The report-driven tests still pin the geometry S10 asks for; the PNGs are how a person checks that
the geometry adds up to the screen.

