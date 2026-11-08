# M12 — Export: the document goes back out

Feature document for milestone **M12** of `odd/roadmap.md`. Read **until `## Log`** before touching code.

## Specs

- **S1 (user, verbatim):** "vamos con el export". The scope question was cancelled, so this document states
  the scope that was recommended and leaves the alternative visible: **DXF and SVG, geometry only**
  (layers and contours). Toolpaths as extra layers are a later slice, and the reason is in S5.
- **S2 (the measured gap):** there is **no writer of any interchange format** in the workspace. What writes
  today is `.c2d` (ZIP), G-code, cut-list CSV, the HTML print sheet and the SQLite library
  (`crates/cam-store/src/document.rs:179`, `crates/cam-cli/src/lib.rs:3109`, `crates/cam-cli/src/pieces.rs:307`,
  `crates/cam-cli/src/printsheet.rs:89`). `crates/cam-import/src/lib.rs:3-5` says it plainly: "nothing is
  written to disk here". And `crates/cam-ui/src/icons.rs:1041`/`:1045` already declare `export-dxf` and
  `export-svg` icons that no command references — the product promised the feature and never shipped it.
- **S3 (what the importer can read back, so what "round trip" means):** the geometry the model holds is a
  contour of vertices carrying a **bulge** (`crates/cam-geom/src/contour.rs:13-18`, sweep `4·atan(bulge)`,
  positive counter-clockwise — the DXF convention), a full circle is **two vertices at bulge 1.0**
  (`primitive.rs:85-95`), and closedness is explicit (`contour.rs:34-42`). The DXF reader parses `LWPOLYLINE`
  bulges (`crates/cam-import/src/dxf.rs:~620`), `ARC`, `CIRCLE`, `ELLIPSE` and `$INSUNITS`
  (`dxf.rs:263-300`), and its layer table order becomes the document's layer order
  (`tests/dxf.rs:404`). The SVG reader parses `<path>` with `A` when `rx == ry`, taking the bulge from
  `arc.bulge()` (`crates/cam-import/src/svg.rs:1674`), and reflects Y as an isometry the tests pin
  (`tests/svg.rs:440`). So both writers are the **inverse of code that exists**, not a new format.
- **S4 (the acceptance test, and the offset that is known):** a document exported and imported again must
  come back **the same geometry and the same layer names in the same order**, compared as the import
  places it — the reader translates the bounding box to a 10 mm margin and re-derives the job size
  (`crates/cam-import/src/document.rs:20,42-45`, pinned by `tests/import.rs:114`). The round trip is
  therefore asserted **as the same shape up to that translation**, never as byte equality, and the test
  says which offset it subtracted.
- **S5 (the losses, named rather than discovered later):** SVG collapses every layer into one named `"svg"`
  (`svg.rs:120-123`, `tests/svg.rs:26`), so **DXF keeps layers and SVG documents that it does not**; the
  model has one entity kind, so a `LINE`/`ARC`/`CIRCLE` distinction and an ellipse's axis ratio are gone
  and re-export as polylines with bulges; flattened input cannot be un-flattened; and blocks, `TEXT`,
  `INSERT` and styling were dropped at parse. Toolpaths are **not** written in this slice: a toolpath is
  not geometry, and writing one would make "the same thing that went in" unprovable, which is the only
  acceptance test this milestone can honestly offer.
- **S6 (repo law):** a new crate rather than a new module inside `cam-import`, mirroring the direction
  rather than bending the reader's name; no external dependency, since both formats are text we can write;
  the CLI flag loop follows `import_command`'s house style (`crates/cam-cli/src/lib.rs:522-610`) with
  `Failure::input` for bad input; and `docs/format.md` gains the row plus the exit codes it already lists.

## Tasks

| ID | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | `cam-export`: the crate, its `Format`/error/dispatch shape mirroring `cam-import`'s, and the **DXF** writer — one `LWPOLYLINE` per contour carrying each vertex's bulge verbatim, `70=1` when closed, `$INSUNITS=4`, the layer table in document order | S2 S3 S5 | worker | `751d918` |
| T2 | The **SVG** writer — one `<path>` per contour with `A` derived from the same bulges and the Y reflection the reader inverts, `viewBox` and size derived from the document's own bounds so the mm scale is 1.0 | S3 S5 | worker | `751d918` |
| T3 | Round-trip tests: export → `cam-import` → the same geometry and layer order up to the known 10 mm placement, for both formats, with the loss each format carries asserted explicitly | S4 S5 | parent | `751d918`, `1b349e4` |
| T4 | `cam-cli export --format dxf\|svg --out`, the help row, `docs/format.md`'s command and exit-code rows, and the roadmap's evidence | S6 | parent | `751d918`, `986678c` |
| T5 | The review of this candidate | — | parent | `1b349e4` |

T1 and T2 landed together in `751d918` — the crate, both writers and their three test files in one commit.
T3's comparator was rewritten in `1b349e4`, which is the same commit as T5's seven fixes and its Log section
(L6); T4's roadmap row arrived separately, in `986678c`.

## Log

- **L1 (user, verbatim):** "vamos con el export". The scope question (DXF only / DXF + SVG / plus
  toolpaths) was **cancelled, not answered**, so S1 records the recommended scope and the alternative —
  toolpaths as extra layers — stays visible for a later slice rather than being silently dropped.
- **L2 (the reconnaissance, which is what makes the round trip provable).** Everything S3 cites was read
  before a line was written: the bulge convention, the two-vertex circle, the DXF layer-table order, the
  SVG `A`-when-`rx == ry` rule and its Y reflection, the `$INSUNITS` codes, and the placement margin that
  makes byte equality the wrong assertion (S4). Two facts decided the design: **the only existing round-trip
  test in the workspace is a circle through `serde_json`** (`crates/cam-import/tests/import.rs:204`), and
  every import fixture is **built in code**, so the export tests can build their documents the same way
  instead of inventing fixture files.
