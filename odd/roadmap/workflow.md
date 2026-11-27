# Git workflow

One trunk: `master`. Everything integrates there, and every release is tagged there; there is no
second branch holding work. The split that used to exist was not a review stage but a clock: measuring
the corpus costs minutes, and spending them on every integration answers a question no release has
asked yet. That clock now lives in the CI triggers — `parity` and `bench` only run when someone
dispatches them by hand — so keeping a branch alive to avoid the measurement no longer buys anything.

## Why the second branch is gone

`dev` existed for one reason: it was the cheap trunk, and the corpus measurement ran only on the way
to `master`. The measurement is still minutes, and still not worth paying per integration, but the
cost is now bounded by a trigger instead of by a branch: a push or a pull request to `master` runs the
unit tests, and `parity`/`bench` run only on `workflow_dispatch`. A second branch would duplicate every
merge, every hook and every claim check to buy minutes the dispatch already saves, and it would leave
two places where "green" could mean different things.

## The trunk

| Trunk    | Receives                | Runs in CI                                                                                                                                              |
| -------- | ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `master` | every integrated change | `gate` on push and pull request — the frontend unit tests (`pnpm test`) and the Tauri host package's tests (`cargo test --manifest-path src-tauri/Cargo.toml`); `parity` and `bench` only by `workflow_dispatch` |

`master` is protected — in the repository's settings, not in this file: no force-push, no deletion,
conversation resolution required, a pull request required to merge, and the administrator bypass
available. That bypass is load-bearing: it is the only reason the direct path exists, so turning it off
turns this workflow into pull-request-only. No status check is required today, and the only context
that may ever be required is `gate`: `parity` and `bench` run only on `workflow_dispatch`, so a
required check that never reports on a pull request would block every pull request forever. The job
names in `.github/workflows/ci.yml` are the status contexts: with no required check configured today
there is nothing to drop, but renaming a job would leave any future required context unresolved — the
check would wait forever for a name no job reports.

## Day to day

Work happens in a claimed worktree on a short branch off `master` (`AGENTS.md`; `just worktree N`
creates the worktree and its branch, `just claim N` claims its surfaces). Integration is hybrid by
risk, because there is one developer:

- **Risky changes enter through a pull request** from their short branch: migrations, public
  contracts, CI, dependencies. These are the changes whose damage is expensive to undo, and a PR is
  both the readable diff and the place CI runs before the trunk takes them.
- **Small changes merge locally and push straight to `master`**, using the admin bypass. A typo, a
  comment, a test that failed for a reason you just found — a PR whose only reviewer is its author
  adds a round trip and no information.

The dividing line is the risk list, not the size of the diff. When a change is small and still risky,
it is risky.

```bash
just hooks               # once per clone: installs .githooks/pre-commit and .githooks/pre-push
just worktree N          # creates .worktrees/N on feat/N-<slug>
cd .worktrees/N && just claim N   # in the task's worktree, before the first write
# … work, commit in reviewable slices …
# small work:
git switch master && git merge --no-ff feat/N-<slug> && git push origin master
# risky work:
gh pr create --base master --head feat/N-<slug> --fill
```

## A record, before anything is called done

`just deliver N <commit>` is the moment a task is called finished, so it is the moment the record is
proved: `scripts/deliver-task.sh` runs `scripts/check-records.sh --task N --engram-required` first and
writes nothing if the document is incomplete, the commit does not resolve, or Engram holds no memory
naming the task's slug. `just check` runs the same verifier as `== records ==`, without the Engram
requirement — the memory proof belongs to delivery, not to every local gate — and reports documents
written before the convention as advisories instead of failing history.

## A release

`master` is the release trunk, so a release is a tag on the commit that is already there, plus notes.

```bash
git switch master && git pull
git tag -a v0.X.0 -m "v0.X.0"
git push origin v0.X.0
gh release create v0.X.0 --generate-notes
```

There is no release pull request: there is no second branch to promote from, and a PR that only moved
the trunk onto itself would be ceremony. A tag is what makes `master` mean something — without one it
is just the branch everything lands on.

## CI: what runs, and when

| Event                    | Jobs                                                       |
| ------------------------ | ---------------------------------------------------------- |
| push to `master`         | `gate`: `pnpm test` (vitest) and the Tauri host package's tests (`cargo test --manifest-path src-tauri/Cargo.toml`) |
| pull request to `master` | the same `gate`                                            |
| `workflow_dispatch`      | `gate`, and `parity`/`bench` when dispatched by hand       |

For now CI runs unit tests only: the claim, docs, records and tooling checks are not on the automatic
path, and neither is the corpus measurement. `scripts/check.sh` is still the full local gate, and
`just check` is how to run it.

The Rust half of `gate` is the Tauri host package only — `cargo test --manifest-path
src-tauri/Cargo.toml`, the same invocation `scripts/check.sh` runs and the one CI demonstrably kept
green. The workspace crates' unit suites are not gated yet: three of their test files resolve
`fixtures/corpus` one directory too high (`src-tauri/fixtures/corpus`, which exists nowhere), and they
only surfaced now that `gate` asked for them. A gate that fails on a path bug it just discovered is not
coverage, it is noise; enabling those suites is its own repair task, with those test files as its
starting surface.

## The hooks, and what they are not

`just hooks` sets `core.hooksPath` to `.githooks`. They exist to keep a push from spending CI minutes on
something a local command would have caught:

- **`pre-commit`** (about a second): `cargo fmt --check` when a Rust file is staged, `check-docs.sh` when
  a markdown file is staged, and the parity measurer's build when the harness or a parser moved — that
  crate lives outside the workspace, so nothing else compiles it until someone dispatches `parity`.
- **`pre-push`** (minutes, on demand): `./scripts/check.sh`, the full local gate — claim, docs,
  records, tooling, biome, svelte-check, `cargo check`, `cargo test` and `pnpm test`. CI now runs only
  the unit tests, so this is where the documentary and records checks run before a push, not a second
  copy of CI.

They are **not** the gate. The gate is `scripts/check.sh`, and `pre-push` runs it on demand; a hook can
be skipped with `git push --no-verify`, and a hook that took as long as CI would be the first thing
anyone skips. That is exactly why CI, not the hook, is the thing that cannot be talked out of the unit
suite: a skipped step in `pre-push` says so, with the reason, rather than passing quietly, and the unit
suite still runs on every push and pull request to `master`.
