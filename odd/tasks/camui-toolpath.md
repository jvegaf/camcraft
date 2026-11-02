# camui-toolpath — the shell learns which tool cuts, and how long it takes

The last two patterns of the reference screenshots that Campilot cannot take: the **current-tool card** at the
top of a toolpath form, and the **estimated cutting time**. Both are blocked by the same thing — the shell has no
access to the cutting tool library, and `cam-app` has no vocabulary for a toolpath at all.

## Specs

- **S1 — the goal, from the reference.** `design/Screenshots-reference/README.md` rows `03-27-41` (the
  `Novo percurso — Chanfro` flyout) and `03-24-32` (the per-item form with the estimated time). The owner's
  authorisation for this feature is recorded in L1 below.
- **S2 — the card's fields are exactly the frame's.** Read off the frame, verbatim: **`T4 - Fresa V 90°`**,
  then **`Fresa V (V-bit) · Ø 12,7 mm · 90° · 2 corte(s)`**, a duplicate `Ø12,7  90°` on the glyph, and the button
  **`Escolher outra ferramenta…`**. That is number, name, type, diameter, included angle and flute count — and
  nothing else. The frame shows **no material, no machine, and no feed, plunge or spindle rate on the card**, so
  the card must not grow them: the numbers a person reads there are the ones the shape needs, not the ones the
  machine has.
- **S3 — resolution is reused, never re-implemented.** The number becomes a tool through the same rule the
  emitter uses. `ToolLibrary::tool_for_number` (`cam-store/src/tools.rs:544-573`) takes no material and no
  machine, and `Document` carries neither, so the card cannot claim to show "the rates for this material".
- **S4 — the library is a snapshot, not a second connection.** `ToolLibrary` owns a rusqlite `Connection` and is
  not `Clone` (`cam-store/src/tools.rs:213-216`), so the shell receives `Vec<StoredTool>`; `StoredTool` is
  `Clone`. No I/O of its own, no default path, no process-wide singleton.
- **S5 — the estimate is published arithmetic, not new arithmetic.** The seconds exist: `move_seconds`
  (`cam-ui/src/view3d.rs:705-754`) already implements the law — plunge rate for a Z-only move, feed rate
  otherwise, arcs along their own circle, helix as the hypotenuse — and `MoveStream::cut_length`
  (`cam-move/src/lib.rs:267-292`) measures the path. A second time formula would be a second opinion.
- **S6 — no new external dependency, and no new edge that closes a cycle.** `cam-store/src/lib.rs:9` forbids that
  crate depending on `cam-toolpath` or any UI crate, so `cam-ui → cam-store` is a straight downstream edge.
  `cam-toolpath` must not gain a `cam-store` edge; the `StoredTool → ToolSpec` mapping belongs where both types
  are already known.
- **S7 — authoring is the missing layer, not the missing button.** `cam_app::Command`
  (`cam-app/src/command.rs:113-380`) has 19 variants and none concerns a toolpath; authoring today exists only as
  `cam-cli toolpath` (`cam-cli/src/main.rs:637`), which pushes a `Toolpath` with its `Operation` directly. This
  feature adds the vocabulary below the UI; a form without it would be a dead control.

## Tasks

| ID | Task | Specs | Route | Risk note |
| --- | --- | --- | --- | --- |
| T1 | A pure `stored_tool_for_number(&[StoredTool], u32) -> Option<&StoredTool>` in `cam-store`, used by `ToolLibrary::tool_for_number` and by the shell so the two cannot disagree; `cam-ui` gains the `cam-store` edge; `Shell::set_tools(Vec<StoredTool>)`; the current-tool card in the inspector, showing exactly S2's fields plus an explicit "T7 is not in the library" state | S1 S2 S3 S4 S6 | worker | — |
| T2 | Publish the seconds: a `pub fn seconds_for(program: &MoveStream) -> f64` lifted out of `move_seconds`/`steps_for`, and the estimated cutting time beside the card, formatted by the `clock_text` T11 added | S1 S5 | worker | — |
| T3 | The toolpath vocabulary in `cam-app`: `AddToolpath`, `SetToolpathToolNumber`, `SetToolpathOperation`, `DeleteToolpath`, each with a refusal that names the number or the entity, plus the `next_toolpath` counter if paid for | S7 | worker | **item 3** — `cam_app::Command` is a contract others consume; its own commit and its own review candidate |
| T4 | `cam-toolpath::program_for(&Document, &[ToolSpec]) -> Result<MoveStream, _>`, the body moved out of `cam-cli::build_segment` so the estimate describes the document's toolpaths and the preview stops being `cam-desktop`'s sample | S5 S6 S7 | worker | **item 3** — the emitter's law moves crates; differential against the CLI's own output |

T1 and T2 are independent and can land in either order. T3 needs T1's list to pick a number from. T4 is what makes
T2's estimate *about the user's document* instead of about `cam-desktop`'s hard-coded `sample_tool()`.

## Log

- **L1 (owner, verbatim).** The two dispositions this feature rests on: *"dale con todo"* (the three fronts after
  the merge) and *"autorizado"*, given to the parent's question — *"¿se autoriza meter `cam-store`/`cam-toolpath` en
  `cam-ui` y abrir autoría de toolpaths en el shell?"* Both were declined earlier for a measured reason and are now
  granted: T14 was recorded as unauthorised in L39/L40 of `camui-shell.md` precisely because the shell could not
  reach the library and the number would have had to be invented.
- **L2 (evidence, the map).** A read-only exploration measured four things that decide the plan. `ToolLibrary` is a
  rusqlite connection opened by path, not `Clone` (`cam-store/src/tools.rs:213-216`), holding `StoredTool
  {geometry, cutting}` (`:205-210`) assembled from `tool_entity` joined to `tool_geometry` (`:125-149`) and
  `tool_cutting_data` (`:152-174`). A toolpath carries only `tool_number` (`cam-model/src/lib.rs:830`) and the
  document only a **filename** in `tools_ref` (`:846-848`); the resolution to feed and plunge lives in
  `cam-cli::build_segment` (`cam-cli/src/main.rs:1881-1910`), which is private to that binary and returns `Failure`.
  `cam_app::Command` has no toolpath variant (grep for `toolpath` over `crates/cam-app` is empty). The seconds exist
  but are private (`cam-ui/src/view3d.rs:705-754`) and reachable only by constructing a `Preview3d`, which needs a
  `Heightfield`; the shell's program today is `cam-desktop`'s sample (`cam-desktop/src/main.rs:153-190`), unrelated
  to `Document.toolpaths`.
- **L3 (the riskiest assumption, falsified by reading the frame).** The exploration's plan wagered that "no field of
  the current-tool card needs data `tool_for_number` does not return", and flagged that if the card named a material
  or a machine, T1 would become a schema change requiring `cutting_data_for` plus a document-side material field.
  The frame settles it against that fear: the card shows the tool's identity and its shape's two numbers, and no
  material, machine or rate at all. **T1 stays read-only.** The card is smaller than the first sketch, not larger —
  the sketch proposed showing feed, plunge and spindle, and the frame does not show them (S2).
- **L4 (a capability that failed, and why it did not become a guess).** The delegated explorer could not render the
  frame: its `find` fails with `failed to spawn thread: WouldBlock` on this machine, `read` on a directory is
  `EISDIR`, and the README elides the filename. It refused to answer the card's fields from the caption, because
  those questions are about pixels. The parent read the PNG directly instead of accepting a provisional verdict.
  Worth remembering for this harness: **an explorer here cannot necessarily list a directory**, so any brief that
  depends on discovering a filename should either hand the exact path over or expect to do that step itself.
