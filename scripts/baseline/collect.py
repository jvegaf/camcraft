#!/usr/bin/env python3
"""Measure the corpus with FlatCAM-Plus into one document, bounded per artefact (E0.6).

`flatcam_measure.py` knows how to measure one artefact and one operation over it; this script is the
other half, exactly as `scripts/parity/collect.py` is the other half of the parity measurer. It drives
the measurer once per artefact and once per operation, so that a run which does not finish is bounded by
`timeout(1)` and recorded as `status: "timeout"` instead of hanging the job. A function call cannot be
interrupted from inside the process; a process can.

The document this prints is what `run.sh` compares against the goldens and, with `--update`, what it
splits into one JSON per board:

    {
      "environment": {"flatcam": "1.3.6", "python": "3.14.4", "shapely": "2.2.0", "platform": "offscreen"},
      "timeout_seconds": 300,
      "operation_parameters": {"isolation": {...params...}, "drill": {...params...}},
      "boards": {
        "<slug>": {
          "artefacts": {
            "top":      {"status": "parsed", "polygons": 165, "contour_area_mm2": 434.09, "vertices": 65353,
                         "seconds": 0.57, "peak_rss_kb": 682804},
            "drill":    {"status": "parsed", "tools": 3, "features": 88, "seconds": 0.40, "peak_rss_kb": 665660}
          },
          "operations": {
            "isolation": {"status": "measured", "moves": 313, "cut_moves": 156, "travel_moves": 157,
                          "path_mm": 2831.93, "seconds": 1.09},
            "drill":     {"status": "measured", "moves": 338, "cut_moves": 165, "travel_moves": 173,
                          "path_mm": 3935.97, "seconds": 0.40}
          }
        }
      }
    }

`status` is `parsed`/`refused`/`timeout`/`error` for an import and `measured`/`refused`/`timeout`/`error`
for an operation. A child that exits non-zero is `error` — a crash, not a refusal — and this script exits
non-zero on it, so `run.sh` refuses to re-pin a crash as a golden.

`seconds` is the operation's own wall time, measured inside the child; the cost of booting FlatCAM is
reported separately and never added to it, because it is the price of the instrument and not of the
measured work.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

# What an artefact gets before it is recorded as `timeout`. FlatCAM reads the whole corpus in seconds,
# so this bound is generous on purpose: it is there to stop a hang, not to grade a machine. The bound is
# recorded in every golden, so changing it is a change to review, never a silent retune.
TIMEOUT_SECONDS = 300

# The roles the corpus is made of, in the order a board is read (the same list `scripts/parity/` uses).
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

# The operations E0.6 measures, per board, and the parameters they are measured with. A parameter here
# changes what the golden means, so all of it is recorded in the golden header and compared exactly.
ISOLATION = {"dia": 0.2, "passes": 2, "overlap": 15.0, "z_cut": -0.1, "z_move": 2.0, "feedrate": 100.0}
DRILL = {"drill_z": -1.8, "travel_z": 2.0, "feedrate_z": 60.0, "toolchange_z": 30.0}

# Which artefact each operation runs on. Isolation is the copper side of the board; the drill file is
# the Excellon table. Both are the artefacts camcraft's own operations read, which is what makes the
# baseline comparable at all.
OPERATION_SOURCES = {"isolation": "top", "drill": "drill"}


def child_environment(scratch: Path) -> dict:
    """The environment every child gets: offscreen, and a configuration of its own.

    FlatCAM reads and writes its options under `$HOME` and `$XDG_CONFIG_HOME`, so without this the
    machine of whoever runs the measurement would change what the corpus measures. An empty directory
    means every run sees FlatCAM's factory options, which is one less variance to explain in a diff.
    """
    return {
        "PATH": os.environ.get("PATH", ""),
        "QT_QPA_PLATFORM": "offscreen",
        "HOME": str(scratch),
        "XDG_CONFIG_HOME": str(scratch / "config"),
        "XDG_DATA_HOME": str(scratch / "data"),
        "XDG_CACHE_HOME": str(scratch / "cache"),
        "TMPDIR": str(scratch / "tmp"),
        "PYTHONDONTWRITEBYTECODE": "1",
    }


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


def empty_import(status: str, drill: bool) -> dict:
    if drill:
        return {"status": status, "tools": 0, "features": 0}
    return {"status": status, "polygons": 0, "contour_area_mm2": 0.0, "vertices": 0}


def empty_operation(status: str) -> dict:
    return {"status": status, "moves": 0, "cut_moves": 0, "travel_moves": 0, "path_mm": 0.0}


def last_object(text: str) -> dict | None:
    """The last line of stdout that is a JSON object.

    FlatCAM logs to stderr, so stdout normally holds exactly one object; reading the last parsable line
    keeps a plugin that prints a banner on stdout from turning a measurement into a parse error.
    """
    for line in reversed(text.splitlines()):
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            return json.loads(line)
        except json.JSONDecodeError:
            continue
    return None


def measure(
    python: Path, script: Path, artefact: Path, environment: dict, operation: str = ""
) -> tuple[dict, str, float | None]:
    """Run the measurer once: its measurement, the reason when it measured nothing, and its boot cost."""
    command = [str(python), str(script), str(artefact)]
    drill = artefact.suffix == ".drl"
    if operation:
        command += ["--operation", operation]
        for key, value in (ISOLATION if operation == "isolation" else DRILL).items():
            command += ["--" + key.replace("_", "-"), str(value)]

    def nothing(status: str) -> dict:
        return empty_operation(status) if operation else empty_import(status, drill)

    try:
        completed = subprocess.run(
            command, capture_output=True, text=True, timeout=TIMEOUT_SECONDS, env=environment
        )
    except subprocess.TimeoutExpired:
        return nothing("timeout"), f"did not finish within {TIMEOUT_SECONDS}s", None

    if completed.returncode != 0:
        # A non-zero exit is the measurer failing — a crash, a killed process, a missing dependency —
        # and not FlatCAM refusing a file. It keeps its own status so the comparison fails instead of
        # quietly re-pinning a failure as if it were a measurement.
        tail = completed.stderr.strip().splitlines()
        reason = tail[-1] if tail else f"exit {completed.returncode}"
        return nothing("error"), f"measurer exited {completed.returncode}: {reason}", None

    measured = last_object(completed.stdout)
    if measured is None:
        return nothing("error"), "the measurer printed no measurement", None
    boot = measured.pop("boot_seconds", None)
    status = measured.get("status")
    return measured, "" if status in ("parsed", "measured") else "refused by FlatCAM", boot


def document(corpus: Path, python: Path, script: Path, boards: list[str]) -> tuple[dict, int, list[float]]:
    scratch = Path(tempfile.mkdtemp(prefix="flatcam-baseline-"))
    environment = child_environment(scratch)
    for name in ("config", "data", "cache", "tmp"):
        (scratch / name).mkdir(parents=True, exist_ok=True)

    failures = 0
    boots: list[float] = []
    measured_environment: dict = {}
    result: dict = {}
    try:
        for board in sorted(path for path in corpus.iterdir() if path.is_dir()):
            if boards and board.name not in boards:
                continue
            artefacts = artefacts_of(board)
            measured_artefacts = {}
            for role, artefact in artefacts.items():
                value, reason, boot = measure(python, script, artefact, environment)
                # The environment belongs to the header, once, not to sixty-six records: the pop runs
                # before the `or`, or a populated header would leave the field in every record.
                reported = value.pop("environment", None)
                measured_environment = measured_environment or reported or {}
                measured_artefacts[role] = value
                if boot is not None:
                    boots.append(boot)
                if value.get("status") == "error":
                    failures += 1
                if reason:
                    print(f"  {board.name}/{artefact.name}: {value['status']} — {reason}", file=sys.stderr)

            measured_operations = {}
            for operation, role in OPERATION_SOURCES.items():
                source = artefacts.get(role)
                if source is None:
                    continue
                value, reason, boot = measure(python, script, source, environment, operation)
                value.pop("environment", None)
                measured_operations[operation] = value
                if boot is not None:
                    boots.append(boot)
                if value.get("status") == "error":
                    failures += 1
                if reason:
                    print(f"  {board.name}/{operation}: {value['status']} — {reason}", file=sys.stderr)

            result[board.name] = {"artefacts": measured_artefacts, "operations": measured_operations}
    finally:
        shutil.rmtree(scratch, ignore_errors=True)

    return (
        {
            "environment": measured_environment,
            "timeout_seconds": TIMEOUT_SECONDS,
            "operation_parameters": {"isolation": ISOLATION, "drill": DRILL},
            "boards": result,
        },
        failures,
        boots,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Measure the corpus with FlatCAM-Plus.")
    parser.add_argument("corpus", type=Path)
    parser.add_argument("python", type=Path, help="FlatCAM's own interpreter")
    parser.add_argument("boards", nargs="*", help="narrow the run to some boards (development only)")
    # A run costs ten minutes, so it can keep its document: a re-pin that fails on a formatting bug
    # should not have to measure the corpus again to prove it.
    parser.add_argument("--out", type=Path, default=None, help="also write the document here")
    arguments = parser.parse_args()
    corpus, python, boards, out = arguments.corpus, arguments.python, arguments.boards, arguments.out
    script = Path(__file__).with_name("flatcam_measure.py")
    if not corpus.is_dir():
        print(f"ERROR: {corpus} is not a corpus directory", file=sys.stderr)
        return 2
    if not os.access(python, os.X_OK):
        print(f"ERROR: {python} is not executable; point FLATCAM_PYTHON at FlatCAM's interpreter", file=sys.stderr)
        return 2
    requested: list[str] = []
    corpus_resolved = corpus.resolve()
    for board in boards:
        candidate = (corpus / board).resolve()
        if candidate.parent != corpus_resolved or not candidate.is_dir():
            print(f"ERROR: {board} is not a board of {corpus}", file=sys.stderr)
            return 2
        requested.append(candidate.name)

    measured, failures, boots = document(corpus, python, script, requested)
    rendered = json.dumps(measured, indent=2, sort_keys=True)
    print(rendered)
    if out is not None:
        out.write_text(rendered + "\n", encoding="utf-8")
        print(f"  kept the measurement in {out}", file=sys.stderr)
    if boots:
        print(
            "  boot: median %.2fs over %d processes (not counted in any measurement)"
            % (sorted(boots)[len(boots) // 2], len(boots)),
            file=sys.stderr,
        )
    if failures:
        print(f"ERROR: {failures} measurement(s) crashed; a crash is not a baseline", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
