# M13 — The cycle closes inside the app

Feature document for milestone **M13** of `odd/roadmap.md`. Read **until `## Log`** before touching code.

## Specs

- **S1 (the owner's decision, verbatim):** `A · El ciclo se cierra dentro de la app (recomendado)` — the
  option's own description, which is the scope: *"G-code real, simulación in-app, lista de toolpaths con
  selección, y el material 3D con los tokens del sistema. Ata los dos callejones sin salida y el cierre del
  flujo."*
- **S2 (dead end one, measured):** the shell's primary action **cannot run**. `crates/cam-ui/src/chrome.rs:323`
  registers `Generate G-code` as `add_enabled(false, …)` with a hover sentence, and `cam-desktop` does not
  even link `cam-post` (`crates/cam-desktop/Cargo.toml:14-24`). So the app can import a drawing, generate
  toolpaths and play a simulation, and then hands the person off to a terminal for the one artefact the
  machine consumes. **The gap to close:** the program is produced from the open document, inside the app,
  written where the person says, and the shell says where it went. A refusal says what is missing, by name —
  never a disabled button with no reason in it.
- **S3 (dead end two, measured):** the 3D half does not simulate; it displays a program somebody else
  produced. `crates/cam-ui/src/view3d.rs:1379-1394` prints `"No program to preview. Simulate one with
  \`cam-cli simulate\` and hand it in."` and draws no overlay row and no playback bar until a `Preview3d` is
  handed to it. `cam-desktop` already links `cam-sim`, `cam-toolpath` and `cam-move`
  (`crates/cam-desktop/Cargo.toml:14-24`), so the capability is one command away and never crosses the seam.
  **The gap to close:** the person presses simulate, the document's own toolpaths become a program and that
  program becomes the preview, with `Simulate again` and the estimated time where the reference app puts
  them. Nothing in the shell may tell the person to leave the shell.
- **S4 (the list the reference has and we do not):** the shell has no toolpath **selection**. The tool card
  describes `app.document().toolpaths.first()` unconditionally (`crates/cam-ui/src/chrome.rs:850-855`), so a
  document with two toolpaths can only ever describe the first, and `TOOLPATHS` shows one editor for
  "the" toolpath rather than a list of them. **The gap to close:** the surface lists the document's
  toolpaths, one is selectable, and the selection is what the tool card and the editor describe.
- **S5 (the material, measured against the design authority):** `design/system.md` fixes the stock as tan
  `#d8b48c` top and `#b8895c` edge; the delivered 3D half paints a near-white slab (owner's screenshot,
  `2026-11-08 18-49-23.426.png`), so the picture reads as an untextured block instead of material. **The gap
  to close:** the 3D half's stock is painted from the design system's own role, not a literal in `view3d.rs`.
- **S6 (out of scope, so review does not have to guess):** no new operation kinds (drilling, V-bit,
  engraving and 3D roughing stay as `crates/cam-ui/src/strategy.rs:19-25` refuses them), no import report
  dialog, no draw-tool overlay, no ribbon relabelling, no `cam-cli` flag changes. Those were the options the
  owner did not choose; naming them here is what keeps this milestone reviewable.
- **S7 (repo law, as the seam actually is):** a document mutation is applied by the **shell**, and the host
  only supplies what the shell cannot: `UiAction::RequestSaveAs` is answered by `cam-desktop`'s file dialog
  (`crates/cam-desktop/src/main.rs:104-111`) and the write itself happens inside `apply_action`
  (`crates/cam-ui/src/lib.rs:3053-3075` via `write_document`, `:3290`). Posting follows that shape: the
  desktop answers the path intent, the posting and the write belong to the shell, no subprocess and no
  `std::process::Command` anywhere, and **no dependency is added to `cam-desktop`**. `cam-post` joins
  `cam-ui`, which already links `cam-program`, `cam-sim`, `cam-store` and `cam-cli`
  (`crates/cam-ui/Cargo.toml`). *(Corrected after measurement; see L4.)*

## Tasks

| ID | What | S | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | `Generate G-code` is real: `cam-post` reaches `cam-ui`, the action is enabled exactly by being pressable, a press posts the open document, one save dialog chooses the path, the file is written, and the status bar names it and its line count. `Post…` holds the dialect for the session, a file that is not a post-processor is refused while the held one stays, and every refusal writes nothing | S1 S2 S7 | inline | — |
| T2 | `Simulate` runs in the shell: the document's toolpaths are generated and simulated in-process, the `Preview3d` is handed to the 3D half, the empty half's console sentence becomes a `Simulate` control and the overlay row gains `Simulate again`, and the duration the playback bar names answers "how long is this job". A drawing with no toolpaths, or one whose tools the library cannot resolve, refuses by name and draws no picture | S1 S3 | inline | — |
| T3 | The `TOOLPATHS` surface becomes a list: one row per toolpath, one selected, and the selection — not `first()` — is what the tool card and the editor describe | S1 S4 | inline | — |
| T4 | The 3D stock is painted from `design/system.md`'s material roles, and the near-white slab is gone | S1 S5 | inline | — |
| T5 | The milestone's row in `odd/roadmap.md` and this document's log, with the commits as evidence | — | inline | — |

## Slices

One commit per task, because each is one reviewable behaviour. Sizes are **authored** lines (lock files excluded),
measured against the ~400-line guidance that protects the reviewer:

| Slice | Task | Commit | Authored lines | Why this boundary |
| --- | --- | --- | --- | --- |
| 1 | T1 + this document | `0c194cb` | 731 | Over the guidance, and the reason is the test file: 359 of those lines are eight behaviour tests, each asserting a refusal's *absence of effect* as well as its message. Splitting the code from its tests would have left a slice that proves nothing, and splitting the two behaviours (choose a post / write a program) would have left a commit that does not compile, because both presses live in one `with_layout` block. Recorded rather than hidden: the candidate is the whole slice. |
| 2 | T2 | — | — | The simulate control and its picture are one behaviour, and the empty half's sentence cannot change without the control that replaces it. |
| 3 | T3 | — | — | The list and the selection that drives the card are one behaviour: a list nobody can select is the `first()` it replaced. |
| 4 | T4 | — | — | One painting rule, one test. |
| 5 | T5 | — | — | Documentation only. |

## Log

- **L1 (the request, verbatim).** "para el proximo milestone tenemos que reducir los gaps evidenciables que
  tenemos con prancheta, porque mira como estamos ahora /home/th3g3ntl3man/Pictures/Screenshot 2026-11-08
  18-49-23.426.png"
- **L2 (the owner's decision, verbatim).** `A · El ciclo se cierra dentro de la app (recomendado)` — "G-code
  real, simulación in-app, lista de toolpaths con selección, y el material 3D con los tokens del sistema. Ata
  los dos callejones sin salida y el cierre del flujo." Recorded as S1.
- **L3 (what the audit measured before the decision).** The eight Prancheta patterns in
  `design/prancheta-reference/README.md`'s "Still to take" list are **delivered** (`camui-prancheta`
  T1–T9), so this milestone is not chrome: the eight items are not restated as work. What the evidence
  produced instead is S2–S5, all of them workflow gaps the chrome was hiding: a disabled primary action, a
  simulation that needs the CLI, no toolpath selection, and a stock painted against the design system.
- **L4 (S7 corrected, and the post's origin).** S7 first said the posting belonged to `cam-desktop` and that
  `cam-post` would not enter `cam-ui`. Measured: `UiAction::SaveFile` writes the document **inside
  `cam-ui`** (`lib.rs:3053-3075` through `write_document` at `:3290`), and `cam-desktop` only answers the
  path intent (`main.rs:104-111`). S7 now says that. The second finding is the post's origin: `Document.postp`
  is documented as "optional post-processor filename, resolved by the store" (`cam-model/src/lib.rs:909-911`)
  and **nothing in the workspace reads it** — `cam-cli emit` takes `--post` only
  (`cam-cli/src/lib.rs:3159-3162`). So T1 gives the shell the one thing the CLI's flag gives it: a
  `Post…` control that names one, held by the shell for the session. `postp` stays unread in this milestone:
  nothing writes it, and adopting a filename the document cannot be given would be a rule no screen can
  reach.
