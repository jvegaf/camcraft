# Debt: the fourteen open review advisories

Feature document for the advisory debt recorded in `odd/tasks/m10-documents.md` L9 and in Engram
observation #165. Read **until `## Log`** before touching code; the Log is the record, not the brief.

## Specs

- **S1 (user, verbatim answer to "¿Por dónde arranco la deuda?"):** "Los 14 advisories".
- **S2 (user, verbatim answer to the store question):** "Sí, solo lectura" — read-only access to the
  review authority store under `.git/gentle-ai` for the findings' text; nothing there is written or
  staged.
- **S3 (consequence of S1, not a user string):** an advisory can only be *closed as resolved* when its
  finding's subject is established. A finding whose reviewer text is unrecoverable and whose anchor no
  longer identifies its subject is **not** closed by a plausible-looking edit: it stays open, named as
  unrecoverable, and is re-filed by a review of the current tree if the owner wants it back.
- **S4 (repo law that binds every task here):** a fix changes the diagnostic or the wording it was
  filed against, never the acceptance of valid input. The one exception is refused below rather than
  smuggled in (T5's step floor is the owner's decision, not a readability fix).

## Tasks

| ID | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | The quoted-token `end_column` off-by-one: a missing argument after a quoted token reports the column *inside* the token instead of one past it | S1 S3 | parent | `68adb00` |
| T2 | The CSV quote check runs before the field-count check, so a quoted field containing a comma is refused as a wrong field count instead of the promised quoted-field refusal | S1 S4 | parent | `106ab8b` |
| T3 | `const HELP`'s scripted `toolpath` and `nest` lines claim to take "every other" flag while the script refuses `--out` and a positional document | S1 S4 | parent | `68adb00` |
| T4 | The `script_panel` test fixture joins a caller-supplied name onto `/var/tmp` and `remove_dir_all`s the result, so a name with a separator escapes the scratch directory | S1 S3 | parent | `3d6589f` |

## Log

