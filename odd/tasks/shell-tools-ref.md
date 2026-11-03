# shell-tools-ref — the window resolves the library its document names

Both features that gave the window a document to work with ended by naming the same hole: a document names
its tool library by a filename (`Document::tools_ref`), `cam-cli` resolves it, and the window does not — so
the tool card has nothing to resolve against and says so. This closes it, and it does so with one rule the
CLI does not have.

## Specs

- **S1 — the decision.** The owner's words, verbatim: *"me parece bien pushea y continua con resolver"*,
  answering the parent's *"seguimos con resolver `tools_ref` al abrir, que es el hueco que quedó declarado en
  las dos features"*.
- **S2 — the resolution.** On a successful open, the shell resolves the loaded document's `tools_ref`
  **relative to that document's own directory** — which is what `docs/format.md:105` already says the field
  is — loads it through a **read-only** open, and hands the entries to `set_tools`. A document with no
  `tools_ref` leaves the snapshot empty, exactly as today. This is the same join `cam-cli` performs
  (`cam-cli/src/main.rs:1828-1844`), with one difference the CLI does not have: the window **never writes a
  library**, so the open cannot create, migrate or touch one.
- **S3 — containment, which the CLI does not have.** A reference that is absolute, or that escapes the
  document's directory through `..`, is **refused**: the window reads nothing outside that directory and
  creates nothing anywhere. This is not a tightening of the documented rule but its implementation — a
  reference that leaves the directory is not a reference relative to it. The guard also matters because
  `ToolLibrary::open` (`cam-store/src/tools.rs:219-241`) creates the parent directory and the file when the
  path does not exist, so a hostile document could otherwise make the window write into arbitrary paths. The
  window checks existence before opening, so a library that is not there is never created by looking at it,
  **and it opens read-only, so looking at one never writes it either** — which is what closes the case the
  verifier found in L4, a symlink beside the document pointing at an existing file outside it.
- **S4 — a library that cannot be used does not cost the user the drawing.** Missing, unreadable, or not a
  library: the document still opens, the snapshot stays empty, and the status line says the library could not
  be used and why. The card already names an unresolved tool in its own words
  (`cam-ui/src/chrome.rs:820-835`), and the CLI itself is soft about this for `toolpath`
  (`cam-cli/src/main.rs:1634-1659`, `Ok(None)` justified at `:1632-1633`) while refusing it only where the
  tool data is load-bearing (`emit`, `simulate`).
- **S5 — what must not change.** `cam-cli`'s resolution rule and its exit codes; the open and save refusals
  as they stand; the card's existing sentences; and `cam-ui`'s synchronous I/O on the caller's thread, which
  is what `DocumentFile::load` already does inside `apply_action`.
- **S6 — risk.** Task Size item 2: a document naming a path the window reads is a security surface, and one
  of the two paths it could name would create files. Contained by S3.
- **S7 — recorded, not fixed.** `cam-cli` still accepts an absolute `tools_ref` and one that escapes through
  `..`, and nothing in the repository pins that either way: every CLI test sets an absolute `tools_ref`, so
  the rule is untested. Fixing that changes a CLI behaviour and its tests, which is a separate decision, so it
  is named here and left visible.

## Tasks

- T1 — the resolver in `cam-ui` (S2, S3, S4): containment, existence guard, soft failure message, the snapshot
  it sets, and the tests — including a document whose `tools_ref` tries to leave the directory. Route:
  delegated worker.
- T2 — the documents (S1, S2, S7): `README.md`, both shell records, and the two feature documents that named
  this as their remaining gap. Route: delegated worker.
- T3 — gate, commits, independent verification, native review (S6). Route: parent. Gate and independent
  verification: L5. Native review: **closed, not pending** — see the last paragraph of L5.

## Log

- **L1 (the request, verbatim).** *"me parece bien pushea y continua con resolver"*, after the save feature was
  pushed at `a1069f2`.
- **L2 (reconnaissance, before any write).** The join is `.parent().unwrap_or(".").join(reference)`, written
  three times in the CLI — `emit` (`cam-cli/src/main.rs:1828-1844`), `simulate` (`:1924-1942`) and
  `open_library_hint` (`:1634-1659`) — and the last one is the only one that guards with `path.exists()`
  before opening, which is exactly the guard that keeps `ToolLibrary::open` from creating the file. The
  library is `cam_store::tools::ToolLibrary` with `open` (`:220`), `tools()` (`:511`) and
  `stored_tool_for_number` (`:620-624`); the shell stores `Vec<StoredTool>` (`cam-ui/src/lib.rs:835`) and
  exactly two consumers read it, both in `chrome.rs` (`:806` the card, `:839` the cutting-time estimate).
  **The finding that shaped the design:** nothing in the repository forbids `tools_ref` from being absolute,
  empty, or `..`-relative — no guard, no test, no doc — and `Path::join` with an absolute argument discards the
  base, so the CLI's rule lets a document read anything the process can read and, through `open`'s
  `create_dir_all`, create it. That is why S3 exists.
