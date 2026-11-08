# M11 — Feeds & speeds: the material the program is asked to cut

Feature document for milestone **M11** of `odd/roadmap.md`. Read **until `## Log`** before touching code.

## Specs

- **S1 (user, verbatim option label, on where the selector lives):** "Campo en el documento + flag".
  `Job` gains an optional material, and `emit` gains an override that wins.
- **S2 (the measured gap, from reconnaissance):** the store already keys cutting data per
  **(geometry, material, machine)** — `material` and `tool_entity` in `crates/cam-store/src/tools.rs`
  (tables at `:54-88`), read by `cutting_data_for(geometry, material, machine)` at `:647`, and proved by
  `crates/cam-store/tests/tools.rs:57` where one tool carries **120** mm/s in one material and **60** in
  another. What is missing is the **selector**: `crates/cam-program/src/lib.rs:121-135` maps
  `CuttingData → ToolSpec` with no material parameter, so `crates/cam-toolpath/src/lib.rs:1358` stamps
  the tool's own record into every segment, and `crates/cam-cli/src/lib.rs:739` writes the same
  `100 / 30 / 18000` for every tool created from the CLI.
- **S3 (the invariant the whole milestone hangs on):** **`None` must reproduce today's bytes exactly.**
  The 17 golden programs in `crates/cam-cli/tests/cli.rs` each pin one `F100.0` and one `S18000`, and a
  document's re-save must stay byte-identical (`crates/cam-model/tests/document.rs:1257-1290`), so the
  new field is `#[serde(default, skip_serializing_if = "Option::is_none")]` — the precedent `nesting`
  set at `crates/cam-model/src/lib.rs:906-921` — and the tag stays `c2d/document@2`.
- **S4 (which wins, and what an unknown material does):** the explicit `--material` on `emit` beats the
  document's field. A material the library has no cutting data for is **refused by name**, naming the
  material and the tool; it is never silently replaced by the tool's own rate, because a program that
  quietly cuts at a rate nobody chose is the defect this milestone exists to remove.
- **S5 (repo law):** `Job` derives `Copy` today (`crates/cam-model/src/lib.rs:127`) and a `String` field
  ends that, so every `..Job::default_sheet()` site moves mechanically in the same change; no second
  epsilon, no second name for the same thing, and no schema bump.
- **S6 (out of scope, and recorded so it is not smuggled in):** **no derivation.** Chipload, surface
  speed and an rpm calculator are not in this milestone: they would change the one number the goldens
  pin and place a derivation between the library record and the program that nothing in the workspace
  tests. They land when a measurement asks for them.

## Tasks

| ID | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | `Job.material`, optional and omitted when absent, with `Copy` lost and every site updated; a document written before this loads and re-saves byte-identically | S1 S3 S5 | worker | |
| T2 | Thread the material to the store's existing lookup: `program_for`, `program_for_snapshot` and `program_tool` take it, `None` keeps `tool_for_number`'s behaviour, and `cutting_data_for` decides when it is `Some` | S2 S3 | worker | |
| T3 | `cam-cli emit --material`, the flag winning over the document, and the refusal naming material and tool when the library has nothing; one new golden with a material, and all 17 existing ones unchanged | S4 S6 | parent | |
| T4 | Docs: `docs/format.md`'s `emit` row, the tool-library story (a tool's card still says it shows no rates), and the roadmap row's evidence | S2 | parent | |
| T5 | The review of this candidate | — | parent | |

## Log

- **L1 (user, verbatim):** the milestone choice and the design answer — "Feeds & speeds (recomendado)" for
  M11, and "Campo en el documento + flag" for where the selector lives.
- **L2 (what the reconnaissance changed about my own proposal).** I offered feeds & speeds as *derive F
  and RPM from material, chipload and geometry*. The evidence says otherwise, and the milestone is
  smaller for it: the library **already** stores feed, plunge and spindle per material and per machine,
  and already has the query. Inventing a chipload derivation would move the single number 17 goldens
  pin and would add arithmetic nothing in the workspace tests. So M11 is a **selector**, not a
  calculator — and the derivation is deferred by S6 rather than dropped.
