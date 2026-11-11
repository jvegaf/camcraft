# M14 — tool library

[Back to index](index.md)

### 4.17 M14 delivered (evidence)

The reference screen was three screenshots and a sentence in the module header saying why this shell drew a
paragraph where they draw six buttons. Delivering it meant three things the screenshots could not say.

**The kind had to be stored.** `tool_type` was a raw `u8` whose mapping the schema refused to guess, and the
tree grouped by shape derived from geometry — which works for four shapes and fails for six kinds, because a
drill and a V-bit are both described by an included angle. `ToolKind` now writes 1-6 and reserves 0 for "the
writer did not say", which is exactly what `cam-cli tools add` writes, so every library that already exists
still reads as the shape its geometry proves. `ToolKind::from_shape` is still the fallback and still tested.

**The store could not write.** `cam-store` had no delete, no reset and no validation: `put_geometry` is an
upsert that inserts whatever it is handed. `ToolProblem` is one variant per refusal carrying the number that
failed, and the three writers are each one transaction with `replace_tools` validating every tool *before* it
deletes one — the alternative is a library emptied by a typo.

**A feed rate was two units at once.** Four documents and the cutting-time model said millimetres per second;
the emitter handed the same number to the program's `F` word and the CLI's goldens read `feed_rate: 100.0` as
`F100.0`. Both had a test, so whichever unit was declared, one of them was sixty times wrong. The owner
settled it on millimetres per minute — the emitter, the posts and every golden stay where they are — and the
cost was four documents, the timing model, every `view3d` fixture restated at the same physical speed, and
`Estimated cutting time · 0:13` becoming `13:42` (`02ea9ef`).

The suite was rewritten as behaviour over the report rather than shapes over a paint: 43 tests in the
`tool_library` target, including the deliberate absence test for the action rows that this slice then flipped.
Two defects it found are worth naming because they were invisible from outside: a `decimal` call handed an
`f64` where a `Length` was expected, and a seeding guard that never fired, which made `Use this tool`
unreachable for *every* tool.

