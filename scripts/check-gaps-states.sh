#!/usr/bin/env bash
# Fails when a finding of `docs/GAPS_CAMCRAFT.md` §3 loses its id, its state or its resolvable citation.
#
# The document states its own discipline: every finding carries one of three states (task 110, S3) and
# evidence that resolves from the repository root (110, S4), and nothing breaks when a re-survey drops
# one of them. This gate is that breakage made cheap and mechanical. It reads only §3, from the
# `## 3.` heading to the next `## ` heading at the same level (`### ` phase headings do not close it),
# and it never edits the document it checks (110 task, S9).
#
# It is deliberately not a prose linter: rewriting a finding while keeping its id, one valid state and
# a resolvable path:line citation passes. What it enforces is the verifiable shape of 110 S3/S4, not
# the truth of the finding — that is still a person's job.
#
# Output follows `check-docs.sh`: one defect per line on stderr as `<rule>  <id>  <detail>`, ordered by
# id and then rule, plus a final summary, and exit 1. With no defect it prints
# `gaps states ok: 30 findings` on stdout and exits 0. There is no third verdict: an unreadable file or
# an unreadable §3 is a defect, not a different exit code.
#
# The expected id set is hard-coded below on purpose. Deriving it from the document would make a
# deleted finding pass, which is exactly the loss this gate exists to detect; a sweep that discovers a
# finding has to register it here instead of the gate silently accepting it. The sketch
# `scripts/check-gaps-states.py` this was adapted from is not part of the tree, so the logic lives
# inline (standard library only, like `check-docs.sh`, so CI needs no toolchain).
set -euo pipefail
cd "$(dirname "$0")/.."

python3 - <<'PY'
import re
import sys
from pathlib import Path, PurePosixPath

ROOT = Path.cwd()
DOC_NAME = "docs/GAPS_CAMCRAFT.md"
SECTION_HEADING = "## 3."

# The 30 identifiers task 110 left in the document (S2). Not derived: see the header comment.
EXPECTED = [
    "G0.1", "G0.2", "G0.3",
    "G1.1", "G1.2", "G1.3", "G1.4",
    "G2.1", "G2.2", "G2.3", "G2.4", "G2.5",
    "G3.1", "G3.2", "G3.3", "G3.4", "G3.5", "G3.6", "G3.7",
    "G4.1", "G4.2", "G4.3", "G4.4",
    "G5.1", "G5.2", "G5.3",
    "G6.1", "G6.2", "G6.3", "G6.4",
]
STATES = ("sigue siendo cierto", "ya no aplica", "cambió de forma")

# A finding starts at a bullet, never at a mention of its id somewhere in the prose. The bold header
# runs from that bullet to the `**` that closes it and may span lines; the state marker lives inside.
FINDING = re.compile(r"^- \*\*(G\d+\.\d+) — ", re.M)
SPAN = re.compile(r"`([^`\n]+)`")
# `path:N` or `path:N-M`; S4 only calls the ones with `/` in the path a citation of the repository.
CITE = re.compile(r"^(\S*?):(\d+)(?:-(\d+))?$")
ID = re.compile(r"^G(\d+)\.(\d+)$")

_line_counts = {}


def line_count(path):
    key = str(path)
    if key not in _line_counts:
        try:
            _line_counts[key] = len(path.read_text(encoding="utf-8", errors="replace").splitlines())
        except OSError:
            _line_counts[key] = 0
    return _line_counts[key]


def id_key(name):
    match = ID.match(name)
    return (int(match.group(1)), int(match.group(2))) if match else (-1, -1)


def header_of(text, start):
    close = text.find("**", start + 4)
    return text[start:] if close == -1 else text[start:close]


def check_state(gid, header, defects):
    """"Exactly one of the three states of 110 S3, inside the bold header, and nothing else."""
    markers = list(re.finditer("Estado:", header))
    if len(markers) != 1:
        defects.append((gid, "state-count",
                        f"expected exactly one «Estado:» marker in the bold header, found {len(markers)}"))
        return
    rest = header[markers[0].end():].lstrip()
    if rest.startswith("«"):
        close = rest.find("»", 1)
        if close == -1:
            defects.append((gid, "state-value", f"{rest} has no closing »"))
            return
        value = rest[1:close]
        if value not in STATES:
            defects.append((gid, "state-value", f"«{value}» is not one of the three states of 110 S3"))
        return
    token = re.split(r"[\s.]", rest, maxsplit=1)[0] if rest else ""
    defects.append((gid, "state-value", f"Estado: {token} is not a quoted state"))


def escaped(rel):
    """`None` when `rel` stays inside the checkout, else why it cannot be a repository citation.

    This is checked before joining, not after: `ROOT / "/etc/passwd"` is `/etc/passwd` (an absolute
    path discards the root) and `ROOT / "../x"` walks out of the tree, so either one would cite a
    file the checkout does not contain and still look like a resolved citation.
    """
    path = PurePosixPath(rel)
    if path.is_absolute():
        return "absolute"
    if ".." in path.parts:
        return "parent"
    return None


