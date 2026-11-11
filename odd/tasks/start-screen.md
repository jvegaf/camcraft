# Start screen — no demo project at launch

The desktop app must open with **no project**, and greet the operator with one modal that offers a new project, opening
one from disk, or resuming a project opened earlier. Today it starts with a demo document (rectangle + circle), a
generated pocket preview that auto-plays, and the 3D split showing.

Design: `design/start-screen/` (three directions; the handed-off one is recorded below).

## Specs

- **S1** — The user's words: *"ahora siempre abre con la prueba del rectangulo y el circulo, junto con la vista 3d, y no
  debe de ser asi"*. No launch path may create the sample rectangle, the sample circle, or the sample pocket program,
  and nothing may start the 3D view or its playback.
- **S2** — The user's words: *"debe de iniciarse sin nada abierto"*. After launch the app holds no project: the drawing
  area draws neither stock nor geometry, the app bar names no document, and every command that needs a project reports
  that none is open instead of acting on an invisible empty one.
- **S3** — The user's words: *"agregar un modal para elegir entre nuevo proyecto, abrir proyecto, y una lista de los
  ultimos proyectos abiertos"*. One modal, opened at launch, offering exactly three things: **New project**, **Open
  project**, and the list of projects opened last. Choosing a recent project opens that project.
- **S4** — The user's words: *"debemos de crear un system design para disenar ese modal"*. The modal is designed as
  artboards under `design/`, on the existing tokens of `design/system.md`, before it is implemented.

## Tasks

