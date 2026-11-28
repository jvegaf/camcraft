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

# A repository that looks enough like this one: the scripts, a task index, task documents with and
# without a `## Surfaces` block, and the `docs/GAPS_CAMCRAFT.md` §3 the gaps gate reads. Sets MAIN,
# builds worktrees wt100…wt107 and a `cargo` stub, because
# running the real workspace suite is not what these tests judge.
#
# The optional tag lets one test build a second, independent repository beside the first (`repo2`,
# `wt1002`…). Without it every path is what the single-repository tests have always named.
build_repo() {
  local tag=${1:-}
  MAIN="$WORK/repo$tag"
  mkdir -p "$MAIN/scripts" "$MAIN/odd/tasks" "$WORK/bin"

  local script
  for script in claim-task release-task check-worktree check check-docs check-records check-gaps-states deliver-task; do
    [ -f "$REPO_ROOT/scripts/$script.sh" ] && cp "$REPO_ROOT/scripts/$script.sh" "$MAIN/scripts/"
  done
  chmod +x "$MAIN"/scripts/*.sh

  cat >"$WORK/bin/cargo" <<'STUB'
#!/usr/bin/env bash
echo "cargo $*" >>"${CARGO_STUB_LOG:-/dev/null}"
exit "${CARGO_STUB_EXIT:-0}"
STUB
  chmod +x "$WORK/bin/cargo"

  # `check-records.sh --engram-required` asks the real CLI only two questions: whether its daemon is
  # running (`engram ps`) and whether a memory mentions the task (`engram search SLUG --all --limit 1`,
  # whose first line must match `Found N memor…`). The fixture has no Engram store, so this answers
  # both with a fixed reply instead of making delivery depend on the machine's memory.
  cat >"$WORK/bin/engram" <<'STUB'
#!/usr/bin/env bash
case "${1:-}" in
  ps)
    echo "engram daemon: running"
    ;;
  search)
    echo "Found 1 memory for '${2:-}'"
    ;;
  *)
    echo "engram stub: unhandled command ${1:-}" >&2
    ;;
esac
exit 0
STUB
  chmod +x "$WORK/bin/engram"
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

## Specs

- S1: fixture task, complete enough for the records check.

## Tasks

- T1: fixture task.

## Log

- **L1:** fixture task.
MD

  cat >"$MAIN/odd/tasks/101-beta.md" <<'MD'
# 101 — beta

## Surfaces

- `crates/cam-geom/src/boolean.rs`

## Specs

- S1: fixture task, complete enough for the records check.

## Tasks

- T1: fixture task.

## Log

- **L1:** fixture task.
MD

  cat >"$MAIN/odd/tasks/102-gamma.md" <<'MD'
# 102 — gamma

## Surfaces

- `crates/cam-gerber/`

## Specs

- S1: fixture task, complete enough for the records check.

## Tasks

- T1: fixture task.

## Log

- **L1:** fixture task.
MD

  # The empty `## Surfaces` heading is deliberate: `check-records.sh` requires the section, and the
  # claim still falls back to the crate tokens because no paths are declared. Removing the heading
  # (not the block) is what the legacy-claim cases assert against.
  cat >"$MAIN/odd/tasks/103-legacy.md" <<'MD'
# 103 — legacy

## Surfaces

No declared surfaces here; the claim has to fall back to the crate tokens in the text, which is
`crates/cam-excellon` for this task.

## Specs

- S1: fixture task, complete enough for the records check.

## Tasks

- T1: fixture task.

## Log

- **L1:** fixture task.
MD

  cat >"$MAIN/odd/tasks/104-legacy-clash.md" <<'MD'
# 104 — legacy clash

## Surfaces

Also `crates/cam-excellon`, so it must collide with the previous task.

## Specs

- S1: fixture task, complete enough for the records check.

## Tasks

- T1: fixture task.

## Log

- **L1:** fixture task.
MD

  cat >"$MAIN/odd/tasks/105-glob.md" <<'MD'
# 105 — glob

## Surfaces

- `crates/cam-geom/src/*.rs`

## Specs

- S1: fixture task, complete enough for the records check.

## Tasks

- T1: fixture task.

## Log

- **L1:** fixture task.
MD

  cat >"$MAIN/odd/tasks/106-dotslash.md" <<'MD'
# 106 — dotslash

## Surfaces

- `./fixtures/corpus`

## Specs

- S1: fixture task, complete enough for the records check.

## Tasks

- T1: fixture task.

## Log

- **L1:** fixture task.
MD

  cat >"$MAIN/odd/tasks/107-root-glob.md" <<'MD'
# 107 — root glob

## Surfaces

- `*.rs`

## Specs

- S1: fixture task, complete enough for the records check.

## Tasks

- T1: fixture task.

## Log

- **L1:** fixture task.
MD

  printf '# fixture justfile\n' >"$MAIN/justfile"

  # `check.sh`'s `== gaps ==` step reads §3 of `docs/GAPS_CAMCRAFT.md` and the file its citations name,
  # so the fixture carries both. Without them the fixture's `check.sh` cannot pass the real gate, and
  # the tooling test would be measuring a fixture that no real repository resembles.
  #
  # COUPLING: this id list is a third copy of the gate's expected set, after
  # `scripts/check-gaps-states.sh` (`EXPECTED`) and `scripts/tests/gaps-states.sh` (`IDS`). The gate
  # hard-codes the 30 ids on purpose (deriving them would let a deleted finding pass), so a change to
  # that set has to change this fixture and the gate's own test together — the fixture does not learn
  # the set from anywhere.
  mkdir -p "$MAIN/docs" "$MAIN/src"
  {
    printf '# GAPS (fixture del arnés)\n\n'
    printf 'Este documento **no es** `docs/GAPS_CAMCRAFT.md`: es el fixture que escribe\n'
    printf '`scripts/tests/lib/common.sh` para que el `check.sh` de los repositorios de prueba pueda\n'
    printf 'pasar el gate real. No lo actualices a partir del documento de verdad, y no lo tomes por\n'
    printf 'la diagnosis: sus hallazgos son de relleno y sólo importa su forma.\n\n'
    printf '## 3. Gaps por fase del plan (dashboard)\n\n'
    local gid
    for gid in G0.1 G0.2 G0.3 G1.1 G1.2 G1.3 G1.4 G2.1 G2.2 G2.3 G2.4 G2.5 \
      G3.1 G3.2 G3.3 G3.4 G3.5 G3.6 G3.7 G4.1 G4.2 G4.3 G4.4 G5.1 G5.2 G5.3 \
      G6.1 G6.2 G6.3 G6.4; do
      printf -- '- **%s — Hallazgo de relleno. Estado: «ya no aplica».** Cita `src/lib.rs:1`.\n' "$gid"
    done
    printf '\n## 4. El gap transversal\n\nFuera del §3, para que el gate tenga dónde parar.\n'
  } >"$MAIN/docs/GAPS_CAMCRAFT.md"
  printf 'one\ntwo\nthree\n' >"$MAIN/src/lib.rs"

  # The real repository ignores the claims directory; without this the fixture's own claims make the
  # primary tree dirty and the gate test would be measuring the wrong thing.
  printf 'odd/tasks/.claims/\n' >"$MAIN/.gitignore"

  git -C "$MAIN" init -q
  git -C "$MAIN" add -A
  git -C "$MAIN" -c user.email=t@t -c user.name=t commit -qm init

  # A delivery records the merge commit and refuses a hash it cannot resolve, so the fixture exposes
  # the one commit it really made instead of leaving the tests to invent `abc1234`.
  FIXTURE_COMMIT=$(git -C "$MAIN" rev-parse HEAD)

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
