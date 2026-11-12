#!/usr/bin/env bash
# Behaviour tests for the parallel-work tooling: `claim-task.sh`, `check-worktree.sh`, `check.sh` and
# `deliver-task.sh`. Runs every `scripts/tests/*.sh`, which build their own throwaway repository, so
# the assertions are about what an agent sees: exit codes, messages, the claim file and the shared
# index. `cargo` is a stub there because the gate's cargo steps are not what these tests judge.
set -uo pipefail
cd "$(dirname "$0")/.."

status=0
for test in scripts/tests/*.sh; do
  echo "-- $test"
  if ! bash "$test"; then status=1; fi
done
exit "$status"