- **L3 (the resolver, and the hole in my own spec).** The unit landed `tools_beside_document`: it refuses an
  absolute or `..`-bearing reference **before any filesystem call**, checks `try_exists` before
  `ToolLibrary::open` (so a document can never make the writer create what it names), and hands
  `tools()` to `set_tools`. Failure is soft — the drawing opens, the snapshot stays empty, and the open's
  own sentence is kept byte-identical with an appended " — <library> could not be used: <why>". RED was
  observed for three of the six new tests before the fix (`["TOOL — Profile 01", "T4 is not in the tool
  library this window was given…"]`), and both hostile tests failed on their guard; `shell.rs` went 98 → 104
  and the card now draws `T4 - Tool (6mm)` with a real estimate instead of "No estimated cutting time".
  **The hole it found in S3, recorded as a limit and not papered over:** containment is **lexical**, so a
  symlink *inside* the document's directory that points outside is still followed and read. It cannot
  *create* anything there, because `try_exists` follows the link and a dangling one reads as absent, so the
  write surface is closed; what remains open is reading a file that whoever owns the directory put there.
  Canonicalising would close that read and would also refuse a legitimate shared-library symlink, which is a
  real arrangement in a shop, so the rule stays lexical and the trade is stated rather than taken silently.
  Three cases stay unpinned and are named rather than implied: `sub/../tools.sqlite` is refused although it
  lands back inside (deliberate, documented on the helper, no test), an unreadable file or directory is not
  tested (only "missing" and "not a library"), and `Some("")` resolves to the document's own directory and
  fails softly through SQLite. The parent also fixed the host's own comment, which still said resolving
  `tools_ref` was "the next step".
