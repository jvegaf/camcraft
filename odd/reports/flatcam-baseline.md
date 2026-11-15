# The FlatCAM baseline (E0.6)

FlatCAM-Plus, the reference implementation this port is measured against, run over the committed corpus on
one machine and recorded here with its numbers: how long it takes to read each fabrication artefact, how
much memory it costs, and what it takes to isolate and to drill each board. `docs/ROADMAP_CAMCRAFT.md`
does not write phase 4 until E0.6 and E0.7 are green, "porque sin baseline la promesa no es medible"; this
is that baseline, and the instrument that keeps it honest is `scripts/baseline/` with one pinned golden per
board in `fixtures/baseline/`.

Measured **2026-11-15** with FlatCAM-Plus 1.3.6 on Python 3.14.4, Shapely 2.2.0 and PyQt6 6.11.0, offscreen,
one process per artefact and one per operation. `just baseline` (or `FLATCAM_PYTHON=…
scripts/baseline/run.sh`) measures the corpus again and compares; the numbers below are what it pinned.

## 1. What the baseline says

**FlatCAM reads 51 of the 54 corpus artefacts and refuses 3, with no timeout anywhere.** The slowest single
layer is 43.05 s and 2.85 GB (`kicad-4l-pours/silk-top`), the whole corpus imports in 215.84 s of measured
work, and every board finishes its isolation and its drilling in under five seconds.

That number reframes the one this repository had been living with. The parity goldens of E0.4 record the
tree reading **12** artefacts, refusing **24**, and **18** that do not terminate inside thirty seconds. So
the eighteen timeouts are not a property of the corpus and not a property of Gerber: the reference
implementation reads those same files in seconds, and the bound exists because our kernel does not finish
them. E1.4's exit criterion — *the six corpus boards produce geometry that passes E0.4 against FlatCAM* —
was blocked on an oracle that did not exist; it now has numbers to be measured against, and the thirty-eight
artefacts we do not read are a defect with a size, not an unknown.

Where the two implementations **do** overlap, they agree, and that is the first external evidence this port
has that its geometry is right rather than merely self-consistent:

- all six drill files: the tool count and the feature count are **identical** to the last one;
- every Gerber layer both of them read: the **ring count is identical** and the sum of absolute ring areas
  matches within 0.02 %, the residue of FlatCAM flattening arcs in Shapely and the workspace tessellating
  them at its own tolerance;
- two refusals are shared: `kicad-2l-simple/bottom` and `kicad-2l-simple/mask-bottom` are rejected by both,
  which is worth knowing before anyone spends a day on them.

One disagreement runs the other way and is the most interesting line in the table:
`substitute-easyeda/silk-bottom` is **refused by FlatCAM** and our reader reports it as `parsed` with zero
rings — so the honest statement is that our reader accepts a file and produces nothing where the reference
rejects the file. That is a follow-up for the reader, not a baseline question, and it is recorded here so it
is not rediscovered.

## 2. Import: time, memory and what comes out, per board

`Import, all layers` is the sum of the per-artefact times measured inside each process; the ~6.5 s cost of
booting FlatCAM is reported separately and is not in any number here.

| Board | Artefacts | Parsed | Refused | Import, all layers | Slowest layer | Peak RSS |
| --- | ---: | ---: | ---: | ---: | --- | ---: |
| `kicad-2l-simple` | 8 | 6 | 2 | 8.90 s | `top` 4.97 s | 759 MB |
| `kicad-4l-pours` | 10 | 10 | 0 | 113.33 s | `silk-top` 43.05 s | 2852 MB |
| `kicad-large-pours` | 8 | 8 | 0 | 5.48 s | `silk-top` 1.31 s | 759 MB |
| `substitute-altium` | 10 | 10 | 0 | 22.60 s | `top` 5.27 s | 1593 MB |
| `substitute-eagle` | 10 | 10 | 0 | 53.46 s | `bottom` 22.21 s | 2194 MB |
| `substitute-easyeda` | 8 | 7 | 1 | 12.08 s | `bottom` 7.62 s | 1193 MB |
| **6 boards** | **54** | **51** | **3** | **215.84 s** | **43.05 s** | **2852 MB** |

## 3. Operations: isolation and drilling, per board

Both operations are measured on the artefact camcraft's own operations read — `top.gbr` for isolation,
`drill.drl` for drilling — so the numbers are comparable rather than merely adjacent. Isolation is tool ⌀
0.2 mm, 2 passes, 15 % overlap, full isolation, passes combined; drilling is −1.8 mm into the material, 2 mm
travel, 60 mm/min plunge, toolchange at Z 30. Moves are counted on FlatCAM's own `CNCjob.gcode_parsed`
(travel/cut), not in the emitted G-code text, and the parenthesised pair is `cut/travel`.

