#!/usr/bin/env python3
"""Compare a fresh measurement of the corpus against the committed goldens.

The rule is E0.4's, and it is stated once here so no caller has to remember it:

  * counts (`polygons`, `points`, `tools`, `features`) compare exactly;
  * `contour_area_mm2` compares with a **relative** epsilon of 1e-9 (`|a - b| <= 1e-9 * max(|a|, |b|)`,
    and two zeroes are equal), because an area of a whole layer is a sum of hundreds of terms and an
    absolute epsilon would be a different rule for a small board and a large one;
  * `status` and the tolerances recorded in the golden compare exactly.

The boards are enumerated from the corpus on disk and from the goldens, never from the measurement:
a board directory that disappears has to be a divergence, not a smaller comparison, or the corpus could
shrink to nothing while the harness kept reporting that it matches. The six slugs E0.5 names are listed
here so that a deletion has to be argued with in a diff, and the same list is asserted by
`crates/cam-geom/tests/parity.rs`.

A golden's shape is `fixtures/corpus/<slug>/parity.json`:

    {"flatten_tolerance_mm": 0.001, "timeout_seconds": 30, "artefacts": {"<role>": {...}}}

What a divergence means is not this script's business: it exits non-zero, names the board, the role and
the field, and the person reading it decides whether the new number is progress (a layer that now
reads) or a regression (a layer that used to).
"""

import json
import math
import sys
from pathlib import Path

RELATIVE_EPSILON = 1e-9

COUNT_FIELDS = ("polygons", "points", "tools", "features")

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
AREA_FIELD = "contour_area_mm2"


def close(left: float, right: float) -> bool:
    """Relative comparison with the epsilon E0.4 asks for, with two zeroes equal."""
    if left == right:
        return True
    scale = max(abs(left), abs(right))
    return abs(left - right) <= RELATIVE_EPSILON * scale


def compare_value(where: str, measured: dict, golden: dict, problems: list[str]) -> None:
    # A measurer failure is never a measurement. If the tree crashed, or a golden ever recorded a crash,
    # there is nothing to compare and the harness has to say so instead of finding `error` == `error` and
    # calling it a match - which is the whole reason `error` is a status distinct from `refused`.
    for side, value in (("the tree", measured), ("the golden", golden)):
        if value.get("status") == "error":
            problems.append(
                f"{where}: {side} records a measurer failure; a crash is not a measurement and cannot be compared"
            )
            return
    for field in ("status",):
        if measured.get(field) != golden.get(field):
            problems.append(
                f"{where}: {field} is {measured.get(field)!r} in the tree and {golden.get(field)!r} in the golden"
            )
    for field in COUNT_FIELDS:
        if field not in golden and field not in measured:
            continue
        if measured.get(field) != golden.get(field):
            problems.append(
                f"{where}: {field} is {measured.get(field)!r} in the tree and {golden.get(field)!r} in the golden"
            )
    if AREA_FIELD in golden or AREA_FIELD in measured:
        tree = measured.get(AREA_FIELD)
        pinned = golden.get(AREA_FIELD)
        if not isinstance(tree, (int, float)) or not isinstance(pinned, (int, float)):
            problems.append(f"{where}: {AREA_FIELD} is not a number ({tree!r} against {pinned!r})")
        elif not math.isfinite(tree) or not math.isfinite(pinned):
            problems.append(f"{where}: {AREA_FIELD} is not finite ({tree!r} against {pinned!r})")
        elif not close(float(tree), float(pinned)):
            problems.append(
                f"{where}: {AREA_FIELD} is {tree!r} in the tree and {pinned!r} in the golden, "
                f"past a relative {RELATIVE_EPSILON:g}"
            )


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: compare.py <corpus-root> <measured.json>", file=sys.stderr)
        return 2
    corpus, measured_path = Path(sys.argv[1]), Path(sys.argv[2])
    measured = json.loads(measured_path.read_text(encoding="utf-8"))
    problems: list[str] = []
    checked = 0

    boards = measured["boards"]

    # The corpus on disk is the authority for which boards exist; the measurement says nothing about a
    # board it never saw.
    on_disk = sorted(path.name for path in corpus.iterdir() if path.is_dir())
    if on_disk != sorted(EXPECTED_BOARDS):
        problems.append(
            f"corpus: boards on disk are {on_disk} and the fixed corpus is {sorted(EXPECTED_BOARDS)}"
        )
    if sorted(boards) != on_disk:
        problems.append(f"corpus: measured boards are {sorted(boards)} and the corpus holds {on_disk}")

    for slug in sorted(boards):
        board = corpus / slug
        golden_path = board / "parity.json"
        if not golden_path.is_file():
            problems.append(f"{slug}: no parity.json; run scripts/parity/run.sh --update and commit it")
            continue
        golden = json.loads(golden_path.read_text(encoding="utf-8"))

        for field in ("flatten_tolerance_mm", "timeout_seconds"):
            if golden.get(field) != measured.get(field):
                problems.append(
                    f"{slug}: {field} is {measured.get(field)!r} in the tree and {golden.get(field)!r} in the golden"
                )

        artefacts = golden.get("artefacts", {})
        fresh = boards[slug]
        for role in sorted(set(artefacts) | set(fresh)):
            where = f"{slug}/{role}"
            if role not in fresh:
                problems.append(f"{where}: the golden pins an artefact the corpus no longer has")
            elif role not in artefacts:
                problems.append(f"{where}: the corpus has an artefact the golden does not pin")
            else:
                compare_value(where, fresh[role], artefacts[role], problems)
            checked += 1

    for slug in sorted(boards):
        for role, value in sorted(boards[slug].items()):
            print(f"  {slug}/{role}: {value}")

    parsed = sum(
        1 for board in boards.values() for value in board.values() if value["status"] == "parsed"
    )
    total = sum(len(board) for board in boards.values())
    print(f"{parsed} parsed, {total - parsed} not parsed, of {total} artefacts ({checked} compared)")

    if problems:
        print(f"\n{len(problems)} divergence(s) from the committed goldens:", file=sys.stderr)
        for problem in problems:
            print(f"  {problem}", file=sys.stderr)
        print(
            "\nIf the new numbers are what the code should produce, re-pin them with "
            "`scripts/parity/run.sh --update` and review the diff of the goldens.",
            file=sys.stderr,
        )
        return 1
    print("the corpus matches its goldens")
    return 0


if __name__ == "__main__":
    sys.exit(main())
