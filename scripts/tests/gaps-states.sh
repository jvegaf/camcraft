#!/usr/bin/env bash
# Behaviour tests for `scripts/check-gaps-states.sh`, the gate over the findings of
# `docs/GAPS_CAMCRAFT.md` §3.
#
# The gate exists to fail when a finding loses its id, its state or its resolvable citation, so every
# rule gets a case that proves it *fails*: the exact defect line, the exact summary and the exit code,
# not merely "the command was not happy". The document under test is a throwaway fixture built here
# (the real `docs/GAPS_CAMCRAFT.md` changes with every re-survey, and a test that reads it would go red
# for a reason that is not this gate).
#
# It follows the convention of `scripts/tests/*.sh`: `common.sh` supplies the assertions and the
# `mktemp -d` work directory, `scripts/test-worktree-tooling.sh` picks the file up because it runs
# every `scripts/tests/*.sh`, and nothing is ever written inside the repository.
set -uo pipefail
# shellcheck source=scripts/tests/lib/common.sh
. "$(dirname "$0")/lib/common.sh"

MAIN="$WORK/gaps"
DOC="$MAIN/docs/GAPS_CAMCRAFT.md"

# The expected set of 110 S2, restated on purpose: the gate hard-codes it (deriving it from the
# document would let a deleted finding pass), and a test that read it from the gate would not notice
# the set shrinking. This is the fixture's copy of the contract.
IDS=(
  G0.1 G0.2 G0.3
  G1.1 G1.2 G1.3 G1.4
  G2.1 G2.2 G2.3 G2.4 G2.5
  G3.1 G3.2 G3.3 G3.4 G3.5 G3.6 G3.7
  G4.1 G4.2 G4.3 G4.4
  G5.1 G5.2 G5.3
  G6.1 G6.2 G6.3 G6.4
)

# Exit codes and message shapes are what the CI step and `scripts/check.sh` consume, so they are
# asserted exactly. `common.sh` has no exit-code assertion beyond "zero / not zero", and the gate has
# only two verdicts (0 and 1): a third one would be a crash, not a result.
expect_exit() {
  local label=$1 want=$2
  shift 2
  local status=0
  "$@" >"$WORK/out" 2>"$WORK/err" || status=$?
  if [ "$status" -eq "$want" ]; then pass "$label (exit $status)"; else
    fail "$label (expected exit $want, got $status)"
    sed 's/^/      /' "$WORK/err" >&2
  fi
}

# One defect means one line on stderr plus the summary line: counting them proves a case fails for the
# rule it names and for nothing else.
expect_err_lines() {
  local label=$1 want=$2
  local got
  got=$(wc -l <"$WORK/err")
  if [ "$got" -eq "$want" ]; then pass "$label ($want line(s) on stderr)"; else
    fail "$label (expected $want line(s) on stderr, got $got)"
    sed 's/^/      /' "$WORK/err" >&2
  fi
}

expect_empty() {
  local label=$1 file=$2
  if [ -s "$file" ]; then
    fail "$label ($file is not empty)"
    sed 's/^/      /' "$file" >&2
  else pass "$label"; fi
}

# The gate reads the tree it is run from, so the fixture is the working directory.
gate() {
  ( cd "$MAIN" && ./scripts/check-gaps-states.sh )
}

# A repository-shaped tree with the gate, the file the citations point at, and a §3 holding all 30
# findings in their canonical shape. §3 ends at `## 4.` on purpose: the gate must stop there.
build_fixture() {
  rm -rf "$MAIN"
  mkdir -p "$MAIN/scripts" "$MAIN/docs" "$MAIN/src"
  cp "$REPO_ROOT/scripts/check-gaps-states.sh" "$MAIN/scripts/"
  chmod +x "$MAIN/scripts/check-gaps-states.sh"
  printf 'one\ntwo\nthree\n' >"$MAIN/src/lib.rs"
  {
    printf '# GAPS\n\n## 3. Gaps por fase del plan\n\n'
    local id
    for id in "${IDS[@]}"; do
      printf -- '- **%s — Un hallazgo. Estado: «ya no aplica».** Cita `src/lib.rs:1`.\n' "$id"
    done
    printf '\n## 4. El gap transversal\n\nNada que ver con el §3.\n'
  } >"$DOC"
}

# Insert a line at the end of §3 (before `## 4.`), which is where a new finding would appear.
append_to_section() {
  sed -i 's|^## 4\. El gap transversal$|'"$1"'\n\n## 4. El gap transversal|' "$DOC"
}