| Board | Isolation | Isol. moves | Isol. path | Drilling | Drill moves | Drill path |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `kicad-2l-simple` | 0.83 s | 19 (9/10) | 586 mm | 0.23 s | 177 (87/90) | 994 mm |
| `kicad-4l-pours` | 3.92 s | 835 (417/418) | 4737 mm | 0.59 s | 1383 (688/695) | 6309 mm |
| `kicad-large-pours` | 2.31 s | 653 (326/327) | 5141 mm | 0.57 s | 516 (251/265) | 6714 mm |
| `substitute-altium` | 4.84 s | 809 (404/405) | 9570 mm | 0.60 s | 1056 (523/533) | 6777 mm |
| `substitute-eagle` | 1.47 s | 375 (187/188) | 2224 mm | 0.67 s | 889 (439/450) | 6035 mm |
| `substitute-easyeda` | 1.15 s | 313 (156/157) | 2832 mm | 0.38 s | 338 (165/173) | 3936 mm |
| **6 boards** | **14.53 s** | **3004** | | **3.03 s** | **4359** | |

Three things the table says out loud. Isolation is **not** the expensive half on these boards — 14.53 s for
all six, against 215.84 s for importing them; the cost of the baseline lives in the parsers, which is where
the product's work will be too. Drilling costs a fifth of what isolation does in time and produces more
moves, because a drill program is mostly travel between holes (2 206 of its 4 359 moves are travel), which is
the property F2's ordering has to beat. And `kicad-2l-simple` isolates in 19 moves while `kicad-4l-pours`
needs 835: the corpus' spread is two orders of magnitude, so a single number will never describe a board.

## 4. Where camcraft and FlatCAM agree, and where they do not

Left column is what `fixtures/corpus/<board>/parity.json` recorded for the tree, right column what FlatCAM
measured today. Rows where both read the layer compare ring counts and area; rows where they differ say so.

| Artefact | camcraft | FlatCAM | Agreement |
| --- | --- | --- | --- |
| `kicad-2l-simple/bottom` | refused | refused | **both refuse** |
| `kicad-2l-simple/drill` | 3 tools, 88 features | 3 tools, 88 features | **identical** |
| `kicad-2l-simple/mask-bottom` | refused | refused | **both refuse** |
| `kicad-2l-simple/mask-top` | refused | parsed (126 rings) | **disagree** |
| `kicad-2l-simple/outline` | refused | parsed (2 rings) | **disagree** |
| `kicad-2l-simple/silk-bottom` | refused | parsed (347 rings) | **disagree** |
| `kicad-2l-simple/silk-top` | timeout | parsed (152 rings) | **disagree** |
| `kicad-2l-simple/top` | timeout | parsed (36 rings) | **disagree** |
| `kicad-4l-pours/bottom` | timeout | parsed (341 rings) | **disagree** |
| `kicad-4l-pours/drill` | 6 tools, 692 features | 6 tools, 692 features | **identical** |
| `kicad-4l-pours/inner1` | timeout | parsed (841 rings) | **disagree** |
| `kicad-4l-pours/inner2` | timeout | parsed (839 rings) | **disagree** |
| `kicad-4l-pours/mask-bottom` | 144 rings, 427.898 mm² | 144 rings, 427.860 mm² | same ring count, area -0.01% |
| `kicad-4l-pours/mask-top` | refused | parsed (567 rings) | **disagree** |
| `kicad-4l-pours/outline` | refused | parsed (2 rings) | **disagree** |
| `kicad-4l-pours/silk-bottom` | refused | parsed (1026 rings) | **disagree** |
| `kicad-4l-pours/silk-top` | refused | parsed (1893 rings) | **disagree** |
| `kicad-4l-pours/top` | refused | parsed (533 rings) | **disagree** |
| `kicad-large-pours/bottom` | timeout | parsed (266 rings) | **disagree** |
| `kicad-large-pours/drill` | 13 tools, 251 features | 13 tools, 251 features | **identical** |
| `kicad-large-pours/mask-bottom` | refused | parsed (246 rings) | **disagree** |
| `kicad-large-pours/mask-top` | refused | parsed (245 rings) | **disagree** |
| `kicad-large-pours/outline` | refused | parsed (2 rings) | **disagree** |
| `kicad-large-pours/silk-bottom` | 6 rings, 12.389 mm² | 6 rings, 12.389 mm² | same ring count, area +0.00% |
| `kicad-large-pours/silk-top` | refused | parsed (395 rings) | **disagree** |
| `kicad-large-pours/top` | timeout | parsed (397 rings) | **disagree** |
| `substitute-altium/bottom` | refused | parsed (569 rings) | **disagree** |
| `substitute-altium/drill` | 10 tools, 526 features | 10 tools, 526 features | **identical** |
| `substitute-altium/inner1` | timeout | parsed (553 rings) | **disagree** |
| `substitute-altium/inner2` | timeout | parsed (642 rings) | **disagree** |
| `substitute-altium/mask-bottom` | timeout | parsed (301 rings) | **disagree** |
| `substitute-altium/mask-top` | timeout | parsed (793 rings) | **disagree** |
| `substitute-altium/outline` | refused | parsed (2 rings) | **disagree** |
| `substitute-altium/silk-bottom` | timeout | parsed (100 rings) | **disagree** |
| `substitute-altium/silk-top` | timeout | parsed (1383 rings) | **disagree** |
| `substitute-altium/top` | refused | parsed (634 rings) | **disagree** |
| `substitute-eagle/bottom` | refused | parsed (393 rings) | **disagree** |
| `substitute-eagle/drill` | 12 tools, 445 features | 12 tools, 445 features | **identical** |
| `substitute-eagle/inner1` | timeout | parsed (406 rings) | **disagree** |
| `substitute-eagle/inner2` | timeout | parsed (297 rings) | **disagree** |
| `substitute-eagle/mask-bottom` | refused | parsed (620 rings) | **disagree** |
| `substitute-eagle/mask-top` | refused | parsed (62 rings) | **disagree** |
| `substitute-eagle/outline` | refused | parsed (2 rings) | **disagree** |
| `substitute-eagle/silk-bottom` | refused | parsed (1252 rings) | **disagree** |
| `substitute-eagle/silk-top` | timeout | parsed (119 rings) | **disagree** |
| `substitute-eagle/top` | timeout | parsed (194 rings) | **disagree** |
| `substitute-easyeda/bottom` | refused | parsed (175 rings) | **disagree** |
| `substitute-easyeda/drill` | 7 tools, 165 features | 7 tools, 165 features | **identical** |
| `substitute-easyeda/mask-bottom` | 165 rings, 434.138 mm² | 165 rings, 434.099 mm² | same ring count, area -0.01% |
| `substitute-easyeda/mask-top` | 165 rings, 434.138 mm² | 165 rings, 434.099 mm² | same ring count, area -0.01% |
| `substitute-easyeda/outline` | 2 rings, 16114.899 mm² | 2 rings, 16114.899 mm² | same ring count, area +0.00% |
| `substitute-easyeda/silk-bottom` | parsed (0 rings) | refused | **disagree** |
| `substitute-easyeda/silk-top` | refused | parsed (586 rings) | **disagree** |
| `substitute-easyeda/top` | timeout | parsed (188 rings) | **disagree** |

