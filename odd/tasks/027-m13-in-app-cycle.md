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
- **S5 (the material — the owner chose tan, see L8; corrected once already, see L6).** The owner's screenshot reads as a white slab, and
  this spec first said the 3D half stood *against* the design system's material roles. Measured: it does not.
  `design/system.md:53` fixes `StockTop #d6dce6` ("the lit face of the material"), `StockBottom #424c58` and
  `StockEdge #6e7b8a`; `theme.rs:221`'s `stock_shade(t)` is the shell's only interpolation and mixes exactly
  those two ends; the 3D half paints through them (`chrome.rs:2482`, `StockEdge` at `view3d.rs:963`), and
  `theme.rs:692` proves their contrast in both sets. So the near-white slab **is** the system's own `StockTop`,
  not a literal in `view3d.rs`, and there is no drift for a painting rule to fix. What differs from the
  reference app is a **palette value**. The owner decided it (L8: *"1. prefiero tan material"*): the stock
  roles become material tan in **both** palettes, and the four contrast proofs are **re-derived with the new
  values** — never deleted, never weakened. `StockEdge` keeps its 3:1 floor against both the canvas and the
  stock's lit face, and every pair the shell actually writes on keeps its AA or AA-large floor.
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
| T4 | ✅ delivered in `18ded99` — the stock reads as **material**: dark `StockTop #705c42` / `StockBottom #453726` / `StockEdge #ddc9a3`, light `#e8cb9d` / `#cfad7b` / `#664d2e`; four stock proofs re-derived, two new (the wood band, and `Cut` on the lit face at 3:1 — a pair that had never been proved and measured 1.47:1). No painting code changed | S1 S5 | worker | `18ded99` |
| T5 | The milestone's row in `odd/roadmap.md`, its §4.16 evidence section and this document's log, with the commits as evidence | — | inline | `0f7dc4e`+ |

## Slices

One commit per task, because each is one reviewable behaviour. Sizes are **authored** lines (lock files excluded),
measured against the ~400-line guidance that protects the reviewer:

| Slice | Task | Commit | Authored lines | Why this boundary |
| --- | --- | --- | --- | --- |
| 1 | T1 + this document | `e84164d` | 731 | Over the guidance, and the reason is the test file: 359 of those lines are eight behaviour tests, each asserting a refusal's *absence of effect* as well as its message. Splitting the code from its tests would have left a slice that proves nothing, and splitting the two behaviours (choose a post / write a program) would have left a commit that does not compile, because both presses live in one `with_layout` block. Recorded rather than hidden: the candidate is the whole slice. |
| 2 | T2 | `4729b00` (boundary) + `2be9298` | 406 | The simulate control and its picture are one behaviour, and the empty half's sentence cannot change without the control that replaces it. Two sentences in `chrome.rs` (the RELIEF AND 3D task) still name `cam-cli`; they are outside this slice and named below. |
| 3 | T3 | — | 585 test + ~120 source, plus the one-line door | The list and the selection that drives the card are one behaviour: a list nobody can select is the `first()` it replaced. The slice closed the deviation the first version left — `open_tool_library` still aimed at `toolpaths.first()` (`tool_library.rs:538`), which would have retooled a different toolpath than the card named, one press away from writing the defect this task removes from reading. |
| 4 | T4 | `18ded99` | two palettes, one table, five proofs | The roles move, the shader does not: `stock_shade` already mixed `StockTop` into `StockBottom`, so the slice is two palettes, one table and the proofs that hold them. |
| 5 | T5 | — | docs only | Documentation only: the roadmap row, §4.16 and L7. |

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
- **L5 (a new requirement, quoted once and elsewhere, and why it is its own document).** The requirement
  arrived as a verbatim message from the owner and is recorded verbatim in
  `odd/tasks/028-repository-name-hygiene.md` L1 — one record, not two, because repeating it here would
  reproduce the very names it asks the repository to drop. In words: no reference to the product's name
  or its vendor survives the sweep, and the licensing concern behind the request lives in that quote
  rather than being restated here.

  The sweep is **not** a task of this milestone: it touches 32 files, renames thirteen documents and deletes
  a dossier, and folding it in would put a provenance change inside a UI candidate. It lives in
  `odd/tasks/028-repository-name-hygiene.md`, which runs after T5. What this document records is the requirement
  and its boundary, so the milestone cannot be called closed with it unread or unstarted.
