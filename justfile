# Camcraft — task runner (Tauri + Svelte + Panda CSS)
#
# `just` lists recipes. `just check` is the gate.

set shell := ["bash", "-uc"]

default:
  @just --list

# Gate principal
check:
  ./scripts/check.sh

check-docs:
  ./scripts/check-docs.sh

records:
  ./scripts/check-records.sh

check-worktree:
  @./scripts/check-worktree.sh

# Frontend
install:
  pnpm install

dev:
  pnpm dev

tauri-dev:
  pnpm tauri dev

build:
  pnpm build

tauri-build:
  pnpm tauri build

lint:
  pnpm lint

fmt:
  pnpm format

fmt-check:
  pnpm lint

check-ui:
  pnpm check

test:
  pnpm test

# Tauri/Rust
check-rust:
  cargo check --workspace

test-rust:
  cargo test --manifest-path src-tauri/Cargo.toml

test-rust-workspace:
  cargo test --workspace --exclude cam-ui --exclude cam-desktop

# Panda
codegen:
  pnpm codegen

# Worktrees / ODD
worktree N:
  #!/usr/bin/env bash
  set -euo pipefail
  N="{{N}}"
  MAIN=$(git worktree list --porcelain | awk '/^worktree /{sub(/^worktree /,""); print; exit}')
  cd "$MAIN"
  TASKFILE=$(ls odd/tasks/"$N"-*.md 2>/dev/null | head -1 || true)
  if [ -z "$TASKFILE" ] && [[ "$N" =~ ^[0-9]+$ ]]; then
    PAD=$(printf '%03d' "$((10#$N))")
    TASKFILE=$(ls odd/tasks/"$PAD"-*.md 2>/dev/null | head -1 || true)
    [ -n "$TASKFILE" ] && N="$PAD"
  fi
  if [ -z "$TASKFILE" ]; then
    echo "ERROR: No task document for $N in odd/tasks/." >&2
    exit 1
  fi
  SLUG=$(basename "$TASKFILE" .md)
  SLUG=${SLUG#"$N"-}
  DIR=".worktrees/$N"
  BRANCH="feat/$N-$SLUG"
  mkdir -p .worktrees
  if [ -d "$DIR" ]; then
    echo "Worktree already exists: $DIR"
  else
    git worktree add "$DIR" -b "$BRANCH"
    echo "Created $DIR on $BRANCH"
  fi
  echo "Next: cd $DIR && just claim $N"

claim N:
  @./scripts/claim-task.sh {{N}}

release N:
  @./scripts/release-task.sh {{N}}

deliver N commit="":
  @./scripts/deliver-task.sh {{N}} {{commit}}

# Visual review (webview/Tauri; adaptar harness)
ui-review:
  @echo "TODO: wire webview/Tauri visual review harness"
  @echo "See .agents/skills/ui-visual-review/SKILL.md and fixtures/ui/manifest.json"

ui-review-sway *scenes:
  scripts/ui-review-sway.sh {{scenes}}
