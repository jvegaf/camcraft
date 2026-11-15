#!/usr/bin/env python3
"""Compare a fresh FlatCAM measurement against the committed baseline (E0.6).

The comparison is deliberately narrow, and what it leaves out is the interesting part:

* **Compared exactly**: every `status`, every count (`polygons`, `vertices`, `tools`, `features`, and the
  move counts of each operation), the tolerances and the operation parameters the numbers were measured
  with, and the `contour_area_mm2` of every layer within E0.4's relative `1e-9` — because an area is a sum
  of hundreds of terms, and an absolute epsilon would be a different rule for a small board and a large one.
* **Compared as identity, not as a number**: the FlatCAM, Python and Shapely versions the golden was
  measured with. A golden from another FlatCAM is not a baseline for this one, so the check fails and the
  remedy is to review the diff and re-pin, never to compare across it.
* **Never compared**: `seconds` and `peak_rss_kb`. They are recorded because the roadmap asks for a
  measured baseline and because a reviewer should see them move, but they are a property of the machine
  and its load, exactly like the `bench` comparison of E0.7. This script prints them next to the golden so
  a person can read the drift; it never turns that drift into a failure.

Also never compared, and for a reason worth writing down: FlatCAM's `vertices` against camcraft's `points`.
FlatCAM flattens every arc in Shapely while the workspace preserves arcs, so the two count different
things (`docs/ROADMAP_CAMCRAFT.md`, "Nota de tolerancia de arcos"). The area is the comparable number.
"""

import json
import math
import sys
from pathlib import Path

RELATIVE_EPSILON = 1e-9

# The corpus is fixed: E0.5 names six boards. A new board is a deliberate change to this list, the
# corpus and the goldens together — never something a directory appearing or vanishing does on its own.
EXPECTED_BOARDS = (
    "kicad-2l-simple",
    "kicad-4l-pours",
    "kicad-large-pours",
    "substitute-altium",
    "substitute-eagle",
    "substitute-easyeda",
)

ARTEFACT_COUNTS = ("polygons", "vertices", "tools", "features")
OPERATION_COUNTS = ("moves", "cut_moves", "travel_moves")
AREA_FIELD = "contour_area_mm2"
PATH_FIELD = "path_mm"
VOLATILE = ("seconds", "peak_rss_kb")


def close(left: float, right: float) -> bool:
    """Relative comparison with the epsilon E0.4 asks for, with two zeroes equal."""
    if left == right:
        return True
    scale = max(abs(left), abs(right))
    return abs(left - right) <= RELATIVE_EPSILON * scale


def compare_counts(where: str, measured: dict, golden: dict, fields, problems: list[str]) -> None:
    for side, value in (("the run", measured), ("the golden", golden)):
        if value.get("status") == "error":
            problems.append(
                f"{where}: {side} records a measurer failure; a crash is not a measurement and cannot be compared"
            )
            return
    if measured.get("status") != golden.get("status"):
        problems.append(
            f"{where}: status is {measured.get('status')!r} in the run and {golden.get('status')!r} in the golden"
        )
    for field in fields:
        if field not in golden and field not in measured:
            continue
        if measured.get(field) != golden.get(field):
            problems.append(
                f"{where}: {field} is {measured.get(field)!r} in the run and {golden.get(field)!r} in the golden"
            )


def compare_float(where: str, field: str, measured: dict, golden: dict, problems: list[str]) -> None:
    if field not in golden and field not in measured:
        return
    fresh, pinned = measured.get(field), golden.get(field)
    if not isinstance(fresh, (int, float)) or not isinstance(pinned, (int, float)):
        problems.append(f"{where}: {field} is not a number ({fresh!r} against {pinned!r})")
    elif not math.isfinite(fresh) or not math.isfinite(pinned):
        problems.append(f"{where}: {field} is not finite ({fresh!r} against {pinned!r})")
    elif not close(float(fresh), float(pinned)):
        problems.append(
            f"{where}: {field} is {fresh!r} in the run and {pinned!r} in the golden, past a relative {RELATIVE_EPSILON:g}"
        )


def timing(where: str, measured: dict, golden: dict, rows: list[tuple]) -> None:
    fresh, pinned = measured.get("seconds"), golden.get("seconds")
    rows.append((where, pinned, fresh))


def main() -> int:
    if len(sys.argv) != 4:
        print("usage: compare.py <corpus-root> <baseline-root> <measured.json>", file=sys.stderr)
        return 2
    corpus, baseline, measured_path = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    measured = json.loads(measured_path.read_text(encoding="utf-8"))
    problems: list[str] = []
    timings: list[tuple] = []
    checked = 0

    boards = measured["boards"]

    # The corpus on disk is the authority for which boards exist; a measurement says nothing about a
    # board it never saw.
    on_disk = sorted(path.name for path in corpus.iterdir() if path.is_dir())
    if on_disk != sorted(EXPECTED_BOARDS):
        problems.append(f"corpus: boards on disk are {on_disk} and the fixed corpus is {sorted(EXPECTED_BOARDS)}")
    if sorted(boards) != on_disk:
        problems.append(f"corpus: measured boards are {sorted(boards)} and the corpus holds {on_disk}")

    for slug in sorted(boards):
        golden_path = baseline / f"{slug}.json"
        if not golden_path.is_file():
            problems.append(f"{slug}: no fixtures/baseline/{slug}.json; run scripts/baseline/run.sh --update")
            continue
        golden = json.loads(golden_path.read_text(encoding="utf-8"))

        for field in ("timeout_seconds",):
            if golden.get(field) != measured.get(field):
                problems.append(
                    f"{slug}: {field} is {measured.get(field)!r} in the run and {golden.get(field)!r} in the golden"
                )
        for field in ("environment", "operation_parameters"):
            if golden.get(field) != measured.get(field):
                problems.append(
                    f"{slug}: {field} is {measured.get(field)!r} in the run and {golden.get(field)!r} in the golden;"
                    " re-measuring with another version or other parameters is a re-pin, not a comparison"
                )

        fresh = boards[slug]
        for family, fields, floats in (
            ("artefacts", ARTEFACT_COUNTS, (AREA_FIELD,)),
            ("operations", OPERATION_COUNTS, (PATH_FIELD,)),
        ):
            pinned_family = golden.get(family, {})
            fresh_family = fresh.get(family, {})
            for name in sorted(set(pinned_family) | set(fresh_family)):
                where = f"{slug}/{name}"
                if name not in fresh_family:
                    problems.append(f"{where}: the golden pins a {family[:-1]} this run did not measure")
                    continue
                if name not in pinned_family:
                    problems.append(f"{where}: measured a {family[:-1]} the golden does not pin")
                    continue
                compare_counts(where, fresh_family[name], pinned_family[name], fields, problems)
                for field in floats:
                    compare_float(where, field, fresh_family[name], pinned_family[name], problems)
                timing(where, fresh_family[name], pinned_family[name], timings)
                checked += 1

    print("  seconds   golden -> run")
    for where, pinned, fresh in timings:
        if not isinstance(pinned, (int, float)) or not isinstance(fresh, (int, float)):
            print(f"  {where}: no timing recorded on one side")
            continue
        ratio = (fresh / pinned) if pinned else float("inf")
        print(f"  {where:<34} {pinned:8.2f} -> {fresh:8.2f}  ({ratio:4.2f}x)")

    for problem in problems:
        print(f"DIVERGENCE: {problem}", file=sys.stderr)
    print(f"{checked} measurements compared, {len(problems)} divergence(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
