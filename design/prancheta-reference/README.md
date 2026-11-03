# Full-resolution frames of Prancheta CNC

Fifteen screenshots of the reference app at its own resolution (2560 × 1368), captured on 2026-11-03 between
03:16 and 03:48. They supersede `design/Screenshots-reference/`, which held eight frames photographed off two
YouTube videos: those are small, letterboxed and half of the chrome is cropped, so the shell's layout had to be
guessed at. Here nothing has to be guessed.

Where these frames and any earlier evidence disagree, **these frames win**. See "What it corrects" below,
because one disagreement is expensive: the shell artboard that `camui-shell` T6 implemented was drawn from the
video frames and invented a stage bar that the app does not have.

Each entry says which pattern the frame proves, because a screenshot nobody annotated becomes decoration.

## The frame, once, so the entries can be short

```
┌────────────────────────────────────────────────────────────┬─────────────────┐
│ titlebar: "Prancheta CNC — Sem título"              ─ □ ✕  │                 │
├──────────┬─────────────────────────────────────────────────┤   Usinagem      │
│ ARQUIVO  │  mm ────────── regla horizontal ───────   [2D|3D]│  ┌ ribbon ───┐  │
│ (2 filas)│  ┌───────────────────────────────────────────┐  │  │ PERCURSOS│  │
│ DESENHAR │  │                                           │  │  │ RELEVO   │  │
│ (12)     │ m│              CANVAS                       │  │  │ MÁQUINA  │  │
│ EDITAR   │ m│                                           │  │  └──────────┘  │
│ (8)      │  │                                           │  │  ┌ editor ──┐  │
│ ORGANIZAR│  │                                           │  │  │ tarea    │  │
│ (12)     │  │                                           │  │  │ ✓    ✕   │  │
│ EXIBIR   │  └───────────────────────────────────────────┘  │  └──────────┘  │
│ (6)      │                                                 │  ┌ Camadas ─┐  │
│ ──────── │                                                 │  └──────────┘  │
│ brand    │                                                 │                 │
├──────────┴─────────────────────────────────────────────────┴─────────────────┤
│ X 0 Y 0 │ Encaixes Ponto Meio Centro Grade 10 │  …mensaje contextual…       │
└──────────────────────────────────────────────────────────────────────────────┘
```

Five facts that the earlier evidence got wrong or never saw, and that every frame below assumes:

1. **There is no stage bar.** No `Design → Edit → Toolpaths → Simulate → G-code`. The workflow is carried by the
   left rail plus the state of whatever task is open in the right panel.
2. **The right panel is a ribbon of icons**, three semantic categories — `PERCURSOS`, `RELEVO E PEÇA 3D`,
   `MÁQUINA E SAÍDA` — and under it a task editor that *replaces* the panel's content and carries `✓` / `✕`.
3. **Two rulers in mm**, labelled, with an amber marker tracking the cursor on both.
4. **`Camadas` is its own collapsible panel** at the bottom right, one row per layer: colour swatch, name, vector
   count, eye, bin.
5. **The status bar has three zones**: live `X/Y` · snap chips (`Encaixes`, `Ponto`, `Meio`, `Centro`,
   `Grade 10`) · a contextual message on the right.

## The frames