- **L4 (L3's write claim was false, and the verifier broke it).** L3 said the write surface was closed because
  `try_exists` follows a symlink and a dangling one reads as absent. That covers **creating**. It does not cover
  **writing into a file that already exists**, and the verifier reproduced exactly that: a symlink beside the
  document pointing at an existing zero-length file outside it passes `try_exists`, `ToolLibrary::open` runs,
  and `migrate` writes its schema into the outside file — **0 → 73 728 bytes**, silently, with the status
  `open file — opened cut.c2d` and no note. A hostile `.c2d` could therefore write into any existing file
  outside its own directory. It was **caused by this feature**: at the baseline the window opened no library at
  all.
  **The fix is not another path guard.** Canonicalising would close the write and also refuse the shared-library
  symlink L3 chose to keep working. `cam-store` gained `ToolLibrary::open_read_only` — `SQLITE_OPEN_READ_ONLY`,
  no `create_dir_all`, no `migrate`, with a version gate in the constructor — and the resolver uses it. The
  window now reads a library and cannot write one, through a symlink or otherwise, and the TOCTOU between
  `try_exists` and the open stops mattering for writes because there is no write to race into.
  **The trade, stated because it is real:** read-only refuses what `open` used to accept by writing — a file
  that was never initialised, a library **below head** (a legacy library a user still has, refused with a
  message naming both versions), one above head, and a missing path. All of them are soft in the window (the
  drawing opens, the card says the tool is not in the library), and `cam-cli` is untouched because it keeps
  calling `open` and still migrates. Migrating a legacy library from the window is therefore impossible by
  design, and that is the price of never writing.
  **Evidence:** the blocker test failed before the fix with `left: 73728 right: 0` and passes after; `cam-store`
  went 7 → 10 tests and `shell.rs` 104 → 108, with no existing test moved. The other three probes the verifier
  handed over — the climb-back refusal, the unusable-reference soft path and the stale-card class — are pinned
  now too, and the stale-card one closed a gap neither the spec nor the unit had thought about: a card from the
  previous document must not survive an open whose library fails to resolve.
- **L5 (T3: the gate, the independent verification, and the one advisory it produced).**
  **The gate:** `./scripts/check.sh` with `XDG_CONFIG_HOME` set, **exit 0** — `fmt --check`, `clippy -D
  warnings`, and the whole suite: **96 suites, 1 211 tests, 0 failed** (up from 1 166; `cam-store` 7 → 22 and
  `cam-ui` 186, of which `shell` 108). Before it, `pids.max` was 64 again and was raised to 4096 — the recurring
  cgroup gotcha, per-session, not durable.
  **The commits:** `aa26315` the resolver (`cam-ui/src/lib.rs` +104, `tests/shell.rs` +260, and the host's own
  comment, which still called resolving `tools_ref` "the next step"); `6134c1a` T2, the documents; `40625fa`
  `open_read_only`, which closes L4's hole; `5d77d77` the S2/S3 wording and L4; `75e5174` the allowlist, where a
  reviewer found that `is_absolute()` plus a `ParentDir` check lets two **Windows** shapes through —
  root-relative (`\drv\file`) and drive-relative (`C:file`) are neither absolute nor carry `..` — and the rule
  became "every component is a plain name or `.`", which refuses those by construction.
  **The independent verification** (a fresh read-only `gentle-ai-verify`, run after the incident below): **S1–S6
  met, S7 still true.** It executed the hostile cases rather than reading them: an absolute reference and
  `../outside.sqlite` pointed at *existing* outside libraries are refused with `it is not inside the document's
  directory` and the outside bytes are untouched; a symlink to a 100-byte non-library leaves 100 bytes; a
  hardlink to a 0-byte file leaves 0; a missing library reached through a *directory* symlink creates nothing;
  a genuine hand-built v1 library is refused naming both versions with its bytes identical (no migration), and
  so is a v3 one. It also confirmed `open_read_only` has `SQLITE_OPEN_READ_ONLY`, no `create_dir_all`, no
  `migrate` and a version gate (`cam-store/src/tools.rs:259-296`), and that `git diff a1069f2..HEAD -- crates/cam-cli
  crates/cam-ui/src/chrome.rs` is **empty**, so S5's "what must not change" did not move.
  **The refinement it made to S7:** the absolute half *is* incidentally pinned — `cam-cli/tests/cli.rs:974-995`
  and `:1017-1046` set an absolute `tools_ref` and require exit 0 — so "nothing in the repository pins that
  either way" was slightly overstated; the `..` half is pinned nowhere, which is what S7 actually needs decided.
  **F1, the advisory it produced, and it is caused by this feature:** a library in **WAL** mode reached through
  a symlink beside the document still makes the window **create files outside the document's directory** —
  `waltools.sqlite-shm` (32 768 bytes) and `waltools.sqlite-wal` (0 bytes) appear beside it. The library's own
  bytes are identical (73 728 → 73 728), the status stays clean (`open file — opened cut.c2d`), and L4's
  `0 → 73 728` write does **not** reproduce: these are SQLite's own WAL companions, not a write into an existing
  file, and no library campilot produces is in WAL mode. It does literally contradict S3's "creates nothing
  anywhere", so it is recorded as an open decision rather than closed by silence. Two candidate closes, each
  with a cost: `immutable=1` (reads can then be **stale**, because WAL content is ignored — the opposite of what
  a shop with a live library wants) or refusing a WAL library outright (a restriction, but an honest and total
  one). Not fixed here.
  **Not executed, and said rather than implied:** the Windows `Component` shapes were verified by reading the
  guard, not by running them — this build has no Windows std and no network. The allowlist refuses `RootDir`
  and `Prefix` by construction, which is the strongest thing a Linux-only build can claim.
  **The incident that cost the first verification run:** the cgroup OOM killer killed the whole unit at
  01:03 (`Failed with result 'oom-kill'`, 3 G peak) while the gate and the verifier were both running cargo
  against a `memory.max` of 3 GiB with `memory.swap.max` = 0, and the session restarted, taking the first
  run's findings with it. The limits are per session and come back at every restart: memory 3 → 6 GiB, swap
  0 → 2 GiB, pids 64 → 4096. **Never run two cargo invocations at once in this box**; the gate is parallel
  inside itself.
  **The native review of the guard is closed, and was not re-run.** `assess` over the committed candidate
  `5d77d77..HEAD` — the allowlist alone, 1 path, 12 lines — answers `nativeRisk: high` with
  `reviewDue: false` / `already_reviewed`, `nativeReviewOutcome: closed`, `outcome_source: derived` and
  `candidate.consumed: true`. That is the native candidate fact, not a reading of the log: the review for this
  tree was approved and acknowledged already, and the finding it produced **is** `75e5174`. Starting a second
  review of the same tree would have burned authority to learn what the store already records, so none was
  started. What the review's advisory list was is not recorded here, because the authority does not expose it
  after the burn; the one finding that mattered became code.