- **L1 (user, verbatim):** "si, la deuda".
- **L2 (store reconnaissance, delegated; the findings' provenance).** The review authority store at
  `.git/gentle-ai` holds **three** complete lineages (`review-61e0b405dc7f498b`,
  `review-305eaa8746f90d29`, `review-3592c554bf15c02f`) and only *terminal-consumption* records for
  `bd99706d`, `12219c3f` and `1ab52143` — id, lineage and three hashes, no findings. So **10 of the 14
  advisories exist only as metadata** (lens, severity, path:line) in `docs/m10-documents.md:355-370`,
  `odd/tasks/m10-documents.md:160`, `docs/m9-nesting.md:392-396`, `odd/roadmap.md:813,899`.
  The reviewer's own words survive for the four below, through `review-61e0b405dc7f498b`, documented as
  `bd99706d`'s superseded predecessor over the same code with shifted lines:
  1. *readability WARNING* — "`end_column` is computed as `token.column + token.text.len()`, but for a
     quoted token `Token::column` points at the opening quote while `text` excludes the quotes… reports
     a column one short". Verified live: `crates/cam-cli/src/script.rs:491-493`, lexer sets
     `column: start + 1` at the `"` (`script.rs:452-486`). **T1.**
  2. *readability SUGGESTION* — "The new script help says `toolpath` takes 'every other toolpath flag',
     but the scripted command deliberately refuses `--out` (and a positional document path) with
     `NO_OUT`/`NO_POSITIONAL`. The help therefore misstates the accepted options". Verified live:
     `crates/cam-cli/src/lib.rs:172`; `NO_OUT`/`NO_POSITIONAL` at `script.rs:42,45`; `nest`'s help line
     carries the identical false claim. **T3.**
  3. *reliability WARNING* — "The field-count check runs before the quote check, so a quoted field
     containing a comma, a common spreadsheet export, is refused as a field-count error instead of the
     promised quoted-field refusal". Verified live: `crates/cam-import/src/csv.rs:175-183` (count)
     precedes `:185-196` (quote scan). **T2.**
  4. *risk WARNING* — `crates/cam-ui/tests/script_panel.rs:29-31`, the `scratch` fixture: a hard-coded
     `/var/tmp` joined with a caller-supplied `name`, then `remove_dir_all`. Text unrecovered; the
     subject is the code's own shape, so T4 fixes the escape and does **not** claim the finding by name.
- **L3 (why the other ten stay open).** `bd99706d/R2-001`, `R2-003`, `R2-005`, `R2-006`, `R3-002`,
  `R3-003`, `12219c3f/R1-001` (identity, once T4's subject is read off the code), `1ab52143/R1-STEP-DOS`,
  `R2-002`, `R2-003`, `R3-002`: without the reviewer's text their authors' subject cannot be
  established, and three anchors are worse than stale — `crates/cam-cli/src/main.rs` was split by
  `6cc3b2d`, so `main.rs:162-163`, `:860` and `:3003` name a file that is now 16 lines long. Two M9
  anchors (`cam-geom/src/nest.rs:348`, `nest_profile.rs:241-246`) land on **doc comments**, so the
  finding is a doc-versus-behaviour claim nobody can now read. Guessing a fix for those would manufacture
  a resolution the record cannot support (S3).
- **L4 (deferred to the owner, not fixed here).** The one M9 advisory whose subject *is* inferable from
  the code: `crates/cam-geom/src/nest_profile.rs:171-175` refuses a step that is not finite or not
  positive, and sets no floor, so `--step 0.001` pays an unbounded lattice (`lattice_steps` at `:537`,
  used at `:521-522`). A floor **refuses input that used to be accepted**, which is a contract change —
  the owner's call, not a readability fix (S4).
- **L5 (T1-T4, evidence).** **T1**: `lex` now records `end_column` as each token is pushed — the quoted branch adds its two quotes — instead of deriving it from the last token's `column + text.len()`. RED first: `sheet "100 200"` reported `line 2:14` (inside the token) where the law says `line 2:16`; after the fix `crates/cam-cli/tests/script.rs` is **21 passed**, including the pre-existing unquoted case (`sheet 100` → `line 2:10`), so the behaviour the new test does not cover is pinned and unchanged. **T2**: the quote scan moved ahead of the field-count check, with a line-level fallback when the quote lands past the last column the format has (`COLUMNS[index]` would otherwise index out of bounds on the row it is refusing). RED first: `"post, left",120,80,4` answered `expected 4 fields (found 5)` where the format's own message is the quoting refusal; after, `crates/cam-import` is **30 passed**, and the two quoting tests that predate the change (`"bracket"`, `brack"et`) still print their original message. **T3**: `const HELP` now says what the scripted `toolpath` and `nest` refuse and why, and `the_script_help_says_which_destinations_a_scripted_command_refuses` pins both claims against a whitespace-collapsed help, so the wording is asserted without coupling the test to `HELP`'s indentation. **T4**: `scratch` refuses a name that is not one path segment (`/`, `\\` or `..`), so the fixture's own `remove_dir_all` cannot be aimed outside its directory; `cam-ui`'s `script_panel` is **6 passed**.
- **L6 (the differential, because `split` is shared code).** `cam-cli parts --csv` is a command this request never named and it reads part lists through the same `split` a scripted `parts` uses, so the previous code (`077f48e`, built from a scratch worktree) and this change both ran it over five fixtures, comparing stdout, stderr, exit code and the document each run wrote. `plain` (part authored, exit 0), `count` (3 fields, exit 2) and `quoted` (a quoted field with no separator, exit 2, field-level message) are **byte-identical**. `quoted-comma` and `past-columns` differ only in the message the fix names — the count error became the quoting refusal, exit code `2` in both. `VERDICT=no regression in a command the request did not name`. The harness lives in `/var/tmp/campilot-differential.sh`, outside the repository, and is not committed.
- **L7 (the gate, and why nothing is committed).** `./scripts/check.sh` ran `fmt --check`,
  `clippy -D warnings` and the whole suite: **all gates green, `CHECK_EXIT=0`**. `cargo test --workspace
  -- --list` counts **1511 tests over 110 result lines**, up from M10's floor of 1508/110 — exactly the
  three tests this change adds (T1's quoted column, T2's quoting refusal, T3's help claim). The four
  fixes and this document sit **uncommitted** in the working tree: no commit was authorized, and the
  record of which advisories are closable belongs with them. `assess` could not measure the candidate —
  `nativeRisk: unassessable`, `changedPaths: 0` — because this document is untracked and native review
  demands an explicit untracked declaration, which is the trap the M10 record's L9 already names. Its
  plan is the writer's self-verification, which is the gate above: no independent verifier was required
  and none ran.

- **L8 (committed, owner-authorised).** Three work-unit commits, tests alongside the behaviour:
  **`68adb00`** (T1 + T3, the cam-cli script: the quoted-token column and the help's claim),
  **`106ab8b`** (T2, the CSV quoting refusal) and **`3d6589f`** (T4, the fixture's path segment), with
  this document in `docs(odd):` on top of them. Local only — **not pushed**, and pushing stays the
  owner's decision. L7's "uncommitted" describes the moment it was written, not the tree now.
