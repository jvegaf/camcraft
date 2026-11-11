# M13 — in-app cycle

[Back to index](index.md)

### 4.16 M13 delivered (evidence)

The four gaps M13 named were all in the **seams**, not in the machinery, which is why they survived twelve
milestones of green gates. Measured before writing anything: the primary action could not run
(`crates/cam-ui/src/chrome.rs:323` registers `Generate G-code` as `add_enabled(false, …)`), the 3D half would
not simulate (`crates/cam-ui/src/view3d.rs:1390` prints a sentence naming `cam-cli`), the shell had no
toolpath selection at all (`chrome.rs:850-855` described `toolpaths.first()` unconditionally), and the disused
`Document.postp` ("resolved by the store", `cam-model/src/lib.rs:909-911`) has no reader in the workspace, so
nothing in the app could name the dialect a program is written in.

What was delivered, in four commits: `e84164d` (the program, and the `Post…` control that supplies the
dialect), `4729b00` (the slice boundary, recorded rather than inferred), `2be9298` (simulation in the shell,
with `Simulate` on the empty half and `Simulate again` in the overlay row) and `0f7dc4e` (the toolpath list,
the one resolver the card, the editor and the tool-library modal now share, and the stale-`first()` door that
slice closed). Ten behaviour tests, gate green at **1604** workspace tests across 117 targets.

Three limits are recorded instead of discovered later:

- **A snapshot cannot select a material.** `program_for_snapshot` refuses one by name
  (`crates/cam-program/src/lib.rs:95-113`), so the shell's program cuts at the tools' own rates and diverges
  from `cam-cli emit --material` exactly as the shell's cut-time estimate already does (M11's follow-up).
  Passing the document's material would have made every drawing that names one refuse to produce a program;
  a test pins that a named material still writes.
- **Two sentences still name `cam-cli`**, both in `chrome.rs` and both drawn only while the RELIEF AND 3D
  task is open (`chrome.rs:890`, `:911`). They are outside every slice's surfaces and need their own task.
- **The launch demonstrator stays.** `cam-desktop` still hands the shell a sample pocket, because the sample
  document holds no toolpaths of its own; its test pins M5's field/sweep identity, and replacing it is a
  separate decision about what the first frame should demonstrate.

`T4` started as a repaint and became a proof. The 3D stock **already** painted from the design system's roles
(`theme.rs:221`'s `stock_shade` mixing `StockTop` into `StockBottom`, `view3d.rs:963`'s `StockEdge` outline),
and the near-white slab in the owner's screenshot was `StockTop #d6dce6` itself — so the gap was a palette
value, not a rule. The owner chose tan material (`L8`), and resolving the tan ran into a pair nobody had
proved: the amber `Cut` is painted **on** the stock, and it measured **1.47:1** against the lit face. Holding
that to the 3:1 a 1.6 px signal needs leaves no light face available — the dark stock becomes mid-value walnut
`#705c42` (3.14:1 under the amber, 2.99:1 off the canvas), its shade `#453726` the same wood 1.80:1 deeper,
and the rim **lighter** at `#ddc9a3`, because nothing darker clears 3:1 against that face. Paper gets wheat
`#e8cb9d` inside a window 0.024 of luminance wide. Four stock proofs are re-derived, two are new (the wood band
with its hue family and channel step, and the amber on the lit face), and the cost is stated in the palette
and in `design/system.md`: the dark ramp narrows from 6.33:1 to 1.80:1, and the light set's amber over the
deepest stop is 2.25:1 — under the floor, and not fixable by moving these three roles.

