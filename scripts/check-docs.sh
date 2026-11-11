#!/usr/bin/env bash
# Fails when a markdown link in the repository points at something that is not there.
#
# The documentation is the deliverable it checks, so a rename that forgets one reference has to be a
# red gate and not a broken link nobody notices for a month. It reads every tracked `.md` file, takes
# relative targets only (anything with a scheme, a mailto or a bare `#anchor` is not a path), strips
# the `#fragment` and the query, and requires the target to exist. Links that point outside the
# repository are reported as unverifiable instead of failing, because this script cannot see them.
set -euo pipefail
cd "$(dirname "$0")/.."

python3 - <<'PY'
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path.cwd()
LINK = re.compile(r"(?<!!)\[[^\]]*\]\(([^)]+)\)")
# A link may percent-encode spaces or accents; the filesystem does not.
UNQUOTE = __import__("urllib.parse", fromlist=["unquote"]).unquote
# A markdown file may quote a link on purpose (a spec, a diff, a schema). Markdown gives no way to
# tell an example from a real link, so the escape hatch is an explicit marker on the same line.
SKIP_MARKER = "docs-check: ignore"

tracked = subprocess.run(
    ["git", "ls-files", "-z", "*.md"], capture_output=True, text=True, check=True
).stdout.split("\0")

broken = []
unverifiable = []
for name in filter(None, tracked):
    path = ROOT / name
    # A deletion that is staged or simply not committed yet is still in `git ls-files`: its links are
    # gone with it, and reading it is not the check's business.
    if not path.is_file():
        continue
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if SKIP_MARKER in line:
            continue
        for raw in LINK.findall(line):
            target = raw.strip()
            if target.startswith("<") and target.endswith(">"):
                target = target[1:-1]
            target = UNQUOTE(target.split("#", 1)[0].split("?", 1)[0])
            if not target or target.startswith("#"):
                continue
            if re.match(r"^[a-zA-Z][a-zA-Z0-9+.-]*:", target):
                unverifiable.append(f"{name}:{number}: {raw.strip()}")
                continue
            resolved = (path.parent / target).resolve()
            if not resolved.exists():
                broken.append(f"{name}:{number}: {raw.strip()}")

for item in sorted(unverifiable):
    print(f"unverifiable  {item}")
if broken:
    for item in sorted(broken):
        print(f"broken        {item}", file=sys.stderr)
    sys.exit(f"{len(broken)} broken markdown link(s)")
print(f"markdown links ok: {len(tracked)} files")
PY
