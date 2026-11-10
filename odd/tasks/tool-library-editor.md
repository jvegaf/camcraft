# tool-library-editor

Rebuild the tool-library modal so it matches the three reference screenshots, and give it the editing
and library actions those screenshots show: a three-column master-detail editor that writes tools.

Supersedes the read-only framing of `camui-prancheta` S9 (see L2, L3).

## Specs

**S1 — Header bar.** The modal has a header bar across the top with a lighter fill than the body, the
dialog's title, and a close `X` at the far right that closes without writing. The title is the tool's
own title once a tool is selected.

**S2 — Three columns.** Master, detail and preview, left to right.

- Master: a search field at the top; below it a scrolling tree grouped by tool family, each heading
  collapsible with a caret and carrying the count of tools under it; each row has a mini tool glyph, the
  tool's name on one line and the dim `Ø6 · 2 cut(s)` line under it; the selected row has a highlighted
  band.
- Detail: the editable form (S4-S7).
- Preview: a dark inset card holding the tool drawn with dimension callouts, with the tool's type name
  captioned under the card.

**S3 — Library action rows.** The master column ends with three rows of buttons, in this order and
grouping:

- `+ New`, `Duplicate`, `Delete`
- `Export…`, `Import…`
- `Restore defaults`

**S4 — Name and Type.** The detail column starts with a full-width text field holding the tool's name,
then a `Type` row whose combo offers exactly these six kinds, in this order: `Flat end mill`,
`Ball nose`, `Corner radius`, `V-bit`, `Engraving`, `Drill`.

**S5 — Geometry rows.** The `GEOMETRY` group's rows depend on the tool's kind and follow this order:
`Diameter`, `Included angle` (only for the kinds whose tip is pointed), `Flute length`, `Shank diameter`,
`Overall length`, `Number of cuts`.

**S6 — Passes and speeds.** `PASSES` holds `Depth per pass`, `Stepover (mm)`, `Stepover (%)`. `SPEEDS`
holds `Spindle` in `rpm`, `Feed` in `mm/min`, `Plunge` in `mm/min`, and `Chipload` — a computed value
with no input box, drawn in the warning colour, equal to `feed / (spindle × cuts)` rendered in `mm`.

**S7 — Machine.** `MACHINE` holds `Tool number`, drawn as `T` followed by the number's box.

**S8 — Footer.** Above the button row, a dim sentence: "Changes apply to new toolpaths; each toolpath
keeps its own copy of the tool." Below it, right-aligned and in this order: `Cancel`, then the primary
`Use this tool`.

**S9 — Editing and validation.** Every numeric box is editable. A draft that does not validate blocks
`Use this tool` and draws one sentence naming the first thing that is wrong. A valid draft is written
when `Use this tool` is pressed; `Cancel` discards the draft without writing.

**S10 — Library actions.** `New` appends a tool with a fresh id and the lowest free tool number.
`Duplicate` copies the selected tool as a new tool named `<name> (copy)`. `Delete` removes the selected
tool. `Restore defaults` reinstalls the factory set. Each of them writes through `cam-store` to the
library file beside the document when there is one; with no library file they are disabled and one
sentence says why.

**S11 — Import and Export.** `Export…` writes the library to a JSON file; `Import…` reads one and adds
its tools. Both go through the host's file dialogs.

**S12 — English.** Every user-facing string is English. The reference's Portuguese strings are never
copied.

**S13 — No fabricated library.** Opening the modal must not invent tools: it lists the library it was
given, and an empty library says so.

**S14 — Pinned by report.** Every behaviour above is pinned by a test that drives the shell and reads
its report, not by pixels.

## Tasks

| ID | Specs | Route | State |
| --- | --- | --- | --- |
| T1 | S4, S5, S10 | inline | done — `a21b56a` (kinds, migration v3), `6b84b31` (validation, three writers) |
| T2 | S1, S2, S4-S9, S12 | worker | done — `d04d8c6` (header, three columns, editor, preview, footer) |
| T3 | S9, S6 | worker | done — `d04d8c6` (drafts, combo, validation, computed chipload) |
| T4 | S1, S2, S14 | worker | done — `d04d8c6` (the suite rewritten as behaviour over the report) |
| T5 | S11 | inline + worker | store half done — `c760cf8`; the file dialogs are in the last slice |
| T6 | S13, S14 | worker | done — `d04d8c6` (`default_tools()` retired, no fabricated library) |
| T7 | S3, S10, S11 | worker | the library action rows and the import/export dialogs |
| T8 | — | inline | docs, `odd/roadmap.md` row, full gate |

## Log

**L1** — the owner's request, verbatim:

```
tienes que conseguir que el modal del library tools sea igual que en estas capturas /home/th3g3ntl3man/Code/campilot/design/prancheta-reference/Screenshot 2026-11-03 032331.png
/home/th3g3ntl3man/Code/campilot/design/prancheta-reference/Screenshot 2026-11-03 032653.png
/home/th3g3ntl3man/Code/campilot/design/prancheta-reference/Screenshot 2026-11-03 032737.png
```

**L2** — the owner's answer on language: matching the reference's *shape*, not its words.

```
Question: Las capturas están en portugués ("Escolher ferramenta", "Diâmetro", "Avanço"...) y toda la app
está en inglés. ¿Qué hago con los textos del modal?
Answer:   Inglés, solo la maqueta
```

