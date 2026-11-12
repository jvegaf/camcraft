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
# without a `## Surfaces` block. Sets MAIN, builds worktrees wt100…wt104 and a `cargo` stub, because
# running the real workspace suite is not what these tests judge.
build_repo() {
  MAIN="$WORK/repo"
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

  git -C "$MAIN" init -q
  git -C "$MAIN" add -A
  git -C "$MAIN" -c user.email=t@t -c user.name=t commit -qm init

  local n
  for n in 100 101 102 103 104; do
    git -C "$MAIN" worktree add -q "$WORK/wt$n" -b "feat/$n" >/dev/null 2>&1 ||
      git -C "$MAIN" worktree add -q "$WORK/wt$n" "feat/$n" >/dev/null 2>&1
  done
}
