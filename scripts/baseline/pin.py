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
import os
import sys
import tempfile
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

    # A run that could not measure is never a baseline. `error` and `timeout` are the two statuses that
    # mean "the harness did not see this artefact", and pinning one would replace a real measurement with
    # a record that says nothing — the comparison would then treat the gap as the truth. Refusing here,
    # before anything is written, keeps the pinned set either whole or untouched.
    unpinnable = sorted(
        f"{slug}/{name}"
        for slug, board in boards.items()
        for family in ("artefacts", "operations")
        for name, value in board.get(family, {}).items()
        if value.get("status") in ("error", "timeout")
    )
    if unpinnable:
        print("ERROR: these were not measured, and a baseline is only what was measured:", file=sys.stderr)
        for name in unpinnable:
            print(f"  {name}", file=sys.stderr)
        return 1

    baseline.mkdir(parents=True, exist_ok=True)
    targets = {}
    for slug, board in sorted(boards.items()):
        golden = {**header, **board}
        targets[baseline / f"{slug}.json"] = json.dumps(golden, indent=2, sort_keys=True) + "\n"
    replace_all(targets)
    for path in targets:
        print(f"  pinned {path}")
    return 0


def replace_all(targets: dict[Path, str]) -> None:
    """Stage every golden, rename them over the old ones, and put the old bytes back on failure.

    `Path.write_text` truncates in place: a full disk, a permission that changed or an interrupt would
    leave one golden empty and the others already rewritten. Staging beside the target and renaming makes
    each file either its old content or its new one, and holding the old bytes lets a failure midway
    through the renames put the set back instead of leaving a baseline that is neither version.
    """
    originals = {path: (path.read_bytes() if path.exists() else None) for path in targets}
    staged: list[tuple[Path, str]] = []
    installed: list[Path] = []
    try:
        for path, text in targets.items():
            handle, temporary = tempfile.mkstemp(dir=str(path.parent), prefix=f".{path.name}.")
            # Registered before anything else can fail: `staged` is the list the `finally` unlinks, so a
            # failure between `mkstemp` and the old append point (a write error, a permission that
            # changed, an interrupt) left the temp file in the baseline directory for good.
            staged.append((path, temporary))
            with os.fdopen(handle, "w", encoding="utf-8") as file:
                file.write(text)
                file.flush()
                os.fsync(file.fileno())
            # `mkstemp` creates 0600, and a golden is a committed file: keep the mode it already had, or
            # a plain 0644 for a new one, rather than making the pinned set owner-only.
            mode = path.stat().st_mode & 0o777 if path.exists() else 0o644
            os.chmod(temporary, mode)
        for path, temporary in staged:
            os.replace(temporary, path)
            installed.append(path)
    except BaseException:
        for path in installed:
            content = originals[path]
            if content is None:
                # This run created it; putting the set back means it is not there either.
                path.unlink(missing_ok=True)
            else:
                path.write_bytes(content)
        raise
    finally:
        for _, temporary in staged:
            try:
                os.unlink(temporary)
            except OSError:
                pass


if __name__ == "__main__":
    sys.exit(main())