| ID | Specs | What it delivers | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | S1, S2 | `cam-ui`: the shell can sit with no project open — start-screen state, empty canvas (no stock, no rulers' origin), app bar and status bar naming no project, 3D refusing without one | inline | `37550dc` |
| T2 | S3, S4 | `cam-ui`: the start modal drawn from the handed-off artboard — New project, Open project, recent list, first-run and unreadable-list states, Esc leaves the empty workspace | inline | `37550dc` |
| T3 | S3 | `cam-ui`: the recent-project list as shell state (`RecentProject`), recorded on a successful open and save-as, capped and de-duplicated by path | inline | `37550dc` |
| T4 | S1, S2, S3 | `cam-desktop`: launch with no project, no sample document, no preview autoplay, no tool-library modal at launch; host the `recents.json` file in the app data directory and answer the file dialog | inline | `37550dc` |
| T5 | S1, S2, S3 | Tests: twelve — nine in `crates/cam-ui/tests/shell/start.rs`, three host-side in `cam-desktop/src/main.rs`, plus the launch state | inline | `37550dc` |
| T6 | S2, S3 | Docs: README startup section, `design/system.md` modal rule (a second dialog, still `radius_dialog`), the artboards | inline | `ad1e029`, `37550dc` |

## Log

- **L1** — The user's request, verbatim: *"hay que modificar el inicio de la app, porque ahora siempre abre con la prueba
  del rectangulo y el circulo, junto con la vista 3d, y no debe de ser asi. debe de iniciarse sin nada abierto, y agregar
  un modal para elegir entre nuevo proyecto, abrir proyecto, y una lista de los ultimos proyectos abiertos. debemos de
  crear un system design para disenar ese modal"*.
- **L2** — Evidence for S1, as found (explorer pass, `path:line`): `crates/cam-desktop/src/main.rs:58` builds
  `AppState::new(sample_document())`; `sample_document()` (`:201-239`) makes `Document::new(Job::default_sheet())` plus
  the rectangle contour (`:203-212`), the circle contour (`:214-221`) and layer `Sample`; `CamDesktopApp::new` installs
  `sample_preview` (`:59-62`), sets `preview.set_playing(true)` (`:66`) and calls `shell.open_app_library()` (`:74`).
  `Shell::show_3d` defaults to `false` (`crates/cam-ui/src/shell/mod.rs:441`), so the 3D half is the *preview's* doing,
  not a split flag. `AppState` stores `document: Document`, never an `Option` (`crates/cam-app/src/lib.rs:105`), and
  `Document::new(job)` (`crates/cam-model/src/lib.rs:1477`) is the empty constructor.
- **L3** — No app-level preference, config or recents store exists in the workspace: `cam-store` holds `.c2d` documents
  and the SQLite tool library only, and `odd/tasks/shell-save-document.md:116` records that "the shell keeps no
  recent-file list". Existing intents to reuse: `OpenFile { path }`, `RequestOpenFile`, `NewProject`, `SaveFile`,
  `RequestSaveAs` (`crates/cam-ui/src/actions/mod.rs:174-216`).
- **L4** — Design written: `design/start-screen/` with `01-command-board.html`, `02-recent-first.html`,
  `03-split-workbench.html`, all on `design/system.md` tokens, each with dark/light and
  returning/first-run/unreadable-list states. Directions are alternatives; the handed-off one is appended here when the
  operator chooses it.
- **L5** — Decision taken to satisfy S1 without contradicting S3: `shell.open_app_library()` no longer runs at launch.
  Two modals cannot own the first frame, and the library stays reachable from the rail's `ORGANISE` group.
- **L6** — Handoff: `design/start-screen/03-split-workbench.html` (**Split workbench**) is the chosen artboard; the
  other two in `design/start-screen/` are alternatives and are not implemented. Its words became the module's
  constants in `crates/cam-ui/src/start_screen.rs`, and `design/system.md` gained the modal's component rule.
- **L7** — Two decisions the artboard left open. (1) The recent list's stamp is absolute (`3 Nov 2026 18:04`) and not
  relative (`22 min ago`): the shell reads exactly one clock, its playback delta, so the label travels as opaque text
  the host writes when it persists the list, and a relative label would also go stale in the file. (2) `Try again` on
  an unreadable list is a retry request the host answers (`Shell::take_recents_retry`), not a second read inside a
  frame: the shell never touches the file.
- **L8** — The first pass at the rows' hit testing was rejected and rewritten. It accepted a pointer release within
  **200 px** of a row when no widget had reported a click, so a drag released near the list opened whichever project
  was closest — a silent, data-triggering control. Both the tile and the row now allocate their own rectangle with
  `Sense::click()` and paint into it (`crates/cam-ui/src/start_screen.rs`), the way `chrome::chip` does, and every
  action comes from a real `response.clicked()`. The measured cause of the original miss: a floating, auto-sized modal
  is positioned on the frame it first appears and settles on the next one, so a hit read from that first frame names a
  rectangle the modal has left. `DrawnState::start_hits` documents the settled-frame rule, and the tests ask for a
  settled frame instead of tolerating a miss.
- **L9** — Coverage added beyond the brief while closing it: pressing the `New project` tile through the real widget
  (the primary act had no click test) and the unreadable-list state, which must not read as "nothing was opened yet".
  Nine tests in `crates/cam-ui/tests/shell/start.rs`.
- **L10** — Slices and evidence. Two commits on branch `start-screen`: `ad1e029` holds the design (`design/start-screen/`,
  the `design/system.md` rule) and this document; `37550dc` holds the implementation, its twelve tests, the README and
  `Cargo.lock`. The gate that closed it: `cargo fmt --all -- --check` clean, `cargo clippy --workspace --all-targets --
  -D warnings` clean, `cargo test --workspace` **1695 passed, 0 failed**, and the real binary left running for 12 s
  without a panic. The slice boundary sits there: the artboards are the only reason the first slice is larger than a
  review-sized diff, and they are design deliverables rather than code to review.
- **L11** — Native review of this candidate, closed. Lineage `review-630e229555badde1`, tier `high`, lenses `review-risk`,
  `review-resilience`, `review-reliability` (the provider rejected four lenses with `lenses-invalid`, so readability was
  not reviewed). Three reviewer runs on the frozen candidate tree `dab3905f` (74 s / 154 s / 160 s, 0 cost reported),
  verdict **approved**, final revision `d766b3b0`, acknowledged and the authority burned
  (`gentle-ai.review-acknowledged/v1`). The provider's own risk signal was `process_boundary` on
  `crates/cam-ui/src/lib.rs` — pre-existing script-panel code that appears because the file is in the diff. Delivery
  stays ordinary repository policy: the branch is local and pushing it is the operator's call.
- **L12** — After the operator looked at it: the modal was too short and the two acts sat too close. The artboard was
  right and the code was the defect, per `design/system.md`'s own rule. Now `MIN_HEIGHT = 360` (`start_screen.rs`, added
  to the system's declared-per-module list), tiles at `space(6) + BODY + SMALL` = **49 dp** instead of 41, and 16 dp of
  `item_spacing` between the two acts instead of the style's 8 — measured from the frame, not assumed: content 360 dp
  tall, tile gap 16 dp. `assess` on the follow-up candidate: risk `medium`, `reviewDue: false` (`under_budget`), so no
  native review; the writer's self-verification is the gate that ran (fmt, clippy `-D warnings`, 1695 tests).
- **L13** — Defect found by the operator after the merge and fixed on `fix-startup-library-message`: `Editor::new()`
  assigned the sentence a failed `open_app_library()` returns and then cleared the status slot three lines lower. The
  clear was a leftover of the sample document — `Shell::open_app_library` writes no success line — so once the sample
  was gone it could only erase that refusal, and a data directory that cannot hold `tools.sqlite` produced a window
  saying nothing. The clear is gone and the constructor takes the data directory, so the failure is reachable from a
  test: two tests, and the first was observed **red** with the clear put back before being accepted.
