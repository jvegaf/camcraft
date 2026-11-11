# M11 — feeds and speeds

[Back to index](index.md)

### 4.14 M11 delivered (evidence)

| Deliverable | Where | Proof |
| --- | --- | --- |
| `Job.material`, optional and omitted when absent, `Copy` lost and every `..Job::default_sheet()` site moved with it, the tag still `c2d/document@2` | `crates/cam-model` | a document written before M11 loads, keeps its fields and re-saves byte-identically; the round-trip and frame tests moved in the same commit rather than being relaxed |
| The selector threaded to the lookup the store already had: `program_for`, `program_for_snapshot` and `program_tool` take `Option<&str>`, and `None` is the pre-M11 program | `crates/cam-program` | all 17 golden programs keep `F100.0` and `S18000`; the library's per-material keying was already proved before the milestone (`crates/cam-store/tests/tools.rs`: one tool at 120 mm/s in one material and 60 in another) |
| `cam-cli emit --material`, the flag winning over the document's field, and an unknown material refused by name rather than silently cut at the tool's own rate | `crates/cam-cli` | `emit_cuts_as_the_material_it_is_asked_for` pins the four cases (no selector → `F100.0`; flag `mdf` → `F120.0` and no `F100.0`; document `mdf` → `F120.0`; flag `hardwood` over a document that says `mdf` → `F60.0`); `emit_refuses_a_material_the_library_has_no_record_for` asserts `oak`, exit `2` and the tool's name; 21 golden tests pass untouched |
| One function answers "which material" for both `emit` and `simulate`, so the two commands cannot ask the document different questions | `selected_material` in `crates/cam-cli/src/lib.rs` | the same selector is the only caller path for both commands |
| The review | lineage **`review-c54c930ddcde3f9f`** | tier `high`, 667 lines over 26 paths, four lenses, `correction_required` on five corroborated findings, one bounded correction of **79 diff lines** (budget 200), a targeted validator that passed quoting the frozen findings one by one, closure **`approved`**, receipt burned (`sha256:93d41308…`) |

Two defects were found and fixed at the root, both of them mine. The flag's parser used
`rest.get(index + 1).cloned()`, which made a **trailing** `--material` indistinguishable from *no* selector:
`emit` exited `0` and cut at the rate the operator had explicitly asked not to use, and a following flag was
swallowed as the name. It now refuses both shapes — a missing value and a value starting with `-` — with
exit `2` and "emit --material needs a material name". The second is the more instructive: the correction had
also made the **no-selector** path prefer the library's null-material row, which changes what an existing
library shape emits (`F60.0` → `F100.0`) and contradicts S3; it was reverted, and the comment says what the
code does instead of claiming a byte-identity it did not deliver. Both were invisible to a green gate, and
the review caught them by reading the spec the tests did not.

Recorded and not decided by this milestone, each a successor finding rather than a silent gap: `R4-002` and
`R3-003` — the shell's cutting-time estimate passes no material, so a document that names one emits with
that material's rates while the window estimates from the tool's own record; the snapshot needs a material
path `cam-store` can answer, which is a change to that crate. `R4-003` — `simulate` has no `--material`, so
after `emit --material <name>` the simulation rebuilds the program for the document's material and cannot
reproduce the emitted program. And a measured limit carried in the feature document: **no shipped post block
references `[FP]`**, so a plunge rate cannot be seen through the emitter at all; it is pinned at the
`cam-program` level, and changing a post contract to make it visible is not this milestone's business.

