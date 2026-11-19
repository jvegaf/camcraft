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

Per artefact: `status` (`parsed`, `refused`, `timeout`, `skipped` or `error`), `polygons`,
`contour_area_mm2` and `points` (after flattening at `flatten_tolerance_mm`) for a Gerber layer;
`tools` and `features` for an Excellon file.

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

### Comparing against FlatCAM

`scripts/parity/compare-flatcam.py` reads the two committed JSON files — this tree's goldens and
`fixtures/baseline/`, which task 059 pinned from a real FlatCAM-Plus run — and reports, per artefact,
where the two disagree. It measures nothing, so it needs no FlatCAM environment.

Its rules: `status` compares exactly; `polygons` is reported and never failed, because FlatCAM flattens
every arc in Shapely while this tree keeps arcs native and the two therefore count different things; an
artefact measured on only one side is listed as not comparable and never as agreement.

`contour_area_mm2` is the one field where the comparison has to be read with care, and the reason is in
the section above: the field is the **sum of absolute contour areas**, so a boundary with a hole inside
it contributes twice, while FlatCAM reports the material area. Where the two metrics coincide the areas
agree to about `1e-4`; where a layer is drawn as a closed ring the tree's number is a different
quantity and the comparison is not meaningful. The script uses a relative `1e-3`, which is loose enough
to absorb arc flattening (observed `<= 9e-5`) and tight enough to still fail every genuine geometric
divergence in the corpus (`>= 2.2e-3`), but a divergence it reports on an `outline` should be read as
"these are different quantities" until a hole-aware material area exists (D2 / G0.2).

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

### The measurer's own lock file

`scripts/parity/measure/Cargo.lock` is committed, and `run.sh` builds the measurer with `--locked`: the
lock file is what decides which versions the corpus is measured with, so resolving a newer one silently
would change the measurement without changing the corpus. The consequence is deliberate: when a
dependency of the workspace moves (a `serde_json` bump, or a new dev-dependency in `cam-geom`), the
build **fails with a lock error before measuring anything** instead of quietly resolving. The remedy is
to regenerate the measurer's lock file and commit the diff:

```bash
cargo update --manifest-path scripts/parity/measure/Cargo.toml
```

## What the corpus measures today (re-pinned 2026-11-16, task 060)

The W2 importers changed what the tree makes of the corpus, so the goldens were re-pinned; E1.7 then
changed it again and they were re-pinned a second time. The shape of the current measurement is worth
stating once, because it is the baseline every later change is compared against — of the **54
artefacts**: 17 are `parsed`, 15 are still `refused`, and **22 are `timeout`**.

**What the E1.7 re-pin changed, and why it was not a regression.** The diff was classified with the
rules below, and nothing that was already `parsed` moved a single number. Five `outline` layers went
`refused` → `parsed` with real geometry (`kicad-2l-simple` 10.26 mm²/8 polygons, `kicad-4l-pours`
28.52 mm²/7, `kicad-large-pours` 35.82 mm²/1, `substitute-altium` 129.78 mm²/1, `substitute-eagle`
21.28 mm²/21), and four layers went `refused` → `timeout` (`kicad-2l-simple` bottom, mask-bottom and
silk-bottom, `substitute-eagle/bottom`). One cause explains both moves: a Gerber layer that used to be
refused whole over a construct it could not build (an arc whose centre cannot be derived, a region
that is not a valid region) now imports the rest, so the reader accepts the layer (`parsed`) or hands
it to a kernel that does not finish (`timeout`). The cost is honest and visible: four more 30 s bounds.

Those 22 are the important line. They are not the layers the reader rejects: they are layers the
reader now *accepts* and hands to the geometry kernel, which does not finish evaluating them inside the
harness' 20 s bound, down from 30 s: at ten the run takes four minutes but loses two `substitute-easyeda`
layers that do terminate between ten and thirty, so twenty is the largest saving that keeps every
measurement. Before those waves the same artefacts were `refused` — cheaply and instantly — so the pin
moved from "the reader says no" to "the kernel does not finish", and `timeout` is the harness recording
that honestly instead of hanging CI. Their measurement is zeroed and the status is compared, so the day
one of them finishes, the divergence says so in a single line.

Two consequences worth keeping in view:

- **A `timeout` in a golden is a performance debt, not a pass.** It is tracked here rather than hidden:
  the corpus cannot measure those layers' geometry until the kernel terminates on them.
- **The measurer is compiled by `just check`.** It lives outside the workspace (navigation above), so
  `cargo clippy`/`cargo test --workspace` never touched it; a tolerance type change (`f64` →
  `TolerancePolicy`) compiled fine locally and failed only in the CI `parity` job. The last gate in
  `scripts/check.sh` now builds `scripts/parity/measure/Cargo.toml` so that breakage is found on the
  machine that causes it.

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

`error` is the measurer itself failing — a panic, an allocation failure — and it is deliberately not
`refused`: `run.sh` stops on it before comparing or re-pinning (see `guard_errors.py`), and a golden may
never hold it. A crash is not a measurement, so there is nothing for it to match.

`skipped` is different: it is a deliberate "not checked now" record. An artefact pinned `skipped` is
carried through without being measured or compared, so CI and local hooks do not spend time on it. It
records a budget-closure or deferral decision, not a measurement. Changing one back to `timeout` or
`parsed` is the moment it is checked again.

The board set is fixed too. Both `compare.py` and the workspace test enumerate the six slugs E0.5 names
and fail if the corpus holds a different set, so a board directory that disappears is a divergence
instead of a quietly smaller comparison.

`run.sh --update` re-pins all of it. It is never part of a gate: CI runs `run.sh` and fails when the
goldens and the tree disagree.
