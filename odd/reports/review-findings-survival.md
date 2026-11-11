# What survives a review, and what this repository does about it

A method note with two halves: the mechanism as it was **measured** in this clone, and the three changes that
would end the loss — recorded here rather than reported anywhere, because this repository is not a fork of the
tooling and has no upstream to file against. It exists because the same loss has now happened four times — M9's
advisories, M10's, the advisory-debt review's own `R3-001`, and the six findings of
`review-cf16da5a0b71e7b8` — and because a repository that keeps losing evidence should say so where the next
session will read it.

## 1. The mechanism

The review authority store lives in `.git/gentle-ai`, in the repository. What it keeps depends on the
**contract version of the lineage**, not on the severity of anything it found:

| Lineage kind | What it leaves | Finding prose |
| --- | --- | --- |
| `v2` (older) | `<lineage>/review-state.json`, 4.6 KB to 23.9 KB | **Yes**: `state.admitted_role_results[].value` is a `gentle-ai.review-admitted-result/v2` carrying the admitted artifact, findings included |
| `compact-v2` (what this repository's facade runs now) | `terminal-consumption/v1/<hash>.json`, **259 bytes**: `schema`, `repository`, `target`, `lineage` and nothing else | **No.** There is no state directory, no artifact, and no record of a finding beyond the ids the closure envelope happened to print |

Measured, not inferred:

- The three `v2` lineages under `.git/gentle-ai/review-transactions/v2/` each hold one `review-state.json`,
  and the two that admitted findings carry `admitted_role_results` with the artifact `value` inside —
  `review-61e0b405dc7f498b` is 23 867 bytes and its `fix_finding_ids` are `R1-001`, `R2-001` and `R3-001`.
- `review-cf16da5a0b71e7b8` — the review of `odd/tasks/023-script-panel-document.md` — has **no file anywhere in
  the store**. Its only trace is the 259-byte terminal-consumption record for
  `target sha256:557f4547…`, and the receipt burned on acknowledgement
  (`sha256:506f7b2a356f1076b478767fd04b0edf02b4aa52bf762bd0730ae32386d213fc`).
- `.git/gentle-ai/candidate-views/` is empty: the candidate view is removed when the transaction closes.
- Reviewer results are written to disk **only when they are rejected**:
  `.git/gentle-ai/rejected-results/review-5b02c33628496345/review-reliability-1-b04d9b6deaed.json` is a raw
  artifact. Admitted ones are not kept in any comparable form.
- An authority that was abandoned keeps more than one that was approved: the quarantine residue at
  `review-transactions/quarantine/review-5eb75a623cbfc72e-441271739/residue/review-state.json` is a full
  state record, because reclaim moves the state aside instead of consuming it.

Outside the repository, the same search came back empty (`printf` of every path, negative): nothing under
`~/.gentle-ai`, `~/.pi` (`agent/sessions/<cwd-slug>/*.jsonl`, `agent/gentle-agents/…`) or the gentle-shell
logs contains the lineage id, and none contains any of the finding ids — including the live Pi session
transcript of the session that produced them, which means the four host-relayed results (944, 4 302, 2 805
and 2 766 bytes; prompts ≈80 KB each) existed only in the relay's memory. Two directories were left
unsearched for budget reasons and are named here rather than glossed: `~/.cache` and `~/.local/state`, the
`pi`/`gentle`-named paths inside them.

**The conclusion the evidence supports**: for a `compact-v2` lineage the reviewer's sentences are gone the
moment the transaction closes, and no later session can recover them. "Compact" is doing exactly what it
says; this is a design tradeoff, not corruption.

## 2. The ask that has no destination here

This repository is **not a fork** of the tooling and has no upstream to send anything to: `gentle-ai` is
separate software that campilot uses, installed outside this tree. So the three points below are not a report
filed somewhere — they are the **record of a defect this repository can describe precisely and cannot fix**,
kept so that no later session has to rediscover it, and ready to hand over the day whoever owns that code asks
what is wrong. Without this paragraph the next session reads the measured facts and reconstructs the argument
from scratch; that has already happened once.

The three changes that would end the loss, most useful first:

1. **Persist the admitted results** for `compact-v2` lineages — the file that already exists for rejected
   results (`rejected-results/<lineage>/<lens>-<n>-<hash>.json`) is the shape, and keeping it for admitted
   ones costs a few kilobytes per review.
2. **Or carry the findings' prose in the closure envelope.** Today it carries id, lens, severity, location and
   disposition, which is enough to *name* a finding and not enough to *understand* one. The relay already has
   the text in hand when it builds that envelope.
3. **Or expose a read path**: a verb that prints the admitted artifacts of a lineage while its authority is
   live, so a caller can record them before acknowledging. The window is the transaction, so the verb has to
   work before the receipt is burned rather than after.

What this repository cannot do is patch the store: `gentle-ai` is not code here and no `Cargo.toml` in this
workspace depends on it — it is a tool this repository calls, owned elsewhere. That is why the second half of
this note exists: the practice is the part this repository can execute.

## 3. The practice this repository adopts

Binding on every session that starts a native review here:

1. **Write the findings down in the session that receives them, before acknowledging.** A closure envelope
   prints the id, lens, severity, location and disposition; those are the record. Ack after writing, never
   before — the receipt burn is what closes the window.
2. **Never present an inference as a quotation.** Where the anchor plus the code makes the subject
   reconstructible, say so and say that it is a reading (`odd/tasks/023-script-panel-document.md` L8 is the
   worked example: six anchors, each with a marked reading). Where it is not reconstructible, the finding
   stays open and is named unrecoverable — `odd/tasks/021-debt-advisories.md` L3 is the other worked example.
3. **Never close a finding by guessing an edit that looks like it.** A stale anchor means the record cannot
   support a resolution, and a plausible edit in its place manufactures one.
4. **Read the store before assuming it is empty.** A lineage that was abandoned or reclaimed keeps a full
   `review-state.json` in quarantine; that is the one case where the prose outlives the transaction, and it
   is worth checking before declaring a finding unreadable.
5. **Do not assess a documentation-only candidate.** The provider classifies one as `passive` /
   `non_executable_only` every time — measured four times in a row on `1ad99e8`→`ca24f9e` — so the measurement
   is a turn spent learning what is already known. This is the owner's standing instruction (2026-11-05): a
   commit that touches no executable file is left unreviewed without asking, and the disposition is stated in
   the reply rather than re-derived. Any candidate that does touch executable code is assessed as usual.

The cost of the alternative is already in the record: ten advisories in `odd/tasks/021-debt-advisories.md` that
nobody can close without paying for a fresh review of the tree, and five more from
`review-cf16da5a0b71e7b8` whose anchors survive and whose words do not.
