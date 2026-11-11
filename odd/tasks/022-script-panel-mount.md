# Mount the script panel: the shell's sixth surface, drawn as S9's modal

Feature document for wiring `cam_ui::script_panel::ScriptPanel` into the shell. Read **until `## Log`**
before touching code; the Log is the record, not the brief.

## Specs

- **S1 (user, verbatim option label chosen for "¿Dónde montás el ScriptPanel en el shell?"):** "Modal
  sobre el canvas (precedente S9)". The surface is a window over the canvas, not a new zone of the
  frame: the delivered artboards say *"Five zones and nothing else … The strip has no place left to be
  drawn in"* (`design/camui-shell/02-stages.html:358`), and the S9 tool library already set the
  precedent for a surface too large for a zone.
- **S2 (measured, not assumed):** the mount re-baselines **no goldens** — `cam-ui` keeps no shell
  snapshot, no `*.golden*` and no snapshot directory, and its tests assert geometry and text lists
  rather than pixels. Because the surface is a window, `output.canvas` does not move either, so no
  geometry test is expected to fail. A mount that did move the frame would have to answer for
  `the_palette_and_the_tools_tab_build_the_same_seven_actions` (`crates/cam-ui/tests/shell.rs:3941`),
  whose buttons only clear `available_height() * 0.55` (`chrome.rs:711`).
- **S3 (known limit carried by the panel, not introduced here):** `run_script_to` takes no `Document`
  (`crates/cam-cli/src/script.rs:164`), so a script run from the shell edits a file on disk and the
  shell's in-memory `Document` does not change. The mount shows that honestly instead of pretending
  otherwise; giving the panel the shell's document is a separate task with a `Document`-taking entry
  point behind it.
- **S4 (repo law that binds every task here):** one implementation per command, located diagnostics,
  and the panel is over `cam-script`, never a spawned CLI.

## Tasks

| ID | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | ✅ delivered (working tree) — The panel's body becomes bounded and scrollable, so a window can hold it: output that grows without limit cannot be a modal's content | S1 S4 | parent | working tree |
| T2 | ✅ delivered (working tree) — The shell grows one field, one entry point and one window: the panel opens from the palette, draws over the canvas, and leaves `output.canvas` untouched | S1 S2 | parent | working tree |
| T3 | ✅ delivered (working tree) — The record: `docs/m10-documents.md`'s known limit stops saying "not mounted", and this document says what the artboards do **not** draw | S3 | parent | working tree |

## Log

- **L1 (user, verbatim):** "continua con el cableado de" — the message was cut off; the only pending
  wiring in the session's record was `ScriptPanel` into `Shell`, and the owner confirmed the reading by
  choosing S1's option.