- **L3 (T1 and T2, and what the reader demanded that DXF did not).** `cam-export` exists as its own crate
  mirroring `cam-import`'s shape — `Format`, `ExportError`, one module per format, an exhaustive dispatch
  so a new format cannot be half-wired — with no external dependency, because both formats are text. It
  writes **ASCII DXF** (`$ACADVER AC1015`, `$INSUNITS 4`, a `LAYER` table in document order, one
  `LWPOLYLINE` per contour with `90` the real vertex count, `70` closedness only, and `42` carrying each
  vertex's bulge verbatim) and **SVG** (one `<path>` per contour, `A` with `rx == ry` and `rot == 0`
  because that is the only arc form the reader keeps exact, plus the two flags it derives a bulge from).
  The reader's requirements that DXF never imposed, each measured before writing: a **Y reflection** about
  the page's horizontal centre; `rx == ry` numerically, or the arc is flattened; **two SVG-only flags** —
  `sweep` 1 exactly when the document arc turns clockwise, `large-arc` 1 when `|sweep| > π` — which mirror
  the arc instead of failing loudly when wrong; an **explicit closing `A` before `Z`** when a closed
  contour's closing segment is an arc, because `Z` closes with a straight line and the bulge has nowhere
  else to live; and a **positive page** whose `width`/`height` equal the `viewBox` extents, since the
  reader's scale is `min(w/box_w, h/box_h)` and is 1.0 only when they agree.
- **L4 (T3: the round trip, and the mutation that proves it discriminates).** Four tests over one
  comparison: both sides tessellated, each **normalised to its own bounding box** (the 10 mm placement the
  reader performs, S4), compared as point sets **in both directions** — a subset would pass a shape that
  merely contains the other — with closedness asserted separately, since `70` is the only signal DXF
  carries for it. Fixtures: an open contour with a straight and a curved leg, a closed rectangle, and a
  full circle as two half turns. **DXF keeps its layers and their order; SVG is asserted to lose them**,
  including that no layer name reaches the file. The mutation: forcing the DXF bulge to `0.0` fails the
  round trip on its own — "a written point (20.2317, 0.2372) is 0.3316 mm from the read shape" — so the
  test observes the arc rather than the file's shape. One fault was mine and worth recording: the SVG test
  first compared a two-layer page against one collapsed layer and saw the circle twice, which is correct
  behaviour of the reader and a wrong fixture of mine; the shape comparison now runs over a one-layer page
  and the layer loss is asserted on the two-layer one.
- **L5 (T4: the command, and what it refuses).** `cam-cli export <document> --format dxf|svg [--out]`, with
  `--out` writing and announcing and no `--out` printing to stdout the way `emit` prints a program; a
  format this build does not write is refused by name (`export writes dxf or svg, not "pdf"`, exit `2`)
  rather than guessed from an extension, and a missing `--format` is refused rather than defaulted.
  `Format::from_name` joins the extension lookup the crate already had. `docs/format.md` gains the command
  row and the losses it carries; the roadmap gains the M12 row.
- **L6 (the review: approved first pass, and the nine findings that were still worth fixing).** Lineage
  **`review-a7e624d841d3d0c6`**: tier `high`, 2156 lines over 15 paths, four lenses, closure **`approved`
  on the first admitted event** with no correction, receipt burned (`sha256:193e1fd6…`). Nine findings,
  all **informational** by the provider's own statement, and seven of them were real defects rather than
  taste — fixed in the same milestone because each is small and one is destructive:
  1. `R4-SAME-PATH` and `R3-001`, the same finding twice: `export part.c2d --out part.c2d` **replaced the
     document with DXF and reported success**. `--out` is now refused when it canonicalises to the
     document being read, and the test asserts the document is byte-identical afterwards.
  2. `R4-ATOMIC-OUT`: `fs::write` truncates before it completes, so an interrupted export left an empty or
     half-written `.dxf` beside a live document. The bytes now land in a sibling temporary and are renamed;
     the test asserts a second export over the same path succeeds and leaves no temporary behind.
  3. `R4-STDOUT-PANIC`: `print!` panics on a closed pipe, so `export | head` exited `101` with a backtrace
     instead of a failure a script can read. It now writes with `write_all` on the locked stdout and maps
     the error. The closed-pipe case is the one thing this repository's harness cannot reproduce, and that
     is recorded rather than claimed.
  4. `R2-003` and `R3-002`, again one finding from two lenses: my round-trip comparator normalised **each
     contour to its own bounding box**, so a writer that shifted one contour relative to another would
     still pass — it did not prove S4 at all. Both sides are now framed by **one origin for the whole
     side**, and the comparators take slices rather than pairs.
  5. `R3-003`: a point-set comparison cannot see a phantom zero-length segment, so the round trip also
     asserts vertex and segment counts for every contour — the circle being the case the SVG writer's
     explicit closing `A` could have doubled.
  6. `R2-002`: `export`'s doc said the writer "refuses nothing the model allows", which hid a real refusal
     surface (DXF layer names the reader would alter, an SVG of a document with no geometry). It now names
     them.
  7. `R2-001`: the SVG module's identity omitted `min.y` from `Page::place`'s formula, so the sentence that
     explains the Y reflection was false for any page not anchored at zero.
  What the review did **not** find is as telling: neither writer lost an arc, a layer order or a closed
  contour, and the round trip held through both formats on the first run — the byte-level tests the two
  worker units were built against did their job.
