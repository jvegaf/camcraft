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
        "pass_depth": 6.0, "start_depth": 0.0, "cut_depth": 9.0 } },
    { "id": "TP10", "name": "Thread 10", "entity_ids": ["E7"], "tool_number": 5,
      "operation": { "kind": "thread", "pitch": 1.5, "direction": "internal",
        "start_depth": 0.0, "cut_depth": 9.0 } },
    { "id": "TP11", "name": "V carve 11", "entity_ids": ["E2"], "tool_number": 5,
      "operation": { "kind": "v_carve", "included_angle": 1.0471975511965976,
        "tolerance": 0.1 } },
    { "id": "TP12", "name": "Moulding 12", "entity_ids": ["E8"], "tool_number": 4,
      "operation": { "kind": "moulding",
        "profile": [ { "lateral": 0.0, "depth": 0.0 },
                     { "lateral": 6.0, "depth": -4.0 },
                     { "lateral": 12.0, "depth": 0.0 } ],
        "stepover": 2.0 } }
  ],
  "tools_ref": "tools.sqlite",
  "postp": "mach3.pp"
}
```

### Rules

| Topic | Rule |
| --- | --- |
| Units | Every length in the document is **millimetres**. `job.units` records the unit the user works in and nothing else converts on read. |
| Material | `job.material` is an **optional** job field naming the material the program is cut as, and it is **omitted when absent**: a document that names no material serialises exactly as one written before the field existed, so the tag stays `@2`. `cam-cli emit --material <name>` overrides it for one run, and the flag wins. The name is resolved against the tool library, which keys cutting data **per material and per machine** (see the library section below): a name the library has no record for is **refused by name**, naming the tool too, rather than falling back to the tool's own rate, because a program that quietly cuts at a speed nobody chose is worse than one that stops. |
| Schema | This build **writes** `c2d/document@2` and still **reads** `c2d/document@1`, upgrading it in memory on load so a save never writes the old tag back. Anything else is rejected naming both the found and the supported version. `@2` only added `Toolpath.island_ids`: the bump exists because an older build ignores a field it does not know, so keeping the tag would let it pocket straight through an island. |
| Geometry | A contour is an ordered list of vertices; `bulge` is the DXF bulge of the segment leaving that vertex (`0` = straight, `tan(sweep/4)` = arc, positive counter-clockwise). A full circle is two half-turn arcs. |
| Validation | Contours are validated **on deserialisation**: too few vertices, coincident consecutive vertices and impossible bulges fail the load instead of producing geometry. |
| References | Toolpath `entity_ids` must resolve. Every operation that cuts a closed contour — `profile`, `pocket`, `drilling`, `chamfer`, `v_carve`, `inlay`, `thread` — must reference closed contours, and an open one is refused by name. `texture` needs closed geometry too, because its grooves are clipped to a region and an open vector bounds none, and so does `v_carve`, whose region is what the contours enclose. `engrave` and `fluting` are the exceptions: they cut along the vector, so an open one is legal — engraving a single line is the engrave's point, and a flute is one groove per vector drawn as a line as often as a loop. `moulding` is open **or** closed by design: its entity is the **rail**, and an open rail sweeps a strip while a closed one sweeps a frame. Ids are unique within their kind. |
| Job | `width`, `height` and `thickness` must be positive and finite. |
| Origin | `bottom_left`, `top_left` or `center`. The mapping onto machine coordinates happens once, in the toolpath layer. |
| Extensibility | `EntityKind` and `Operation` are internally tagged enums. An unknown tag is a **hard error**: a document written by a newer build never half-loads, it is refused by name. Because of that, adding a variant is additive and keeps the schema tag; changing what an existing field means, or its units, is what makes a `@2`. |
| `RetractMode` | `{"mode":"full"}` or `{"mode":"amount","amount":1.0}`. A tagged **newtype** over a bare number cannot carry the tag, so the amount is a named field. |
| `bank` | Omitted when the machine has no drill gang, so a document that does not use one is byte-identical to one written before banks existed. |
| `island_ids` | Omitted when empty. Names the closed contours to leave standing inside a `pocket`'s region; nothing is inferred from the drawing, so the field *is* the island list. The model refuses an island that is not a closed contour, that is not strictly inside one of the toolpath's boundaries, that is not disjoint from another island, that repeats an id, or that belongs to any operation other than a pocket or an inlay. For an **inlay** the field is also what says which half the toolpath is (see the `inlay` row). See [region-booleans.md](region-booleans.md). |
| `chamfer` | A V-bit bevel on the selected closed contours: the tip travels **on** the vector, in one pass, and the bevel is symmetric about it. `chamfer_width` is the bevel's horizontal width in millimetres and `included_angle` is the bit's full included angle in **radians**, as the library stores it. There is no `cut_depth`: the depth is **derived** — `start_depth + chamfer_width / tan(included_angle / 2)` — and never stored, so a stored value cannot disagree with the bit it came from. A width that is not positive, or an angle outside `(0, π)`, is refused naming the offending value. The derivation assumes a sharp point, so a bit with a flat tip is refused by name as well: with a flat of diameter `f` the bevel would reach the surface `f/2` wider than `chamfer_width`. See [m4-toolpaths.md](m4-toolpaths.md) for the neighbouring operations. |
| `fluting` | A flute on the selected vectors, **open or closed**: one pass at `cut_depth`, with the tool's centre **on** the vector, and one **ramp**. `ramp` is a *length* in millimetres, not a depth: with `ramp > 0` the pass enters over the first `ramp` millimetres of the vector and leaves over the last, so the tool never plunges at full depth, and `ramp = 0` is the plain groove — a plunge in place at the start and a retract in place at the end. A ramp longer than **half** the vector is an input error naming the ramp and the vector's length rather than a silently shortened ramp, and a negative one is refused by name when the document is validated and again by the strategy; a closed contour measures the ramp against its perimeter. A contour that contains an **arc** carries its ramp **along the arc**, as a **helical** move per arc: a ramped move cannot ride on `Move::Arc`, which is constant-Z by contract, so the ramp descends as it follows the curve instead of chording across it (`ramp² / (8·r)` inside the vector, half a millimetre for a 10 mm ramp on a 25 mm circle — the defect T4 refused and T7 lifted). `ramp = 0` on the same contour is the plain groove, cut as constant-Z arcs. `cut_depth` must be greater than `start_depth`, as for an engrave. There is no side, no `pass_depth`, no tab, no lead, no `stepover` and no `allowance`, and the CLI refuses each of those flags by name. `ramp` **defaults to zero** on load, so a hand-written or older document that names a `fluting` toolpath without it loads as the in-place plunge it describes instead of failing the load. See [m5-simulation.md](m5-simulation.md) for the groove the simulation claims. |
| `engrave` | A quick engrave on the selected vectors, **open or closed**: one pass at `cut_depth`, with the tool's centre **on** the vector, and nothing else. `start_depth` is where the pass begins, so the deepest cut is `cut_depth` itself; `cut_depth` must be greater than `start_depth` or the toolpath is an input error naming both numbers. There is no side, no `pass_depth`, no tab and no lead — a second pass would cut a second groove into the first — so the CLI refuses each of those flags by name instead of ignoring it. Every other operation here needs a closed contour; an engrave of a single open line is the strategy's point. The depth is authored, never derived: an engrave with a V-bit and one with an end mill cut the same path, and only the simulated groove differs (see [m5-simulation.md](m5-simulation.md)). |
| `texture` | Parallel grooves across a selected **closed** region: `spacing` millimetres apart, at `angle` **radians** counter-clockwise from the X axis. Each groove is **one pass** at `cut_depth` with the tool's centre on it, and the tool retracts to the job's `safe_z` between grooves. **It does not clear:** the material between the grooves is meant to stand, so a texture is a pattern and never a pocket with a wide stepover, and `cam-cli toolpath --kind texture` says so in its help and in its refusal of the clearing flags. The pattern is **derived, never stored**, exactly as a chamfer's depth is: `cam-model` clips it to the region in the groove frame with the kernel's raster spans, so the toolpath that cuts and the simulation that authorises read the same groove vertices, arcs and islands included. `cut_depth` must be greater than `start_depth`; a spacing that is not a positive distance is refused naming the value, both when the document is validated and by the strategy; a spacing that yields no groove across the region is refused naming the spacing and the angle rather than emitted as an empty program; and an open contour is refused as such. `spacing` and `angle` **default** to 3 mm and 0 on load, so a hand-written document that names a `texture` toolpath without them loads as the pattern it describes. `--spacing` falls back to the library's `stepover` the way a pocket's `--stepover` does, and its absence is an input error naming the flag. There is no side, no `pass_depth`, no tab, no lead, no `allowance`, no `stepover` and no `ramp`, and the CLI refuses each of those flags by name. See [m5-simulation.md](m5-simulation.md) for the groove the simulation claims. |
| `inlay` | A male/female **pair** of pockets, written as **two** toolpaths from one command — `inlay female` first, `inlay male` second, with consecutive ids — that share one copy of the five parameters. The **female** clears the selected region **grown by `allowance`**: the cavity. The **male** clears the job rectangle with that same region standing as an island: the plug, `allowance` millimetres smaller than the cavity on every side, which is why there is one allowance and no second gap parameter — the number is the clearance. `allowance` **defaults to zero**, and a zero allowance is the plain pocket of the region cell for cell; a negative one is an interference fit (a cavity smaller than the plug), refused by name when it shrinks the region away. `stepover` and `pass_depth` are the pocket's own and fall back to the tool library's `stepover` and `stepdown` exactly as a pocket's do; `cut_depth` must be greater than `start_depth`, and a stepover that is not a positive distance is refused naming the value. Two halves, one set of numbers, and the document says which half a toolpath is where it already says such things: the female names the region as its `entity_ids` and leaves nothing standing, and the male has **no boundary entity at all** — its boundary is the job rectangle, which is not an entity — and names the region as its `island_ids`. A toolpath that names both is refused rather than guessed at, and a male's islands must lie strictly inside the stock, because a region on or across the stock's edge leaves the male nothing to clear around it. The pair is refused by name for an open contour and for a region that is not strictly inside the stock. Both halves reuse the pocket's own clearing machinery (`crates/cam-toolpath/src/pocket.rs`), so linking, depth chunking and the coverage bound are the pocket's and not a second implementation of them: the female clears with the offset rings that follow the wall the plug fits, and the male with the raster rows, because the ring lattice converges on the frame a male clears and stops short of the four diagonal corners (measured: 2300 mm² of a 8556 mm² frame never cut at a 2 mm stepover, against 80 mm² for the rows). The command takes no side, no tab, no lead, no ramp, no spacing, no angle, no chamfer width and no clearing strategy, and refuses each of those flags by name. See [m4-toolpaths.md](m4-toolpaths.md) for the pocket it reuses. |
| `v_carve` | A V-carve of the selected region: the selected closed contours are **one** region under the parity rule, and its **medial axis** is the toolpath. `included_angle` is the bit's full included angle in **radians**, as the library stores it, and it is the only number the depth law needs: a tip whose exact distance to the boundary is `d` sits at `-d / tan(included_angle / 2)`, so the surface the carve leaves is the distance field and `gouge` is exactly zero for any tip set inside the region. `tolerance` is the axis's **lattice step** in millimetres and **defaults to `0.1`**, so a hand-written or older document that names a `v_carve` toolpath without it loads as the axis the CLI's own default would have written; it bounds the chording between certified tips, the shortest piece of axis the walk can see, the band of material left standing near a convex corner (measured `0.20` mm of missed depth at `0.5`, `0.00` at `0.1`) and the truncation at a curvature extremum, and every one of those costs **coverage only, never depth**. There is no `cut_depth` and no `start_depth`: the depth is a function of position, and with a start depth the cone's top would reach outside the vectors, so the carve would stop being the distance field. The program is one walk per axis branch — a rapid to `safe_z`, a plunge at the first tip's own depth, one linear feed per further tip carrying that tip's own Z, and a retract — so **no arc move is emitted**, and `Move::Arc`'s constant-Z contract is untouched. An angle outside `(0, π)` and a tolerance that is not a positive length are refused by name when the document is validated, an open contour is refused as such, and a bit the library does not describe as an angled V-bit, a flat-tipped bit, a lattice the kernel refuses and a region with **no axis at all** (a step at least as large as the region's own clearance, so there is nothing to carve) are refused by the strategy and again by `cam-cli toolpath` before anything is written. The command takes no `--cut-depth`, `--start-depth`, `--pass-depth`, `--stepover`, `--allowance`, `--side`, `--tab`, `--lead-in`, `--lead-out`, `--ramp`, `--spacing`, `--angle`, `--pitch`, `--chamfer-width`, `--inlay-allowance`, `--clear` or `--strategy`, and refuses each of those flags by name. See [m7-vcarve.md](m7-vcarve.md) for the law, the axis's certificate and the simulation's claim. |
| `thread` | A thread milled on a **circle**: one helical descent around it, one `pitch` per revolution, from the surface to `cut_depth` and then a retract to `safe_z`. `direction` is `internal` — a thread inside a hole — or `external` — a thread around a boss — and it fixes the one number the strategy derives, the radius of the tool centre's orbit: `(D − t)/2` for an internal thread and `(D + t)/2` for an external one, `D` being the selected circle's diameter and `t` the tool's. Nothing derived is stored here: the circle is the entity and the tool is the library's, so a document cannot disagree with the geometry it was written against, and there is no diameter field to drift. `pitch` must be a positive number of millimetres and `cut_depth` must be greater than `start_depth`; a pitch that descends nowhere and a depth with no descent behind it are both refused by name. The selection must be a **circle** — refused by name otherwise, at authoring and by the strategy — and an internal thread whose tool is as wide as the hole is refused too, because `(D − t)/2` is then not a radius. The command takes no side, no `pass_depth`, no tab, no lead, no `stepover`, no `allowance`, no `ramp`, no `chamfer_width`, no `strategy` and no `inlay_allowance`, and refuses each of those flags by name. A helical arc is a **move kind of its own** and a **post block of its own**: the program's helixes are `Move::Helix` moves, and a post that defines no helical block refuses the program by name rather than cutting a flat circle at the wrong depth (see the post-processor section). See [m5-simulation.md](m5-simulation.md) for the spiral the simulation claims, cell by cell. |
| `moulding` | A cross-section swept along a **rail**: the selected entity is the rail — an open path sweeps a strip, a closed loop sweeps a frame, and two selected rails are two sweeps in one toolpath — and `profile` is authored data, at least two `(lateral, depth)` points with **strictly increasing** laterals, because a cross-section is one depth per lateral distance. `lateral` is the distance from the rail and is **positive to the right of travel**; `depth` is zero at the stock surface and negative below. `stepover` spaces the **overlapped lateral passes**: the law starts at the profile's minimum lateral, steps by the stepover, and always makes the maximum lateral a pass, so a stepover equal to the profile's width is the two edge passes and a stepover wider than it is one pass centred on the profile; the coverage radius is at most half a stepover. Each pass is the rail walked by **arc length** (arcs are kept as arcs) at the depth its own lateral point demands, so the program is a family of Z heights: a rapid to the pass's first point at `safe_z`, a plunge to that pass's depth, one linear feed per rail sample, a closing feed where the walk does not end on its own first point, and a retract. **No arc move is emitted** — a rail's corner has two normals and the walk bridges them with a straight feed — and a corner whose two offset samples coincide emits a zero-length feed block. The depth is authored, so the bit's diameter moves nothing except the width of the simulation's claim. There is no `cut_depth`, no `start_depth`, no pass law, no side, no allowance, no tab, no lead, no ramp and **no tolerance flag**: the rail's walk chords an arc at the fixed `0.001` mm tolerance the geometry kernel declares. A profile shorter than two points, a lateral that does not increase and a stepover that is not a positive length are refused by name when the document is validated, again by the strategy and again by the claim; a rail that is not a contour, a rail with no length and a moulding that names no rail are refused without authorising anything. See [m8-moulding.md](m8-moulding.md) for the sweep, the stepover certificate, the gouge-is-zero argument and the one seam corner where it does not hold. |

| `nesting` | The optional **top-level** section that describes a sheet-nesting run: `sheets` (an id, a width and a height), `parts` (an id, the `entity_ids` it is cut from and how many `instances` are wanted), `placements` (the part, the sheet, the `position` and the `rotation` the nester chose) and `cost` (`sheets_used`, `utilisation`, `waste`). It is `Option` with `serde(default, skip_serializing_if)`, so a document written before M9 loads and re-saves **byte for byte** and the schema tag does not move — the section adds a meaning rather than changing one. `cost` is absent until a run writes it, and utilisation and waste have **one** derivation, `Document::nesting_utilisation`: the placed footprints' area over the area of the sheets the placements **name** — never every sheet the section holds — with waste its complement, so a stored record and the report cannot claim two different numbers and a record that disagrees with the document is refused by name. A part's `instances` must equal the placements naming it **once the section holds one placement**; a section with parts and no placements at all is a request that has not been nested yet, which is the state a user authors, and it is valid. A sheet dimension that is not a positive finite length, a part naming no entity or an unknown one, a placement naming a missing part or sheet, a placement outside its sheet or at a negative position, two placements overlapping on one sheet, duplicate sheet or part ids and the cost record disagreeing with the document are each refused by name. **A build older than M9 silently drops the section** rather than refusing it, because an unknown top-level field is ignored — the tag was not bumped, so a document nested here and re-saved there comes back without its placements. See [m9-nesting.md](m9-nesting.md). |

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

### Output blocks (20)

`HEADER`, `FOOTER`, `INITIAL_RAPID_MOVE`, `FIRST_RAPID_MOVE`, `RAPID_MOVE`, `PLUNGE_MOVE`,
`FIRST_FEED_MOVE`, `FEED_MOVE`, `RETRACT_MOVE`, `FIRST_CW_ARC_MOVE`, `CW_ARC_MOVE`,
`FIRST_CCW_ARC_MOVE`, `CCW_ARC_MOVE`, `FIRST_CW_HELIX_MOVE`, `CW_HELIX_MOVE`,
`FIRST_CCW_HELIX_MOVE`, `CCW_HELIX_MOVE`, `FEED_RATE_CHANGE`, `NEW_SEGMENT`, `TOOLCHANGE`.

The four **helical** kinds are M6's addition (D-M6-8), and their templates carry `[Z]` as well as
`[X][Y][I][J]`, because the move they emit descends. The arc kinds keep their constant-Z templates:
nothing about them changed meaning, and a post that works today works unchanged.

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
- A post with no **helical** block refuses a program containing helical moves — a thread's descent —
  for the same reason and in the same words, naming the post and the move kind. An arc block is not a
  substitute: its template has no `[Z]`, so the descent would be lost.
- Identical input produces byte-identical output: no timestamps, no map iteration order.

## Command-line contract

`cam-cli` is the verification surface: every behaviour that matters is reachable headlessly.

| Command | Purpose |
| --- | --- |
| `validate <document>` | Schema plus referential invariants |
| `inspect <document>` | Stable summary of job, layers and toolpaths |
| `import <input> [--out <document>] [--layer <name>]` | Import DXF, SVG, PDF, EPS or PNG into a new document and print the report; see [m3-import.md](m3-import.md) |
| `export <document> --format dxf\|svg [--out <path>]` | Write the document's **geometry** back out. DXF keeps layer identity: one `LWPOLYLINE` per contour carrying each vertex's bulge verbatim, `70 = 1` when the contour is closed, and `$INSUNITS = 4` so the millimetre scale is 1.0, with the layer table in document order. SVG writes one `<path>` per contour with `A` commands whose `rx == ry` — the only form `import` keeps exact — a Y reflection it inverts, and a `viewBox` in the document's own millimetres, so a re-import has `unit_scale == 1.0`. **SVG carries no layers**: the reader files every SVG under one layer named `svg`, and this writer emits no layer name at all rather than pretend otherwise. What is not written: toolpaths (not geometry), blocks, text and styling, and any curve the document already holds flattened. A format this build does not write is refused by name, exit `2`. Without `--out` the bytes go to stdout |
| `check <document>` | Vector validation report: open contours, self-intersections, degenerate segments |
| `postp validate <file>` | Parse a post-processor and print a summary |
| `tools list <library>` | List tools |
| `tools add <library> --id --diameter --number` | Add or replace a tool |
| `toolpath <document> --kind profile\|pocket\|drilling\|chamfer\|vcarve\|engrave\|fluting\|texture\|inlay\|thread\|moulding --entities <ids> [--islands <ids>] --tool <n>` | Append one toolpath and write the document back (`--out` writes elsewhere); `--islands` is a pocket-only list of contours to leave standing; `chamfer` takes `--chamfer-width` and derives its depth, so it has no `--side` and no `--cut-depth`; `engrave` takes `--start-depth` and `--cut-depth` and refuses `--side`, `--pass-depth`, `--tab`, `--lead-in` and `--lead-out` by name; `fluting` takes the same two depths plus `--ramp` and refuses those five flags and `--stepover` and `--allowance` as well; `texture` takes the same two depths plus `--spacing` and `--angle`, falls back to the library's `stepover` when `--spacing` is absent, refuses `--side`, `--pass-depth`, `--tab`, `--lead-in`, `--lead-out`, `--allowance`, `--stepover` and `--ramp` by name, and states that the material between its grooves stands; `inlay` appends **two** toolpaths — `inlay female` then `inlay male`, ids `TPn` then `TPn+1`, named by the command itself so it takes no `--id` and no `--name` — takes `--inlay-allowance` (default 0, the clearance between plug and cavity), falls back to the library's `stepover` and `stepdown`, and refuses `--side`, `--tab`, `--lead-in`, `--lead-out`, `--ramp`, `--spacing`, `--angle`, `--chamfer-width` and `--strategy` by name; `thread` takes `--pitch` (required), `--thread-direction internal\|external` (default `internal`), `--start-depth` and `--cut-depth`, refuses `--side`, `--pass-depth`, `--tab`, `--lead-in`, `--lead-out`, `--stepover`, `--allowance`, `--ramp`, `--chamfer-width`, `--strategy` and `--inlay-allowance` by name, and refuses a selection that is not a circle and a tool that does not fit inside the hole while it can still say no; `vcarve` takes `--tolerance` (default `0.1`, the medial axis's lattice step) and **no** depth at all — the depth is a function of each tip's distance to the boundary — so it refuses `--cut-depth`, `--start-depth`, `--pass-depth`, `--stepover`, `--allowance`, `--side`, `--tab`, `--lead-in`, `--lead-out`, `--ramp`, `--spacing`, `--angle`, `--pitch`, `--chamfer-width`, `--inlay-allowance`, `--clear` and `--strategy` by name, and refuses a bit the library does not describe as a sharp V-bit and a region whose axis is empty before writing the document; `moulding` takes a repeatable `--profile <lateral>:<depth>` (at least two points, strictly increasing laterals) and `--stepover` (falling back to the library's `stepover`), and refuses **22** flags by name — `--cut-depth`, `--start-depth`, `--pass-depth`, `--allowance`, `--side`, `--tab`, `--lead-in`, `--lead-out`, `--ramp`, `--spacing`, `--angle`, `--pitch`, `--thread-direction`, `--chamfer-width`, `--inlay-allowance`, `--strategy`, `--tolerance`, `--clear`, `--peck-depth`, `--retract`, `--retract-amount` and `--bank`; see [m4-toolpaths.md](m4-toolpaths.md), [m6-toolpaths.md](m6-toolpaths.md), [m7-vcarve.md](m7-vcarve.md), [m8-moulding.md](m8-moulding.md), [region-booleans.md](region-booleans.md) and [m5-simulation.md](m5-simulation.md) |
| `emit <document> --post <file> [--out <path>] [--library <path>] [--material <name>]` | Compute toolpaths and write the program. `--material` selects the material the program is cut as and **overrides** `job.material`; with neither, the program uses the record the library keys as the tool's own (the one with no material), which is what every program written before this flag did, byte for byte. A material the library has no cutting data for is refused, exit `2`, naming the material and the tool. `simulate` resolves the material the same way without a flag, so a simulation cannot report on a program the machine would not run |
| `simulate <document> [--library <path>] [--resolution <mm>]` | Simulate the program's material removal over the stock and print the report: removed volume, through-cut cells, gouge, uncut material and every crash. `--resolution` is the cell size in mm (default 1); the deviation tolerance that separates "gouge" from "uncut" is an eighth of a cell; see [m5-simulation.md](m5-simulation.md) |
| `nest <document> [--sheets <WxH>]… [--gap <mm>] [--rotation <on\|off>] [--algorithm auto\|maxrects\|guillotine] [--step <mm>] [--out <path>]` | Nest the document's parts onto its sheets and store the placements plus the cost record, printing a stable report. Rectangular parts go to maxrects or guillotine by `--algorithm`; any other selection goes to the free-profile no-fit heuristic at `--step`. The cost is written from the document's own `nesting_utilisation`, never from the nester's view, so the report and the stored field cannot disagree. `--sheets` replaces the document's authored sheets. See [m9-nesting.md](m9-nesting.md) |
| `parts <document> --csv <file> [--layer <name>] [--out <path>]` | Read a part list (name, width, height, quantity — header required, fixed column order) and author one closed rectangle plus one nesting part per row. A name the document already holds is refused at the row's line rather than renamed. See [m10-documents.md](m10-documents.md) |
| `cutlist <document> [--out <path>]` | Write the placements as a cut list: part, sheet, x, y, rotation in degrees, width, height, three decimals, one row per placed copy. Refused by name for a document with no nesting section or with parts but no placements, rather than writing an empty file. See [m10-documents.md](m10-documents.md) |
| `report <document> [--out <path>]` | Write an HTML print sheet of the document — the job, its layers, every toolpath with its authored parameters, the tools it names and the nesting with the utilisation the document derives. One self-contained file, no external asset, no script, deterministic bytes, every id and name escaped. See [m10-documents.md](m10-documents.md) |
| `script <file>` | Run a line-oriented command script against one document: `open`, `sheet`, `part`, `nest`, `toolpath`, `validate`, `emit`, `simulate`, `parts`, `cutlist`, `save`. Every command dispatches to the same function its subcommand calls, so a script cannot nest or emit with different defaults; a failure reports `line:column`. See [m10-documents.md](m10-documents.md) |

Machine output goes to **stdout**, diagnostics to **stderr**.

| Exit code | Meaning |
| --- | --- |
| `0` | Success |
| `2` | Invalid input: unusable path, malformed document, missing tool data, a cut the geometry refuses, a `--resolution` whose grid exceeds the simulation's cell cap |
| `3` | Unsupported document schema |
| `4` | Post-processor error, including a program the post cannot express |
| `5` | The document has vector issues (`check` only): open, self-intersecting or degenerate geometry |
| `6` | The simulation found gouge, uncut material or a crash (`simulate` only); the report says which |
| `7` | A nesting could not place every instance (`nest` only), so **nothing was written**: the report names the part and how many of its copies landed. A finding about the document's own request, not invalid input, which is why it is not `2` |
