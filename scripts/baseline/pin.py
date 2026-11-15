#!/usr/bin/env python3
"""Pin a fresh FlatCAM measurement as the committed baseline, one file per board (E0.6).

Kept out of `run.sh` on purpose: the re-pin is the one part of this harness that writes, and write logic
buried in a shell heredoc is logic nobody can run twice or test. `run.sh --update` is the only caller.

The header (`environment`, `timeout_seconds`, `operations`) is copied into every board's file, so a board
is a self-contained record of what it means: reading one golden tells you which FlatCAM measured it, with
which bound, and with which operation parameters. One file per board also makes a re-pin a diff over the
board it changed rather than over the whole baseline.
"""

import json
import sys
from pathlib import Path


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: pin.py <measured.json> <baseline-root>", file=sys.stderr)
        return 2
    measured_path, baseline = Path(sys.argv[1]), Path(sys.argv[2])
    measured = json.loads(measured_path.read_text(encoding="utf-8"))
    header = {
        "environment": measured["environment"],
        "timeout_seconds": measured["timeout_seconds"],
        "operation_parameters": measured["operation_parameters"],
    }
    boards = measured["boards"]
    if not boards:
        print("ERROR: the measurement holds no board; nothing to pin", file=sys.stderr)
        return 1

    baseline.mkdir(parents=True, exist_ok=True)
    for slug, board in sorted(boards.items()):
        golden = {**header, **board}
        path = baseline / f"{slug}.json"
        path.write_text(json.dumps(golden, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"  pinned {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
