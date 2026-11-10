# The tool library, finished: one library for the application

Feature document for M15. `odd/tasks/tool-library-editor.md` built the screen and pinned it; this document
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
| T22 | Flat end mill · 1 mm · 3.175 shank | FlatEndMill | 1 | 1 | shank 3.175 |

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
everything else is appended. Nothing is written when every tool was skipped. `Cancel` and Escape close the
question and write nothing at all.

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

## Delivery

**The slice strategy for this branch, recorded before the third commit.** The work splits into the five rows
above, one commit each, in this order: `T1` (the catalogue — large by nature, because 22 tools are 22 rows of
shape data, not a design choice), `T2` (the ownership change, whose diff is mostly the deletion of the old
per-document resolver and its suite), `T3` (the import question), `T4` (the proportions), `T5` (the record).
Nothing is pushed and no pull request is opened until the owner asks for one: the branch is where the feature
is reviewed, and `T1`/`T2` together are the reason the slice sizes are named here rather than measured against
400 lines each.

## Tasks

| # | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | The factory catalogue: 22 tools, six families, one derived speeds rule | S3, S4 | worker (`cam-store`) | `feat(store): the reference's 22-tool factory library` |
| T2 | The app owns the library: data directory, first-launch creation, no `tools_ref` on open, the actions' gate | S1, S2, S8, S9 | parent inline | `feat(ui): the tool library belongs to the application` |
| T3 | Import asks replace or add; add skips duplicates by id, name or number | S5, S6 | parent inline | `feat(ui): importing a tool library asks replace or add` |
| T4 | The reference's proportions and the light preview card | S7 | parent inline | `feat(ui): the library dialog at the reference's proportions` |
| T5 | Close: roadmap M15 row, the editor document's superseded notes, whole suite | S10 | parent inline | `docs(odd): record M15` |

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
dibujo.` — this supersedes S10 of `odd/tasks/tool-library-editor.md` ("write the tool library beside the
document") and the sentence that carried it.

**D2** (owner answer): the factory catalogue is the reference's 22 tools, with speeds and feeds derived from
the four existing ones and accepted as starting values, not as a machine's numbers.
