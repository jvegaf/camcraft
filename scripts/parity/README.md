# Parity harness (E0.4) and the corpus (E0.5)

`fixtures/corpus/` is a fixed set of real fabrication artefacts — six boards, each with `top`,
`bottom`, `mask-top`, `mask-bottom`, `silk-top`, `silk-bottom`, `outline` and `drill` (plus `innerN`
copper for the boards that have it). This directory is the instrument that measures what the tree makes
of them, and that fails when the answer changes.

```bash
scripts/parity/run.sh            # measure the corpus, compare against the goldens, non-zero on divergence
scripts/parity/run.sh --update   # re-pin the goldens to what the tree measures now (review the diff)
scripts/parity/export.sh         # regenerate fixtures/corpus/ from the KiCad demo boards
scripts/parity/export.sh --check # prove the committed corpus is what the plotter produces now
```

`cargo test -p cam-geom --test parity` runs the part that needs no parser: the corpus is whole (six
roles, provenance, a golden that pins exactly the files the board carries, refusals pinned without
geometry). That test is what `cargo test --workspace` sees; the measurement itself is the script.

## What is measured, and against what

Per artefact: `status` (`parsed`, `refused` or `timeout`), `polygons`, `contour_area_mm2` and `points`
(after flattening at `flatten_tolerance_mm`) for a Gerber layer; `tools` and `features` for an Excellon
file.

- `polygons` is the number of boundaries of the layer's image.
- `contour_area_mm2` is the **sum of the absolute contour areas**, which is the same rule this
  repository's boolean corpus already uses. It is deliberately not called "material area": a region
  with a hole contributes both boundaries, so a board outline reads as roughly twice the area it
  encloses. Subtracting holes needs the parity depth of each boundary, which is a question of D2 and
  G0.2; when it is answered it gets its own field instead of silently changing this one.
- `points` is the length of every boundary after `tessellate(flatten_tolerance_mm)`, so it is a
  function of the tolerance the golden records.

The comparison is: counts exactly, `contour_area_mm2` with a **relative epsilon of 1e-9**, `status` and
the tolerances exactly. That epsilon is E0.4's and it is relative because the area of a whole layer is a
sum of hundreds of terms — an absolute epsilon would be a different rule for a small board and a large
one.

**What this is not.** It is not parity against FlatCAM. D5 asks for an external oracle and that is a
different instrument with its own cost; what this harness compares against is the tree's own recorded
output over a fixed corpus, which is what "runs in CI over the minimum corpus and fails on divergence"
means in E0.4's exit criterion. Calling a golden an oracle would be the kind of claim this repository
writes down instead of making.

## Two halves, and why the measurement lives outside the workspace

`cargo test` runs the half it can bound: every artefact the goldens pin as *parsed* is measured again in
`crates/cam-geom/tests/parity.rs` and compared with the same rules, plus the corpus-integrity assertions.
That half is fast (0.1 s today) and needs no external process.

`scripts/parity/measure` is the other half: a standalone cargo project (its own `[workspace]`, path
dependencies on `crates/cam-gerber`, `crates/cam-excellon` and `crates/cam-geom`) that measures **one
artefact per process** so the script can bound it with `timeout(1)`.

The reason is not taste. Six layers of the corpus do not terminate in the geometry kernel today, and a
function call cannot be interrupted from inside the process while a process can. If the whole corpus were
measured inside a test, `cargo test --workspace` would hang on a real board — which is also why a layer
that exceeds the bound is recorded as `status: "timeout"` with a zeroed measurement, and why
`timeout_seconds` is part of every golden.

(Until task 044/W1c landed, the obvious home — dev-dependencies in `crates/cam-geom/Cargo.toml` — was
not available either, that file belonging to another task of the same wave. Its owner has since added
`cam-gerber` and `cam-excellon` there, which is what lets the in-workspace half measure real layers; the
out-of-workspace half stays because of the timeouts above, not because of the file.)

## The corpus, and why four of its six boards are substitutes

S1 of the task names six sources: KiCad 2 layers simple, KiCad 4 layers with pours, EasyEDA, Eagle,
Altium, and a board larger than 100×100 mm with pours. Only KiCad's own demo boards were available, so
four entries fill those slots with KiCad stand-ins, and each one says so in its `provenance.json`
(`is_substitute: true`, and `fills` naming the slot). That is not satisfaction of S1 and is not presented
as such: an Eagle export reads differently from a KiCad one, and a corpus that relabelled one as the
other would hide exactly the difference it exists to find.

Each `provenance.json` records the source board, the SHA-256 of that board, and the `kicad-cli` version
that plotted it. `export.sh` strips the creation timestamps and generator comments the plotter embeds,
so a replay lands on the same bytes — which is what makes the corpus a fixture instead of a snapshot of
some afternoon.

## The measurement of 2026-11-13, and what it says

`5 parsed, 49 not parsed, of 54 artefacts`. That is not parity; it is the distance, quantified:

| Artefacts | State | Why |
| --- | --- | --- |
| 42 Gerber layers | 38 refused, 4 timeout | three constructs stand between this reader and a real board: an aperture macro (`RoundRect`) with a non-zero rotation, an arc `D01` outside a `G36`/`G37` region, and the boolean engine refusing real copper with "the arrangement revisits a piece" |
| 6 drill files | 2 parsed, 4 refused | routed slots written as `X…Y…G85X…Y…`, which the Excellon reader does not accept |
| 3 Gerber layers parsed | 1 with geometry, 2 empty | `substitute-easyeda/outline` (2 polygons, 16 114.898 739 518 914 mm² of contour area), and two layers whose source file is legitimately empty of material |

Each refused artefact maps to an item the roadmap already names — `RoundRect` and the aperture macros to
E3b, the arc stroke and the region re-entry to E1.4, the arrangement refusal to G0.1/G0.2, the slots to
E1.6 — so the harness did not discover new work: it made the size of the existing work a number that
moves. The six timeouts are the same class of finding and the most expensive one to ignore, because a
kernel that does not terminate on a 234×140 mm board is not a slow kernel, it is a stopped one.

## When a golden moves

A divergence is either progress or a regression, and the diff decides which:

- a layer that was `refused` becomes `parsed` with geometry: progress, and the new numbers are the
  evidence for whatever item of the roadmap made it possible;
- a layer that was `parsed` becomes `refused`, or its area or point count moves: a regression until
  somebody explains why the geometry should be different;
- a `timeout` becomes `parsed`: a fix, and worth naming in the commit message;
- the artefact list changes: the corpus changed, which means `export.sh` ran, which deserves its own
  commit and its own provenance diff.

`run.sh --update` re-pins all of it. It is never part of a gate: CI runs `run.sh` and fails when the
goldens and the tree disagree.
