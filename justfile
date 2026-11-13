# campilot — task runner
#
# `just` on its own lists every recipe. `just check` is the gate the project promises; everything
# else is a narrower door onto the same commands, so nothing here can drift from scripts/check.sh.

set shell := ["bash", "-uc"]

# List the recipes.
default:
  @just --list

# Every gate: markdown links, fmt --check, clippy -D warnings, all tests.
check:
  ./scripts/check.sh

# Fail if a markdown link in the repository points at something that is not there.
check-docs:
  ./scripts/check-docs.sh

# Where the tree stands: branch, sync with origin, uncommitted files, suite size.
status:
  @printf 'branch:  %s\n' "$(git rev-parse --abbrev-ref HEAD)"
  @printf 'head:    %s %s\n' "$(git rev-parse --short HEAD)" "$(git log -1 --pretty=%s)"
  @if git rev-parse --verify --quiet '@{u}' >/dev/null; then \
    printf 'sync:    %s vs %s upstream\n' "$(git rev-parse --short HEAD)" "$(git rev-parse --short '@{u}')"; \
  else \
    printf 'sync:    no upstream configured\n'; \
  fi
  @printf 'dirty:   %s changed path(s)\n' "$(git status --porcelain | wc -l | tr -d ' ')"
  @printf 'features: %s\n' "$(ls odd/tasks/*.md 2>/dev/null | wc -l | tr -d ' ')"

# Format the whole workspace.
fmt:
  cargo fmt --all

# Fail if any file is not formatted.
fmt-check:
  cargo fmt --all -- --check

# Lint with warnings as errors, tests included.
clippy:
  cargo clippy --workspace --all-targets -- -D warnings

# Run the whole suite.
test:
  cargo test --workspace

# Run one crate's tests: `just test-crate cam-geom`.
test-crate crate:
  cargo test -p {{crate}}

# Run the tests whose name contains a filter: `just test-match island`.
test-match filter:
  cargo test --workspace {{filter}}

# Run the suite without swallowing stdout: `just test-loud pocket`.
test-loud filter="":
  cargo test --workspace {{filter}} -- --nocapture

# E0.4: measure the committed corpus and fail on any divergence from its goldens (W1a).
# Never pass `--update` here: re-pinning goldens is a reviewed human change, not a gate.
parity:
  ./scripts/parity/run.sh

# E0.7: run the benchmarks and compare them against the committed criterion baseline (W1c).
# The comparison is reported by criterion; it is hardware-coupled, so it is a tripwire, not a
# number to gate a shared runner on.
bench:
  ./crates/cam-geom/benches/check_regression.sh --noplot
  ./crates/cam-ops/benches/check_regression.sh --noplot

# Build the workspace in debug.
build:
  cargo build --workspace

# Build the workspace optimised.
build-release:
  cargo build --workspace --release

# Open the editor window. Needs a display and a GPU or a software renderer.
run:
  cargo run -p cam-desktop

# Call the headless CLI: `just cli inspect part.c2d`, `just cli emit part.c2d --post mach3.pp`.
cli *args:
  cargo run -q -p cam-cli -- {{args}}

# Build the API docs for the workspace only (no dependency docs).
doc:
  cargo doc --workspace --no-deps

# List the feature documents, newest last.
features:
  @ls -1 odd/tasks/*.md

# Show the milestone table.
milestones:
  @sed -n '/^| \*\*M/p' odd/roadmap/index.md

# Remove build artefacts.
clean:
  cargo clean

# Update the lock file within the declared version ranges.
update:
  cargo update

# Print the dependency tree of a crate: `just deps cam-toolpath`.
deps crate="cam-cli":
  cargo tree -p {{crate}} --depth 2

# Build against the committed lock file, which is what CI and a fresh clone see.
build-locked:
  cargo build --workspace --locked


# Worktrees for parallel agent work are created by herdr (~/.herdr/worktrees); this recipe explains.
worktree N:
  @echo "Worktrees are created by herdr, not by this recipe."
  @echo "Create one for task {{N}} at ~/.herdr/worktrees/<slug> on branch feat/{{N}}-<slug>,"
  @echo "then from inside it run: just claim {{N}}"
  @echo "Scripts resolve the tree from 'git worktree list', so any location works."

# Claim a task for this worktree, by the paths in its `## Surfaces` block (not by crate).
claim N:
  @./scripts/claim-task.sh {{N}}

# Release a claim without delivering (abandoned work, or a claim taken in the wrong tree).
release N:
  @./scripts/release-task.sh {{N}}

# Record a delivered task in the shared index and release its claim. Run it after the merge.
deliver N commit="":
  @./scripts/deliver-task.sh {{N}} {{commit}}

# Validate we're in a properly claimed linked worktree.
check-worktree:
  @./scripts/check-worktree.sh

# Behaviour tests for the claim/merge tooling (throws away its own repository).
test-tooling:
  @./scripts/test-worktree-tooling.sh
