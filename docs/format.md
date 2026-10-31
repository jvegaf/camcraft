# Formats and contracts

Three artefacts carry the data this application owns. All three are versioned and all three refuse
input they do not fully understand, because a CAM program that half-reads a file produces a wrong
part.

- [Document `.c2d`](#document-c2d)
- [Tool library SQLite](#tool-library)
- [Post-processor `.pp`](#post-processor)
- [Command-line contract](#command-line-contract)

---

## Document `.c2d`

A ZIP archive. One required entry, `document.json`; every other entry is preserved byte for byte on a
load/save cycle even though this build does not interpret it, so a future version's assets survive a
round trip.

```
part.c2d
└── document.json      pretty-printed JSON, schema c2d/document@2
```

```json
{
  "schema": "c2d/document@2",
  "job": {
    "width": 1219.0, "height": 2438.0, "thickness": 18.0,
    "units": "mm", "origin": "bottom_left", "safe_z": 5.0
  },
  "layers": [
    { "id": "L1", "name": "Part", "visible": true,
      "entities": [
        { "id": "E1", "type": "contour",
          "contour": { "closed": true,
            "vertices": [ { "point": { "x": 0.0, "y": 0.0 }, "bulge": 0.0 } ] } }
      ] }
  ],
  "toolpaths": [
    { "id": "TP1", "name": "Outside profile", "entity_ids": ["E1"], "tool_number": 1,
      "operation": { "kind": "profile",
        "side": "outside", "allowance": 0.0, "pass_depth": 6.0, "start_depth": 0.0,
        "cut_depth": 18.0, "lead_in": 3.0, "lead_out": 3.0, "tabs": [] } },
    { "id": "TP2", "name": "Pocket 2", "entity_ids": ["E2"], "tool_number": 1,
      "operation": { "kind": "pocket", "strategy": "offset", "allowance": 0.0,
        "stepover": 2.4, "pass_depth": 6.0, "start_depth": 0.0, "cut_depth": 9.0 } },
    { "id": "TP3", "name": "Drilling 3", "entity_ids": ["E3", "E4"], "tool_number": 2,
      "operation": { "kind": "drilling", "peck_depth": 3.0,
        "retract": { "mode": "amount", "amount": 1.0 },
        "start_depth": 0.0, "cut_depth": 12.0,
        "bank": { "count": 3, "pitch": 32.0 } } },
    { "id": "TP4", "name": "Chamfer 4", "entity_ids": ["E1"], "tool_number": 3,
      "operation": { "kind": "chamfer", "chamfer_width": 3.0,
        "included_angle": 1.5707963267948966, "start_depth": 0.0 } },
    { "id": "TP5", "name": "Engrave 5", "entity_ids": ["E5"], "tool_number": 3,
      "operation": { "kind": "engrave", "start_depth": 0.0, "cut_depth": 2.0 } },
    { "id": "TP6", "name": "Fluting 6", "entity_ids": ["E6"], "tool_number": 4,
      "operation": { "kind": "fluting", "start_depth": 0.0, "cut_depth": 1.5, "ramp": 10.0 } },
    { "id": "TP7", "name": "Texture 7", "entity_ids": ["E2"], "tool_number": 4,
      "operation": { "kind": "texture", "start_depth": 0.0, "cut_depth": 0.8,
        "spacing": 2.0, "angle": 0.7853981633974483 } },
    { "id": "TP8", "name": "inlay female", "entity_ids": ["E2"], "tool_number": 1,
      "operation": { "kind": "inlay", "allowance": 0.5, "stepover": 2.4,
        "pass_depth": 6.0, "start_depth": 0.0, "cut_depth": 9.0 } },
    { "id": "TP9", "name": "inlay male", "entity_ids": [], "island_ids": ["E2"],
      "tool_number": 1,
      "operation": { "kind": "inlay", "allowance": 0.5, "stepover": 2.4,
        "pass_depth": 6.0, "start_depth": 0.0, "cut_depth": 9.0 } }
  ],
  "tools_ref": "tools.sqlite",
  "postp": "mach3.pp"
}
```

### Rules

| Topic | Rule |
| --- | --- |
| Units | Every length in the document is **millimetres**. `job.units` records the unit the user works in and nothing else converts on read. |
| Schema | This build **writes** `c2d/document@2` and still **reads** `c2d/document@1`, upgrading it in memory on load so a save never writes the old tag back. Anything else is rejected naming both the found and the supported version. `@2` only added `Toolpath.island_ids`: the bump exists because an older build ignores a field it does not know, so keeping the tag would let it pocket straight through an island. |
| Geometry | A contour is an ordered list of vertices; `bulge` is the DXF bulge of the segment leaving that vertex (`0` = straight, `tan(sweep/4)` = arc, positive counter-clockwise). A full circle is two half-turn arcs. |
| Validation | Contours are validated **on deserialisation**: too few vertices, coincident consecutive vertices and impossible bulges fail the load instead of producing geometry. |
| References | Toolpath `entity_ids` must resolve. Every operation that cuts a closed contour — `profile`, `pocket`, `drilling`, `chamfer`, `inlay` — must reference closed contours, and an open one is refused by name. `texture` needs closed geometry too, because its grooves are clipped to a region and an open vector bounds none. `engrave` and `fluting` are the exceptions: they cut along the vector, so an open one is legal — engraving a single line is the engrave's point, and a flute is one groove per vector drawn as a line as often as a loop. Ids are unique within their kind. |
| Job | `width`, `height` and `thickness` must be positive and finite. |
| Origin | `bottom_left`, `top_left` or `center`. The mapping onto machine coordinates happens once, in the toolpath layer. |
| Extensibility | `EntityKind` and `Operation` are internally tagged enums. An unknown tag is a **hard error**: a document written by a newer build never half-loads, it is refused by name. Because of that, adding a variant is additive and keeps the schema tag; changing what an existing field means, or its units, is what makes a `@2`. |
| `RetractMode` | `{"mode":"full"}` or `{"mode":"amount","amount":1.0}`. A tagged **newtype** over a bare number cannot carry the tag, so the amount is a named field. |
| `bank` | Omitted when the machine has no drill gang, so a document that does not use one is byte-identical to one written before banks existed. |
| `island_ids` | Omitted when empty. Names the closed contours to leave standing inside a `pocket`'s region; nothing is inferred from the drawing, so the field *is* the island list. The model refuses an island that is not a closed contour, that is not strictly inside one of the toolpath's boundaries, that is not disjoint from another island, that repeats an id, or that belongs to any operation other than a pocket or an inlay. For an **inlay** the field is also what says which half the toolpath is (see the `inlay` row). See [region-booleans.md](region-booleans.md). |
| `chamfer` | A V-bit bevel on the selected closed contours: the tip travels **on** the vector, in one pass, and the bevel is symmetric about it. `chamfer_width` is the bevel's horizontal width in millimetres and `included_angle` is the bit's full included angle in **radians**, as the library stores it. There is no `cut_depth`: the depth is **derived** — `start_depth + chamfer_width / tan(included_angle / 2)` — and never stored, so a stored value cannot disagree with the bit it came from. A width that is not positive, or an angle outside `(0, π)`, is refused naming the offending value. The derivation assumes a sharp point, so a bit with a flat tip is refused by name as well: with a flat of diameter `f` the bevel would reach the surface `f/2` wider than `chamfer_width`. See [m4-toolpaths.md](m4-toolpaths.md) for the neighbouring operations. |
| `fluting` | A flute on the selected vectors, **open or closed**: one pass at `cut_depth`, with the tool's centre **on** the vector, and one **ramp**. `ramp` is a *length* in millimetres, not a depth: with `ramp > 0` the pass enters over the first `ramp` millimetres of the vector and leaves over the last, so the tool never plunges at full depth, and `ramp = 0` is the plain groove — a plunge in place at the start and a retract in place at the end. A ramp longer than **half** the vector is an input error naming the ramp and the vector's length rather than a silently shortened ramp, and a negative one is refused by name when the document is validated and again by the strategy; a closed contour measures the ramp against its perimeter. A contour that contains an **arc** cannot carry a ramp, and that is refused by name too: a ramped move cannot ride on a move kind that is constant-Z, and the move vocabulary has no helical move yet (T7 adds one), so the ramp would be cut as a straight chord — `ramp² / (8·r)` inside the vector, half a millimetre for a 10 mm ramp on a 25 mm circle. The command refuses that selection at authoring and the strategy refuses it again at `emit` and `simulate`, so a document that already carries one is an input error rather than a chorded program; `ramp = 0` on the same contour is unaffected, because a constant-depth pass along an arc is exactly what an arc move is for. `cut_depth` must be greater than `start_depth`, as for an engrave. There is no side, no `pass_depth`, no tab, no lead, no `stepover` and no `allowance`, and the CLI refuses each of those flags by name. `ramp` **defaults to zero** on load, so a hand-written or older document that names a `fluting` toolpath without it loads as the in-place plunge it describes instead of failing the load. See [m5-simulation.md](m5-simulation.md) for the groove the simulation claims. |
| `engrave` | A quick engrave on the selected vectors, **open or closed**: one pass at `cut_depth`, with the tool's centre **on** the vector, and nothing else. `start_depth` is where the pass begins, so the deepest cut is `cut_depth` itself; `cut_depth` must be greater than `start_depth` or the toolpath is an input error naming both numbers. There is no side, no `pass_depth`, no tab and no lead — a second pass would cut a second groove into the first — so the CLI refuses each of those flags by name instead of ignoring it. Every other operation here needs a closed contour; an engrave of a single open line is the strategy's point. The depth is authored, never derived: an engrave with a V-bit and one with an end mill cut the same path, and only the simulated groove differs (see [m5-simulation.md](m5-simulation.md)). |
| `texture` | Parallel grooves across a selected **closed** region: `spacing` millimetres apart, at `angle` **radians** counter-clockwise from the X axis. Each groove is **one pass** at `cut_depth` with the tool's centre on it, and the tool retracts to the job's `safe_z` between grooves. **It does not clear:** the material between the grooves is meant to stand, so a texture is a pattern and never a pocket with a wide stepover, and `cam-cli toolpath --kind texture` says so in its help and in its refusal of the clearing flags. The pattern is **derived, never stored**, exactly as a chamfer's depth is: `cam-model` clips it to the region in the groove frame with the kernel's raster spans, so the toolpath that cuts and the simulation that authorises read the same groove vertices, arcs and islands included. `cut_depth` must be greater than `start_depth`; a spacing that is not a positive distance is refused naming the value, both when the document is validated and by the strategy; a spacing that yields no groove across the region is refused naming the spacing and the angle rather than emitted as an empty program; and an open contour is refused as such. `spacing` and `angle` **default** to 3 mm and 0 on load, so a hand-written document that names a `texture` toolpath without them loads as the pattern it describes. `--spacing` falls back to the library's `stepover` the way a pocket's `--stepover` does, and its absence is an input error naming the flag. There is no side, no `pass_depth`, no tab, no lead, no `allowance`, no `stepover` and no `ramp`, and the CLI refuses each of those flags by name. See [m5-simulation.md](m5-simulation.md) for the groove the simulation claims. |
| `inlay` | A male/female **pair** of pockets, written as **two** toolpaths from one command — `inlay female` first, `inlay male` second, with consecutive ids — that share one copy of the five parameters. The **female** clears the selected region **grown by `allowance`**: the cavity. The **male** clears the job rectangle with that same region standing as an island: the plug, `allowance` millimetres smaller than the cavity on every side, which is why there is one allowance and no second gap parameter — the number is the clearance. `allowance` **defaults to zero**, and a zero allowance is the plain pocket of the region cell for cell; a negative one is an interference fit (a cavity smaller than the plug), refused by name when it shrinks the region away. `stepover` and `pass_depth` are the pocket's own and fall back to the tool library's `stepover` and `stepdown` exactly as a pocket's do; `cut_depth` must be greater than `start_depth`, and a stepover that is not a positive distance is refused naming the value. Two halves, one set of numbers, and the document says which half a toolpath is where it already says such things: the female names the region as its `entity_ids` and leaves nothing standing, and the male has **no boundary entity at all** — its boundary is the job rectangle, which is not an entity — and names the region as its `island_ids`. A toolpath that names both is refused rather than guessed at, and a male's islands must lie strictly inside the stock, because a region on or across the stock's edge leaves the male nothing to clear around it. The pair is refused by name for an open contour and for a region that is not strictly inside the stock. Both halves reuse the pocket's own clearing machinery (`crates/cam-toolpath/src/pocket.rs`), so linking, depth chunking and the coverage bound are the pocket's and not a second implementation of them: the female clears with the offset rings that follow the wall the plug fits, and the male with the raster rows, because the ring lattice converges on the frame a male clears and stops short of the four diagonal corners (measured: 2300 mm² of a 8556 mm² frame never cut at a 2 mm stepover, against 80 mm² for the rows). The command takes no side, no tab, no lead, no ramp, no spacing, no angle, no chamfer width and no clearing strategy, and refuses each of those flags by name. See [m4-toolpaths.md](m4-toolpaths.md) for the pocket it reuses. |

`tools_ref` and `postp` are optional references resolved relative to the document.

## Tool library

SQLite. The schema separates tool **geometry**, tool **cutting data** and the (material, machine)
context, joined by `tool_entity`: the same 6 mm end mill feeds differently in MDF and in hardwood, and
differently again on a light machine.

| Table | Purpose |
| --- | --- |
| `tool_geometry` | `id`, `name_format`, `tool_type`, `units`, `diameter`, `included_angle`, `flat_diameter`, `num_flutes`, `flute_length`, `tip_radius`, `custom_attributes` |
| `tool_cutting_data` | `id`, `rate_units`, `feed_rate`, `plunge_rate`, `spindle_speed`, `stepdown`, `stepover`, `clear_stepover`, `tool_number`, `notes` |
| `tool_entity` | joins a geometry to cutting data for one optional material and machine |
| `material`, `machine` | named context records |
| `tool_tree_entry` | user-visible folders of tools, self-referencing with `sibling_order` |
| `migration` | applied schema versions, with a checksum per migration |

### Rules

- **Forward-only migrations.** `cam_store::tools::MIGRATIONS` is an append-only ladder; opening a
  library applies whatever is missing inside one transaction and never edits an applied entry.
- `tool_type` is preserved as the **raw integer** found in the analysed product. Its numeric mapping is
  not established from the artifact, so no enum pretends to know it; strategies validate the geometric
  parameters they actually need.
- Display names come from `name_format` templates: `{Tool Type} ({Diameter|F}{Units Short})`. Both
  format spellings seen in the real library are supported: `F` (fewest decimals that keep the value
  readable) and a printf-style precision such as `.0`.
- Stored lengths are millimetres and stored rates are millimetres per second; `units`/`rate_units`
  record what the user authored in.

## Post-processor

Plain text, parsed into directives, `VAR` declarations and output blocks. Comments start with `+` or
`|`.

```
POST_NAME = "Mach3 Arcs (mm) (*.nc)"
FILE_EXTENSION = "nc"
UNITS = "MM"
LINE_ENDING = "[13][10]"
LINE_NUMBER_START = 0
LINE_NUMBER_INCREMENT = 10
LINE_NUMBER_MAXIMUM = 999999
VAR X_POSITION = [X|C|X|1.4]
begin HEADER
"[N]G00G17G90G21"
```

### Directives (13)

`POST_NAME`, `FILE_EXTENSION`, `UNITS`, `LINE_ENDING`, `LINE_NUMBER_START`,
`LINE_NUMBER_INCREMENT`, `LINE_NUMBER_MAXIMUM`, `DIRECT_OUTPUT`, `SUBSTITUTE`, `ROTARY_WRAP_X`,
`ROTARY_WRAP_Y`, `RAPID_PLUNGE_TO_STARTZ`, `PRINT_DIRECT`.

### `VAR` form

```
VAR <label> = [<source>|<A|C>|<prefix>|<decimals>|<scale>]
```

| Field | Meaning |
| --- | --- |
| `source` | The field a block references. `X`, `Y`, `Z`, `XH`, `YH`, `ZH`, `SAFEZ`, `I`, `J`, `Radius`, `F`, `FC`, `FP`, `S`, `T`, `N`, `WRAP_DIA` |
| `A` / `C` | Always emit, or emit only when the value changed since the last line |
| `prefix` | Literal letter(s) placed before the number, e.g. `X`. May be empty. |
| `decimals` | Written in the dialect's own form: **`1.4` means four decimals**, `1.0` means none |
| `scale` | Optional multiplier applied before formatting, e.g. `0.0166` to convert a per-minute rate to a per-second one |

`<label>` is the human name of the rule (`X_POSITION`); blocks reference the **source** (`[X]`).

Text variables need no declaration: `TOOLPATH_NAME`, `PATHNAME`, `FILE_NAME`, `FILE_PATHNAME`,
`POST_NAME`, `UNITS`, `TOOLNAME`, `DATE`, `TIME`.

### Output blocks (16)

`HEADER`, `FOOTER`, `INITIAL_RAPID_MOVE`, `FIRST_RAPID_MOVE`, `RAPID_MOVE`, `PLUNGE_MOVE`,
`FIRST_FEED_MOVE`, `FEED_MOVE`, `RETRACT_MOVE`, `FIRST_CW_ARC_MOVE`, `CW_ARC_MOVE`,
`FIRST_CCW_ARC_MOVE`, `CCW_ARC_MOVE`, `FEED_RATE_CHANGE`, `NEW_SEGMENT`, `TOOLCHANGE`.

### Emission rules

- Coordinates leave the pipeline in millimetres and are converted to inches here if the post asks,
  then rounded once, per variable, from its decimal count. Nothing rounds earlier.
- A block line whose expansion is blank is dropped, so a fully conditional line never emits whitespace.
- The line number advances only on lines that actually carry it.
- `INITIAL_RAPID_MOVE` is the first rapid of the program; `FIRST_RAPID_MOVE` the first rapid of each
  later segment; `RAPID_MOVE` the rest.
- A move that descends without changing XY is a `PLUNGE_MOVE`.
- `TOOLCHANGE` fires before `NEW_SEGMENT` when the tool number changes between segments.
- A post with no arc block **refuses** a program containing arcs. Emitting straight lines through an
  arc is a dimensional error, not a fallback.
- Identical input produces byte-identical output: no timestamps, no map iteration order.

## Command-line contract

`cam-cli` is the verification surface: every behaviour that matters is reachable headlessly.

| Command | Purpose |
| --- | --- |
| `validate <document>` | Schema plus referential invariants |
| `inspect <document>` | Stable summary of job, layers and toolpaths |
| `import <input> [--out <document>] [--layer <name>]` | Import DXF, SVG, PDF, EPS or PNG into a new document and print the report; see [m3-import.md](m3-import.md) |
| `check <document>` | Vector validation report: open contours, self-intersections, degenerate segments |
| `postp validate <file>` | Parse a post-processor and print a summary |
| `tools list <library>` | List tools |
| `tools add <library> --id --diameter --number` | Add or replace a tool |
| `toolpath <document> --kind profile\|pocket\|drilling\|chamfer\|engrave\|fluting\|texture\|inlay --entities <ids> [--islands <ids>] --tool <n>` | Append one toolpath and write the document back (`--out` writes elsewhere); `--islands` is a pocket-only list of contours to leave standing; `chamfer` takes `--chamfer-width` and derives its depth, so it has no `--side` and no `--cut-depth`; `engrave` takes `--start-depth` and `--cut-depth` and refuses `--side`, `--pass-depth`, `--tab`, `--lead-in` and `--lead-out` by name; `fluting` takes the same two depths plus `--ramp` and refuses those five flags and `--stepover` and `--allowance` as well; `texture` takes the same two depths plus `--spacing` and `--angle`, falls back to the library's `stepover` when `--spacing` is absent, refuses `--side`, `--pass-depth`, `--tab`, `--lead-in`, `--lead-out`, `--allowance`, `--stepover` and `--ramp` by name, and states that the material between its grooves stands; `inlay` appends **two** toolpaths — `inlay female` then `inlay male`, ids `TPn` then `TPn+1`, named by the command itself so it takes no `--id` and no `--name` — takes `--inlay-allowance` (default 0, the clearance between plug and cavity), falls back to the library's `stepover` and `stepdown`, and refuses `--side`, `--tab`, `--lead-in`, `--lead-out`, `--ramp`, `--spacing`, `--angle`, `--chamfer-width` and `--strategy` by name; see [m4-toolpaths.md](m4-toolpaths.md) and [region-booleans.md](region-booleans.md) |
| `emit <document> --post <file> [--out <path>] [--library <path>]` | Compute toolpaths and write the program |
| `simulate <document> [--library <path>] [--resolution <mm>]` | Simulate the program's material removal over the stock and print the report: removed volume, through-cut cells, gouge, uncut material and every crash. `--resolution` is the cell size in mm (default 1); the deviation tolerance that separates "gouge" from "uncut" is an eighth of a cell; see [m5-simulation.md](m5-simulation.md) |

Machine output goes to **stdout**, diagnostics to **stderr**.

| Exit code | Meaning |
| --- | --- |
| `0` | Success |
| `2` | Invalid input: unusable path, malformed document, missing tool data, a cut the geometry refuses, a `--resolution` whose grid exceeds the simulation's cell cap |
| `3` | Unsupported document schema |
| `4` | Post-processor error, including a program the post cannot express |
| `5` | The document has vector issues (`check` only): open, self-intersecting or degenerate geometry |
| `6` | The simulation found gouge, uncut material or a crash (`simulate` only); the report says which |