**L3** — the owner's answer on scope: the whole thing, including the writes.

```
Question: La captura muestra campos editables y una fila completa de botones de librería (Nova,
Duplicar, Apagar, Exportar, Importar, Restaurar padrão). Hoy el modal es un snapshot de solo lectura.
¿Hasta dónde llego?
Answer:   Paridad total: edición + botones
```

**L4** — evidence for the plan, from two bounded read-only sweeps before the first write:

- `cam-ui` reaches the library only through `ToolLibrary::open_read_only`
  (`crates/cam-ui/src/shell/action.rs:681`), and `cam-desktop` has no `cam-store` dependency at all, so
  "persist a tool" is new API and new wiring, not a flag.
- `cam-store` has no delete, no reset and no validation: `put_geometry` (`crates/cam-store/src/tools.rs:502`)
  inserts whatever it is given.
- `ToolGeometry.tool_type` is a raw `u8` whose mapping the module header refuses to guess
  (`crates/cam-store/src/tools.rs:12-16`), and `cam-cli tools add` writes `0`. The six kinds S4 needs
  cannot all be derived from geometry — a drill and a V-bit share `included_angle` — so a kind has to be
  stored, and legacy `0` has to keep meaning "derive it".
- `ToolGeometry` has no shank diameter and no overall length, which S5 draws. Two nullable columns.
- `open_tool_library` fabricates four tools when the snapshot is empty
  (`crates/cam-ui/src/tool_library/shell.rs:82-84`), which S13 removes.

**L5** — slice boundaries, and the one deviation from S5.

Slice 1 landed as two commits so the gate could be green at each: `a21b56a` is the schema and the kind
vocabulary (331 lines), `6b84b31` is the validation and the three writers with their sixteen tests
(~830 lines — over the 400-line slice, because a new module and the tests that pin it are one unit and
`clippy -D warnings` has to pass at every commit). Slice 2 stays inside the modal and splits at the
form's inputs.

Deviation: S5 lists six geometry rows and gives `Included angle` to the pointed kinds, but the
`Corner radius` kind needs the radius its name comes from, and none of the three screenshots selects that
kind — so the spec could not have shown it. The form draws `Corner radius` for that kind and keeps S5's
six rows in S5's order for the rest.

**L6** — T5's store half landed first because it is independent of the modal: `crates/cam-store/src/tool_file.rs`
writes and reads the JSON library, with two refusals instead of two guesses (an unknown kind slug, and any tool
that fails `validate_tool`). The four fields a form writes and a file normalises are worth one sentence: the store
keeps millimetres per second, a feed is quoted in millimetres per minute, `rate_units` records what the operator
typed rather than rescaling the number, and the file says which unit it means in each field's own name.

**L7** — `feed_rate` was two units at once, and the owner settled which one (verbatim answers, after the first
question was re-asked because its price had been mis-stated):

```
Question: ¿Cuál es la unidad real de `feed_rate`? De esto depende si el modal muestra 100 o 6000, y si hay un
defecto de 60× esperando en el G-code.
Answer:   La práctica manda: mm/min   (corrected to:)   mm/min, y arreglar el tiempo de la simulación
```

The evidence, because it is not a detail: `cam-store`'s `CuttingData`, `cam-move`'s `Segment` and
`docs/format.md` all said millimetres per SECOND, and the cutting-time model divided a length by the rate and
called the answer seconds. The emitter handed the same number to the program's `F` word, which is millimetres
per MINUTE, and the CLI's goldens read `feed_rate: 100.0` as `F100.0`. Both consumers had a test, so whichever
unit is declared, one of them was 60× wrong. Declaring mm/min keeps the emitter, the posts and every golden
where they are and makes the estimate honest: the fixture that read `Estimated cutting time · 0:13` now reads
`13:42`. Four documents were corrected rather than the code being bent to match them.

**L8** — the unit decision's fallout, recorded because it was larger than the question suggested.

Declaring millimetres per minute (`02ea9ef`) moved three things nobody had counted: the cutting-time model
(`crates/cam-ui/src/view3d/playback.rs`), every `view3d` fixture whose rates were chosen so a duration would
come out whole — restated as the same physical speed in mm/min so those durations stay what they were, rather
than rewriting a page of expectations — and `tests/tool_card.rs`, where `Estimated cutting time · 0:13`
became `13:42`. The lesson worth keeping: a unit is not a comment. Four documents and a timing model said one
thing, two behavioural consumers said the other, and both had tests.

Also closed here, found by the suite's own rewrite: `close_tool_library` promised in its doc that "the drafts
go with it" and left `tool_draft` behind, so a cancelled visit's typed diameter came back on the next one.
One line, now pinned by `editing::cancel_discards_the_draft_…`.

**L9** — delivery. One branch, and the commits are the review units:

| commit | lines | what a reviewer reads |
| --- | --- | --- |
| `a21b56a` | 331 | the schema and the six kinds |
| `6b84b31` | ~830 | the validation rules and the three writers |
| `c760cf8` | ~300 | the file format |
| `02ea9ef` | ~120 | the unit correction |
| `d04d8c6` | ~1500 | the modal itself, and its suite |

The modal commit is far past the 400-line slice and cannot be cut smaller without a commit that does not
compile: `form.rs` is a new file, and the unit correction surfaces through it. If the owner wants chained
pull requests, the seams are exactly those five commits.
