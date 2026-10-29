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
└── document.json      pretty-printed JSON, schema c2d/document@1
```

```json
{
  "schema": "c2d/document@1",
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
        "bank": { "count": 3, "pitch": 32.0 } } }
  ],
  "tools_ref": "tools.sqlite",
  "postp": "mach3.pp"
}
```

### Rules

| Topic | Rule |
| --- | --- |
| Units | Every length in the document is **millimetres**. `job.units` records the unit the user works in and nothing else converts on read. |
| Schema | `schema` must equal `c2d/document@1`. Anything else is rejected naming both the found and the supported version. |
| Geometry | A contour is an ordered list of vertices; `bulge` is the DXF bulge of the segment leaving that vertex (`0` = straight, `tan(sweep/4)` = arc, positive counter-clockwise). A full circle is two half-turn arcs. |
| Validation | Contours are validated **on deserialisation**: too few vertices, coincident consecutive vertices and impossible bulges fail the load instead of producing geometry. |
| References | Toolpath `entity_ids` must resolve. Profile toolpaths must reference closed contours. Ids are unique within their kind. |
| Job | `width`, `height` and `thickness` must be positive and finite. |
| Origin | `bottom_left`, `top_left` or `center`. The mapping onto machine coordinates happens once, in the toolpath layer. |
| Extensibility | `EntityKind` and `Operation` are internally tagged enums. An unknown tag is a **hard error**: a document written by a newer build never half-loads, it is refused by name. Because of that, adding a variant is additive and keeps the schema tag; changing what an existing field means, or its units, is what makes a `@2`. |
| `RetractMode` | `{"mode":"full"}` or `{"mode":"amount","amount":1.0}`. A tagged **newtype** over a bare number cannot carry the tag, so the amount is a named field. |
| `bank` | Omitted when the machine has no drill gang, so a document that does not use one is byte-identical to one written before banks existed. |

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
| `toolpath <document> --kind profile\|pocket\|drilling --entities <ids> --tool <n>` | Append one toolpath and write the document back (`--out` writes elsewhere); see [m4-toolpaths.md](m4-toolpaths.md) |
| `emit <document> --post <file> [--out <path>] [--library <path>]` | Compute toolpaths and write the program |

Machine output goes to **stdout**, diagnostics to **stderr**.

| Exit code | Meaning |
| --- | --- |
| `0` | Success |
| `2` | Invalid input: unusable path, malformed document, missing tool data, a cut the geometry refuses |
| `3` | Unsupported document schema |
| `4` | Post-processor error, including a program the post cannot express |
| `5` | The document has vector issues (`check` only): open, self-intersecting or degenerate geometry |
