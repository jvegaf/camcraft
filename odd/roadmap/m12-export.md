# M12 — export

[Back to index](index.md)

### 4.15 M12 delivered (evidence)

| Deliverable | Where | Proof |
| --- | --- | --- |
| A `cam-export` crate mirroring the reader's shape: bytes in millimetres out, no filesystem, no document mutation, and the same document always exporting to the same bytes | `crates/cam-export` | the crate writes a file the caller owns, keeps every bulge verbatim so the arc the kernel holds is the arc the file carries, and its identity is `(Format, error, dispatch)` beside `cam-import`'s |
| **DXF**: one `LWPOLYLINE` per contour, each vertex's bulge verbatim, `70 = 1` when closed, `$INSUNITS = 4`, the layer table in document order | `crates/cam-export/src/dxf.rs` | `crates/cam-export/tests/dxf.rs`: layer identity survives, a closed contour closes, the unit header is written |
| **SVG**: one `<path>` per contour with `A` commands whose `rx == ry`, the Y reflection the reader inverts, a `viewBox` in the document's own millimetres | `crates/cam-export/src/svg.rs` | `crates/cam-export/tests/svg.rs`: a circle stays two arcs and is not doubled by the explicit closing `A`; the documented loss — SVG carries no layers — is asserted on a two-layer document rather than omitted |
| The acceptance test is the **round trip against our own reader**, never byte equality: exported and imported again, the same shape and the same layer order, compared up to the 10 mm placement the reader performs | `crates/cam-export/tests/round_trip.rs` | both sides framed by **one origin for the whole side**, the comparators taking slices rather than pairs, plus vertex and segment counts per contour so a phantom zero-length segment cannot pass |
| `cam-cli export <document> --format dxf\|svg [--out]`, writing and announcing with `--out` and printing to stdout without it | `crates/cam-cli` | a format this build does not write is refused by name (`export writes dxf or svg, not "pdf"`, exit `2`) and a missing `--format` is refused rather than defaulted |
| The review | lineage **`review-a7e624d841d3d0c6`** | tier `high`, 2156 lines over 15 paths, four lenses, closure **`approved` on the first admitted event** with no correction, receipt burned (`sha256:193e1fd6…`); nine findings, all informational by the provider's own statement, and seven of them real defects |

The seven fixes are worth naming because each was small and one was destructive: `--out` canonicalising to
the document being read **replaced a document with DXF and reported success** (now refused, with the document
asserted byte-identical afterwards); `fs::write` truncating before it completes (now a sibling temporary
renamed into place); `print!` panicking on a closed pipe, so `export | head` exited `101` with a backtrace
(now `write_all` on the locked stdout with the error mapped — and the closed-pipe case is the one thing this
repository's harness cannot reproduce, which is recorded rather than claimed); a round-trip comparator that
normalised **each contour to its own bounding box**, so a writer shifting one contour relative to another
would still pass and S4 was not proved at all; a point-set comparison blind to a phantom zero-length segment;
`export`'s doc claiming the writer "refuses nothing the model allows", which hid a real refusal surface; and
the SVG module's identity omitting `min.y`, which made the sentence explaining the Y reflection false for any
page not anchored at zero.

What the review did **not** find is as telling as what it did: neither writer lost an arc, a layer order or a
closed contour, and the round trip held through both formats on the first run — the byte-level tests the two
worker units were built against did their job. **Toolpaths are not written**: a toolpath is not geometry, and
writing one would make "the same thing that went in" unprovable.

