#!/usr/bin/env bash
# Shared fixture and assertions for the parallel-work tooling tests.
#
# Sourced by `scripts/tests/*.sh`. Each test file builds its own throwaway repository so the files can
# be read (and committed) one behaviour at a time: the claim test does not depend on `deliver-task.sh`
# existing, and the gate test does not depend on the claim test having run.
set -uo pipefail

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

passed=0
failed=0

fail() {
  printf 'FAIL  %s\n' "$*" >&2
  failed=$((failed + 1))
}
pass() {
  printf 'ok    %s\n' "$*"
  passed=$((passed + 1))
}

# The scripts read the tree they are run from (that is the interface `just` gives them), so every
# invocation names its working directory explicitly.
run_in() {
  local dir=$1
  shift
  ( cd "$dir" && "$@" )
}

expect_ok() {
  local label=$1
  shift
  if "$@" >"$WORK/out" 2>"$WORK/err"; then pass "$label"; else
    fail "$label (expected exit 0, got $?)"
    sed 's/^/      /' "$WORK/err" >&2
  fi
}

expect_fail() {
  local label=$1
  shift
  if "$@" >"$WORK/out" 2>"$WORK/err"; then
    fail "$label (expected a non-zero exit, got 0)"
  else pass "$label"; fi
}

expect_contains() {
  local label=$1 file=$2 needle=$3
  if grep -Fq -- "$needle" "$file" 2>/dev/null; then pass "$label"; else
    fail "$label (no «$needle» in $file)"
    sed 's/^/      /' "$file" >&2 2>/dev/null
  fi
}

expect_absent() {
  local label=$1 file=$2
  if [ -e "$file" ]; then fail "$label ($file still exists)"; else pass "$label"; fi
}

summary() {
  echo
  printf '%d passed, %d failed\n' "$passed" "$failed"
  [ "$failed" -eq 0 ]
}

