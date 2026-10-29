# M2 — authoring and editing vectors

The 2D editor's authoring half: everything you can draw, move and change in a document. The whole
milestone is geometry; the shell only collects numbers and turns them into commands.

## The shape of it

Three layers, one direction of dependency:

1. **`cam-geom`** builds and changes geometry. Every builder returns a `Contour`; nothing is
   tessellated on the way in, so an arc you draw is an arc the post-processor can emit as `G2`/`G3`.
2. **`cam-app`** owns the document: it validates, mints ids, records undo and refuses bad input. It
   never computes geometry.
3. **`cam-ui`** collects a gesture or a number and sends one command. It never edits the document.

That split is why the same command is tested headless and why a refusal cannot half-apply.

## Drawing

| What | Command | Geometry you get |
| --- | --- | --- |
| Line | `AddLine` | two vertices, open |
| Polyline | `AddPolyline` | straight segments, open or closed |
| Arc | `AddArc` | one arc through three points, open |
| Rectangle | `AddRectangle` | four corners, closed, counter-clockwise |
| Circle | `AddCircle` | **two half-turn arcs** — an arc may never be a full turn |
| Ellipse | `AddEllipse` | arcs, approximated (see below) |
| Polygon | `AddPolygon` | N straight segments, at least 3 |
| Star | `AddStar` | 2N segments, inner radius strictly inside the outer |
| Text | `AddText` | one entity per glyph stroke |

Closed shapes always come back counter-clockwise. Winding is part of the contract, not an accident of
the order the vertices were computed in.

Every drawing command takes a caller-supplied layer id and entity id (text and the copying commands
take a starting index instead, because they create several entities). Applying a command is a pure
function of its arguments, which is what makes golden tests and bug reports reproducible.

## The two approximations, and why they are visible

An ellipse is not representable by circular arcs. Scaling X and Y differently turns a circle into an
ellipse that circular arcs cannot represent either.

Both cases therefore **approximate and say so**, instead of silently changing the shape:

- `primitive::ellipse(center, rx, ry, tolerance)` returns the contour **and the radial error it
  actually achieved**, measured by dense sampling. Four arcs minimum, doubling until the tolerance is
  met or 2048 arcs are reached — in which case the best attempt comes back with its real error. An
  ellipse whose radii are equal returns the exact two-arc circle with zero error.
- `transform::scale(..)` with `sx != sy` over geometry containing arcs tessellates those contours at
  the tolerance you pass, tightened by the largest stretch so the output never deviates more than you
  asked, and returns `ScaleOutcome { contour, lost_arcs, approximation_error }`. The error is measured
  against the true curve, not assumed from the request.

The shell carries the number through to the status bar: *"…, 3 arc(s) lost to tessellation"*. A user
who scales a circle into an ellipse deserves to know their arcs became straight segments.

## Editing

`transform` — translate, rotate about a point, scale about a point, mirror about a line. All exact on
arcs: the centre, radius and angles move and the bulge is untouched, except that a mirror negates the
bulge because a reflection does flip the turn direction. A closed contour is normalised back to
counter-clockwise after any transform that can flip it.

`edit` — move a vertex, insert one on a segment, delete one. **Insertion projects the click onto the
segment**: along the line for a straight segment, radially onto the circle for an arc. The two halves
of a split arc therefore share the original centre and radius, and their sweeps add up to the
original's. Trusting the raw click would move the geometry; gluing it to an endpoint would create the
zero-length segment that later shows up as a stray move in the program.

`join` — chains a group of open contours end to end, reversing a contour drawn end to start. When the
chain's free ends meet within tolerance it **closes**, so four lines drawn as sides become a rectangle
a profile toolpath can cut.

`trim` — cuts the span between the two crossings with a cutter and returns the removed span plus the
pieces that survive. The removed span is deterministic: walking the contour forward from the first cut
point — the middle of an open contour, the forward half of a closed one. A trim whose result depended
on clicking order could not be reproduced in a bug report.

`offset` — arc-preserving. A shrinking offset that would pass through the shape is **refused**, not
topologically resolved.

## What is refused, and what the message says

