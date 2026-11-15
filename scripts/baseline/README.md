# The FlatCAM baseline (E0.6)

FlatCAM-Plus is the reference implementation this port is measured against, and E0.6 asks for its
numbers **before** any PCB operation is judged: `docs/ROADMAP_CAMCRAFT.md` does not write phase 4 until
E0.6 and E0.7 are green, "porque sin baseline la promesa no es medible". The parity harness of E0.4 says
in its own README that it is *not* parity against FlatCAM; this is the other instrument, the one that
runs FlatCAM over the same corpus and records what it costs.

```
scripts/baseline/run.sh            # measure the corpus with FlatCAM and compare against the goldens
scripts/baseline/run.sh --update   # re-pin the goldens to what FlatCAM measures now
```

FlatCAM is a separate Python environment, so the script needs one:

```
FLATCAM_PYTHON=~/Code/FlatCAM-Plus/.venv/bin/python scripts/baseline/run.sh
```

`FLATCAM_PYTHON` defaults to `~/Code/FlatCAM-Plus/.venv/bin/python` and the script refuses to run against
an interpreter that cannot import FlatCAM, Shapely and PyQt6. The measured environment is FlatCAM-Plus
1.3.6 on Python 3.14.4 with Shapely 2.2.0 and PyQt6 6.11.0, and **that is part of the golden**: a
measurement from another FlatCAM or another Shapely is a different baseline, so the comparison fails
instead of comparing across versions.

A full run boots FlatCAM once per artefact and once per operation — sixty-six processes, about eight
seconds of boot each — so it takes roughly ten minutes. `BASELINE_BOARDS=kicad-2l-simple` narrows it to
one board while working on the instrument; `--update` refuses to take that filter, because a partial
re-pin would leave the other goldens describing another FlatCAM. A narrowed run compares only the boards
it measured, names the ones it skipped on stderr, and treats a run that measured **nothing** as a
divergence rather than a clean corpus.

## What it measures

FlatCAM runs **offscreen** (`QT_QPA_PLATFORM=offscreen`) with `HOME`/`XDG_*` pointed at a temporary
directory, so every run sees FlatCAM's factory options rather than the options of whoever ran it, and its
startup update check is switched off: an instrument does not reach the network. Operations are driven
through FlatCAM's Tcl shell — `open_gerber`, `isolate`, `cncjob`, `open_excellon`, `drillcncjob` — which
is the surface FlatCAM documents for scripting.

Per artefact, one process:

- `status`: `parsed`, `refused` (FlatCAM reported the file and created no object), `timeout` or `error`
  (the measurer crashed — never a measurement).
- `seconds`: the time to open and evaluate the file, measured inside the process.
- `peak_rss_kb`: the peak resident set of that process.
- `polygons`, `contour_area_mm2`, `vertices` for a Gerber layer, `tools` and `features` for an Excellon
  file. `polygons` is the number of rings and `contour_area_mm2` is the **sum of the absolute ring
  areas** — the same two definitions `scripts/parity/` uses, deliberately, so the two goldens can be read
  side by side.

Per board, per operation:

| operation | source | parameters |
| --- | --- | --- |
| `isolation` | `top.gbr` | tool ⌀ 0.2 mm, 2 passes, 15 % overlap, full isolation, passes combined |
| `drill` | `drill.drl` | drill −1.8 mm, travel 2 mm, plunge feed 60 mm/min, toolchange at Z 30 |

Each records `seconds` for the operation itself, and the move count read from `CNCjob.gcode_parsed`: the
list of tool paths FlatCAM generated, each tagged `T`/`C` (travel/cut) × `F`/`S` (fast/slow), plus the
total path length in millimetres. Counting the moves on the object rather than in the G-code text keeps
the number independent of the post-processor dialect and of how it formats numbers. The toolchange is on
for the drill operation on purpose: without it FlatCAM drills every hole with the first tool in the file,
which would record a move count no machine would run for a multi-tool board.

The cost of booting FlatCAM (~8 s) is measured and reported per run, and never added to any `seconds`: it
is the price of the instrument, not of the measured work.

## What the comparison checks — and what it deliberately does not

A re-pin also refuses an incomplete run: if any artefact or operation came back `error` or `timeout`, the
whole set is left untouched, because a record that says "the harness did not see this" is not a
measurement and must never replace one. Each golden is staged beside its target, renamed only after all
of them are staged, and the old bytes are put back if a rename fails, so the pinned set is either the
old version or the new one and never a mix of both.

Checked exactly: every `status`, every count (`polygons`, `vertices`, `tools`, `features`, `moves`,
`cut_moves`, `travel_moves`), `path_mm` and `contour_area_mm2` within a **relative** `1e-9` (E0.4's
epsilon; an area is a sum of hundreds of terms, and an absolute epsilon would be a different rule for a
small board and a large one), the tolerances, the operation parameters, and the FlatCAM, Python and
Shapely versions.

Never checked, on purpose:

- **`seconds` and `peak_rss_kb`.** They are recorded — the roadmap asks for a measured baseline, and a
  reviewer should see them move — but they belong to the machine and its load, exactly like the `bench`
  comparison of E0.7. `compare.py` prints them next to their goldens with a ratio and never fails on it.
- **FlatCAM's `vertices` against camcraft's `points`.** FlatCAM flattens every arc through Shapely while
  the workspace preserves arcs, so the two count different things. `docs/ROADMAP_CAMCRAFT.md` states the
  rule: "La comparación es de área y cobertura, nunca de conteo de vértices con FlatCAM." The *area* is
  the comparable number, and it is comparable only loosely across the two implementations (an arc drawn
  at 64 segments is not the circle it approximates); the baseline compares FlatCAM with FlatCAM.

## Why it is not in CI

CI has no FlatCAM: it would need Python 3.14 with PyQt6, Qt's runtime and a full GUI stack, for a job
whose useful half is wall time on one machine. `scripts/check.sh` and the `parity` and `bench` jobs do not
call it. What CI does guarantee is that the goldens stay committed and that nothing else rewrites them.

## What this is not

It is not a parity check on camcraft. It records what the reference implementation does with the corpus
and what it costs; the day a camcraft operation is judged against it, the numbers are already here, and
the comparison is area and coverage rather than move-for-move — FlatCAM orders drills with OR-Tools under
a time budget, so two of its own runs do not emit the same G-code (`docs/GAPS_CAMCRAFT.md`, D2).

It also does not measure non-copper clear. That is phase 4's operation and its cost on the pour boards is
not yet bounded; when F4 exists, its own baseline row is what proves the product's thesis, and it is
measured then rather than guessed now.