# The plain `sed -i` mutations below edit one fixture and are rebuilt from scratch for every case, so
# no case can be explained by a previous one.

# ---------------------------------------------------------------- the passing case
build_fixture
expect_ok "the 30 canonical findings pass" gate
expect_contains "the pass line names the count" "$WORK/out" "gaps states ok: 30 findings"
expect_empty "the pass line is the only output" "$WORK/err"

# ---------------------------------------------------------------- S2: the expected id set
build_fixture
sed -i '/^- \*\*G2\.2 — /d' "$DOC"
expect_exit "a missing expected id fails" 1 gate
expect_contains "a missing id is reported as missing" "$WORK/err" \
  "missing  G2.2  no finding starts with «- **G2.2 — » in §3"
expect_contains "the missing summary counts one defect" "$WORK/err" \
  "1 defect(s) in docs/GAPS_CAMCRAFT.md §3"
expect_err_lines "a missing id is the only defect" 2

build_fixture
sed -i '/^- \*\*G2\.2 — /p' "$DOC"
expect_exit "a duplicated id fails" 1 gate
expect_contains "a duplicated id is reported as duplicate" "$WORK/err" \
  "duplicate  G2.2  2 findings start with «- **G2.2 — » in §3 (expected 1)"
expect_contains "the duplicate summary counts one defect" "$WORK/err" \
  "1 defect(s) in docs/GAPS_CAMCRAFT.md §3"
expect_err_lines "a duplicated id is the only defect" 2

build_fixture
append_to_section '- **G7.1 — Un hallazgo que el barrido descubrió. Estado: «ya no aplica».** Cita `src/lib.rs:1`.'
expect_exit "an id outside the expected set fails" 1 gate
expect_contains "an unexpected id is reported as unknown" "$WORK/err" \
  "unknown  G7.1  not in the expected set of 30 ids"
expect_contains "the unknown summary counts one defect" "$WORK/err" \
  "1 defect(s) in docs/GAPS_CAMCRAFT.md §3"
expect_err_lines "an unknown id is the only defect" 2

# A mention of an id inside another finding's prose is not a finding and is not a duplicate: only the
# bullet shape starts one.
build_fixture
sed -i 's|^- \*\*G2\.1 — Un hallazgo\.|& Como en G2.2, y en G2.2 otra vez.|' "$DOC"
expect_ok "a prose mention of an id is not a duplicate" gate
expect_contains "the prose mention keeps the count" "$WORK/out" "gaps states ok: 30 findings"
expect_empty "the prose mention prints no defect" "$WORK/err"

# A finding-shaped line outside §3 is not part of the section: G2.2 stays missing and nothing is
# duplicated.
build_fixture
sed -i '/^- \*\*G2\.2 — /d' "$DOC"
printf -- '- **G2.2 — Fuera de la sección. Estado: «ya no aplica».** Cita `src/lib.rs:1`.\n' >>"$DOC"
expect_exit "a finding outside §3 does not count" 1 gate
expect_contains "the id outside §3 is still missing" "$WORK/err" \
  "missing  G2.2  no finding starts with «- **G2.2 — » in §3"
expect_err_lines "a finding outside §3 hides nothing" 2

# ---------------------------------------------------------------- S3: exactly one state
build_fixture
sed -i '/^- \*\*G0\.1 — /s|Estado: «ya no aplica»\.|.|' "$DOC"
expect_exit "a finding with zero states fails" 1 gate
expect_contains "zero states are reported as state-count" "$WORK/err" \
  "state-count  G0.1  expected exactly one «Estado:» marker in the bold header, found 0"
expect_contains "the state-count summary counts one defect" "$WORK/err" \
  "1 defect(s) in docs/GAPS_CAMCRAFT.md §3"
expect_err_lines "a missing state is the only defect" 2

build_fixture
sed -i '/^- \*\*G0\.1 — /s|\.\*\* Cita|. Estado: «ya no aplica».** Cita|' "$DOC"
expect_exit "a finding with two states fails" 1 gate
expect_contains "two states are reported as state-count" "$WORK/err" \
  "state-count  G0.1  expected exactly one «Estado:» marker in the bold header, found 2"
expect_err_lines "a second state is the only defect" 2

