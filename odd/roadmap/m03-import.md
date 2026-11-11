# M3 — import

[Back to index](index.md)

### 4.5 M3 delivered (evidence)

| Deliverable | Where | Proof |
| --- | --- | --- |
| Import contract: bytes + options in, millimetre contours + report out, no filesystem and no document mutation | `crates/cam-import` | 10 tests; detection by extension then by content signature; the report renders whole and identically every time |
| Document assembly: stock fitted to the imported bounds plus a 10 mm placement margin, `L1..Ln`, `E1..En` | `cam-import::into_document` | 3 tests; exact placement and job size, single-layer override, `validate()` runs on the result |
| DXF: `LINE`, `LWPOLYLINE`/`POLYLINE` with bulges, `ARC`, `CIRCLE`, `ELLIPSE`, layers, `$INSUNITS`/`$MEASUREMENT` | `cam-import::dxf` | 25 tests; a bulge is an exact arc, a full ellipse is rotated exactly, a partial one is flattened and counted, skipped entities carry their line number |
| SVG: hand-written XML reader, the shape set, inherited `transform`, `viewBox` scaling, exact circular arcs, y inversion (D-M3-10) | `cam-import::svg` | 30 tests; a circular `A` stays an `Arc` and an elliptical one is flattened and counted, the path grammar covers relative and implicit repeats, `viewBox` + `width` gives the exact millimetre size, the reflection keeps both of a circle's arcs, a mismatched tag is located |
| PDF: tolerant object scan, page tree with inherited `MediaBox`, `FlateDecode`, content-stream paths through `cm` | `cam-import::pdf` | 16 tests; compressed and uncompressed streams give identical contours, two pages import in order, encryption is refused by name |
| EPS: PostScript path subset with a transformation stack | `cam-import::eps` | 26 tests; `arc` of 360° is a closed circle of two arcs, `curveto` is flattened and counted, `%%BoundingBox` places the drawing |
| Raster trace: hand-written PNG decoder plus deterministic contour extraction | `cam-import::raster` | 16 tests; all five scanline filters, holes wound the opposite way, `--min-area`/`--simplify` reported, same bytes give the same contours |
| Vector validation in the kernel: open gap, self-intersections with segment indices, degenerate segments | `cam-geom::validate` | 10 tests incl. two property tests; adjacent segments are never intersected, so a valid polygon is clean |
| `cam-cli import` and `cam-cli check`, with exit `5` for vector issues | `crates/cam-cli` | 10 binary tests; exact stdout, exit codes, a conversion written and reloaded |
| Shell import: `UiAction::ImportFile`, undoable replacement, outcome in the status bar | `crates/cam-ui`, `crates/cam-app` | 5 tests; one undo restores the previous drawing, a failed import changes nothing, the status sentence is asserted whole |

M3 defects found and fixed at the root: `eps::import` had invented its own parse error for a file that
draws nothing, because one spine test only accepted `Refused` or `Parse` — the test was pinning the stub,
not the contract; every importer now returns an empty layer set and the spine reports `NothingImported`
(`ebb9e1d`). Recorded, not fixed: `replace_document` clears history, so the import path needed
`replace_document_undoable` rather than reusing it.

**SVG (T4) closed in a later session.** The parked work on branch `import/svg` was the starting point, and
its three real defects were found and fixed at the root: the path-data reader scanned each coordinate pair
into a throwaway cursor, so it never advanced and every `d` attribute hung; a unitless number was read as a
CSS pixel on shape attributes too, so a 100 mm page came out at 26.46 mm; and `point()` borrowed `self`
immutably, which is what made the first defect possible. The owner's decision that SVG user space is
**y-down** is now D-M3-10: the root mapping composes a reflection, which is an isometry, so arcs stay exact
with their bulge sign flipped and no curve is flattened for it. One parked test asserted both that a
mismatched tag is located at the *closing* tag and that an unclosed one is located at the *opening* tag; the
two now test their own case, and a genuinely unclosed element at end of input points at the line it opened
on. The parked suite's y expectations were written before the flip and now state it through an `up()`
helper, so no assertion hides the convention.

Recorded deviation: PDF imports one user unit as one millimetre with a warning, because a PDF path carries
no physical size and A9 names no scale. Known limit carried forward: a `W f` clip-and-fill in PDF loses the
fill, because a clip discards the path.

