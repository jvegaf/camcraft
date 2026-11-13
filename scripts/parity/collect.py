#!/usr/bin/env python3
"""Measure the corpus into one document, with a time bound on every artefact.

The harness has two halves and this is the first: `parity-measure` knows how to read one artefact, and
this script drives it once per artefact so that a layer that does not terminate is bounded by
`timeout(1)` and recorded as `status: "timeout"` instead of hanging CI. A function call cannot be
interrupted from inside the process; a process can.

The document this prints is what `run.sh` compares against the goldens and, with `--update`, what it
splits into one `parity.json` per board (a board's golden is stored next to the board, so a diff over
one board is a diff over one file):

    {
      "flatten_tolerance_mm": 0.001,
      "timeout_seconds": 30,
      "boards": {
        "<slug>": {
          "top":    {"status": "refused", "polygons": 0, "contour_area_mm2": 0.0, "points": 0},
          "drill":  {"status": "parsed", "tools": 13, "features": 251}
        }
      }
    }

`status` is one of `parsed`, `refused`, `timeout`, `error`. Everything else is a number the comparison checks;
the parser's own refusal message is not stored, because a golden should move when the geometry moves,
not when a diagnostic is reworded. `collect.py` prints the reasons to stderr so a person can read them
without them becoming part of the fixture.

Keys are sorted and floats are written with Python's shortest round-trip repr, so two runs that measure
the same thing produce the same bytes.
"""

import json
import os
import subprocess
import sys
from pathlib import Path

FLATTEN_TOLERANCE_MM = 0.001
TIMEOUT_SECONDS = 30

ROLES = (
    "top",
    "bottom",
    "mask-top",
    "mask-bottom",
    "silk-top",
    "silk-bottom",
    "outline",
    "drill",
)


def artefacts_of(board: Path) -> dict[str, Path]:
    """The measured artefacts of one board, keyed by role, in role order."""
    found = {}
    for role in ROLES:
        for suffix in (".gbr", ".drl"):
            candidate = board / f"{role}{suffix}"
            if candidate.is_file():
                found[role] = candidate
                break
    for extra in sorted(board.glob("inner*.gbr")):
        found[extra.stem] = extra
    return found


def measure(binary: Path, artefact: Path) -> tuple[dict, str]:
    """Run the measurer on one artefact: its JSON object, and the reason when it measured nothing."""
    command = [str(binary), str(artefact)]
    try:
        completed = subprocess.run(
            command, capture_output=True, text=True, timeout=TIMEOUT_SECONDS
        )
    except subprocess.TimeoutExpired:
        kind = "tools" if artefact.suffix == ".drl" else "polygons"
        zero = {"tools": 0, "features": 0} if kind == "tools" else {
            "polygons": 0,
            "contour_area_mm2": 0.0,
            "points": 0,
        }
        return {"status": "timeout", **zero}, f"did not finish within {TIMEOUT_SECONDS}s"

    if completed.returncode != 0:
        # A non-zero exit that is not the timeout is the measurer *failing*, not the reader refusing:
        # a panic, an allocation failure, a killed process. Reporting it as `refused` would make it
        # indistinguishable from a parser refusal — and for an artefact already pinned as refused that
        # reads as "matches the goldens" while the measurement never happened. It gets its own status,
        # so the comparison fails and `--update` cannot quietly re-pin a crash.
        message = completed.stderr.strip().splitlines()
        reason = message[-1] if message else f"exit {completed.returncode}"
        detail = f"measurer exited {completed.returncode}: {reason}"
        kind = "tools" if artefact.suffix == ".drl" else "polygons"
        zero = {"tools": 0, "features": 0} if kind == "tools" else {
            "polygons": 0,
            "contour_area_mm2": 0.0,
            "points": 0,
        }
        return {"status": "error", **zero}, detail

    measured = json.loads(completed.stdout)
    reason = "" if measured["status"] == "parsed" else "refused by the reader"
    return measured, reason


def document(corpus: Path, binary: Path) -> dict:
    boards = {}
    for board in sorted(path for path in corpus.iterdir() if path.is_dir()):
        measured = {}
        for role, artefact in artefacts_of(board).items():
            value, reason = measure(binary, artefact)
            measured[role] = value
            if reason:
                print(f"  {board.name}/{artefact.name}: {value['status']} — {reason}", file=sys.stderr)
        boards[board.name] = measured
    return {
        "flatten_tolerance_mm": FLATTEN_TOLERANCE_MM,
        "timeout_seconds": TIMEOUT_SECONDS,
        "boards": boards,
    }


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: collect.py <corpus-root> <parity-measure>", file=sys.stderr)
        return 2
    corpus, binary = Path(sys.argv[1]), Path(sys.argv[2])
    if not corpus.is_dir():
        print(f"ERROR: {corpus} is not a corpus directory", file=sys.stderr)
        return 2
    if not os.access(binary, os.X_OK):
        print(f"ERROR: {binary} is not executable; build it first", file=sys.stderr)
        return 2

    measured = document(corpus, binary)
    print(json.dumps(measured, indent=2, sort_keys=True))
    parsed = sum(
        1
        for board in measured["boards"].values()
        for value in board.values()
        if value["status"] == "parsed"
    )
    total = sum(len(board) for board in measured["boards"].values())
    print(f"{parsed} parsed, {total - parsed} not parsed, of {total} artefacts", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
