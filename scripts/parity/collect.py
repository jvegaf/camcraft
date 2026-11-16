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

import concurrent.futures
import json
import os
import subprocess
import sys
from pathlib import Path

FLATTEN_TOLERANCE_MM = 0.001

# The bound an artefact gets before it is recorded as `timeout` instead of hanging the job.
#
# Thirty seconds, and the number is now known to be about stability rather than about speed: two
# layers of `substitute-easyeda` (mask-top, mask-bottom) terminate in roughly twenty seconds, so a
# twenty-second bound flips them between `parsed` and `timeout` from run to run — the goldens then
# disagree with the tree on a measurement nobody changed. Ten seconds is faster still and loses those
# two outright. The eighteen that never terminate dominate the run either way; the honest saving is
# not to lower the bound but to skip the measurement when a push cannot move it (see the `parity` job
# in `.github/workflows/ci.yml`).
#
# The bound is recorded in every golden, so changing it is a change to review, never a silent retune.
# `CAMCRAFT_PARITY_TIMEOUT_SECONDS` overrides it for local experiments; comparisons against committed
# goldens will only match when the recorded bound matches.
def timeout_seconds() -> int:
    override = os.environ.get("CAMCRAFT_PARITY_TIMEOUT_SECONDS")
    if override:
        try:
            value = int(override)
            if value > 0:
                return value
        except ValueError:
            pass
    return 30


TIMEOUT_SECONDS = timeout_seconds()

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


def worker_count() -> int:
    """Bounded parallelism for the per-artefact measurer processes.

    Each artefact runs in its own process with a fixed timeout, so a handful of workers cuts the wall
    time of a corpus run substantially without changing what is measured. The cap keeps a local push
    from saturating the machine. `CAMCRAFT_PARITY_JOBS` overrides the default when set to a positive
    integer (capped at 8).
    """
    jobs = min(os.cpu_count() or 1, 4)
    override = os.environ.get("CAMCRAFT_PARITY_JOBS")
    if override:
        try:
            value = int(override)
            if value > 0:
                jobs = value
        except ValueError:
            pass
    return max(1, min(jobs, 8))


def assumed_timeout(artefact: Path) -> dict:
    kind = "tools" if artefact.suffix == ".drl" else "polygons"
    zero = {"tools": 0, "features": 0} if kind == "tools" else {
        "polygons": 0,
        "contour_area_mm2": 0.0,
        "points": 0,
    }
    return {"status": "timeout", **zero}


def document(corpus: Path, binary: Path, assume_known_timeouts: bool = False) -> dict:
    workers = worker_count()
    boards = {}
    for board in sorted(path for path in corpus.iterdir() if path.is_dir()):
        items = list(artefacts_of(board).items())
        skipped = known_timeouts(board) if assume_known_timeouts else set()
        to_measure = [(role, artefact) for role, artefact in items if role not in skipped]
        measured = {}
        reasons = {}

        for role, artefact in items:
            if role in skipped:
                measured[role] = assumed_timeout(artefact)
                reasons[role] = (artefact, "assumed timeout (pinned in golden; skipped)")

        if to_measure:
            with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as executor:
                futures = {
                    executor.submit(measure, binary, artefact): (role, artefact)
                    for role, artefact in to_measure
                }
                for future in concurrent.futures.as_completed(futures):
                    role, artefact = futures[future]
                    value, reason = future.result()
                    measured[role] = value
                    reasons[role] = (artefact, reason)

        ordered = {}
        for role, artefact in items:
            ordered[role] = measured[role]
            artefact_path, reason = reasons[role]
            if reason:
                print(
                    f"  {board.name}/{artefact_path.name}: {ordered[role]['status']} — {reason}",
                    file=sys.stderr,
                )
        boards[board.name] = ordered
    return {
        "flatten_tolerance_mm": FLATTEN_TOLERANCE_MM,
        "timeout_seconds": TIMEOUT_SECONDS,
        "boards": boards,
    }


def known_timeouts(board: Path) -> set[str]:
    """Artefacts whose committed golden is already a timeout.

    Skipping them is an opt-in local speedup: it cannot discover that a layer now finishes, so it must
    never be used when pinning goldens.
    """
    golden_path = board / "parity.json"
    if not golden_path.is_file():
        return set()
    try:
        golden = json.loads(golden_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return set()
    artefacts = golden.get("artefacts", {})
    return {role for role, value in artefacts.items() if value.get("status") == "timeout"}


def main() -> int:
    args = [a for a in sys.argv[1:] if a != "--"]
    assume_known = False
    positional = []
    for arg in args:
        if arg == "--assume-known-timeouts":
            assume_known = True
        elif arg.startswith("--"):
            print(f"usage: collect.py [--assume-known-timeouts] <corpus-root> <parity-measure>", file=sys.stderr)
            return 2
        else:
            positional.append(arg)

    if len(positional) != 2:
        print("usage: collect.py [--assume-known-timeouts] <corpus-root> <parity-measure>", file=sys.stderr)
        return 2
    corpus, binary = Path(positional[0]), Path(positional[1])
    if not corpus.is_dir():
        print(f"ERROR: {corpus} is not a corpus directory", file=sys.stderr)
        return 2
    if not os.access(binary, os.X_OK):
        print(f"ERROR: {binary} is not executable; build it first", file=sys.stderr)
        return 2

    measured = document(corpus, binary, assume_known_timeouts=assume_known)
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