# A repository that looks enough like this one: the scripts, a task index and task documents with and
# without a `## Surfaces` block. Sets MAIN, builds worktrees wt100…wt107 and a `cargo` stub, because
# running the real workspace suite is not what these tests judge.
#
# The optional tag lets one test build a second, independent repository beside the first (`repo2`,
# `wt1002`…). Without it every path is what the single-repository tests have always named.
build_repo() {
  local tag=${1:-}
  MAIN="$WORK/repo$tag"
  mkdir -p "$MAIN/scripts" "$MAIN/odd/tasks" "$WORK/bin"

  local script
  for script in claim-task release-task check-worktree check check-docs deliver-task; do
    [ -f "$REPO_ROOT/scripts/$script.sh" ] && cp "$REPO_ROOT/scripts/$script.sh" "$MAIN/scripts/"
  done
  chmod +x "$MAIN"/scripts/*.sh

  cat >"$WORK/bin/cargo" <<'STUB'
#!/usr/bin/env bash
echo "cargo $*" >>"${CARGO_STUB_LOG:-/dev/null}"
exit "${CARGO_STUB_EXIT:-0}"
STUB
  chmod +x "$WORK/bin/cargo"
  export PATH="$WORK/bin:$PATH"

  cat >"$MAIN/odd/tasks/INDEX.md" <<'MD'
# Índice de tareas

| # | tarea | estado | desde | último commit |
| --- | --- | --- | --- | --- |
| 100 | [alpha](100-alpha.md) | in progress | 2026-11-12 | — |
| 101 | [beta](101-beta.md) | in progress | 2026-11-12 | — |
| 102 | [gamma](102-gamma.md) | in progress | 2026-11-12 | — |
| 103 | [legacy](103-legacy.md) | in progress | 2026-11-12 | — |
| 104 | [legacy-clash](104-legacy-clash.md) | in progress | 2026-11-12 | — |
| 105 | [glob](105-glob.md) | in progress | 2026-11-12 | — |
| 106 | [dotslash](106-dotslash.md) | in progress | 2026-11-12 | — |
| 107 | [root-glob](107-root-glob.md) | in progress | 2026-11-12 | — |
MD

  cat >"$MAIN/odd/tasks/100-alpha.md" <<'MD'
# 100 — alpha

## Surfaces

- `crates/cam-geom/src/boolean.rs`
- `fixtures/corpus/`
MD

  cat >"$MAIN/odd/tasks/101-beta.md" <<'MD'
# 101 — beta

## Surfaces

- `crates/cam-geom/src/boolean.rs`
MD

  cat >"$MAIN/odd/tasks/102-gamma.md" <<'MD'
# 102 — gamma

## Surfaces

- `crates/cam-gerber/`
MD

  cat >"$MAIN/odd/tasks/103-legacy.md" <<'MD'
# 103 — legacy

No surfaces block here; the claim has to fall back to the crate tokens in the text, which is
`crates/cam-excellon` for this task.
MD

  cat >"$MAIN/odd/tasks/104-legacy-clash.md" <<'MD'
# 104 — legacy clash

Also `crates/cam-excellon`, so it must collide with the previous task.
MD

  cat >"$MAIN/odd/tasks/105-glob.md" <<'MD'
# 105 — glob

## Surfaces

- `crates/cam-geom/src/*.rs`
MD

  cat >"$MAIN/odd/tasks/106-dotslash.md" <<'MD'
# 106 — dotslash

## Surfaces

- `./fixtures/corpus`
MD

  cat >"$MAIN/odd/tasks/107-root-glob.md" <<'MD'
# 107 — root glob

## Surfaces

- `*.rs`
MD

  printf '# fixture justfile\n' >"$MAIN/justfile"

  # The real repository ignores the claims directory; without this the fixture's own claims make the
  # primary tree dirty and the gate test would be measuring the wrong thing.
  printf 'odd/tasks/.claims/\n' >"$MAIN/.gitignore"

  git -C "$MAIN" init -q
  git -C "$MAIN" add -A
  git -C "$MAIN" -c user.email=t@t -c user.name=t commit -qm init

  local n
  for n in 100 101 102 103 104 105 106 107; do
    git -C "$MAIN" worktree add -q "$WORK/wt$n$tag" -b "feat/$n" >/dev/null 2>&1 ||
      git -C "$MAIN" worktree add -q "$WORK/wt$n$tag" "feat/$n" >/dev/null 2>&1
  done
}

# Rewrite a Markdown index in the shape Biome gives it: every cell padded to its column's width, the
# separator row padded to the same widths. That is not what `build_repo` writes on purpose — the
# delivery tests assert the hand-written table's single-space cells — so a test that needs the
# formatted shape asks for it here instead of growing a second copy of the fixture.
#
# The width is the widest cell plus two spaces, with a floor of five because a separator needs three
# dashes; both numbers are what `biome format` produces for the tables in this repository.
format_index() {
  python3 - "$1" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
lines = path.read_text(encoding="utf-8").splitlines()
table = [i for i, line in enumerate(lines) if line.startswith("|")]
if not table:
    sys.exit(f"ERROR: no table in {path}")
sep_at = table[1]
separator = lines[sep_at].split("|")
columns = len(separator) - 2
widths = [0] * columns
for i in table:
    if i == sep_at:
        continue
    cells = lines[i].split("|")
    if len(cells) - 2 != columns:
        continue
    for c in range(columns):
        widths[c] = max(widths[c], len(cells[c + 1].strip()))
widths = [max(w + 2, 5) for w in widths]
out = list(lines)
out[sep_at] = "|" + "|".join(" " + "-" * (w - 2) + " " for w in widths) + "|"
for i in table:
    if i == sep_at:
        continue
    cells = lines[i].split("|")
    if len(cells) - 2 != columns:
        continue
    out[i] = "|" + "|".join(" " + cells[c + 1].strip().ljust(widths[c] - 1)
                            for c in range(columns)) + "|"
path.write_text("\n".join(out) + "\n", encoding="utf-8")
PY
}