def check_citations(gid, body, defects):
    """At least one `path:N[-M]` citation that resolves, and every one of them within the file.

    A citation resolves only as a regular file inside the checkout: a directory is not evidence that a
    finding still holds. A bare path is never evidence on its own — it carries no line number, so it
    cannot be the whole citation — but it stays acceptable next to one, because §1's table cites
    crate-relative paths that way on purpose. A path without a line that does not resolve is not a
    defect: the document cites module and historical paths (`tool_geometry/tool_cutting_data/…`,
    `crates/cam-toolpath`) that are not files of this tree.

    A numbered citation that fails keeps its own `citation`/`line` defect and stands in for the bare
    path's: the rule adds no second defect on top of the one that already says why.
    """
    resolved = False
    bare = None
    reported = False
    for raw in SPAN.findall(body):
        span = raw.strip()
        match = CITE.match(span)
        if match and "/" in match.group(1):
            rel = match.group(1)
            escape = escaped(rel)
            if escape == "absolute":
                defects.append((gid, "citation",
                                f"`{span}` is an absolute path, not one from the repository root"))
                reported = True
                continue
            if escape == "parent":
                defects.append((gid, "citation",
                                f"`{span}` leaves the repository root with a «..» component"))
                reported = True
                continue
            path = ROOT / rel
            first = int(match.group(2))
            last = int(match.group(3)) if match.group(3) is not None else None
            if not path.exists():
                defects.append((gid, "citation", f"`{span}` does not resolve from the repository root"))
                reported = True
                continue
            if not path.is_file():
                defects.append((gid, "citation", f"`{span}` resolves to a directory, not a file"))
                reported = True
                continue
            total = line_count(path)
            if first < 1:
                defects.append((gid, "line", f"`{span}` line 0 is out of range (the file has {total} lines)"))
                reported = True
                continue
            if first > total:
                defects.append((gid, "line", f"`{span}` line {first} is past EOF (the file has {total} lines)"))
                reported = True
                continue
            if last is not None:
                if last > total:
                    defects.append((gid, "line",
                                    f"`{span}` line {last} is past EOF (the file has {total} lines)"))
                    reported = True
                    continue
                if first > last:
                    defects.append((gid, "line", f"`{span}` the range is inverted ({first} > {last})"))
                    reported = True
                    continue
            resolved = True
            continue
        if ("/" in span and not any(ch.isspace() for ch in span)
                and escaped(span) is None and (ROOT / span).is_file()):
            bare = span
    if not resolved and not reported:
        defects.append((gid, "citation",
                        f"`{bare}` is a bare path, not a `path:line` citation"
                        if bare is not None else
                        "no inline code span with a path that resolves from the repository root"))


def collect():
    defects = []
    try:
        text = (ROOT / DOC_NAME).read_text(encoding="utf-8")
    except FileNotFoundError:
        return defects + [("§3", "missing", f"{DOC_NAME} does not exist")]
    except (OSError, UnicodeDecodeError) as exc:
        return defects + [("§3", "missing", f"{DOC_NAME} is unreadable ({exc.__class__.__name__})")]

    lines = text.splitlines()
    opening = next((i for i, line in enumerate(lines) if line.startswith(SECTION_HEADING)), None)
    if opening is None:
        return defects + [("§3", "missing", f"{DOC_NAME} has no «{SECTION_HEADING}» section")]
    closing = next((i for i in range(opening + 1, len(lines)) if lines[i].startswith("## ")), len(lines))
    section = "\n".join(lines[opening:closing])

    findings = [(match.group(1), match.start()) for match in FINDING.finditer(section)]
    counts = {}
    for gid, _ in findings:
        counts[gid] = counts.get(gid, 0) + 1

    for gid in counts:
        if gid not in EXPECTED:
            defects.append((gid, "unknown", f"not in the expected set of {len(EXPECTED)} ids"))
        if counts[gid] > 1:
            defects.append((gid, "duplicate",
                            f"{counts[gid]} findings start with «- **{gid} — » in §3 (expected 1)"))
    for gid in EXPECTED:
        if gid not in counts:
            defects.append((gid, "missing", f"no finding starts with «- **{gid} — » in §3"))

    for index, (gid, start) in enumerate(findings):
        end = findings[index + 1][1] if index + 1 < len(findings) else len(section)
        check_state(gid, header_of(section, start), defects)
        check_citations(gid, section[start:end], defects)
    return defects


def main():
    defects = sorted(set(collect()), key=lambda item: (id_key(item[0]), item[1], item[2]))
    if defects:
        for gid, rule, detail in defects:
            print(f"{rule}  {gid}  {detail}", file=sys.stderr)
        print(f"{len(defects)} defect(s) in {DOC_NAME} §3", file=sys.stderr)
        return 1
    print(f"gaps states ok: {len(EXPECTED)} findings")
    return 0


try:
    status = main()
except Exception as exc:  # an interpreter failure is not a gate verdict, and there is no third code
    print(f"missing  §3  the gate could not read {DOC_NAME} ({exc.__class__.__name__}: {exc})",
          file=sys.stderr)
    print(f"1 defect(s) in {DOC_NAME} §3", file=sys.stderr)
    status = 1
sys.exit(status)
PY