# The confidence labels are not states: a finding that only carries one still has zero.
build_fixture
sed -i '/^- \*\*G0\.1 — /s|Estado: «ya no aplica»\.|INFERIDO.|' "$DOC"
expect_exit "INFERIDO does not substitute for a state" 1 gate
expect_contains "INFERIDO leaves zero states" "$WORK/err" \
  "state-count  G0.1  expected exactly one «Estado:» marker in the bold header, found 0"
expect_err_lines "INFERIDO is the only defect" 2

build_fixture
sed -i '/^- \*\*G0\.1 — /s|Estado: «ya no aplica»\.|Estado: abierto.|' "$DOC"
expect_exit "an unquoted state fails" 1 gate
expect_contains "an unquoted state is unparseable" "$WORK/err" \
  "state-value  G0.1  Estado: abierto is not a quoted state"
expect_err_lines "an unquoted state is the only defect" 2

build_fixture
sed -i '/^- \*\*G0\.1 — /s|«ya no aplica»|«abierto»|' "$DOC"
expect_exit "a state outside the three fails" 1 gate
expect_contains "an unknown state label is reported by value" "$WORK/err" \
  "state-value  G0.1  «abierto» is not one of the three states of 110 S3"
expect_err_lines "an unknown state label is the only defect" 2

build_fixture
sed -i '/^- \*\*G0\.1 — /s|Estado: «ya no aplica»\.|Estado: «abierto|' "$DOC"
expect_exit "an unclosed quote fails" 1 gate
expect_contains "an unclosed quote is unparseable" "$WORK/err" \
  "state-value  G0.1  «abierto has no closing »"
expect_err_lines "an unclosed quote is the only defect" 2

# ---------------------------------------------------------------- S4: citations
build_fixture
sed -i '/^- \*\*G0\.1 — /s| Cita `src/lib.rs:1`\.| Sin cita.|' "$DOC"
expect_exit "a finding with no citation fails" 1 gate
expect_contains "a lost citation is reported as citation" "$WORK/err" \
  "citation  G0.1  no inline code span with a path that resolves from the repository root"
expect_err_lines "a lost citation is the only defect" 2

build_fixture
sed -i '/^- \*\*G0\.1 — /s|`src/lib.rs:1`|`src/missing.rs:1`|' "$DOC"
expect_exit "a line citation that does not resolve fails" 1 gate
expect_contains "an unresolvable path is reported as citation" "$WORK/err" \
  "citation  G0.1  \`src/missing.rs:1\` does not resolve from the repository root"
expect_err_lines "an unresolvable path is the only defect" 2

build_fixture
sed -i '/^- \*\*G0\.1 — /s|`src/lib.rs:1`|`src/lib.rs:0`|' "$DOC"
expect_exit "line 0 fails" 1 gate
expect_contains "line 0 is reported as line" "$WORK/err" \
  "line  G0.1  \`src/lib.rs:0\` line 0 is out of range (the file has 3 lines)"
expect_err_lines "line 0 is the only defect" 2

build_fixture
sed -i '/^- \*\*G0\.1 — /s|`src/lib.rs:1`|`src/lib.rs:99`|' "$DOC"
expect_exit "a line past EOF fails" 1 gate
expect_contains "a line past EOF is reported as line" "$WORK/err" \
  "line  G0.1  \`src/lib.rs:99\` line 99 is past EOF (the file has 3 lines)"
expect_err_lines "a line past EOF is the only defect" 2

build_fixture
sed -i '/^- \*\*G0\.1 — /s|`src/lib.rs:1`|`src/lib.rs:3-1`|' "$DOC"
expect_exit "an inverted range fails" 1 gate
expect_contains "an inverted range is reported as line" "$WORK/err" \
  "line  G0.1  \`src/lib.rs:3-1\` the range is inverted (3 > 1)"
expect_err_lines "an inverted range is the only defect" 2

# A path without a line that does not resolve is not a `path:line` claim: the document cites module
# and historical paths that are not files of this tree, and the gate does not fail them.
build_fixture
sed -i '/^- \*\*G0\.1 — /s| Cita `src/lib.rs:1`\.| Cita `src/lib.rs:1` y el módulo `tool_geometry/tool_cutting_data/material`.|' "$DOC"
expect_ok "a historical module path is not a defect" gate
expect_contains "the historical path keeps the count" "$WORK/out" "gaps states ok: 30 findings"
expect_empty "the historical path prints no defect" "$WORK/err"

