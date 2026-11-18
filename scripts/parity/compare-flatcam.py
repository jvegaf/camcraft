#!/usr/bin/env python3
"""Compare this tree's parity goldens against the FlatCAM oracle, artefact by artefact.

This is the half of E1.4 that `compare.py` deliberately does not do. `compare.py` answers "does the tree
still measure what it measured before?"; this script answers "does the tree measure what FlatCAM
measures?", which is what the roadmap's exit criterion asks for.

Two files are read and nothing is measured here:

  * `fixtures/corpus/<slug>/parity.json` — what this tree produces (the goldens, re-pinned by
    `scripts/parity/run.sh --update`);
  * `fixtures/baseline/<slug>.json` — what FlatCAM produces (task 059, pinned by
    `scripts/baseline/pin.py` from a real FlatCAM-Plus run).

The rule, stated once here so no caller has to remember it:

  * `status` compares **exactly**: a layer one side reads and the other does not is the most important
    divergence there is, and it is never absorbed by a tolerance;
  * `contour_area_mm2` compares with a **relative** tolerance (`|a - b| <= tol * max(|a|, |b|)`, and two
    zeroes are equal), only when both sides are `parsed`. Relative, not absolute, for the reason
    `scripts/baseline/compare.py` already gives: a whole layer's area is a sum of hundreds of terms, so
    an absolute epsilon would be a different rule for a small board and a large one;
  * `polygons` is **reported, never failed**. FlatCAM flattens every arc in Shapely while this tree keeps
    arcs native, so the two count different things — the same reason `compare.py` never compares
    FlatCAM's `vertices` against our `points`. An area is the comparable number;
  * an artefact measured on only one side is **not comparable**. It is listed as such and is never
    counted as agreement.

The tolerance is 1e-3 relative, and the number is anchored in what the corpus actually shows rather than
picked for comfort: the drift that arc flattening produces on these layers is at most 9.0e-5, while every
genuine geometric divergence in this corpus is at least 2.2e-3. A tighter rule fails layers that are
right; a looser one passes layers that are wrong.
"""

import json
import sys
from pathlib import Path

RELATIVE_TOLERANCE = 1e-3

# Per-role overrides live here, in one place, so raising one for one layer is a reviewed diff line and
# never a quiet widening that covers the whole corpus. Empty today: no layer has earned an exemption.
TOLERANCE_BY_ROLE: dict[str, float] = {}

# The corpus is fixed: E0.5 names six boards. A board directory that disappears has to be argued with in
# a diff, exactly as `compare.py` argues it, not silently shrink the comparison.
EXPECTED_BOARDS = (
    "kicad-2l-simple",
    "kicad-4l-pours",
    "kicad-large-pours",
    "substitute-altium",
    "substitute-eagle",
    "substitute-easyeda",
)


def tolerance(role: str) -> float:
    return TOLERANCE_BY_ROLE.get(role, RELATIVE_TOLERANCE)


def area_agrees(ours: float, theirs: float, tol: float) -> bool:
    if ours == 0.0 and theirs == 0.0:
        return True
    return abs(ours - theirs) <= tol * max(abs(ours), abs(theirs))


def load(path: Path) -> dict:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def main() -> int:
    root = Path(__file__).resolve().parents[2]
    corpus = root / "fixtures" / "corpus"
    baseline = root / "fixtures" / "baseline"

    problems: list[str] = []
    agreements = 0
    incomparable = 0
    compared = 0

    print(f"{'artefact':32} {'flatcam':9} {'tree':9} {'pol f/t':12} {'area flatcam':>13} {'area tree':>13}  verdict")

    for slug in EXPECTED_BOARDS:
        golden_path = corpus / slug / "parity.json"
        oracle_path = baseline / f"{slug}.json"
        for path in (golden_path, oracle_path):
            if not path.exists():
                problems.append(f"{slug}: missing {path.relative_to(root)}")
        if not (golden_path.exists() and oracle_path.exists()):
            continue

        ours = load(golden_path).get("artefacts", {})
        theirs = load(oracle_path).get("artefacts", {})

        for role in sorted(set(ours) | set(theirs)):
            where = f"{slug}/{role}"
            mine = ours.get(role)
            other = theirs.get(role)
            if mine is None:
                problems.append(f"{where}: FlatCAM measures an artefact this tree does not pin")
                continue
            if other is None:
                problems.append(f"{where}: the tree pins an artefact FlatCAM does not measure")
                continue

            my_status = mine.get("status")
            their_status = other.get("status")
            my_polys = mine.get("polygons", 0)
            their_polys = other.get("polygons", 0)
            my_area = float(mine.get("contour_area_mm2", 0.0))
            their_area = float(other.get("contour_area_mm2", 0.0))

            if my_status != their_status:
                verdict = "STATUS"
                problems.append(
                    f"{where}: status is {my_status} and FlatCAM's is {their_status}"
                )
            elif my_status != "parsed":
                # Both sides agree the layer does not yield geometry; there is no number to compare.
                verdict = "agree(none)"
                agreements += 1
            else:
                compared += 1
                tol = tolerance(role)
                if area_agrees(my_area, their_area, tol):
                    verdict = f"ok(<={tol:g})"
                    agreements += 1
                else:
                    relative = abs(my_area - their_area) / max(abs(my_area), abs(their_area))
                    verdict = f"DIVERGE({relative:.3g})"
                    problems.append(
                        f"{where}: area is {my_area:.4f} and FlatCAM's is {their_area:.4f} "
                        f"(relative {relative:.3g}, tolerance {tol:g})"
                    )
                if my_polys != their_polys:
                    verdict += f" pol{my_polys}/{their_polys}"

            if my_status != their_status and "parsed" not in (my_status, their_status):
                incomparable += 1

            print(
                f"{slug + '/' + role:32} {str(their_status):9} {str(my_status):9} "
                f"{f'{their_polys}/{my_polys}':12} {their_area:13.4f} {my_area:13.4f}  {verdict}"
            )

    total = agreements + len(problems)
    print(
        f"\n{agreements} agree, {len(problems)} diverge, {compared} area comparisons, "
        f"{total} artefact(s) judged ({incomparable} with mismatched non-parsed status)"
    )

    if problems:
        print(f"\n{len(problems)} divergence(s) from the FlatCAM oracle:", file=sys.stderr)
        for problem in problems:
            print(f"  {problem}", file=sys.stderr)
        print(
            "\nA divergence here is not a stale golden: it means this tree and the reference "
            "implementation disagree about the layer. Decide which one is right before touching "
            "any number.",
            file=sys.stderr,
        )
        return 1

    print("the tree matches the FlatCAM oracle on every comparable artefact")
    return 0


if __name__ == "__main__":
    sys.exit(main())
