# Screenshots of the reference app

Frames from two videos about **Prancheta CNC** (pt-BR), kept as design evidence. They are the ground truth
that replaced an earlier, weaker source: before these existed the shell's layout was *inferred* from strings
inside `data/Prancheta CNC.exe` (see `odd/tasks/camui-shell.md` L2). Where a screenshot and an inference
disagree, the screenshot wins.

Each entry says which pattern the frame proves, because a screenshot nobody annotated becomes decoration.

| File (date/time in the name) | What it shows | The pattern it settles |
| --- | --- | --- |
| `03-23-52 … Cómo creé un software…` | The editor over `prancheta-cnc-local.html`, empty job | **Left palette with labelled groups** (`CRIAR VETORES`, `TRANSFORMAR OBJETOS`, `EDITAR OBJETOS`, `CONTORNAR E POSICIONAR`), one button active with an amber fill; right inspector with `Desenho \| Percursos` tabs, uppercase section headers (`SELEÇÃO`, `MATERIAL`, `ENTRADA PELO TECLADO`), material measurements as labelled boxes two to a row; bottom bar of snap chips (`Ponta`, `Meio`, `Centro`, `Grada 10`) and a hint line |
| `03-24-32 … Cómo creé un software…` | Adaptive toolpath over a star | A per-item form grouped in sections (`FERRAMENTA`, `CORTE`, `RESULTADO`) with **pairs of fields where one being zero derives the other**, and an **estimated time** as the last line of the form; the path itself drawn dashed over the vectors |
| `03-26-19 … Software Gratis…` | Modal `Novo trabalho — defina o material` | **Choice cards** (Chapa plana / Rotativo) plus a live annotated isometric diagram and a caption explaining what the origin marker means; footer `Cancelar` / `Continuar` |
| `03-26-40 … Software Gratis…` | `Encaixar dentro de` flyout | **Segmented controls** for every small choice (`Material \| Último vetor`, `Não \| De 90° \| Livre`, `Rápida \| Normal \| Fina`) with **one bullet per option saying what it costs** |
| `03-26-58 … Software Gratis…` | 3D view of the stock | The top bar **changes with the mode** (`Material \| Vetores \| Percursos \| Peça \| Calcular`) — a view is a mode, not a panel |
| `03-27-21 … Software Gratis…` | `Escolher ferramenta` dialog | Master-detail picker: the library on the left, the chosen tool's form on the right, an annotated tool diagram, and a footer of secondary actions |
| `03-27-41 … Software Gratis…` | `Novo percurso — Chanfro` flyout | A **current-tool card** at the top of the form (glyph, name, the two numbers that matter, and a link to change it) |
| `03-27-57 … Software Gratis…` | 3D simulation cutting a pocket | A **progress bar with elapsed/total and the piece being cut**, plus the stacked primaries (`Simular em 3D`, `Salvar G-code…`) |

## What Campilot has taken from it so far

- The 52 px icon rail became a **200 px palette with labelled groups** (`DRAW`, `TRANSFORM`, `VECTOR EDIT`,
  `VIEW`) — an unlabelled rail stops working past a dozen glyphs, which is why the Edit commands had ended up
  nameless.
- The inspector's Job tab became **labelled measurement boxes two to a row** (`MATERIAL`, `DOCUMENT`), and the
  Tools tab groups its commands under the same uppercase labels.
- The 3D view became an exclusive **mode** rather than a panel stacked over the canvas.

## Still to take

- Choice cards and annotated diagrams for the material dialog (needs an editable job).
- Segmented controls with a bullet per option, wherever a small choice appears.
- The current-tool card and the estimated time in the toolpath form (needs toolpaths authored in the shell).
- The simulation progress bar with elapsed/total and the piece name.
