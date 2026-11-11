# M1 — editor

[Back to index](index.md)

### 4.3 M1 delivered (evidence)

| Deliverable | Where | Proof |
| --- | --- | --- |
| Application state with exact snapshot undo/redo, bounded history | `crates/cam-app` | 12 tests; a command sequence undone five times equals the original document byte for byte |
| Camera: world/screen mapping, zoom about the cursor, clamping, fit | `crates/cam-app` | 11 tests incl. a property test over the round trip |
| Commands: job size, rectangle, circle, delete, layer visibility | `crates/cam-app` | 10 tests; refusals never reach the document |
| Backend-agnostic scene and grid | `crates/cam-view` | 10 tests; arcs stay arcs, hidden layers disappear from scene and picking |
| Picking, marquee and snapping in screen pixels | `crates/cam-view` | 13 tests incl. tie-breaks and zoom independence |
| egui shell: panels, canvas, properties, status | `crates/cam-ui` | 16 tests running real `egui` frames headless and reading back painted text |
| Pointer, wheel and keyboard wiring | `crates/cam-ui` | 10 tests with synthetic clicks, drags, wheel and keys |
| Native window for Windows and Linux | `crates/cam-desktop` | builds; launched under a compositor, screenshot in `docs/old/images/editor-m1.png` |

M1 defects found and fixed at the root: the entity id counter ignored ids already in the opened
document (duplicate `E1`); arc bounds were computed by tessellation, so a circle's bounds were off by
the chord tolerance and every frame re-tessellated every arc; the status bar showed the previous
frame's cursor because panels are laid out before the central area; and auto-framing fitted a
provisional canvas before the panel widths settled.

Recorded deviation from this roadmap's crate description: `cam-view` computes a backend-agnostic
`Scene` and the shell paints it with `egui`; a `wgpu` backend will consume the same `Scene` when the
3D simulation (M5) needs the GPU. See the M1 feature document, decision D-M1-2.

