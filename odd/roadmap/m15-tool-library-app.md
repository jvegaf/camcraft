# M15 — tool library app

[Back to index](index.md)

### 4.18 M15 delivered (evidence)

The screen was finished and unreachable, which is the worst of both: §4.17 measured the layout, the tree, the
form and the preview card, and none of it could ever be filled. `writes` in the modal was
`tool_library_path.is_some()` (`crates/cam-ui/src/tool_library/dialog.rs`), and the only thing that set that path
was `Shell::set_tools_from` when an opened `.c2d` named a `tools_ref` whose `tools.sqlite` already existed. No
code in the window created that file and no code wrote `tools_ref` into a document, so the six action rows were
permanently dark and the populated screen was reachable only by hand-building a SQLite file with `cam-cli tools
add` and editing a document's JSON to point at it.

**The library is the application's** (`odd/tasks/032-tool-library-app.md`, S1; the owner's words, D1). One
`tools.sqlite` in the data directory — `$CAM_PILOT_DATA_DIR` as the only override, else
`$XDG_DATA_HOME/campilot` / `~/.local/share/campilot` on Linux and `%APPDATA%\campilot` on Windows — opened and
seeded at launch, so a fresh install starts with a catalogue instead of an empty screen, and a library a person
emptied stays empty because seeding is a first-launch answer and not a repair. Opening a document no longer
resolves anything: `tools_ref` stays in the model and `cam-cli` keeps honouring it, because the field is a CLI
contract and the ownership rule is the window's. That deleted `tools_beside_document` and the whole ladder that
came with it — lexical containment, the symlink case, the WAL companion refusal, ten tests in
`crates/cam-ui/tests/shell/tool_library.rs` — and replaced it with four tests about ownership, one of which is
the one that used to be impossible to write: an application whose library loads keeps it across an open.

**The catalogue went from four tools to the reference's 22** (S3), T1 to T22 across the six kinds, each number
used once. Speeds and feeds are derived by one rule instead of hand-tuned per tool (S4): feed 100 mm/min, plunge
30, spindle 18000, depth of cut half the diameter capped at 3 mm, stepover 40 % of it. The three tapered ball
tools keep their taper angle in the name only, because the store's corner-radius shape has no taper field —
recorded rather than faked, and the one place the catalogue is less than the reference.

**Import asks before it writes** (S5, the owner's requirement): the file is read and validated first, so a file
this build cannot use still asks nothing and says why; then `Replace all`, `Add` or `Cancel`. `Add` skips a tool
the library already holds by id, by trimmed case-insensitive name, or by tool number — a pure function in
`cam-store` with one test per key — so importing the same file twice is a no-op that says so.

**The dialog took the reference's proportions** (S7): columns 700 tall rather than 372, and the preview card
330 rather than 172, so the tallest family's form ends inside its column and the fit is pinned per kind rather
than eyeballed. The card is painted the way the reference paints it — light, with the tool in dark grey and the
callouts in the accent blue — through three palette roles defined in both themes.

Two defects the work itself produced, both found before the commit that carried them. The import question's
buttons were first proved by calling the shell's answer methods, which is a test skipping the control it claims
to test; pressing the reported rectangle showed why it had been tempting — an egui modal is placed on its first
pass and settled on the second, so the rectangles reported in the frame the question appears are not the ones a
person clicks. The fix is the one this module already keeps: report the frame that was drawn, let the test
settle a frame before it reads the rectangle. The follow-up that fixed it had first tried the opposite — keep
the first frame's rectangles and resolve clicks against those — which would have shipped a control that answers
where it used to be.