- **L3 (what is already true, measured).** `CuttingData` has `feed_rate`, `plunge_rate` and
  `spindle_speed` as `Option`s with no schema defaults (`crates/cam-store/src/tools.rs:152-175`);
  `ToolSpec` carries them into every `Segment` (`crates/cam-toolpath/src/lib.rs:1358-1360`); the post
  emits `[F]`/`[S]` on the first descent and suppresses a repeat (`crates/cam-post/src/emit.rs:424-432`,
  `:478-480`); and **`plunge_rate` is pinned by no golden** — the fixtures declare `[FP]` but no block
  references it, so a plunge change is invisible to the suite today. That last fact decides a test: T3
  must pin the plunge rate where it can be seen, or say why it cannot.
- **L4 (T1 and T2 landed; two findings the parent has to act on in T3).** The selector exists and the
  bytes did not move: `Job.material` is an optional top-level job field
  (`#[serde(default, skip_serializing_if = "Option::is_none")]`), `Job` is no longer `Copy`, and
  `program_for`/`program_for_snapshot`/`program_tool` take `Option<&str>`; `None` is the pre-M11 program
  and the goldens in `crates/cam-cli/tests/cli.rs` still report their `F100.0`/`S18000` unchanged. Two
  things were decided by the code that the specs did not settle, and neither is a task:
  1. **`program_for_snapshot` cannot honour a material today.** A `StoredTool` carries the tool's own
     record with no material key, and `cam-store` was not among the worker's surfaces, so `Some` is
     refused with `ProgramError::NoCuttingDataForMaterial` — naming the material and the tool — rather
     than quietly cutting at the tool's own rate. Both current callers pass `None`. The consequence
     lands on T3: once `emit --material` honours `job.material`, the shell's estimate
     (`crates/cam-ui/src/chrome.rs:1026`) will describe a different program than the CLI emits unless
     the snapshot gains a material path that `cam-store` can answer.
  2. **`plunge_rate` is pinned by no golden.** The post fixtures declare `[FP]` but no block references
     it, so a plunge change is invisible to the suite — the same fact L3 recorded, still unaddressed.
     T3 has to pin the plunge rate where it can be seen, or say why it cannot.
- **L5 (T3, and the two defects the tests found rather than the reviews).** `emit --material <name>` exists,
  the document's `job.material` is honoured when no flag is given, the flag wins over the field
  (`selected_material` in `crates/cam-cli/src/lib.rs`, one function so `emit` and `simulate` cannot ask the
  document different questions), and an unknown material exits `2` with the crate's own sentence naming
  `T1` and the material. Two tests: `emit_cuts_as_the_material_it_is_asked_for` (no selector → `F100.0`;
  flag `mdf` → `F120.0` and no `F100.0`; document `mdf` with no flag → `F120.0`; flag `hardwood` over a
  document that says `mdf` → `F60.0`) and `emit_refuses_a_material_the_library_has_no_record_for`
  (`oak`, exit `2`, names the tool). All 21 golden tests still pass untouched.
  **The first defect is the interesting one.** With a library that carries three records for one tool — its
  own plus one per material — the **no-selector** path cut at `F60.0`, neither the tool's own `100` nor
  anything anybody chose: `tool_for_number` answers with whichever `tool_entity` row the tool number
  matched first. Fixed in `program_for`: with no material selected the record is the one the library keys
  **as the tool's own** (`cutting_data_for(geometry, None, None)`, the null-material row), falling back to
  the tool number's own answer only when the library has no such row — so a library shaped like today's is
  byte-identical and a library keyed per material stops being arbitrary. The second defect was in my own
  test: `.c2d` is not text, so the helper that set the material by patching the file found no document —
  it now reopens the document through the model and saves it.
  **L4.2 stays open by decision:** no shipped post block references `[FP]`, so a plunge rate cannot be seen
  through the emitter at all; it is pinned at the `cam-program` level by T2's tests, and changing a post
  contract to make it visible is not this milestone's business.