| File (time in the name) | What it shows | The pattern it settles |
| --- | --- | --- |
| `Screenshot 2026-11-03 031627.png` | Modal `Novo trabalho — defina o material` over the empty shell | **Choice cards with a live annotated diagram** (Chapa plana / Rotativo 4º eixo), then `MEDIDAS DA CHAPA` as labelled fields, then `ZERO DA MÁQUINA` as a two-way segmented (`Em cima do material` \| `Na mesa`), then an **origin picker**: nine handles on a stock rectangle with the chosen point filled blue. The isometric on the right carries dimension callouts (`18 mm`, `600 mm`, `400 mm`) and the `X0 Y0` marker, and a caption says what the marker means |
| `Screenshot 2026-11-03 031736.png` | The shell after `Começar`, stock empty | The empty-document state. **The origin is the centre of the stock, not its corner**, and both rulers are centred on it; the axis mark (green Y, red X) sits at that centre. Status reads `X -28,38 Y 8,33`; the contextual message reads `Material atualizado` |
| `Screenshot 2026-11-03 032022.png` | Modal `Importar 775-noise-filter-Edge_Cuts.dxf` | **The import dialog reports what it found before offering anything**: `1058 vetor(es) encontrados (1058 abertos). Unidades: polegadas.` Then `Juntar vetores abertos que se tocam` + `Tolerância para juntar` as a checkbox that enables its own number, `Suavizar`, `Posição` as a dropdown (`Centralizar no material`), `Ajustar o tamanho do material ao desenho`. Footer `Cancelar` / `Importar` |
| `Screenshot 2026-11-03 032109.png` | The imported drawing | Import **creates a layer**: `Camadas 2 · ativa: Camada 1`. Vectors draw as magenta dashes; a selection draws as a blue dashed bounding box. Status message `Importado: 14 vetor(es)` |
| `Screenshot 2026-11-03 032331.png` | `Escolher ferramenta` over a `Novo percurso · Furação` panel | The tool library as **master-detail**: a search field, then a tree grouped by tool family with a count (`Fresa reta (topo reto) (8)`), each row a **mini tool glyph** plus name plus a dim second line (`Ø6 · 2 corte(s)`); the detail form grouped under `GEOMETRIA` / `PASSADAS` / `VELOCIDADES` / `MÁQUINA`; a **tool drawn to scale with dimension callouts**; a footer of library actions (`+ Nova`, `Duplicar`, `Apagar`, `Exportar…`, `Importar…`, `Restaurar padrão`) above the commit pair |
| `Screenshot 2026-11-03 032653.png` | Same dialog, `T22 Fresa 1,0 mm` chosen | The dialog's own title becomes the selected tool. In the task panel behind it, the tool is a **current-tool card** (glyph, `T22 · Fresa 1 corte 3,175 mm`, its two numbers, and `Escolher outra ferramenta…`). A tooltip states the copy rule: `Mudanças valem para percursos novos; cada percurso guarda a própria cópia da ferramenta.` |
| `Screenshot 2026-11-03 032737.png` | The `Tipo` dropdown open | The tool taxonomy is closed and short: Fresa reta (topo reto), Fresa esférica (ball nose), Cônica de ponta redonda, Fresa V (V-bit), Fresa de gravação (ponta plana), Broca |
| `Screenshot 2026-11-03 032838.png` | A generated drilling toolpath | `Edição rápida dos parâmetros` is a **collapsible block that duplicates the four numbers you actually change** (Passo vertical, Avanço, Mergulho, Rotação) above the full `CORTE` section. Toolpaths draw violet; the two boundary holes are highlighted pink-dashed. The task ends with the note `O percurso é calculado quando você clicar em "Gerar percurso".` and `Gerar percurso` / `Cancelar` |
| `Screenshot 2026-11-03 032943.png` | The **2D \| 3D split** with a finished simulation | The simulation is a mode that changes the whole surface: canvas splits, and an overlay row appears over the 3D half with `Material` `Vetores` `Percursos` `Fresa` checkboxes plus `Centralizar` and a `?`. A home button and an **axis triad labelled `TOPO` / `FRENTE` / `DIREITA`**. A playback bar: `▶`, speed (`20x`), `Até o fim`, a scrubber, and `0:05 / 0:05  T22 · Furação 1 · Z 5.000`. The right panel lists toolpaths as cards (checkbox, name, `T22 · Furação · 0:05`, a warning line, reorder/edit/delete), then `Tempo estimado dos ativos`, `Qualidade da simulação`, `Simular de novo`, and the stacked secondaries `Simular em 3D` / `Salvar G-code…` |
| `Screenshot 2026-11-03 033350.png` | `Editar · Perfil` panel | A toolpath edit is the same surface as its creation, prefixed `Editar`. `CORTE` carries `Profundidade de corte`, `Sobremetal`, `Lado da fresa` (`Dentro do vetor`), `Sentido` (`Concordante`), the `Pontes para segurar a peça` checkbox and the `Entrar em rampa` checkbox that reveals `Comprimento da rampa`. Canvas semantics: blue = toolpath, green = selected contour, violet = layer vectors, pink dashed = the source vector |
| `Screenshot 2026-11-03 033406.png` | The same frame with `Camadas` expanded | The layer panel is its own block: a `Clique para ativar` row with a `+`, then one row per layer (`F.Cu 26`, `Edge.Cuts 14`, `Camada 1 0`) with swatch, eye and bin, then `Mover seleção para camada ativa` |
| `Screenshot 2026-11-03 034023.png` | `Novo percurso · Desbaste 3D` | Choices that need a picture get **image cards**: `FRONTEIRA DE USINAGEM` (Relevos / Material / Vetores, the third selected) and `ESTRATÉGIA DE DESBASTE` (Desbaste adaptativo / Varimento 3D). Between them the concrete feedback `1 vetor(es) selecionado(s) com fronteira.` and the two numbers that follow from the choice |
| `Screenshot 2026-11-03 034320.png` | `Escolher ferramenta`, V-bit family | The tree scrolls through families (`Cônica de ponta redonda (3)`, `Fresa V (V-bit) (4)`, `Fresa de gravação (ponta plana) (1)`, `Broca (2)`) and the detail form **changes its fields with the family**: a V-bit asks `Diâmetro maior`, `Ângulo incluído`, `Largura da ponta`, and its drawing shows the included angle. The task panel behind reads `Novo percurso · V-bit` with `Profundidade máxima`, `Precisão`, `Altura de reposicionamento` |
| `Screenshot 2026-11-03 034421.png` | A drawing tool active while a simulation plays | A drawing tool **takes over a panel, not a modal**: top-left of the canvas, the instructions (`Clique ponto a ponto. Volte ao quadradinho do primeiro ponto para fechar o vetor. Shift trava em 15°.`), a live count `0 ponto(s)`, and `Terminar` / `Fechar` / `Desfazer ponto` |
| `Screenshot 2026-11-03 034847.png` | Simulation in progress | The playback bar names the move it is on: `0:11 / 1:10  T24 · Perfil 1 · Z -0.400`, with speed `1x`. Status says `Simulando…`; the tool is red where it is cutting. Every frame of the bar answers "where am I in the program" without a separate panel |