# A citation is a claim about *this* tree. `ROOT / "/tmp/x"` is `/tmp/x` (an absolute path discards
# the root on join) and `ROOT / "../x"` walks out of the checkout, so both would cite a file the
# checkout does not contain.
build_fixture
printf 'one\ntwo\n' >"$WORK/outside.rs"
sed -i "/^- \*\*G0\.1 — /s|\`src/lib\.rs:1\`|\`$WORK/outside.rs:1\`|" "$DOC"
expect_exit "an absolute citation fails" 1 gate
expect_contains "an absolute citation is rejected, not joined" "$WORK/err" \
  "citation  G0.1  \`$WORK/outside.rs:1\` is an absolute path, not one from the repository root"
expect_err_lines "an absolute citation is the only defect" 2

build_fixture
mkdir -p "$WORK/escape"
printf 'one\ntwo\n' >"$WORK/escape/outside.rs"
sed -i '/^- \*\*G0\.1 — /s|`src/lib\.rs:1`|`../escape/outside.rs:1`|' "$DOC"
expect_exit "a parent-directory citation fails" 1 gate
expect_contains "a parent-directory citation is rejected, not joined" "$WORK/err" \
  "citation  G0.1  \`../escape/outside.rs:1\` leaves the repository root with a «..» component"
expect_err_lines "a parent-directory citation is the only defect" 2

# Only a regular file is a citation: a directory is not evidence a finding still holds, in either
# branch. The bare path is the one the review found still green.
build_fixture
mkdir -p "$MAIN/src/dir"
sed -i '/^- \*\*G0\.1 — /s| Cita `src/lib\.rs:1`\.| Cita `src/dir`.|' "$DOC"
expect_exit "a directory cited without a line fails" 1 gate
expect_contains "a directory does not resolve as a citation" "$WORK/err" \
  "citation  G0.1  no inline code span with a path that resolves from the repository root"
expect_err_lines "a directory without a line is the only defect" 2

build_fixture
mkdir -p "$MAIN/src/dir"
sed -i '/^- \*\*G0\.1 — /s|`src/lib\.rs:1`|`src/dir:1`|' "$DOC"
expect_exit "a directory cited with a line fails" 1 gate
expect_contains "a directory with a line is reported as a directory" "$WORK/err" \
  "citation  G0.1  \`src/dir:1\` resolves to a directory, not a file"
expect_err_lines "a directory with a line is the only defect" 2

# ---------------------------------------------------------------- the document itself
# The header promises there is no third verdict: a missing document, an unreadable one and a document
# without §3 are all defects with the same exit code and the same one-defect summary.
build_fixture
rm -f "$DOC"
expect_exit "a missing document fails" 1 gate
expect_contains "a missing document is reported as missing" "$WORK/err" \
  "missing  §3  docs/GAPS_CAMCRAFT.md does not exist"
expect_contains "a missing document counts one defect" "$WORK/err" \
  "1 defect(s) in docs/GAPS_CAMCRAFT.md §3"
expect_err_lines "a missing document is the only defect" 2

# A file mode would not do it: the suite may run as root, where chmod 000 is still readable. A
# directory in the document's place raises the same read error for every user.
build_fixture
rm -f "$DOC"
mkdir -p "$DOC"
expect_exit "an unreadable document fails" 1 gate
expect_contains "an unreadable document is reported as unreadable" "$WORK/err" \
  "missing  §3  docs/GAPS_CAMCRAFT.md is unreadable (IsADirectoryError)"
expect_err_lines "an unreadable document is the only defect" 2

build_fixture
sed -i 's|^## 3\. Gaps por fase del plan$|## 3 Gaps por fase del plan|' "$DOC"
expect_exit "a document without §3 fails" 1 gate
expect_contains "a missing §3 heading is reported as missing" "$WORK/err" \
  "missing  §3  docs/GAPS_CAMCRAFT.md has no «## 3.» section"
expect_err_lines "a missing §3 heading is the only defect" 2

# ---------------------------------------------------------------- S5: not a prose linter
build_fixture
sed -i 's|^- \*\*G2\.2 — .*$|- **G2.2 — Otro título y otra redacción por completo, con otra severidad. Estado: «ya no aplica».** Cita `src/lib.rs:1`.|' "$DOC"
expect_ok "changed prose with id, state and citation passes" gate
expect_contains "the reworded finding keeps the count" "$WORK/out" "gaps states ok: 30 findings"
expect_empty "the reworded finding prints no defect" "$WORK/err"

# ---------------------------------------------------------------- the summary
summary
