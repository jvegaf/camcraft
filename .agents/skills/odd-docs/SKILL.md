---
name: odd-docs
description: Procedure for keeping a project's ODD documentation organized - naming and numbering task documents, splitting an oversized doc, sweeping references after a rename, and proving the result instead of asserting it. Use when creating or closing a task document, when a document grew past what a human can read, when two documents say the same thing, or when a move or rename touches files other documents link to (renumerar tareas, partir un documento, reorganizar documentación, enlaces rotos).
---

# ODD documentation discipline

A procedure, not an authority. It says **how** to name, split, move and verify; it never decides what a
document should say, and it never competes with the repository's own conventions: when the repository
already documents a rule, follow the repository and use this only for the steps it does not state.

## When this applies

- Starting or closing an ODD task document.
- A document has grown past what a human can read, or two documents say the same thing.
- Anything that renames or moves a file other documents link to.

## 1. Naming and numbering a task document

- One document per feature, named `NNN-slug.md`: three digits, a hyphen, a stable kebab-case slug.
- **The number records when the feature entered the project**, not its importance and not its theme: sort
  by first commit (`git log --diff-filter=A --format=%ad --date=short -- <path>`), and break ties by the
  previous filename so the order is reproducible rather than invented.
- **Numbers are immutable.** A new task takes the next free number; nothing is ever renumbered or reused.
  A number quoted in a commit, a review or a code comment must still point at the same document a year
  later.
- **Keep the slug stable.** It is what people grep for, with or without the number in front.
- The document has four blocks, in this order: a two- or three-line header; `## Specs` with the owner's
  words quoted verbatim and numbered `S1`, `S2`…; `## Tasks`, one line per task with the specs it covers
  and the commit that holds it; `## Log` last, where `L1` is the original request verbatim and later
  entries are corrections, measured evidence and decisions. A reader who works from the document reads
  until `## Log` and does not need the rest.
- Keep one **index** — a table of number, document, status, first date, last commit — as the single entry
  point to the folder. It is the file someone opens instead of running `ls`.
- A requirement change appends its verbatim entry to the log, rewrites only the affected `S#`, and
  reopens only its own task.

## 2. Splitting or consolidating a document

- Extract **by heading, never by line range**. Sections get appended out of order as a file grows, so a
  range copied from `grep -n` is wrong the moment somebody appends.
- **Prove preservation, do not assert it.** Compare the sorted multiset of the original's non-empty lines
  against the concatenation of the new files. Report both counts and name every line that changed form,
  with the reason. "Preserved verbatim" without a count is a claim, not evidence.
- **One table, one home.** If a split would duplicate a table "to preserve the original", fold it instead:
  keep the readable copy, move the unique prose, and say what moved. Duplication is how a file becomes
  unmanageable in the first place.
- Name the new files so they sort the way a reader expects — zero-pad the numbers (`m00`…`m15`, not
  `m0`…`m15`) — and start each with a title and a link back to its index.
- History goes to an archive directory, never to `/dev/null`. An archived document keeps its content and
  gets one added paragraph saying what supersedes it.

## 3. Sweeping references

- Rewrite **path-qualified references only** (`<dir>/<name>`). A bare filename is ambiguous: two
  directories can hold different files with the same name, and a blanket replacement silently rewrites
  the wrong one across dozens of documents.
- Keep link text truthful. A link whose label names the old path while its target is the new one is a lie
  with a working target; prose that names a moved path needs the same sweep as the links.
- A frozen archive directory is frozen: fix its link depth when the world around it moves, never its prose.
- Move files with `git mv`, so history follows them.
- Run the repository's markdown link check and show its output. **If the repository has no such check,
  that is the first thing to add** — a rename that forgets one reference is invisible otherwise, and the
  check costs one script and earns a gate.

## 4. Before returning

State: documents created or moved, the preservation counts, the link-check output, and every line that
changed form with its reason.

## Non-goals

- It does not decide content, scope, priority, or which directory product documentation belongs in.
- It never renumbers an existing task, never reuses a number, never rewrites archived prose.
- It never duplicates what the repository's own convention documents already state: it points at them.
