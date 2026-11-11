---
name: camcraft-docs
description: Procedure for this repository's documentation - starting an ODD task document, numbering it, splitting or consolidating an oversized doc, sweeping renamed references, and keeping the markdown link gate green. Use when creating or closing a task doc, when a document grew past what a human can read, when two documents say the same thing, or when a rename/move touches files other docs link to (renumerar tareas, partir un documento, reorganizar documentación, enlaces rotos).
---

# camcraft documentation discipline

Three contracts hold this repository's documentation up, and they live in the repo, not here:
`odd/README.md` (task-document shape and numbering), `odd/tasks/INDEX.md` (state of every feature),
`docs/ROADMAP_CAMCRAFT.md` + `docs/SYSTEM_DESIGN_CAMCRAFT.md` (the plan). This skill is the procedure
that keeps them true; it never restates them.

## When this applies

- Starting or closing an ODD task document.
- A document has grown past what a human can read, or two documents say the same thing.
- Anything that renames or moves a file other documents link to.

## 1. A new task document

1. Read `odd/README.md` and `odd/tasks/INDEX.md`.
2. Take the **next free three-digit number**. Numbers are immutable: never renumber, never reuse, never
   reorder by theme. A new task always lands at the end of history.
3. Write `odd/tasks/NNN-slug.md` with the four blocks: header, `## Specs` (the owner's words verbatim,
   numbered `S#`), `## Tasks`, `## Log` where `L1` is the request verbatim.
4. Add its row to `odd/tasks/INDEX.md`.
5. Record the commit that holds each task in `## Tasks`, and state only the status the document's own
   text supports.

## 2. Splitting or consolidating a document

- Extract **by heading, never by line range**. Sections get appended out of order as a file grows: the
  old `odd/roadmap.md` had its §4.16–§4.18 *after* §8, so any `sed -n '156,1034p'` lies.
- **Prove preservation, do not assert it.** Compare the sorted multiset of the original's non-empty
  lines against the concatenation of the new files; report both counts and name every line that changed
  form, with the reason.
- **One table, one home.** If a split would duplicate a table "to preserve the original", fold it
  instead: keep the readable copy, move the unique prose, and say what moved. A duplicated table is how
  a file becomes unmanageable in the first place.
- Name the new files so they sort the way a reader expects (`m00`…`m15`, not `m0`…`m15`), and start each
  one with a title and a backlink to its index.
- History goes to `docs/old/` or `odd/archive/`, never to `/dev/null`.

## 3. Sweeping references

- Rewrite **path-qualified references only** (`odd/tasks/<name>`, `docs/old/<name>`). A bare filename is
  ambiguous: `docs/old/m7-vcarve.md` shares its name with a task document, and a blanket replacement
  rewrites the wrong file in dozens of documents.
- Keep link text truthful. A link labelled `odd/roadmap.md` whose target is now `odd/roadmap/index.md` is
  a lie with a working target, and prose that names a moved path needs the same sweep as the links.
- `docs/old/` is a frozen snapshot: fix its link depth when the world around it moves, never its prose.
- Move files with `git mv`, so history follows them.
- Run `./scripts/check-docs.sh`; it must print `markdown links ok`. It skips a tracked-but-deleted path
  deliberately, and it decodes percent-encoded targets. It is **not** part of `scripts/check.sh` yet, so
  nothing else runs it for you.

## 4. Before returning

State: files created or moved, the preservation counts, the gate output, and anything that changed form
and why. "Preserved verbatim" without the count is a claim, not evidence.

## Non-goals

- It does not decide content, scope, priority or where product documentation belongs.
- It never renumbers an existing task, never reuses a number, never rewrites `docs/old/` prose.
- It never duplicates what `odd/README.md`, `INDEX.md` or the roadmap already own.
