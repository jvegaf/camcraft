# Development Plan — 2.5D CAM application (Windows + Linux)

Status: **executed through M15.** Each milestone's acceptance is that milestone's own measurements — its
tests, oracles, goldens and differentials — and the roadmap is where they are carried: M0–M13 are delivered,

**On the names in this history.** The tree no longer names the third-party product the plan once learned from:
`README.md`, `docs/`, `odd/` and the evidence dossier were swept, and `grep -ri` over the current tree returns
nothing but the record of the instruction itself. The **history** is a different thing: the rewritten commits
are published as `hygiene/master-rewritten` and `hygiene/m13-rewritten`, and `master` was deliberately left in
place because moving it needs a force-push. Swapping them is a one-command decision with an owner, and until it
is taken, `git log -S` on a clone of `master` still finds the old names.

§4 records the evidence for each, section by section through §4.16, and §6 the risk register with the one row M7 closed. **After M10 the work did not
continue as milestones**: the findings those milestones left open became three recorded features with their
own documents — the advisory debt (`odd/tasks/021-debt-advisories.md`, which carries its own counts and the reason
the rest cannot be recovered),
the script panel's mount (`odd/tasks/022-script-panel-mount.md`) and the panel's document
(`odd/tasks/023-script-panel-document.md`) — and the roadmap's Log in §8 names all three.

Owner decisions taken at plan time (verbatim from the decision record, see `## Log`):

| # | Decision |
| --- | --- |
| D1 | Stack: **Rust**, viewport **wgpu**, UI shell `egui`/`cxx-qt` |
| D2 | Scope: **all toolpath strategies except 3D machining and Laser** |
| D3 | Document format: **own open format (ZIP + JSON)**, interop via DXF/SVG/PDF/EPS import |
| D4 | Nesting: **own heuristic engine**, in v1 |

---

