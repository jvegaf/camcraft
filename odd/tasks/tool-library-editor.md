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
| T1 | S4, S5, S10 | inline | schema, kind vocabulary and library writer |
| T2 | S1, S2, S3, S4, S5, S6, S7, S8, S12 | inline | the three-column layout and header |
| T3 | S9, S6 | inline | editable drafts, validation and the computed chipload |
| T4 | S10 | inline | library actions and the writable path |
| T5 | S11 | inline | JSON import and export |
| T6 | S13, S14 | inline | retire the fabricated library; pin every claim |

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
