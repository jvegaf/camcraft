# Git workflow

Two trunks. `dev` is where work integrates; `master` is what has been released. The split exists for
one reason and it is a clock: the corpus measurement costs minutes, and running it on every save spends
them without answering anything a release has not already asked.

## The two trunks

| Trunk | Receives | Runs in CI |
| --- | --- | --- |
| `dev` | every integrated change | `gate` always; `parity` only when the push touches a parser, the kernel, the corpus or the harness; no benchmarks |
| `master` | **only** pull requests from `dev`, merged and tagged as a release | `gate`, `parity` and `bench`, always |

`master` is protected (`enforce_admins`, no force-push, no deletion, conversation resolution required)
with `gate`, `parity` and `bench` as required status checks and `strict` on, so a release pull request
must be up to date and green before it can merge. The three job names in `.github/workflows/ci.yml` are
load-bearing: renaming a job silently unrequires it.

## Day to day

Work happens in a claimed worktree on a branch off `dev` (`AGENTS.md`, `just claim N`). A task branch is
merged into `dev` when its claim is delivered; `dev` is then pushed, and the push runs the cheap gates.

```bash
just hooks          # once per clone: installs .githooks/pre-commit and .githooks/pre-push
just claim N        # in the task's worktree, before the first write
# … work, commit in reviewable slices …
just deliver N <commit>   # records the delivery in odd/tasks/INDEX.md
git push origin dev
```

## A record, before anything is called done

`just deliver N <commit>` is the moment a task is called finished, so it is the moment the record is
proved: `scripts/deliver-task.sh` runs `scripts/check-records.sh --task N --engram-required` first and
writes nothing if the document is incomplete, the commit does not resolve, or Engram holds no memory
naming the task's slug. `just check` runs the same verifier as `== records ==` — without the Engram
requirement, because CI has no memory daemon — and reports documents written before the convention as
advisories instead of failing history.

## A release

`master` moves only through a pull request from `dev`, and a release is that merge plus a tag.

```bash
gh pr create --base master --head dev --title "Release v0.X.0" --fill
# the PR runs gate + parity + bench; merge it once they are green
git switch master && git pull
git tag -a v0.X.0 -m "v0.X.0"
git push origin v0.X.0
gh release create v0.X.0 --generate-notes
```

A tag is what makes `master` mean something: without one, the release trunk is just a slower
integration trunk, and the two branches cost twice without buying anything.

## The hooks, and what they are not

`just hooks` sets `core.hooksPath` to `.githooks`. They exist to keep a push from spending CI minutes on
something a local command would have caught:

- **`pre-commit`** (about a second): `cargo fmt --check` when a Rust file is staged, `check-docs.sh` when
  a markdown file is staged, and the parity measurer's build when the harness or a parser moved — that
  crate lives outside the workspace, so nothing else compiles it before CI does.
- **`pre-push`** (minutes, on demand): `clippy -D warnings` and `cargo test --workspace` when the pushed
  range touches Rust, and `scripts/parity/run.sh` when it touches what the corpus measures.

They are **not** the gate. `scripts/check.sh` is the gate and CI runs it; a hook can be skipped with
`git commit --no-verify`, and any hook that took as long as the gate would be. A skipped step in
`pre-push` says so, with the reason, rather than passing quietly.

## Why not a single trunk

A single trunk with path-filtered jobs buys most of the same minutes. It was not chosen because a
release needs a place where "green" means "measured", and because the trunk that is always green is the
one a release can be cut from at any moment. If the tag practice ever stops, the honest move is to
collapse back to one trunk rather than to keep a second branch for its own sake.