- **L6 (T4 dissolved by measurement: the material is a palette decision, not a defect).** Before writing a
  painting rule I read the authority S5 named. `design/system.md:53` sets `StockTop #d6dce6` and calls it "the
  lit face of the material"; `theme.rs:221`'s `stock_shade(t)` mixes `StockTop` into `StockBottom` and is the only
  interpolation in the shell; `view3d.rs` paints the field through those roles and outlines the sheet with
  `StockEdge` (`:963`); `theme.rs:692` proves the contrast of both ends in both palettes. So nothing drifts: the
  near-white slab in the owner's screenshot **is** `StockTop`, and the rule the task asked for already exists.
  What differs from the reference app is the value itself — its stock is tan material. Changing it would edit
  `design/system.md`, both palettes and the proofs that keep twelve pairs above AA, which is a design decision
  with an owner and not a defect with a fix. S5 now says that, T4 is routed to the owner, and the milestone does
  not pretend to have closed a gap it decided not to touch.
- **L7 (the milestone closed, and what it leaves).** Delivered in four commits: `e84164d` (the program and the
  `Post…` control), `4729b00` (the slice boundary), `2be9298` (simulation in the shell) and `0f7dc4e` (the
  toolpath list and the one resolver). Ten behaviour tests; gate green at 1604 workspace tests across 117
  targets. The roadmap carries the row and §4.16 carries the evidence; the three limits recorded there are (a)
  a tool snapshot cannot select a material, so the shell's program cuts at the tools' own rates exactly as its
  estimate does; (b) two `chrome.rs` sentences still name `cam-cli` and need their own task; (c) the launch
  demonstrator stays, because the sample document holds no toolpaths of its own. **One task is open by
  design**: T4 is the owner's palette decision (L6), not an unfinished code path. The requirement recorded in
  L5 — the reference sweep — is next, in `odd/tasks/028-repository-name-hygiene.md`.
- **L8 (the owner's three decisions, verbatim).** "1. prefiero tan material 2. haz todo lo que sea necesario
  3.me parece genial" — read against the three open items this session put in front of them: (1) the stock
  reads as material tan, so T4 reopens with S5 rewritten; (2) the history rewrite and the force-push that
  goes with it are authorised, since the accepted question was exactly "reescribir la historia o dejarla
  documentada"; (3) the four per-commit review candidates are the plan they approved.
- **L9 (the provider refused the accumulated candidate, and was right).** `review.start` over
  `6cc361f..HEAD` (six commits, ~2900 authored lines) stopped at `preflight` with
  `lens_context_budget_exceeded`: no review authority was created, `mutation_performed: false`, and the
  provider's own continuation is to split the change into smaller candidates. That is the rule this
  repository already had — *a work-unit commit is the candidate, never the accumulated branch* — enforced by
  the provider instead of by my judgement, after I twice deferred the review instead of splitting it. The
  four candidates are `e84164d`, `2be9298`, `0f7dc4e` and the sweep's commit, each with its parent as
  `baseRef`.
- **L10 (T4 delivered: the tan is resolved, not picked, and the ramp pays for it).** `18ded99`. The binding
  constraint came from a pair nobody had proved: the amber `Cut` is painted **on** the stock, and its contrast
  against `StockTop` measured **1.47:1** in the dark set before this change. Holding it to the 3:1 a 1.6 px
  signal needs forces a mid-value wood, so the dark stock is walnut `#705c42` (3.14:1 under the amber, 2.99:1
  off the canvas), its shade `#453726` is the same wood 1.80:1 deeper, and the rim had to go **lighter**
  (`#ddc9a3`, because nothing darker clears 3:1 against that face). The light set is squeezed into a
  0.024-of-luminance window by the same two proofs: `#e8cb9d` carries `#a86200` at 3.05:1 and stands 1.48:1 off
  the page. Two consequences are recorded rather than hidden: the dark ramp narrows from 6.33:1 to 1.80:1, and
  the light set's amber over the *deepest* stop is 2.25:1 — under the floor, and unfixable by moving only these
  three roles (it needs a lighter paper amber, which is its own decision and not this task's).
