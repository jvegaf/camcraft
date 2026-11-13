"""Fail loudly when a measurement carries a measurer failure; never let it become a golden."""
import json
import sys

measured = json.load(open(sys.argv[1], encoding="utf-8"))
broken = [
    f"{slug}/{role}"
    for slug, board in measured["boards"].items()
    for role, value in board.items()
    if value["status"] == "error"
]
for where in broken:
    print(f"ERROR: the measurer failed on {where}; a crash is not a measurement", file=sys.stderr)
sys.exit(1 if broken else 0)
