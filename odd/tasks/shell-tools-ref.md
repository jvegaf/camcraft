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
  is — loads it through `ToolLibrary::open`, and hands the entries to `set_tools`. A document with no
  `tools_ref` leaves the snapshot empty, exactly as today. This is the same join `cam-cli` performs
  (`cam-cli/src/main.rs:1828-1844`).
- **S3 — containment, which the CLI does not have.** A reference that is absolute, or that escapes the
  document's directory through `..`, is **refused**: the window reads nothing outside that directory and
  creates nothing anywhere. This is not a tightening of the documented rule but its implementation — a
  reference that leaves the directory is not a reference relative to it. The guard also matters because
  `ToolLibrary::open` (`cam-store/src/tools.rs:219-241`) creates the parent directory and the file when the
  path does not exist, so a hostile document could otherwise make the window write into arbitrary paths. The
  window checks existence before opening, so a library that is not there is never created by looking at it.
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
- T3 — gate, commits, independent verification, native review (S6). Route: parent.

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