## The visual system these frames pin down

| Token | Observed |
| --- | --- |
| App / chrome ground | near-black slate, roughly `#14161a` |
| Canvas | one step lighter, roughly `#1b1f26` |
| Panel / raised card | `#1e222a` / `#232833` |
| Hairline and border | `#2b3140` |
| Primary accent | blue, roughly `#4f7fe8` — primary button, active tool, the `2D` pill, checkbox fill |
| Modal header | dark navy band, centred title |
| Text | `#e6e9ef` high, `#8b93a3` dim |
| Stock material | tan `#d8b48c` top, `#b8895c` edge |
| Layer colours | magenta, violet, blue, green |
| Canvas semantics | blue = toolpath · green = selected contour · pink dashed = boundary/selection · violet = layer vectors |
| Radii | 6–8 on controls, 10 on cards, 12 on modals |
| Section label | 11 px, uppercase, wide tracking, dim |
| Icons | 24 px, 1.5–1.6 stroke, one weight, fill only for state |
| Fields | ~30–32 px tall, **value right-aligned**, unit as a suffix |

## What it corrects in the earlier evidence

- **The stage bar does not exist.** `design/camui-shell/01-shell-2d.html` draws `Design → Edit → Toolpaths →
  Simulate → G-code`, and `camui-shell` T6 implemented it in `crates/cam-ui/src/chrome.rs`. No frame of the
  reference app has one. The artboard has to be redrawn, not retouched.
- **The right panel is not a tabbed inspector.** The artboard draws `Inspector` with `Selection` / `Toolpath` /
  `Material` tabs. The app draws a ribbon of icon categories, and the task editor replaces the panel.
- **`Camadas` is a panel, not a section.** It collapses and expands on its own, below the task editor.
- **The rulers were never seen at all.** Both of them, with the amber cursor marker.

## What Campilot has taken from it so far

- **The stage bar is gone, and with it the stage.** `camui-prancheta` T1 (`c9f3c59`) removed the
  `Design › Edit › Relief › Toolpaths › Prove` strip, the `Stage` enum behind it, and the rail that showed only
  the current stage's commands. The rail is now the whole command table, grouped and labelled, so a drawing
  command and an edit command are reachable in the same frame. The two sentences the strip carried that were
  worth keeping — an empty sheet, and a document with no toolpath — became `next_step(app)` on the Job tab,
  which asks the document instead of the mode.

The rest of the list below is still `odd/tasks/camui-prancheta.md`'s work.

## Still to take

- The five-group left rail with the brand block and the `Atualizado em` timestamp.
- The two rulers with the amber cursor marker.
- The right panel as a ribbon of three categories plus a task editor carrying `✓` / `✕`.
- The `Camadas` panel as its own collapsible block.
- The tool library as a master-detail modal with a scaled, annotated tool drawing.
- The per-strategy task editors, including image cards for choices that need a picture.
- The `2D | 3D` split with the overlay row, the axis triad and the playback bar.
- The three-zone status bar, including the contextual message on the right.
