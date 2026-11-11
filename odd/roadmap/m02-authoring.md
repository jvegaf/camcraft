# M2 — authoring

[Back to index](index.md)

### 4.4 M2 delivered (evidence)

| Deliverable | Where | Proof |
| --- | --- | --- |
| Seven exact primitive builders (line, polyline, rectangle, circle, regular polygon, star, arc through three points) | `crates/cam-geom` | 20 tests; analytic area, perimeter and winding, no pre-tessellation |
| Ellipse approximated by arcs with a declared error | `crates/cam-geom` | 10 tests; tolerance met up to 100:1 and re-checked by independent sampling; an unmet tolerance is reported, not hidden |
| Translate, rotate, uniform scale and mirror, exact on arcs; non-uniform scale reports what it cost | `crates/cam-geom` | 15 tests incl. two property tests; winding normalised; the sagitta reported is measured against the true curve |
| Tolerant line/arc intersections, with tangency and coincidence distinguished | `crates/cam-geom` | 18 tests; a tangent is one point and cannot divide a contour |
| Node editing that projects the click onto the segment and splits an arc exactly | `crates/cam-geom` | 14 tests; the halves keep the original centre, radius and total sweep |
| Join with named refusals, closing a ring when the ends meet | `crates/cam-geom` | 12 tests; forks, two-chain groups and closed inputs refused by name |
| Deterministic trim | `crates/cam-geom` | 7 tests; arcs split exactly, four refusal classes |
| Every authoring action as one atomic command through `AppState` | `crates/cam-app` | 73 tests in the command suite; refusals leave document and history depth untouched; one undo per command |
| `lost_arcs` aggregated from the kernel to the caller | `crates/cam-app` | reported in `CommandOutcome` and shown in the status bar |
| Single-stroke text, no new dependency, behind a `GlyphSource` trait | `crates/cam-geom` | 10 tests; 79 glyphs; an unsupported character is refused by name |
| Authoring tools in the shell, drag to draw, outcome in the status bar | `crates/cam-ui` | 18 tests, headless: one command per gesture, a too-short gesture cancelled, one test per action group |
| Native window wired through the same outcome path | `crates/cam-desktop` | builds; the host applies actions through `Shell::apply_action` |

M2 defects found and fixed at the root: `serde_json` parsed floats one ULP off, so the documented
lossless round trip of `c2d/document@1` was false (fixed with `float_roundtrip`); `offset_contour`
reported an open-contour precondition as "offset collapsed the contour", a message about the wrong
thing, and the guard had no test at all; and the desktop window bypassed the shell's outcome path, so
created counts, ellipse error and lost-arc counts never reached the user outside the tests.

Recorded deviations from this milestone's description: text is a single-stroke glyph table rather than
a TrueType renderer, decided with the owner as D-M2-1; a non-uniform scale tessellates and reports the
cost rather than refusing, decided as D-M2-2. Both are in the M2 feature document with their
acceptance criteria. Known limit carried forward: `Contour::bounds()` still samples arcs through
tessellation, so bounds, align and the lost-arc counts are accurate to about 1e-6 mm on arc-bearing
selections rather than exact.