- **L2 (reconnaissance, delegated; and a correction to this session's own record).** `ScriptPanel` is
  constructed **nowhere** outside its own test file; `ScriptPanel::run` needs no `Document`, only a path
  and two sinks over one `Rc<RefCell<Vec<u8>>>`; its `ui` is one row of about 500 px and its
  `paint_text` emits a label per line with **no `ScrollArea`** and no height bound
  (`crates/cam-ui/src/script_panel.rs:196-247`). The shell composes its frame in `show_in`
  (`lib.rs:1691`), adding bars, sides and then the canvas (`:1744-1752`), with the regions sized by
  `theme.rs:371-375`. **The correction**: an earlier note — this session's Engram observation #165 —
  said the wiring "moves every existing shell golden". It does not: there are no pixel goldens to move,
  and the claim was corrected in Engram the moment it was measured.
- **L3 (why a modal, in the repo's own terms).** `design/system.md:18-22` enumerates five zones and
  `02-stages.html:69-70` fixes the grid (`200px 1fr 288px` by `40px 1fr 26px`); the right dock's 288 px
  is narrower than the panel's 380 px path field, and no board draws a script surface at all
  (`grep -ri script design/` is empty). A window is the one shape that adds the surface without
  amending the authority that says there is no room for it, which is exactly what S9 did for the tool
  library.
- **L4 (T1, bounded and scrollable — and the metric that made it testable).** The panel reported where it
  drew `Run` and nothing else, so a test could not say how tall its body grew. It now reports
  `body_size`, and `paint_text` runs inside `ScrollArea::vertical().max_height(BODY_HEIGHT)` with
  `BODY_HEIGHT = 240.0`. RED first: a script of `open` plus **80** `validate` lines built a body of
  **1357 points**, past the ceiling; after the scroll area the same script stays inside it. `cam-ui`'s
  script_panel target is **7 passed**, including the six promises that predate this task, so the panel's
  behaviour — a succeeded run that prints nothing clears the area, a failed run keeps it — is untouched.
- **L5 (T2 and T3, and the defect this brief carried).** T2 was delegated with the verified line map of
  `Command::Shortcuts` as the precedent, and delivered: `Command::Script` in the SHOW AND SELECT block,
  `show_script` on the shell, `chrome::script_dialog` as an `egui::Modal` on S9's precedent (`Esc` closes
  it, the pointer gate now names it), `Icon::Gcode` and a tooltip, and `"Script"` appended to the palette
  golden — **27 chips become 28**, and the five headings do not change. Tests: `cam-ui` shell **125
  passed** (including the new one, which asserts the panel is closed before the click, paints
  `PATH_LABEL` after it, and that `output.canvas` is byte-identical with the panel open and closed —
  S2's whole claim), palette **4 passed**, script_panel **7 passed**.
  **The brief was wrong and the writer said so**: the plumbing could not compile as written, because
  `Shell` derives `Clone, Debug` and `ScriptPanel` derived neither. The writer worked around it with an
  `Rc<RefCell<ScriptPanel>>` wrapper, which makes `Shell` **`!Send`** — an API-visible regression on a
  type the desktop binary holds. The parent took the clean fix instead: `ScriptPanel` derives
  `Clone, Debug` (it is two `String`s, three `Option`s and a `Theme`), the wrapper and both imports are
  gone, and the shell target is green again at 125. A workaround that changes a type's auto-traits is not
  a workaround. T3 then corrected the milestone record: `docs/m10-documents.md` no longer says the panel
  is unmounted, and what it says instead names the two residuals the mount does not solve — no artboard
  draws the surface, and a script edits a file on disk rather than the shell's document.
- **L6 (the review of the mount, and a BLOCKER that is inverted).** Lineage **`review-5eb75a623cbfc72e`**,
  tier `high`, 286 lines, four reviewer runs and then a **refuter**. The refuter returned
  **`correction_required`** on one finding, `R3-001`, severe, `deterministic`, `introduced`:
  **`crates/cam-ui/src/chrome.rs:1380`** — *"The new script modal is constructed with
  `egui::Id::unique(...)`, which is not an `egui::Id` constructor; `egui::Id` exposes `new`, so this line
  prevents the crate from compiling and the modal cannot be built or tested."* **Every clause of that is
  false, and the evidence is one command away**: `egui-0.37.0/src/id.rs:91` declares
  `pub fn unique(source: impl Hash + Debug) -> Self` (with doc examples at `:88-95`); the identical call
  already exists twice in this crate (`tool_library.rs:691`, `chrome.rs:1212`); and
  `cargo build -p cam-ui` plus the modal's own test both pass. The owner authorised the one bounded
  correction anyway, to let the review close — and that **cannot be done**: `egui::Id::new`, the spelling
  the finding demands, is **deprecated** in 0.37 (*"Use `Id::unique` … instead"*), so the demanded edit
  turns a green `clippy -D warnings` gate red. The rename was made, the deprecation appeared, and it was
  reverted; the tree is back at `Id::unique` and `cargo clippy -p cam-ui --all-targets -D warnings` is
  clean. **State: the lineage is left in `correction_required`, authority unburned and the correction
  unspent, because the only edit that would satisfy the finding is one the repository's own gate refuses.**
  Nothing was submitted, and closing that lineage is an explicit owner decision (abandon the authority),
  not a technical step a worker can take.
- **L7 (the owner abandoned the authority, and the review of the mount is now closed as discarded).**
  `review-5eb75a623cbfc72e` is **abandoned**, not corrected: `operator_disposition` by the owner, recorded
  by the audited native operation as `gentle-ai.review-reclaim-record/v1` at
  `2026-11-05T03:58:17Z`, revision `sha256:286f6a57…`, snapshot
  `sha256:7271a7de…`, discarded work `00-review-risk`, `01-review-resilience`, `02-review-readability`,
  `03-review-reliability` with `findings_present: true`. Its state moved to
  `review-transactions/quarantine/review-5eb75a623cbfc72e-441271739` (`residue:
  review-state.json`) and the workspace holds no authority for it. L6's "left in `correction_required`"
  describes the state before this entry. **What is lost is the outside view, not the code**: the mount is
  verified by `./scripts/check.sh` (1517 tests over 110 result lines), by the modal's own test (opened
  from its chip, `Esc` closes it, `output.canvas` byte-identical open and closed) and by `clippy -D
  warnings`. `assess` measures the following docs-only commit as `passive` / `reviewDue: false`, which is
  the provider's own reason not to re-review documentation.
- **L8 (a warning for whoever reviews this target next).** Nothing about the mount's candidate changed, so
  a fresh review of the same commits can file the same inverted `R3-001`: the refuter reasoned about
  `egui::Id` without reading `egui-0.37.0/src/id.rs`. If that happens, the cheapest refutation is
  `grep -n 'pub fn unique' /home/th3g3ntl3man/.cargo/registry/src/*/egui-0.37.0/src/id.rs`, and the
  demanded spelling (`Id::new`) is the deprecated one — so the correction must not be made, and the
  authority should be abandoned again rather than appeased.