The shape of that table is the work plan: 40 layers the reference reads and we do not (18 of them timeouts,
22 refusals), 6 drill files already identical, 5 Gerber layers agreeing on geometry within a hundredth of a
percent, 2 shared refusals, and 1 layer where our reader produces nothing and FlatCAM produces nothing
either but refuses to pretend otherwise.

## 5. What this baseline does not measure

- **Non-copper clear.** Phase 4's operation, and the one the product's thesis is about. It is deliberately
  not in this baseline: its cost on the pour boards is not yet bounded, and measuring it is E4.7's job with
  the operation that will compete with it. What is recorded here is the cost of *reading* the boards it will
  run on.
- **Anything but one machine.** `seconds` and `peak_rss_kb` are recorded and compared only as a warning, the
  same way the `bench` comparison of E0.7 is hardware-coupled. A golden diff on time is a note, never a
  failure; a diff on moves, ring counts or area is a failure.
- **Vertex counts against our own.** FlatCAM flattens arcs through Shapely and the workspace preserves them,
  so `vertices` here and `points` in `parity.json` count different things. The roadmap says it plainly: "La
  comparación es de área y cobertura, nunca de conteo de vértices con FlatCAM."
- **Move-for-move equality.** FlatCAM orders drills with OR-Tools under a time budget, so two of its own runs
  need not emit the same G-code (`docs/GAPS_CAMCRAFT.md`, D2). The move *count* it produces is stable — three
  independent full runs recorded here agree exactly — but a future camcraft comparison is area and coverage,
  not a byte diff.

## 6. How to reproduce it

```
FLATCAM_PYTHON=~/Code/FlatCAM-Plus/.venv/bin/python just baseline
```

A run boots FlatCAM 66 times (once per artefact, once per operation) and takes about eleven minutes on this
machine; `scripts/baseline/README.md` explains the harness, and `--update` re-pins the goldens after the diff
has been reviewed. It is not in CI and cannot be: CI has no FlatCAM, and half of what it records is wall time.
