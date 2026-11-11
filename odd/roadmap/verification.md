## 5. Verification strategy

Per ODD, change behaviour with applicable runnable checks, test-first:

1. **Golden-file G-code tests** — the primary regression harness. `cam-cli` on a fixture document
   must emit byte-identical output; every strategy and every output block gets a golden file.
2. **Property tests** (`proptest`) on the geometry kernel — invariants that catch the classic 2.5D
   bugs: an outward offset of a closed loop never self-intersects below the tool radius; chaining
   never produces duplicate endpoints within tolerance; reversing a polyline preserves winding sign.
3. **Round-trip tests** — document → JSON → document is lossless; SQLite tool library migrates
   forward and never loses a tool.
4. **Post-processor conformance** — a fixture set covering all 13 directives and all 20 blocks (the
   four helical kinds M6 added among them), plus a parser that rejects malformed files with a located
   diagnostic (`line:column`, expected token).
5. **Simulation as oracle** — for each strategy, a test asserts the simulated removed volume matches
   the analytic expectation within tolerance; a too-shallow pocket fails the test, not a screenshot.
6. **No-manual-QA rule** — nothing is "done" because it looks right in the window; the CLI proof
   comes first.

Explicitly not used: another CAM package's G-code output as an oracle. Byte-matching a closed-source
implementation's ordering and rounding choices would turn every improvement into a regression, and we
did not decompile its algorithms.

