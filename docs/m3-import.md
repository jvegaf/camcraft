# Importing geometry (M3)

Users bring drawings. This is what `cam-cli import` reads, what survives the trip, and what it tells
you it could not keep.

```console
$ cam-cli import part.dxf --out part.c2d
format: dxf
tolerance: 0.010 mm
unit scale: 1.000000
contours: 6
flattened curves: 0
sources: ARC=2 CIRCLE=1 LINE=3
skipped: 1
  - TEXT at line 412: not a supported DXF entity: dxf imports LINE, LWPOLYLINE, POLYLINE, ARC, CIRCLE and ELLIPSE
wrote 1 layer(s), 6 entity(ies) to part.c2d

$ cam-cli check part.c2d
E7 self-intersection at 42.500, 18.000 (segments 3, 7)
error: 1 vector issue(s) found
```

## Formats

| Format | Geometry | Arcs stay arcs | Units |
| --- | --- | --- | --- |
| DXF (ASCII) | `LINE`, `LWPOLYLINE`/`POLYLINE` with bulges, `ARC`, `CIRCLE`, `ELLIPSE` | yes: a bulge is an `Arc`, a circle is two half-turns | `$INSUNITS`, then `$MEASUREMENT`, then millimetres with a warning |
| SVG | `<path>` (full grammar), `<line>`, `<polyline>`, `<polygon>`, `<rect>`, `<circle>`, `<ellipse>`, `transform` on any element and inherited through `<g>`, `viewBox` | yes for a circular `A` arc | unitless = px at 96 dpi; `mm`, `cm`, `in`, `pt`, `pc`, `px` accepted |
| PDF | content-stream paths of every page, through `cm` and the `MediaBox` origin | no: PDF has no circular-arc operator | one user unit is one millimetre, with a warning (a PDF path carries no physical size) |
| EPS / PS | `moveto lineto curveto rlineto rcurveto closepath arc arcn` under `translate scale rotate concat setmatrix`, painted by `stroke fill eofill clip` | yes: `arc`/`arcn` become `Arc`s, a full turn is a closed circle | PostScript points: 25.4 / 72 mm |
| PNG | traced from a global threshold: luminance → mask → contours | no: a trace is a polyline by nature | `--dpi` (default 96) decides pixels per millimetre |

Everything else is either converted or listed in the report — never dropped in silence.

## The report

Every import returns the same report, and the CLI prints it whole:

| Line | Meaning |
| --- | --- |
| `format` | The format that was detected |
| `tolerance` | The flattening tolerance applied to curves that could not stay arcs |
| `unit scale` | The factor applied to reach millimetres (1.0 when the source is already mm) |
| `contours` | Contours produced across all layers |
| `flattened curves` | Curves that became line segments because they could not be exact arcs |
| `sources` | How many of each source construct were converted (`LINE=3`, `<path>=2`, `re=1`, `region=4`) |
| `skipped` | One entry per construct that produced no geometry, with the reason and a source location |
| `warnings` | Assumptions a user must be able to see (an assumed unit, a lost alpha channel, a substituted default) |

## Rules that apply to every format

- **Millimetres in, millimetres out.** No stage downstream sees a source unit.
- **Arcs stay arcs.** A circular arc in the file never becomes a polyline: G2/G3 output depends on it.
- **Flattening is counted.** A Bézier, an elliptical or rotated SVG arc, a partial DXF ellipse and a
  PDF/EPS curve are flattened at `--tolerance` (default `0.01` mm) and reported. Non-uniform transforms
  in EPS turn a circle into an ellipse; that is flattened, counted and reported too.
- **A file that yields nothing is an error**, not an empty document: `nothing to import: the file
  contains no vector geometry`, or the reason why every construct was skipped.
- **Import never edits an open document.** It builds a new one: the stock becomes the imported bounds
  plus a 10 mm margin, the geometry is placed at 10 mm from the job's lower-left corner, layers become
  `L1..Ln` and entities `E1..En`. In the shell, `ImportFile` does that undoably, so one undo brings the
  previous drawing back.

## Checking what came in

`cam-cli check <document>` reports the two problems that make a vector uncuttable, plus degenerate
segments:

| Line | Meaning |
| --- | --- |
| `E3 open: gap 0.0210 mm` | The contour's ends do not meet; the gap is what the tool would leave |
| `E7 self-intersection at 12.500, 4.000 (segments 3, 7)` | The contour crosses itself; the segment indices locate it |
| `E9 degenerate segment 2` | A segment shorter than the 1 µm tolerance |

Exit code `5` when anything was found, `0` with `all vectors valid` when the document is clean. `check`
is what a build script should gate on: a profile cut through an open or self-crossing contour is a
broken part, not a warning.

## Known limits, recorded so they are not discovered later

- **DXF**: `INSERT`/`BLOCKS` are not expanded, and `TEXT`, `MTEXT`, `DIMENSION`, `HATCH`, `SPLINE`,
  `POINT` are skipped with their line number. Binary DXF is refused by name. `POLYLINE` vertex flags
  (spline frames, 3D faces) are ignored: every vertex is read as a flat 2D vertex with its bulge.
  A partial `ELLIPSE` is the one entity that loses its exactness.
- **SVG**: `<text>`, `<image>`, `<use>`, `<defs>` are skipped; `style`/`class` do not change geometry, so
  a shape is imported even when it is `fill="none"` (a boundary is what a cutter can follow).
  `preserveAspectRatio` is applied as its default `xMidYMid meet`.
- **PDF**: text is not traced (only paths), images, form XObjects, shadings, inline images and Type3
  procedures are counted; encrypted documents are refused. A `W f` clip-and-fill loses the fill, because
  a clip discards the path.
- **EPS**: operators outside the path set are ignored; a full-turn `arc` starts its own subpath, so a leg
  drawn before it stays a separate contour. Style and dictionary operators are ignored without noise.
- **PNG**: 8-bit greyscale, RGB, palette and RGBA, non-interlaced only — 16-bit and interlaced files are
  refused by name. Tracing is a global threshold (default 128), not adaptive; alpha is reported when the
  image is not opaque; chunk CRCs are required but not verified.
- **All of it** is a hand-written reader: no parser, XML or image crate was added. The only new
  dependency is `flate2`, already in the tree as `zip`'s DEFLATE backend, for PDF `FlateDecode` and PNG
  `IDAT`.