Refusals are named rather than absorbed, because a silent no-op looks like a broken program and a
guessed chain produces a cut path nobody asked for:

| Situation | Message |
| --- | --- |
| Three or more contours meet at one point | `edit refused: three or more endpoints meet at one point, so the join is ambiguous` |
| The group does not chain | `edit refused: the contours do not form exactly one chain` |
| Joining a closed contour | `edit refused: a closed contour has no free ends to join` |
| A cutter misses, or only touches tangentially | `edit refused: trim needs exactly two cut points where the cutter crosses the target` |
| A cutter crosses four times | the same message: the choice would be a guess |
| Offsetting an open contour | `edit refused: only closed contours can be offset` |
| An offset larger than the shape | `offset collapsed the contour: an arc radius would collapse` |
| A star whose inner radius is not inside the outer one | `invalid primitive: the star inner radius must be smaller than the outer one` |
| A scale factor of zero | `invalid primitive: the X scale factor must be finite and non-zero` |
| A character the glyph table does not have | `the glyph source has no glyph for '€'` |

A refusal never reaches the document: the command is applied to a copy, validated, and only then
committed, so the document and the undo depth are unchanged.

## Text

Text becomes contours when you create it, so nothing downstream knows text exists and the document
schema stays `c2d/document@1`.

The glyphs come from a **single-stroke table** (79 characters: space, `A-Z`, `a-z`, `0-9` and the
common punctuation), chosen over a TrueType dependency for this milestone. `height` is the cap height
in millimetres; `\n` starts a new line one line advance below. Lowercase is drawn on the x-height
rather than copied from the uppercase shapes.

Both glyph sources implement the same three-method `GlyphSource` trait — a glyph's contours, its
advance, and the line metrics — which is exactly what a TrueType or OpenType implementation will have
to satisfy later. Swapping it changes nothing in `cam-app` or `cam-ui`.

Known limitation, stated plainly: this is not a TrueType renderer. You cannot pick a system font yet.

## Using it in the shell

Pick a tool, then drag on the canvas: press, move, release draws **one** command. A gesture shorter
than three pixels is cancelled with a hint rather than submitting a zero-size shape, and a drag along
a single axis is cancelled too (except for a line, where that is a legitimate horizontal or vertical
line).

Numbers a gesture cannot supply come from documented defaults on the `Shell`:

| Argument | Default |
| --- | --- |
| Rectangle size | 100 × 60 mm |
| Polygon sides | 6 |
| Star points / inner ratio | 5 / 0.5 |
| Array grid / spacing | 2 × 3 / 50 × 25 mm |
| Offset distance | 2 mm |
| Move delta | 10 × 0 mm |
| Rotate angle | 15° |
| Scale factors | 2 × 2 (uniform, so exact on arcs) |
| Text / height | `CAM` / 10 mm |
| Approximation tolerance | 0.01 mm (`DEFAULT_TOLERANCE_MM`) |
| Join tolerance | the kernel's own `DEFAULT_TOLERANCE` (1e-6 mm), not a second number |

Rotate, scale and mirror pivot on the camera centre; the mirror line is the vertical through it.

## Limits worth knowing

- An ellipse and a non-uniform scale are approximations; both report their error and the shell shows
  it. Everything else stays exact.
- `Contour::bounds()` samples arcs through `tessellate(DEFAULT_TOLERANCE)`, so an arc-bearing
  selection's bounds are accurate to about 1e-6 mm rather than exact. Align and the reported lost-arc
  counts inherit that. `Arc::bounds()` is analytic; making `Contour::bounds` union the segments'
  analytic bounds would fix the precision and the per-frame cost at once.
- `OffsetContour` exposes only a distance: the kernel's miter limit and tolerance, and its warning
  when an offset had to drop self-intersections, are not caller-visible yet.
- The shell tops up the entity-id counter after a batch command by consuming the ids the command
  reported. A `AppState::reserve_entity_ids(n)` in `cam-app` would be the honest fix.
- All of it runs on Windows and Linux; the native window needs a display, so its wiring is verified by
  build plus the headless shell tests, not by automation.
