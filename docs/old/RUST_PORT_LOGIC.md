# FlatCAM Plus — Logic Extraction for a Rust Port

**Purpose.** Capture the *portable logic* of FlatCAM Plus — geometry, parsers, manufacturing
pipelines, G-code emission, persistence — with `path:line` evidence, so the program can be
reimplemented in Rust without reverse-engineering the Python again.

**Scope.** The Qt GUI is deliberately out of scope: it is a presentation layer over the same
`obj_options` / `tools` dictionaries described below. What matters for a port is the data model,
the numeric core, and the ordered transformations each tool applies.

**Method.** Targeted `grep` + bounded reads (no whole-file reads of `appMain.py`, `camlib.py`,
`appDatabase.py`). Every claim below carries a file:line. Claims that came from a helper agent and
were not re-read in person are marked *(explorer-reported)*.

**Repository.** `/home/th3g3ntl3man/Code/analisis` — 339 `.py` files, ~192k lines under
`flatcam_plus/`, Python 3.14, PyQt6.

---

## 1. Executive summary

### 1.1 What the program actually does

FlatCAM Plus is a **file → geometry → toolpath → G-code** pipeline with a project container.
Everything else (GUI, editors, Tcl shell, 3D preview) is tooling around that spine.

```
 Gerber / Excellon / DXF / SVG / HPGL2 / G-code
                    │  parsers (appParsers/)
                    ▼
        Geometry  (shapely polygons/lines, camlib.py:539)
                    │  boolean ops, buffer, isolation, cuts
                    ▼
   CNCJob      (toolpaths + emitted G-code, camlib.py:2728)
                    │  preprocessors/ (PreProc subclasses)
                    ▼
                G-code text
```

State lives in a single `App` object (`appMain.py:138`) that owns: `options` (flat dict of ~613
defaults), `preprocessors` (name → `PreProc` instance), `collection` (ordered object store),
`exc_areas` (exclusion areas used during G-code travel), and `app_obj` (object factory).

### 1.2 Porting surface, ranked by value

| Rank | Layer | Why it matters | Python size | Rust difficulty |
| --- | --- | --- | --- | --- |
| 1 | Geometry core (`camlib.py`) | Every toolpath derives from it; shapely semantics are the real spec | ~8.7k lines, the biggest file | **High** — needs a shapely-equivalent (`geo` + `i_overlay` + custom offsetting) |
| 2 | Tool pipelines (`appPlugins/Tool*.py`) | Ordered steps + option contracts; this is the domain knowledge | ~14k lines total | Medium — mostly orchestration, but many options |
| 3 | Parsers (`appParsers/`) | Gerber/Excellon dialects, incl. arcs and aperture macros | ~4k lines | Medium-High (Gerber arc + macro handling) |
| 4 | G-code emission (`linear2gcode`, `excellon_tool_gcode_gen`, `PreProc`) | Output must be byte-compatible with operator expectations | ~1.2k lines | Low-Medium (pure text formatting) |
| 5 | Persistence (project file, defaults, QSettings) | Compatibility with existing projects | ~1.5k lines | Low |
| 6 | Tcl shell (`tclCommands/`) | Scripted/CI usage; also documents the public API | ~10k lines | Low value for a Rust GUI; useful as an API inventory |

### 1.3 Recommended port order

1. **Geometry kernel + Gerber/Excellon parsers** — validate against real boards; nothing else can be
   trusted until polygon counts and areas match.
2. **Isolation + drilling pipelines end-to-end** — these two exercise ~80% of the core (buffering,
   boolean ops, TSP ordering, toolchange, G-code emission).
3. **Cutout, milling, NCC, panelize** — reuse the same primitives with different option sets.
4. **Preprocessors** — after emission is stable, port the ~30 dialects as data + small trait impls.
5. **Persistence** — last, but design the schema early (see §9) so the format is stable.

---

## 2. Runtime architecture

### 2.1 Entry chain

```
flatcam_plus/__main__.py            → thin shim, calls launcher.main()        (14 lines)
flatcam_plus/launcher.py:448        def main()
flatcam_plus/launcher.py:456        multiprocessing.freeze_support()
flatcam_plus/launcher.py:458-478    portable-mode flag (Windows only)
flatcam_plus/launcher.py:570        QApplication(...)
flatcam_plus/launcher.py:576-584    QSettings("Open Source", "FlatCAM_Plus")
flatcam_plus/launcher.py:596        App(qapp, user_defaults=..., startup_project_request=...)
flatcam_plus/launcher.py:599-601    100 ms keepalive QTimer
```

The launcher also owns the Windows PyQt6 DLL bootstrap and the global exception hook. For a Rust
port only three things here are logic: **argument parsing**, **settings namespace**, **portable vs
installed data root**.

### 2.2 `App` construction order (this order is a contract)

`appMain.py:138` `class App(QtCore.QObject)`; `:297`
`__init__(self, qapp, user_defaults=True, startup_project_request=None)`.

| Step | Line | Effect |
| --- | --- | --- |
| CLI parse | `appMain.py:150-167` | `getopt` — `cmd_line_shellfile`, `cmd_line_shellvar`, `cmd_line_headless`. **Unknown option ⇒ process exits.** |
| Parser discovery | `appMain.py:529` | `self.preprocessorpaths = self.preprocessors_path()` |
| Data root | `appMain.py:575` | `app_home` = package dir (checkout / wheel / frozen all resolve the same) |
| cwd | `appMain.py:584` | `os.chdir(app_home)` — *relative paths after this point are package-relative* |
| Defaults object | `appMain.py:589` | `AppDefaults(...)` |
| Load defaults | `appMain.py:592-594` | `self.defaults.load(...)` — user prefs merged over `factory_defaults` |
| Options dict | `appMain.py:599` | `AppOptions(...)` — flat, string-keyed |
| Numeric precision | `appMain.py:629` | `self.decimals = int(self.options['units_precision'])` |
| Preprocessors | `appMain.py:745` | `self.preprocessors = load_preprocessors(self)` |
| PP ordering | `appMain.py:748-760` | `default` is popped, the rest copied into a fresh dict ⇒ **deterministic preprocessor order** |
| PP filtering | `appMain.py:766-769` | names containing `Paste` are excluded from the geometry combo (they belong to solder-paste tooling) |
| Tools | `appMain.py:~1965` | each `Tool*` instantiated and appended to `app.app_plugins` |

**Port note.** `options` is a flat `str → value` dict, not nested config. Every tool reads and writes
it. Preserve the flat shape: it is what project files and the Tcl shell address.

### 2.3 Plugin model

- Base: `AppTool` / `AppToolEditor` in `flatcam_plus/appTool.py`.
- Contract: implement `install()`, then register in `App.app_plugins`.
- Each tool file holds **both** the logic class and its Qt UI class (`ToolX` + `XUI`), sometimes
  3000+ lines apart. The UI class is the option surface; the logic class is the pipeline.
- Tools that need domain primitives **mix in** the parser class to inherit geometry methods:
  `ToolIsolation(Gerber, AppTool)` `ToolIsolation.py:42`, `NonCopperClear(Gerber, AppTool)`
  `ToolNCC.py:40`, `ToolDrilling(Excellon, AppTool)` `ToolDrilling.py:39`,
  `ToolMilling(Excellon, AppTool)` `ToolMilling.py:84`, `ToolPaint(Gerber, AppTool)`
  `ToolPaint.py:46`, `ToolPunchGerber(Gerber, AppTool)` `ToolPunchGerber.py:36`.

  This multiple inheritance is how a tool gets `isolation_geometry()`, `buffer()`, `bounds()` etc.
  without a separate geometry handle. **In Rust this should be a plain function library**, not traits
  on the tool objects.

### 2.4 Long-running work

Pipelines are executed through a worker/thread mechanism and report progress via
`app.inform.emit(msg)`. Message codes are load-bearing (the GUI filters on the prefix):

| Prefix | Meaning |
| --- | --- |
| `[success]` | completed |
| `[WARNING]` <br> `[WARNING_NOTCL]` | recoverable problem (the `_NOTCL` variant suppresses Tcl echo) |
| `[ERROR]` <br> `[ERROR_NOTCL]` | failure; most pipelines `return 'fail'` after emitting |

**Port note.** The `'fail'` return sentinel + `inform.emit` idiom appears in nearly every tool. Model
it as `Result<T, ToolError>` in Rust and keep the message text verbatim — operators and scripts
grep for it.

---

## 3. Document object model

### 3.1 Shapes

Every document object is a **dual-inheritance** class: a domain class plus the Qt shell.

| Object | Domain base | Qt base | Definition |
| --- | --- | --- | --- |
| `GerberObject` | `Gerber` | `FlatCAMObj` | `appObjects/GerberObject.py:37,57` |
| `ExcellonObject` | `Excellon` | `FlatCAMObj` | `appObjects/ExcellonObject.py:36,54` |
| `GeometryObject` | `Geometry` | `FlatCAMObj` | `appObjects/GeometryObject.py:44,64` |
| `CNCJobObject` | `CNCjob` | `FlatCAMObj` | `appObjects/CNCJobObject.py:40,64` |
| `ScriptObject` | — | `FlatCAMObj` | `appObjects/ScriptObject.py:28,42` |
| `DocumentObject` | — | `FlatCAMObj` | `appObjects/DocumentObject.py:28,42` |

`FlatCAMObj` — `appObjects/AppObjectTemplate.py:55`:

| Member | Line | Purpose |
| --- | --- | --- |
| `plot_single_object` signal | `:67` | object asks the canvas to redraw itself |
| `calculations_finished(float,float,float,float,float,object)` | `:70` | bounds + result payload, emitted after a generation run |
| `obj_options` | `:91` | `LoudDict` — **the object's public property bag** |
| `set_change_callback` | `:92` | mutation hook: changing an option triggers recalculation/redraw |
| `form_fields` | `:94` | UI descriptor list (skip in Rust) |
| `load_complete` | set by collection at `ObjectCollection.py:731` | guards against acting on a half-built object |

Two discriminator fields carry all polymorphism:

- **`kind`** — which collection bucket the object lives in *and* the serialization discriminator
  (`gerber`, `excellon`, `geometry`, `cncjob`, `script`). Asserted by tools, e.g.
  `ToolDrilling.py:2077` `assert cnc_job_obj.kind == 'cncjob'`.
- **`ui_type`** — which Qt editor panel to build.

**Port note.** `kind` is the Rust enum tag. `obj_options` should become a typed per-kind struct plus a
generic key-value escape hatch, because saved projects store it as a dict and unknown keys must survive
a round-trip.

### 3.2 `LoudDict`

`obj_options` is a `LoudDict` (`appCommon/Common.py`, imported at `defaults.py:1`): a dict that fires a
callback on mutation. This is why changing an option in the UI can immediately invalidate a
`CNCJob`. In Rust, prefer explicit `mutate` methods that set a `dirty` flag over a magic dict.

### 3.3 Object collection

`ObjectCollection.py` — ordered store keyed by name, with `get_by_name` used everywhere
(e.g. `ToolDrilling.py:1979-1986`). Lookup-by-name failure is handled by emitting
`'[ERROR_NOTCL] Could not retrieve object'` and returning, **not** by raising.

---

## 4. I/O layer — parsers

### 4.1 Uniform contract

- Each parser is a class deriving from `Geometry`, so a parsed object *is* a geometry container.
- Constructor/`parse` fills `self.solid_geometry` and `obj_options` bounds.
- Errors: `app.inform.emit('[WARNING]…' or '[ERROR_NOTCL]…')`; **exception** to that rule is
  `raise grace` when `app.abort_flag` is set (user abort), per `ParseGerber.py` abort checks.
- Return `'fail'` when nothing parseable was produced.
- Circular approximation everywhere uses `app.options["geometry_circle_steps"]`.

### 4.2 Gerber — `appParsers/ParseGerber.py` (126 KB, the hard one)

`class Gerber(Geometry)` — `ParseGerber.py:32`.

Arc handling (`:1535-1624`) is the part that must be reimplemented exactly:

| Behavior | Line |
| --- | --- |
| Multi-quadrant centre resolution via `np.arctan2` | `:1535-1624` |
| `current == stop` ⇒ full circle (360°) | `:1538-1541` |
| Single-centre solution rejected if deviation > 5% | `:1565-1580` |
| Accept when `angle <= (pi + 1e-6)/2` | `:1595` |
| No valid centre ⇒ `log.warning("Invalid arc in line %d.")` **and the segment is dropped** | `:1616` |
| `M02` handled as a no-op terminator | `:1621-1623` |
| Unrecognized line ⇒ warning path | `:1628` |

**Port note.** "Drop the segment and warn" is the important semantic: a bad arc must not abort the
file, and must not be silently approximated. Reproduce the 5% / `pi/2` tolerances literally or board
output will differ.

Also in this file *(explorer-reported)*: aperture macro instantiation (`ApertureMacro` lives in
`camlib.py:95` with `make_circle:248`, `make_vector_line:263`, `make_center_line:287`,
`make_lower_left_line:310`, `make_outline:333`, `make_polygon:378`, `make_moire:409`,
`make_thermal:456`, `make_geometry:482`).

### 4.3 Excellon — `appParsers/ParseExcellon.py`

`class Excellon(Geometry)` — `ParseExcellon.py:31`. Produces a `tools` dict:
`tool_number → { 'tooldia': float, 'drills': [Point…], 'slots': [(Point, Point)…], 'data': {…} }`.
The `data` sub-dict is later mutated by the drilling UI (`tools_drill_toolchange`,
`tools_drill_toolchangexy`, `tools_drill_drill_slots`, `offset`, …) — see §6.2.

*(Per-format detail beyond this is explorer-reported; re-read before implementing.)*

### 4.4 SVG (`ParseSVG.py`) — verified structure

Uses `svg.path` (`Line`, `Arc`, `CubicBezier`, `QuadraticBezier`, `parse_path`, `Move`) and `shapely` (`LineString`, `MultiLineString`, `Point`, `affinity.skew/affine_transform/rotate`) plus `numpy`. Functions (line refs): `svgparselength` :36, `svgparse_viewbox` :61, `path2shapely(path, object_type, res=1.0, units='MM', factor=1.0)` :76, `svgrect2shapely` :233, `svgcircle2shapely` :310, `svgellipse2shapely` :336, `svgline2shapely` :369, `svgpolyline2shapely` :388, `svgpolygon2shapely` :405, `getsvggeo(node, object_type, root=None, units='MM', res=64, factor=1.0, app=None)` :425, `getsvgtext` :549, `parse_svg_point_list` :653, `parse_svg_transform(trstr)` :691.

- **`path2shapely`** discretizes curves (Arc/Cubic/QuadraticBezier): `length = component.length(res/10.0)`, `steps = int(length)*2`, doubled again in `IN` units, **minimum 10 steps** (and guarded at 0) — curve fidelity depends on `res`/units; `Move` starts a new ring/segment set; returns `Polygon` or `LineString`/`MultiLineString` by `object_type`/closure. Porting: keep the min-step guard and the `IN`×25 scaling.
- **`getsvggeo`** recurses children (accumulates `subgeo`; returns on `'fail'`), then parses by tag: `path` → `parse_path` + `path2shapely`; other shapes via their `svg*2shapely` helpers. It reads/applies the node `transform` attribute via `parse_svg_transform` (seen at `:524-525`, `:625-626`) and consults parent attribs (`getparent().attrib` `:576`) for inherited context — **nested/group transforms matter**.
- **`parse_svg_transform`** (:691+) parses a full transform string (regex, numbers support exponents) into a list applied **in reverse order** (`for tr in trlist[::-1]`, :520+). Kinds: `translate(tx[,ty])`, `scale(sx[,sy])` with `origin=(0,0)`, `rotate(angle[,cx,cy])` with origin `(cx,cy)`, `skewX/Y` → `['skew',ax,ay]` with `origin=(0,0)`, `matrix(a,b,c,d,e,f)`. Tag handling in `getsvggeo`: `path/rect/circle/ellipse/polygon/line/polyline`; `use` resolves `href`/`xlink:href` to `#id` and recurses; `defs/namedview/format/type/title/desc/svg` skipped with warning; **`g` (group) warns and returns `"fail"`** (grouped/transform-inherited content is a known fragile path); unknown kinds ignored. Porting: reproduce reverse-application order and per-kind `origin` semantics; treat `g`/nested transform parity as a dedicated fidelity task (current behavior can fail the parse).

### 4.5 Font/outline → geometry (`ParseFont.py:38` `class ParseFont`)

Verified libs: `import freetype as ft` (:22) and `from fontTools import ttLib` (:23); name lookup via `ttLib.TTFont` (:183), outline rasterization via `ft.Face(path_filename)` (:310, FreeType outline convention per tutorial step2). Static helpers: `get_win32_font_path` :44, `get_linux_font_paths` :61, `get_mac_font_paths` :107, `get_win32_fonts` :132, `get_font_name` :171. Instance: `get_fonts` :215, `get_fonts_by_types` :241, `font_to_geometry(char_string, font_name, font_type, font_size, units='MM', coordx=0, coordy=0)` :286. Porting: use `ttf-parser`/`rustybuzz`+`ab_glyph`/`fontdue` or FreeType bindings; preserve TrueType outline winding and unit→MM scaling to keep text geometry parity.

### 4.6 DXF (`ParseDXF.py`, `ParseDXF_Spline.py`) — verified structure

`ParseDXF.py` converters: `dxfpoint2shapely` :28, `dxfline2shapely` :34, `dxfcircle2shapely` :49, `dxfarc2shapely` :64, `dxfellipse2shapely` :160, `dxfpolyline2shapely` :219, `dxflwpolyline2shapely` :231, `dxfsolid2shapely` :244, `dxfspline2shapely` :254, `dxftrace2shapely` :276, `getdxfgeo` :286 (modelspace → `get_geo`), `get_geo_from_insert` :296, `get_geo(container)` :347, `getdxftext` :389.

`get_geo` dispatches by `dxf_entity.dxftype()` → `POINT, LINE, CIRCLE, ARC, ELLIPSE, LWPOLYLINE, POLYLINE, SOLID, TRACE, SPLINE, INSERT` (recurse blocks via `get_geo_from_insert`, applying insert rotation/translation/scale); unsupported types are logged and skipped; returns a flat list of shapely geoms.

`dxfspline2shapely` uses `spline.control_points`/`knots`/`closed`/`degree` and calls `spline2Polyline(..., segments=20, knots=...)` from `ParseDXF_Spline.py` (NURBS/B-spline: `Vector`, rational basis `_rbasis/_rbspline/_rbsplinu`, knot helpers; :26/:157/:229/:286). **INSERT/block transforms** (`get_geo_from_insert` :296): reads `rotation`, `insert` base, `xscale/yscale`, `row_count/row_spacing`, `column_count/column_spacing`; resolves block by name, translates relative to `block.base_point`, expands row/column arrays, applies `scale(sx,sy)`, then `rotate(phi, origin=tr)` (origin may be `'c'`/`'center'` or a vector). **Porting:** preserve fixed `segments=20` (or same default), full INSERT array+scale+rotate+base_point semantics, and closed polylines; confirm `$INSUNITS`/unit handling and non-uniform scale edge cases when implementing DXF fidelity.

### 4.7 HPGL2 (`ParseHPGL2.py`) — verified structure

`class HPGL2` :31 (`__init__` :36, `parse_file` :153, `parse_lines` :166, `parse_number` :436 returns `float(strnumber)/40.0` **mm**). Holds `tools` (pen → `{tooldia, offset, offset_value, type, tool_type, data, solid_geometry}`), `default_data` (travelz/cut/feedrates/dwell/multidepth/pp/end/start/toolchange, `:60-110`), regex-driven state machine with `abort_flag`/`grace`. Commands (verified): `CO` comment, `IN` init, `SP<n>` select pen (flushes current path as `LineString` to previous tool, creates tool with `tools_mill_tooldia`), `PU/PD` pen up/down, `PA x,y` absolute / `PR x,y` relative (PD appends, PU flushes + starts new path; warns on missing coords), `CI r` circle at current point → buffered exterior `LineString`, `AA cx,cy,angle` arc (deg, signed dir → ccw/cw, start at current, end updates current), `AT x1,y1,x2,y2` 3-point arc via `three_point_circle`/`arc` (end becomes p2). Unmatched lines warned+ignored; fails if no geometry. Final `unary_union` of `geo_buffer` → `solid_geometry`. Porting: stateful plotter (tool, pen, current point, abs/rel, path flush on PU/SP/end) with `steps_per_circle`; confirm any remaining commands (LB/labels, EA/ER, scaling) if present beyond the read range.

---

## 5. Geometry core — `appParsers`-agnostic `camlib.py`

### 5.1 Module-level facts

| Symbol | Line | Note |
| --- | --- | --- |
| `HAS_ORTOOLS` | `camlib.py:61` | optional `ortools` gate; imports at `:65-66` (`pywrapcp`, `routing_enums_pb2`) |
| `is_isolation_job_type()` | `:84` | object-type helper |
| `ParseError` | `:91` | raised by geometry construction on invalid input |
| `ApertureMacro` | `:95` | Gerber aperture macro evaluator (see §4.2) |
| `AttrDict` | `:2722` | dict + attribute access (used by preprocessors as `p[...]` **and** `p.x`) |
| `CNCjob(Geometry)` | `:2728` | toolpath container + emitter |

### 5.2 `Geometry` — `camlib.py:539`

The universal geometry container. Holds shapely geometries (polygons, lines, points) in
`solid_geometry` (list or single geometry), plus `fill_*` companions for copper-fill use cases.

| Method | Line | Semantics |
| --- | --- | --- |
| `add_circle(origin, radius, tool)` | `:606` | circle → polygon with `steps_per_circle` |
| `add_polygon(…, tool)` | `:649` | polygon with optional tool-diameter handling |
| `add_polyline(…, tool)` | `:690` | polyline; closed polyline ⇒ polygon |
| `subtract_polygon` | `:742` | boolean difference |
| `bounds(flatten=False)` | `:771` | recursive bounds (`bounds_rec` `:783`) |
| `find_polygon(point)` | `:898` | point-in-geometry lookup |
| `get_interiors()` | `:928` | holes |
| `get_exteriors()` | `:944` | outer rings |
| `flatten()` | `:968` | **listify** nested geometry into a flat list of primitives |
| `flatten_exterior_interiors()` | `:1009` | split each polygon into (exterior, [interiors]) |
| `isolation_geometry(offset, geometry=None, iso_type=2, corner=None, passes=0, prog_plot=False)` | `:1112` | **the isolation primitive** (§5.3) |
| `buffer(distance, join, factor, only_exterior=False, muted=False)` | `:2655` | shapely-style buffer; `buffer_geom` helper `:2671` |
| `bounds(flatten=None)` (CNCjob override) | `:7400` | path bounds |

`flatten()`-family helpers are used constantly; every tool starts by normalizing to a flat list. A
Rust port should make "flat list of `Polygon`/`LineString`" the canonical internal representation and
provide the same normalization at the boundary.

### 5.3 Isolation geometry (verified, parity-critical)

`camlib.Geometry.isolation_geometry(offset, geometry=None, iso_type=2, corner=None, passes=0, prog_plot=False)` (`:1112-1189`):
1. Flattens `geometry`/`self.solid_geometry`. Per primitive (abort-checked): if `offset==0` keep it, else `pol.buffer(offset, int(self.geo_steps_per_circle), join_style=corner_type)` with `corner_type = "round" if corner is None else corner` (`:1153-54`). Progress reports `"Pass %d"` as `passes+1` (`:1164`).
2. `geo_iso = unary_union(geo_iso)`.
3. Filter by `iso_type`: `2`→`flatten_shapely_geometry(geo_iso)` (complete); `0`→`self.get_exteriors(geo_iso)` (returns `[geo.exterior for geo]`, `:944-962`); `1`→`self.get_interiors(geo_iso)` (returns `[interior for geo.interiors]`, `:928-943`); else log + **`return "fail"`** (`:1181-83`).
4. `prog_plot=='progressive'` plots each element to a temp shape; returns the list/geometry.

**Semantics:** `passes` is a 0-based *reporting* index only — the multi-pass/orbit loop is tool-orchestrated. `geo_steps_per_circle` controls buffer resolution. `offset` sign is meaningful (NCC notes the envelope goes to the *left*, `ToolNCC.py:3842`). Join styles passed through GEOS/Shapely include `round` (default) and `"mitre"` (used across isolation/NCC/milling). `iso_type 0/1` yield ring/linear geometries (exterior/interior boundaries), not filled polygons.

`ToolIsolation.generate_envelope` (`:3682+`) wraps this: calls `isolation_geometry(..., passes=nr_passes)`; if `invert`, reverses orientation — `Polygon(exterior.coords[::-1], interiors)` / `LinearRing.coords[::-1]` (handles Polygon/LinearRing/Multi*), keeping interiors as-is. **Parity risk:** interior ring orientation on invert and ring-vs-polygon outputs from `iso_type 0/1`.

### 5.4 Path ordering (TSP)

Drill/paint ordering minimizes travel using ortools:

| Symbol | Line |
| --- | --- |
| `optimized_ortools_meta(locations, start=None, opt_time=0)` | `camlib.py:3040` |
| `optimized_ortools_basic(locations, start=None)` | `camlib.py:3104` |
| call sites (7 pipelines) | `:3486`, `:4017`, `:4482`, `:4723`, `:5623` |

**Port note.** `ORTools` constraint-solver TSP maps to a small nearest-neighbour + 2-opt implementation
in Rust for typical board sizes (<10k points); only adopt a real solver if benchmarks demand it. Keep
`opt_time` semantics (bounded effort) so results stay deterministic across runs — operators compare
G-code between machines.

### 5.5 Miscellaneous numeric helpers (pure, easy port, high test value)

| Helper | Line |
| --- | --- |
| `flatten_shapely_geometry` | `camlib.py:7917` |
| `min_element_distance` | `:7942` |
| `get_bounds` | `:7988` |
| `arc` | `:8014` |
| `arc2` | `:8054` |
| `arc_angle` | `:8061` |
| `to_dict` | `:8099` |
| `dict2obj` | `:8124` |
| `autolist` | `:8452` |
| `three_point_circle` | `:8462` |
| `distance` | `:8496` |
| `AppRTree` | `:8504` |
| `AppRTreeStorage` | `:8579` |

`AppRTree`/`AppRTreeStorage` wrap shapely STRtree for spatial indexing (used for hit-testing and
`find_polygon`). In Rust: `rstar`, or a plain grid for board-scale inputs.

---

## 6. Tool pipelines

Ordered, option-driven transformations. Each entry: the logic class, its UI class (the option
surface), and the verified skeleton.

### 6.1 Tool inventory

| Tool | Logic class | UI class |
| --- | --- | --- |
| Isolation | `ToolIsolation.py:42` `ToolIsolation(Gerber, AppTool)` | `IsoUI` `:3927` |
| Cutout | `ToolCutOut.py:42` `CutOut` | `CutoutUI` `:2366` |
| Drilling | `ToolDrilling.py:39` `ToolDrilling(Excellon, AppTool)` | `DrillingUI` `:2397` |
| Milling | `ToolMilling.py:84` `ToolMilling(Excellon, AppTool)` | `MillingUI` `:4283` |
| Paint | `ToolPaint.py:46` `ToolPaint(Gerber, AppTool)` | — |
| Non-copper clear | `ToolNCC.py:40` `NonCopperClear(Gerber, AppTool)` | (UI `__init__` `:3991` *(explorer-reported)*) |
| Panelize | `ToolPanelize.py:34` `Panelize` | `PanelizeUI` `:1134` |
| Copper thieving | `ToolCopperThieving.py:39` | `ThievingUI` `:1233` |
| Film | `ToolFilm.py:47` `Film` | `FilmUI` `:1271` |
| Punch | `ToolPunchGerber.py:36` `ToolPunchGerber(Gerber, AppTool)` | `PunchUI` `:1998` |
| Solder paste | `ToolSolderPaste.py:39` `SolderPaste` | `SolderUI` `:1248` |
| Design rules | `ToolRulesCheck.py:34` `RulesCheck` | `RulesUI` `:1191` |
| CNC control | `ToolCNCControl.py:43` `ToolCNCControl` | — |
| Plotter | `ToolPlotter.py:33` `ToolPlotter` | — |

### 6.2 Drilling — fully traced example (`ToolDrilling.py:1968 on_generate_cnc_job`)

This is the cleanest full pipeline; use it as the template for the others.

```
1. resolve source          app.collection.get_by_name(name)                 :1979-1986
2. read bounds             obj.obj_options['xmin'|'ymin'|'xmax'|'ymax']      :1996-1999
3. job name                obj_options['name'] + "_cnc"                     :2001
4. validate                is_valid_excellon()  → '[ERROR_NOTCL] no drills'  :2004-2008
5. selected tool ids       get_selected_tools_uid() (+ single-row rule)      :2011-2023
6. tool ordering           sort by diameter, 'fwd' | 'rev' | none            :2038-2047
7. drill points            create_drill_points(...)                          :2053  (def :1906)
8. exclusion check         app.exc_areas: any drill inside ⇒ 'fail'          :2055-2058
9. optimizer choice        app.options['excellon_optimization_type'];
                           forced 'T' on 32-bit platforms                    :2062-2065
10. job_init(cnc_job_obj, app_obj):                                          :2076
      per tool:
        cnc_job_obj.tools[id] = deepcopy(excellon_tools[id])
        nr_drills = len(points[id])
        solid_geometry += [ p.buffer(tooldia/2, resolution=geo_steps_per_circle)
                            for p in points[id] ]                        :2080-2106
        slots (unless tools_drill_drill_slots):
          solid_geometry += [ LineString([start,stop]).buffer(tooldia/2, …) ] :2098-2106
        offset = -float(data['offset'] or 0)                             :2108-2133
        gcode = ''; gcode_parsed = []                                    :2130-2133
11. preprocessor wiring     pp_excellon_name / pp_excellon from UI           :2196-2198
                            obj_options: type='Excellon', ppname_e, bounds    :2200-2206
                            coords_decimals, fr_decimals, multitool=True,
                            seg_x/seg_y from options                         :2212-2218
12. toolchange XY           parse "(x, y)" via re.sub + eval → oldx/oldy     :2220-2242
13. no-toolchange branch:
      merge all points, used_tool = sel_tools[0]
      excellon_tool_gcode_gen(tool, pts, tools, first_pt,
                              is_first=True, is_last=True,
                              opt_type=..., toolchange=False)            :2262-2269
      excellon_tool_gcode_parse(tooldia, gcode=..., start_pt=...)        :2272-2273
      store gcode / gcode_parsed per tool                                 :2276-2280
14. toolchange branch: same gen/parse pair per tool                        (tail not fully read)
```

Emitter entry points:

| Function | Line |
| --- | --- |
| `excellon_tool_gcode_gen(tool, points, tools, first_pt, is_first=False, is_last=False, opt_type='T', …)` | `camlib.py:3324` |
| `excellon_tool_gcode_parse(dia, gcode, start_pt=(0,0), force_parsing=None)` | `camlib.py:6396` |

**Port note (important).** `offset` is stored **negated** (`-1 * option`), and G-code is stored twice
per tool: as text (`gcode`) and as parsed moves (`gcode_parsed`). The parsed form is what the GUI
plots and what 3D preview consumes — so the emitter is effectively parsable output, and both forms
must agree.

### 6.3 Isolation (`ToolIsolation.py`)

- `set_tool_ui()` — `:287` (reads `tools_iso_passes`, `tools_iso_pad_passes`, `tools_iso_overlap`,
  `tools_iso_milling_type`, `tools_iso_combine_passes`, `tools_iso_isotype` …) *(explorer-reported)*.
- `isolate(isolated_obj, sel_tools, tools_storage, geometry=None, limited_area=None, negative_dia=None, …)`
  — `:2332`; kwargs `combine` `:2355`, `iso_except` `:2357`, `e_passes` `:2359`,
  `area_except` `:2361` → calls into `camlib.isolation_geometry`.
- `isolate_with_copper(...)` — `:2200`.
- `combined_normal(...)` — `:2792`.
- **2026-era safety layer** (worth porting, it encodes real failure modes):
  - `_path_geometry_for_cut_model(geometry, include_interiors=False) :1956` (verified) → `LineString`s: `Polygon` → exterior (+ interiors if `include_interiors`), `LinearRing` → its coords, `LineString` kept, `MultiLineString` expanded; other kinds ignored. `_isolation_cut_paths :1971` uses `include_interiors=True` and keeps only non-empty `length>0` paths (these become the emitted cut path set). `_cut_model_overlaps_copper :1978+` builds **exterior paths only** (`include_interiors` default false), buffers each by `tool_radius` (`gerber_circle_steps`) and unions → `cut_model`; penetration = `cut_model ∩ source_core`, `source_core = unary_union(source).buffer(-edge_tolerance)` (`edge_tolerance=max(r*0.01,1e-5)`), flagged when `penetration_area > max(1e-8, source_area*1e-9)` (ratio `penetration/source`). `combined_normal :2792+` emits `_isolation_cut_paths(new_solid_geo)` (and reads `overlap` but buffers per pass at `abs(tool_dia)/2`); `combined_rest :2572` delegates per tool to `generate_rest_geometry` (overlap/`env_iso_type` applied there) and also emits path geometry per storage flow.
  - `_safe_isolation_geometry :2018` rebuilds `unary_union(source).buffer(abs(offset), gerber_circle_steps, "round")` (reverses exteriors if `invert`); `_guard_isolation_geometry :2044` keeps result if clean, else repaired non-overlapping or ERROR %.
  - Copper-pour split `_generate_copper_envelope :2165+`: `iso_offset = tool_dia*((2*i+1)/2)-(i*overlap*tool_dia)` (negable). `_split_copper_geometry :2136` drops frame-like; classifies pours if `(spans_board w/h≥55% & area≥15%) or (interiors>0 & area≥8%)` → `iso_type=1` (interiors) else regular `iso_type=2`; output frame-filtered, then subtract/intersect, simplify, guard, `_isolation_cut_paths`. `area_subtraction/intersection :3008/:3039` operate on `poly2rings` (exterior+interiors `:3671`) against a union.
  - `generate_envelope` call `:3710`.

**Port note.** This guard is the difference between "plausible toolpath" and "cuts into the copper".
Port the *metric* (overlap area ratio) as a pure function with tests, even if the retry strategy
changes.

### 6.4 Non-copper clear (`ToolNCC.py`)

- `clear_copper_tcl(ncc_obj, sel_obj=None, ncctooldia=None, isotooldia=None, margin=None, has_offset=None, …)`
  — `:2836`: the Tcl-facing entry — **this is the best-documented signature of the whole pipeline**.
- Envelope computation via `isolation_geometry(offset, iso_type=envelope_iso_type)` — `:3849`.
- Result semantics at `:3842`: envelope lies to the *left* of the geometry.

### 6.5 Milling (`ToolMilling.py`)

- Documented as a wrapper over `generate_from_geometry_2()` — `:3308`.
- Call site `:3530` `new_cncjob_obj.generate_from_geometry_2(...)`; signature at
  `camlib.py:5301`:
  `generate_from_geometry_2(geo_obj, append=True, tooldia=None, offset=0.0, tolerance=0, z_cut=None, …)`.
- Milling has two modes with different sources: *milling drills* (`ExcellonObject.py:959
  generate_milling_drills`) and *milling slots* (`ExcellonObject.py:1064 generate_milling_slots`),
  both UI-triggered from `:1183` / `:1188`.

### 6.6 Paint / NCC (verified)

- **`ToolPaint(Gerber, AppTool)`** `:46`, `on_paint_button_click` :1131: resolves source (circle steps `gerber_circle_steps`/`geometry_circle_steps`), rejects multigeo with >1 tool (`'Fail'`), collects selected tool diameters, builds `<name>_mt_paint`. **Selection state machine** (`:1212+/:1233-34/:1348-1505/:1472+`): disconnects default canvas handlers and connects `mouse_release/move` (+ key/Escape restores and re-enables notebook). `area_shape_radio`: `square` = 2-click rect (arm → finalize, draw temp); polygon = accumulate `points`, right-click closes (needs ≥3, invalid dropped). Zones accumulate in `self.sel_rect`; on finish `sel_rect = unary_union(self.sel_rect)` → `paint_poly_area(obj, sel_obj=sel_rect, tooldia=..., outname=...)` (`:2472`).
- **`paint_poly_area/ref/all` → `paint_geo`** (`:1856`): `paint_poly_area` intersects `unary_union(obj.solid_geometry)` with `sel_obj` (gerber progressive may `buffer(0)`), flattens/merges (abort-checked `recurse` with `grace`), coerces to closed `Polygon`s, warns/returns on none, then `paint_geo`. `paint_geo` resolves method (`ui.paintmethod_combo`), order (`paint_order_combo`, 1=forward, 2=reverse), tool list (selected rows or `tooldia`), `prog_plot`, `use_rest_strategy` (UI/rest). **Normal clear** (`job_normal_clear`): for each tool (in order) finds `current_uid` by matching tooldia at `decimals`, reads `tools_paint_overlap` (%→fraction), `tools_paint_connect`, `tools_paint_contour`, `tools_paint_offset`; buffers each polygon by `-paint_offset` (drops empties; too-big tool → error/continue); per polygon calls `paint_polygon_worker` (abort-checked, progress 0-100%), collects `get_objects()` (optionally `simplify(0.01)`), stores per tool and unions `final_solid_geometry`. `paint_polygon_worker` (`:1626`) delegates by method: `0` Standard→`clear_polygon_shrink`, `1` Seed→`clear_polygon_seed` (+optional seedpoint), `2` Lines→`clear_polygon_lines`, `3` Laser_lines (gerber-aperture-driven, separate path) — all `(polygon,tooldia,steps_per_circle,overlap,connect,contour,prog_plot)` returning an `AppRTreeStorage`/object set via `get_objects()`. **Rest machining** (`job_rest_clear`): sorts tools **reverse** (large→small), uses `rest_offset_entry` to buffer the whole geometry once, then for each tool does a speedup check `pp.buffer(-tool_dia/2)` (skip if empty), clears, computes `rest = pp.difference(unary_union(cleared).buffer(tool_dia/2))` and carries `rest_list` forward, shrinks `poly_buf = MultiPolygon(poly_buf).difference(buffered_cleared(tool_dia/2))` (cleaned/flattened) — breaks when rest is empty/invalid. Empty result / no tool solid geo → error; creates multigeo/multitool geometry (threaded via worker).
- **camlib clear primitives (fill core, verified):** all return `AppRTreeStorage` indexed by path endpoints (`get_pts = [coords[0],coords[-1]]`), with `get_objects()`/`nearest()`; abort via `grace` + `processEvents`.
  - `clear_polygon_shrink :1414` (contour arg inconsequential): `current = polygon.buffer(-tooldia/2, steps).flatten()`; insert each `exterior`+`interiors`. While shrinking `cl_pol.buffer(-tooldia*(1-overlap), steps)`: for every piece with `area>0` insert exterior+interiors (progressive plot), union pieces for next iter, stop when nothing added. If `connect`: `Geometry.paint_connect(geoms, polygon, tooldia, steps)`.
  - `clear_polygon_seed :1531`: margin `polygon.buffer(-tooldia/2).flatten().simplify(simplify_tol)` → `MultiPolygon`; seed = `seedpoint` or `representative_point()`; `radius` starts at `tooldia/2*(1-overlap)`. Loop: `Point(seed).buffer(radius).exterior.simplify.intersection(path_margin)` → flatten+insert until empty, `radius += tooldia*(1-overlap)`. If `contour`: insert margin `exterior`+`interiors`. If `connect`: `paint_connect`.
  - `clear_polygon_lines :1645` (horizontal/vertical scanlines): margin `polygon.buffer(-tooldia/1.99999999, steps).simplify`. Orientation by bbox aspect: if `|w|>=|h|` sweep **Y** down from `top - tooldia/1.99999999` while `y > bot + tooldia/1.999999999` stepping `-tooldia*(1-overlap)`, intersect `LineString[(left,y),(right,y)]` with margin, flatten+collect (+plot), plus a final pass at `y = bot + tooldia/2`; else sweep **X** up from `left + tooldia/1.99999999` while `x < right - tooldia/1.999999999` stepping `+tooldia*(1-overlap)` along vertical lines, final `x = right + tooldia/2`. `unary_union` trimmed lines, insert; if `contour`: insert margin edges; if `connect`: `paint_connect`.
  - **`AppRTree`/`AppRTreeStorage` `:8504/:8579`**: rtree over endpoint/coords via `get_points`, `nearest`/`intersection`, stores objects (`get_objects`).
  - **`paint_connect` `:2055` (verified, edge cases):** `max_walk = max_walk or 10*tooldia` (falsy → default; `0` would default). Seeds from `(0,0)` → `storage.nearest((0,0))`; empty storage → `None`. Endpoints indexed as `[coords[0], coords[-1]]`. Per step: nearest `candidate` to `current_pt=geo.coords[-1]`; **reverse candidate iff** `pt != candidate.coords[0] and pt == candidate.coords[-1]` (prefer start; degenerate first==last keeps order). Link test: `walk_path = LineString([current_pt, pt])`, `walk_cut = walk_path.buffer(tooldia/2, int(steps_per_circle))`; **join only if `walk_cut.within(boundary) and walk_path.length < max_walk`** (strict `<`, uses Shapely `within` — boundary-touching `walk_cut` is not `within`) → append `candidate` coords to `geo`; else insert `geo` (lift) and continue with `candidate`. `StopIteration` inserts final `geo`. Returns optimized `AppRTreeStorage` (or `None`).
  - **`path_connect` `:2165` (verified):** joins paths whose endpoints **touch exactly** (coordinate equality), nearest-first from `origin`. Tries `left = nearest(geo.coords[0])` first: for `LineString`, 4 endpoint-equality cases concatenate (reversing `geo`/`left` as needed, keeping `geo` on the joined end) and `continue`. If no left join, tries `right = nearest(geo.coords[-1])` with 4 cases; **note a source quirk (parity):** case `right.coords[-1] == geo.coords[0]` does `storage.remove(right)` but builds `geo = LineString(list(left.coords)+list(geo.coords))` using the previously computed `left` (not `right`) — preserve this behavior for parity and flag as a likely bug if deviating. If right is a `LinearRing` and nothing extended, it is inserted and chain restarts with `right`; else `geo` is stowed and chain continues with `right`. `StopIteration` inserts final `geo`.
  - **Epsilon/parity notes:** `clear_polygon_lines` uses margins/steps with `1.99999999` / `1.999999999` (see `:1645`); `generate_rest_geometry` uses `buffer(iso_offset*1.99999)` for clearance checks. These literals affect path spacing/acceptance — keep them (or prove equivalence) for geometric parity.
- **`ToolNCC`** (`ToolNCC.py`): `clear_copper_tcl` `:2836`, envelope via `isolation_geometry(iso_type=...)` `:3849`; envelope goes to the *left* of geometry (`:3842`, semantic). **Milling slots:** `ToolMilling.py` :3308/:3530.

### 6.7 Remaining tools (verified entry points)

`CutOut :709` (rect/free, gaps `None/LR/TB/2LR/2TB/4/8`, mouse-bites), `Film :354/:432/:515` (SVG/PNG/PDF ± Excellon punch, scale/skew/mirror), `PunchGerber :538` (auto exc/fixed/ring/prop o pick interactivo), `Panelize :309` (rows×cols/spacing/constraints), `RulesCheck :193/:275/:386/:413/:468/:508` (violaciones), `CopperThieving :266/:361`, `SolderPaste :74` (paste + `AppPreProcTools`). All: opciones `tools_*` → geometría → `new_object`/export, `inform`/worker. *Leer state machines internas por herramienta al portar ese slice.*

---

## 7. G-code generation

### 7.1 Geometry path → G-code (`linear2gcode`)

`camlib.py:6844-6985`:

```
6870-6886  resolve z/feed values from kwargs + obj_options
6889-6897  simplify(tolerance) then segment() the shapely path
6912-6946  TRAVEL: for each move, ask app.exc_areas.travel_coordinates(start, end, tooldia);
           if the straight move crosses an exclusion area, route around it (z_move override)
6951-6956  PLUNGE: z_feedrate_code / down_code / feedrate_code
6961-6980  CUT: linear_code per point
6983-6984  RETRACT: lift_code
```

`linear2gcode_extra` — `:6987-7233` — adds the "extra cut" pass:

| Behavior | Line |
| --- | --- |
| conditional plunge | `:7098-7104` |
| `extracut_length` handling | `:7148-7176`, `:7180-7227` |
| known dead code (values assigned, never used) | `:7214-7215` |

Exclusion-area routing: `appCommon/Common.py:707` `travel_coordinates(self, start_point, end_point,
tooldia)` — returns detour points when the direct segment intersects a stored forbidden area.

**Port note.** `travel_coordinates` is a hard requirement, not a nicety: without it the head crashes
through fixtures. Port it together with the exclusion-area data model.

### 7.2 Preprocessor contract — `appPreProcessor.py` (169 lines, read in full)

```
:20   preprocessors = {}                      # global registry, name → PreProc instance
:25   sys.modules.setdefault('appPreProcessor', sys.modules[__name__])
                                              # back-compat for user preprocessors written pre-4.4b
:28   class ABCPreProcRegister(ABCMeta)
:34   → on class creation: instantiate and register by __name__;
        duplicate name logs 'Preprocessor %s has been overriden'
:39   class PreProc(metaclass=ABCPreProcRegister)
:155  def load_preprocessors(app)
:158  preprocessors_path_search = [data_path/'preprocessors', app_home/'preprocessors']
:163  loop: SourceFileLoader per *.py → module exec → class registration
```

**Required members of `PreProc`** (all abstract — a port must implement all of them):

| Geometry/CNC | Line | Solder-paste API (`AppPreProcTools`, `:85`) | Line |
| --- | --- | --- | --- |
| `start_code` | `:41` | `start_code` | — |
| `lift_code` | `:45` | `lift_code` / `lift_z_dispense_code` | — |
| `down_code` | `:49` | `down_z_start_code` / `down_z_stop_code` | — |
| `toolchange_code` | `:53` | `toolchange_code` | — |
| `up_to_zero_code` | `:57` | — | — |
| `rapid_code` | `:61` | `rapid_code` | — |
| `linear_code` | `:65` | `linear_code` | — |
| `end_code` | `:69` | `end_code` | — |
| `feedrate_code` | `:73` | `feedrate_xy_code` / `z_feedrate_code` / `feedrate_z_dispense_code` | — |
| `spindle_code` | `:77` | `spindle_fwd_code` / `spindle_rev_code` / `spindle_off_code` | — |
| `spindle_stop_code` | `:81` | `dwell_fwd_code` / `dwell_rev_code` | — |

**Class-level config** (`preprocessors/default.py:14`):

```python
class default(PreProc):
    include_header    = True        # :16  → caller decides whether to prepend the header block
    coordinate_format = "%.*f"      # :17  → decimals come from the format args
    feedrate_format   = '%.*f'      # :18
```

**Parameter object `p`.** `default.py` reads it both as a mapping (`p['units']`,
`p['xy_toolchange']`, `p['xy_end']`, `p['obj_options']`, `p['coords_decimals']`, `p['feedrate']`,
`p['z_feedrate']`, `p['feedrate_rapid']`, `p['z_cut']`) and by attribute (`p.x`, `p.y`,
`p.coords_decimals`). That is `AttrDict` (`camlib.py:2722`).

`default.py:20 start_code` emits, in order: header comments `(TOOL DIAMETER: …)`,
`(Feedrate_XY: …)`, `(Feedrate_Z: …)`, `(Feedrate rapids …)`, `(Z_Cut: …)` derived from
`p['obj_options']['xmin'|'xmax'|'ymin'|'ymax'|'type'|'tool_dia']`, then `G21`/`G20` per
`p['units']`, `G90`, `G94`, sleeve `M6` if `xy_toolchange`, and a return move to `xy_end`.

### 7.3 Header resolution in `CNCJobObject`

| Behavior | Line |
| --- | --- |
| preprocessor name for milling | `:926` (`tools_mill_ppname_g`), used `:929` |
| `toolchange` special cases by name: `'marlin'`, `'repetier'` | `:947-948` |
| `"toolchange_probe"` | `:955` |
| `'nccad'` | `:962` |
| `include_header` resolution | `:1133-1170`, fallbacks `ppname_g` → `tools_mill_ppname_g` → `preprocessors['default'].include_header` at `:1148` |

**Port note.** Name-based special-casing (`'marlin'` / `'repetier'` / `'toolchange_probe'` /
`'nccad'`) is a smell, but it is *behavior*: those dialects need M6 handled differently. In Rust,
replace it with a `toolchange_style` enum field on the preprocessor trait impl — same output, no
string matching.

### 7.4 Shipped preprocessors (27 files, `flatcam_plus/preprocessors/`)

`default`, `Default_no_M6`, `GRBL_11`, `GRBL_11_no_M6`, `GRBL_11_pen_plotter`, `GRBL_laser`,
`GRBL_laser_z`, `grbl_laser_eleks_drd`, `Marlin`, `Marlin_laser_FAN_pin`,
`Marlin_laser_Spindle_pin`, `Marlin_laser_z`, `Repetier`, `NCCAD9`, `ISEL_CNC`, `ISEL_ICP_CNC`,
`Berta_CNC`, `Roland_MDX_20`, `Roland_MDX_540`, `hpgl`, `Line_xyz`, `Check_points`,
`Toolchange_Manual`, `Toolchange_Probe_MACH3`, `Paste_1`, `Paste_GRBL`, `Paste_Marlin`.

Default geometry preprocessor: `tools_mill_ppname_g = "GRBL_11_pen_plotter"` (`appMain.py:1638`) —
note the plotter dialect is the stock default, not GRBL milling.

---

## 8. Tcl / scripting API

- **Base:** `TclCommand.py:10` `class TclCommand(object)` and `TclCommandSignaled(TclCommand)` :314. `flatcam_plus/tclCommands/` defines **75 `TclCommand*` classes** across 75 `TclCommand*.py` files (76 including the base module).
- **Arg parsing** (`parse_arguments` :164, `check_args` :201): `-keyword value` pairs (regex `^-([a-zA-Z].*)`) → options dict + positional arguments list; a dangling `-flag` becomes `None`. `execute_wrapper` :277/:348, `execute` :300.
- **Execution:** `tcl_shell_command` / `_execute_tcl_command` on the main thread (`appMain.py` ~:9429/:9451). Long-running commands should schedule work like tools do and report via signals/`inform`. Known command: `clear_copper_tcl` (`ToolNCC.py:2836`).

**Command inventory (verified by class/file enumeration):** `AddAperture, AddCircle, AddDrill, AddPolygon, AddPolyline, AddRectangle, AddSlot, AlignDrill, AlignDrillGrid, Bbox, Bounds, Buffer, ClearShell, Cncjob, CopperClear, Cutout, Delete, Drillcncjob, ExportDXF, ExportExcellon, ExportGcode, ExportGerber, ExportSVG, Exteriors, Follow, GeoCutout, GeoUnion, GetActive, GetNames, GetPath, GetSys, Help, ImportSvg, Interiors, Isolate, JoinExcellon, JoinGeometry, ListPP, ListSys, MillDrills, MillSlots, Mirror, New, NewExcellon, NewGeometry, NewGerber, Nregions, Offset, OpenDXF, OpenExcellon, OpenFolder, OpenGCode, OpenGerber, OpenProject, OpenSVG, Options, Paint, Panelize, PlotAll, PlotObjects, Quit, SaveProject, SaveSys, Scale, SetActive, SetOrigin, SetPath, SetSys, Skew, SplitGeometry, SubtractPoly, SubtractRectangle, Version, WriteGCode`.

**Porting:** replace Tcl with a Rust CLI/REPL or JSON protocol; mirror `-key value` + positional parsing for compatibility where useful, and preserve these semantic operations/option names. Read each `TclCommand*.py` for per-command required/optional args when implementing that command.

## 9. Data formats and persisted state

### 9.1 Defaults / options

`flatcam_plus/defaults.py` — `FIXED_UI_DEFAULTS` `:10` (**16** UI/color keys); `AppDefaults.factory_defaults` holds **613 keys** (prefixes `tools_*` 333, `global_*` 92, `gerber_*` 54, `excellon_*` 45, `geometry_*` 36, `cncjob_*` 25, `document_*` 9, `script_*` 4, `fa_*` 3, others `zoom*`/`units*`/`cnc*` 2). `factory_defaults["version"] = 8.992` is the defaults/preferences **format** version (not the app version). QSettings identity: `("Open Source", "FlatCAM_Plus")` (`launcher.py:576-584`). User data/log: `~/.FlatCAM` (Linux) / `%APPDATA%\FlatCAM` (Windows); `portable`/`headless` in `config/configuration.txt` are read **only on Windows**.

### 9.2 Project file format — `.FlatPrj` (verified: `appHandlers/appIO.py`)

- **Container:** JSON object `{"objs": [ ... ], "options": { ... }, "version": <app version>}` (`save_project` :2971-3080). `options` = `app.options` minus `APP_SCOPED_OPTIONS = frozenset({"global_theme","global_appearance","version"})` (`appIO.py:75,2998`). Saved `indent=2, sort_keys=True`, `default=to_dict`. `inf`/`nan` cleaned on the compressed path (`allow_nan=False` then a clean pass).
- **Compression (optional, `global_save_compressed`):** LZMA/XZ; compressed files start with magic `b'\xfd7zXZ\x00'` (`open_project` :2691 parse_worker detects magic and `lzma.decompress(...).decode('utf-8')`; otherwise plain `utf-8-sig`).
- **Load:** `json.loads(..., object_hook=dict2obj)` → nested dicts become `AttrDict`-like objects; then `on_file_new_project`, options updated, each object restored via `from_dict()` + `app_obj.new_object()`, then `plot_all()` if `plot` (`:2691` docstring). I/O also includes SVG/PNG/PDF/Gerber/Excellon/DXF export/import (`appIO.py` method inventory).

### 9.3 Object serialization (verified)

- **Base:** `FlatCAMObj.from_dict` (`AppObjectTemplate.py:144`) iterates `self.ser_attrs`; `attr == 'options'` does `self.obj_options.update(d[attr])` (**merge, not overwrite**) so app-set options survive; unknown attrs tolerated with a debug log (old project compat). `to_dict` serializes `ser_attrs` (`DocumentObject.to_dict` :314, `ScriptObject.to_dict` :242); subclasses may override (e.g. `GerberObject.serialize` :1850 returns `{"obj_options","kind"}`).
- **`ser_attrs` examples:** `ScriptObject`/`DocumentObject` `['obj_options','kind','source_file']` (:54/:45); `GerberObject` prepends `['obj_options','kind','fill_color','outline_color','alpha_level']` (:115). `kind` is the persisted type tag; `load_complete` wired in `ObjectCollection.py:731`.
- **`to_dict` / `dict2obj` helpers** (`camlib.py:8099/8124`, imported by `appIO.py:37`): serialize `ApertureMacro` as `{"__class__":"ApertureMacro","__inst": am.to_dict()}` and Shapely `BaseGeometry` as `{"__class__":"Shply","__inst": sdumps(obj)}` (via `sdumps`/`sloads`); reverse reconstructs them. **Rust implication:** represent geometry explicitly (WKT/WKB or an explicit schema) — not Python/pickle-like object graphs; model apertures as typed structs.

**Rust note:** define a stable, versioned on-disk schema (serde JSON/TOML/CBOR) for the option subset you need + object geometry/metadata; support reading `.FlatPrj` (plain JSON or XZ + `__class__` Shply/ApertureMacro tags) only if backward compatibility is required.

## 10. Rust porting matrix

| Python dependency | Used for | Rust replacement | Risk |
| --- | --- | --- | --- |
| `shapely` (GEOS) | all polygon/line ops, buffering, boolean ops | `geo` + `geo-types`, `i_overlay` for booleans | **High** — polygon `buffer` with join styles and `resolution` is not shipped by `geo`; needs `geo-buffer`/custom offsetting and numeric parity tests |
| `numpy` | arc math, coordinate arrays | `nalgebra` or plain `f64` math | Low |
| `ortools` (optional, `camlib.py:61`) | drill/paint TSP ordering | NN + 2-opt | Low-Medium (determinism) |
| `rtree`/STRtree (`AppRTree` `:8504`) | spatial index for hit tests | `rstar` | Low |
| `PyQt6` | GUI, canvas, editors | eframe/egui or Tauri | High effort, out of scope |
| `simplejson` | project serialization | `serde_json` (`preserve_order` for key stability) | Low |
| `gettext` + `_()` | UI/messages | keep message strings as constants; `rust-i18n` if needed | Low |
| Tcl (`appPlugins/ToolShell.py`, `tk.Tcl()`) | scripting shell | CLI + JSON-RPC | Low value |
| `multiprocessing.Pool` | parallel generation | `rayon` | Low (drop `freeze_support` complexity) |

**Geometry parity strategy.** Do not trust "looks right". Build a parity harness that, for a fixed
corpus of Gerber/Excellon files, compares: number of polygons, total area (ε = 1e-9 relative),
point counts after `flatten()`, and the exact G-code text. That single harness will catch the
majority of porting bugs.

---

## 11. Gotchas and invariants

1. **`app_home` + `os.chdir`** (`appMain.py:575,584`) — the process changes directory at startup.
   Any path handling in a port must be explicit; don't inherit "relative means package-relative".
2. **Two G-code representations** per tool (`gcode` text + `gcode_parsed` moves). Keep both in sync
   (`ToolDrilling.py:2276-2280`).
3. **`offset` is stored negated** for drills (`ToolDrilling.py:2108-2133`).
4. **Failures are messages, not exceptions.** Emit `[ERROR_NOTCL]` / `[WARNING]` and `return 'fail'`.
   Only `grace` (user abort) is raised.
5. **Bad geometry is dropped, not fatal**: invalid Gerber arcs are skipped with a warning
   (`ParseGerber.py:1616`).
6. **Isolation has an anti-collision guard** that can reject/repair a toolpath
   (`ToolIsolation.py:1978-2068`). Skipping it silently changes physical results.
7. **Exclusion areas affect travel**, not just validation (`Common.py:707`, used at
   `camlib.py:6912-6946`).
8. **Preprocessor name matching** for toolchange styles (`CNCJobObject.py:947-962`) is behavior.
9. **`kind` is polymorphic** (collection bucket + serializer tag); `ui_type` is presentation only.
10. **Determinism matters.** `optimized_ortools_meta(opt_time=…)` is time-bounded — a time-bounded
    heuristic in Rust will produce different paths. Prefer a deterministic budget (iteration count)
    and accept a one-time output change, or no G-code diffs will ever be reviewable.

---

## 12. Remaining gaps / verify-when-implementing

Verified (high fidelity): isolation core+branches+guard/cut-model penetration+copper-pour split+`poly2rings` sub/intersect; paint `paint_geo` normal/rest + shrink/seed/lines algorithms + `paint_connect`/`path_connect` (nearest-neighbor, boundary-constrained, `max_walk=10*tooldia`, endpoint-reverse) + `AppRTreeStorage`; DXF dispatch/INSERT arrays+scale+rotate+spline20; SVG discretization+reverse-order transforms (translate/scale/rotate/skew/**matrix affine**) + `<g>`/`use`; font libs; `.FlatPrj`/XZ/`ser_attrs`/Shply; worker+`grace`; Tcl base+75 inventory; HPGL2 IN/SP/PU/PD/PA/PR/CI/AA/AT (`/40` mm); `excellon_tool_gcode_gen/parse` core + `calculate_depths`. **Port decision (recorded): SVG = strict parity** (preserve `<g>` hard-fail + unit/viewBox quirks). **Read line-by-line when implementing:**

- **SVG `<g>`/units/viewBox (verified 2026-05-08, strict parity):** `getsvggeo` recurses children first; handles `path/rect/circle/ellipse/polygon/line/polyline/use`, skips `defs/namedview/format/type/title/desc/svg`, and **`kind == 'g'` logs warning and returns `'fail'`** (propagated up as failure → `import_svg` returns `'fail'`) — groups break import. `use` resolves `href`/`xlink:href` via `root.find(".//*[@id='…']")`. Transforms (when `geo` not None): parsed by `parse_svg_transform` (translate/scale/rotate(cx,cy)/skewX|Y/matrix `[a..f]`), applied in **reverse list order**; `matrix` → `affine_transform(geoi, tr[1:])` passing `[a,b,c,d,e,f]` directly (shapely mapping). `svgparse_viewbox(root) :61` = `width/height` length (`svgparselength` float, units string like `em/ex/px/in/cm/mm/pt/pc/%` or None) scaled by `viewBox` w (splits space/comma): returns `w / v_w` (or `1.0` if no viewBox). `camlib.Geometry.import_svg :1198` rejects SVG units in `{em,ex,pt,px}` → fail; if units `cm` multiplies `factor *= 10`; parses with `factor=svgparse_viewbox`, flips Y as `translate(scale(g,1,-1,origin (0,0)), yoff=h)` using root `height` length `h` (ignores its units beyond the rejection); also processes text via `getsvgtext` (fontTools/freetype, px→pt `*2.2`, Y flip around `h+h2`). `ParseGerber.import_svg :1990` uses `gerber_circle_steps`, `factor=svgparse_viewbox` (no `cm` adjust, no px-unit reject shown), flips with `h=svgparselength(height)[0]`, QR-code special case, builds `tools[0]` geometry.
- **DXF (verified 2026-05-08):** no `$INSUNITS` handling found in `ParseDXF.py`/`appHandlers/appIO.py` (unit scaling likely from app context/defaults; INSERT applies rotation/base_point and `xscale/yscale` row/col arrays as read — confirm ezdxf `$INSUNITS`/non-uniform scale behavior before parity-critical port).
- **HPGL2 (verified 2026-05-08):** only `CO,IN,SP,PU,PD,PA,PR,CI,AA,AT` regex-handled in `ParseHPGL2.parse :31-440` (stateful pens/point store, `SP`/`PU` flush `LineString`, `parse_number/40.0` mm, `grace` on abort, `unary_union` out). No LB/EA/ER/EW/SC/IP/OP/VS/LT/CT/AR handlers found in source.

### 12.2 Tool internals (verified entry points / semantics, 2026-05-08)

- **CutOut `ToolCutOut.py`**: `_classify_contours(geometries)` `:106` fills each shape via `Polygon(geo.exterior)` (or `convex_hull` fallback), picks max-area as exterior, marks others interior if `exterior_filled.contains(geo.representative_point())` — interior contours get inverted offset (cut inside slot). `on_freeform_cutout :716` resolves object/mode (gerber/geometry, single/multi, convex box), normalizes gaps to `gapsize+abs(cut_dia)` (`:770`), classifies interiors (`:855`), buffers margins per kind (gerber adds `cut_dia/2` on margin≥0 path), runs `any_cutout_handler`; gap types: thin-gaps (tool 99, alternate `cutz`/`thin_depth`, override color) and mouse-bites (Excellon drills interpolated along rest segments by `mb_dia+mb_spacing`; creates geometry + Excellon via `export_excellon(local_use)`). `any_cutout_handler :1051` centers by bounds, subtracts rectangular `box` gap cutters for `2LR/2TB/LR/TB/4/8` (gap extents include `gapsize+cut_diameter`), returns `(proc_geometry, rest_geometry)`. `rect_cutout_handler :1482` similar without rest collection. `on_drill_cut_click :1563+` offsets `LineString.parallel_offset(margin,'left')` or `Polygon.buffer(margin).exterior` and interpolates drills by `pitch`. *Read full gap placement math/mouse-bite spacing edge cases when implementing.*
- **Panelize `ToolPanelize.py :309` (verified):** resolves source+box (missing box or ref `bbox`→source), `outname=name+'_panelized'`, `lenghtx/y=(bounds+spacing)`. Constrain shrinks `cols/rows` to fit `x_width/y_height`. Deep-copies tools/apertures per kind. Grid `rows×cols` advances `currentx+=lenghtx`, `currenty+=lenghty` via `translate_recursion` (recursive over lists). `job_init_excellon`: resets `drills/slots` per tool, appends translated drills/slots with progress, `create_geometry()`, copies `zeros/units`, `source_file=export_excellon(local_use)`. `job_init_geometry`: initializes per kind (Geometry multigeo/tools or apertures), translates per-tool solid_geometry **and always** `panel_source_obj.solid_geometry`; Gerber translates `tools[apid]['geometry']` keeping `solid/clear/follow` (deepcopy) + `solid_geometry`. If `to_optimize` (Geometry multigeo): snap pairs `tol=1e-7`, `linemerge`+`unary_union` lines, recompose `fused+other`. *Parity caution:* `job_init_geometry` contains a gerber-conversion branch ending with `del new_obj.tools` and retooling from `solid_geometry` (source behavior as written; confirm the dispatched branch for `panel_type`). `job_init_gerber`: analogous grid/translation; geometry→gerber conversion builds `tools[0]` follow/solid then `del new_obj.tools`; exports `export_gerber(local_use)`. `app.abort_flag`→`grace` throughout.
- **Thieving `ToolCopperThieving.py :266/:361/:550+`**: robber bar = `envelope.buffer(rb_margin+rb_thickness/2).exterior` buffered to thickness, added as Gerber aperture/follow+solid; thieving worker buffers `solid_geometry` by clearance (`geo_steps_per_circle/4`), unions, builds bounding box (itself/area/box, convex/min), fills by pattern (dot/square/line params) respecting min area and `grace` — *fill-pattern geometry and PPM calc read-on-implement.*
- **SolderPaste `ToolSolderPaste.py`**: uses `AppPreProcTools` preprocessor contract (`appPreProcessor.py:85`, start/lift/down_z_start/lift_z_dispense/down_z_stop/toolchange/rapid/linear/end) distinct from `PreProc` (cnc/excellon) — port separate dispense preprocessor interface.
- **Film `ToolFilm.py :432`**: positive punched film = `MultiPolygon(film).difference(exc_solid)` (Excellon source) or difference against pad-center punch circles (`punch_size < aperture` checks; C/REG/other handling) → new gerber then normal positive film export.
- **PunchGerber `ToolPunchGerber.py :36`**: punch methods (exc/fixed/ring/prop per UI), circular ring option adds ring punches; creates gerber objects via init funcs — *read auto-detect/method geometry per method when implementing.*
- **Tcl `scale` quirk (verified):** `TclCommandScale` declares `required=['name']` but `arg_names` is empty (`:67`/`:41`); targets come from `unnamed_args` and `-factor|-x|-y|-origin` options — do not enforce `name` as a named arg in Rust; preserve `origin` forms (`origin|min_bounds|center|(x,y)` via eval in source → explicit parse).

### 12.1 Tcl command schemas (verified 2026-05-08, `flatcam_plus/tclCommands/`)

**Base (`TclCommand.py`):** `arg_names`/`option_types` are `OrderedDict`; `parse_arguments` accepts `-key value` (regex `^-([a-zA-Z].*)`), a dangling flag becomes `None` (uses tool default in command logic) and `check_args` casts via the declared type; unknown options (except `timeout`) error, missing `required` errors. `TclCommandSignaled` runs via `worker_task` + event loop, accepts `-timeout` ms (else `global_background_timeout`, default 300000 ms). `help` text per command is authoritative for semantics/units. Inventory = **74 command classes** (+ base; `__init__` imports 75). Schema table = `aliases | required args(arg:type) | options(opt:type) | required`:

| class / aliases | required args | options | required |
|---|---|---|---|
| `TclCommandAddAperture` `['add_aperture']` | `name:str` | `apid:int, type:str, size:float, width:float, height:float` | `name` |
| `TclCommandAddCircle` `['add_circle']` | `name, center_x, center_y, radius:float` | — | all args |
| `TclCommandAddDrill` `['add_drill']` | `name:str` | `dia,x,y:float` | `name` |
| `TclCommandAddPolygon` `['add_polygon','add_poly']` | `name:str` | `p_coords:str` | `name` |
| `TclCommandAddPolyline` `['add_polyline']` | `name:str` | — | `name` |
| `TclCommandAddRectangle` `['add_rect','add_rectangle']` | `name,x0,y0,x1,y1` | — | all |
| `TclCommandAddSlot` `['add_slot']` | `name:str` | `dia,startx,starty,stopx,stopy:float` | `name` |
| `TclCommandAlignDrill` `['aligndrill']` | `name:str` | `box,axis,holes:str, grid,minoffset,gridoffset,axisoffset,dia,dist:float, outname:str` | `name,axis` |
| `TclCommandAlignDrillGrid` `['aligndrillgrid']` | — | `dia,gridx,gridoffsetx,gridy,gridoffsety:float, columns,rows:int, outname:str` | `gridx,gridy,columns,rows` |
| `TclCommandBbox` `['bounding_box','bbox']` | `name:str` | `outname:str, margin:float, rounded:str` | `name` |
| `TclCommandBounds` `['get_bounds','bounds']` | `objects:str` | — | — |
| `TclCommandBuffer` `['buffer']` | `name:str` | `dist:float, join,factor:str` | `name,dist` |
| `TclCommandClearShell` `['clear']` | — | — | — |
| `TclCommandCncjob` `['cncjob']` *(Signaled)* | `name:str` | `dia,z_cut,z_move,feedrate,feedrate_z,feedrate_rapid,extracut_length,dpp,toolchangez,startz,endz,dwelltime,las_power,las_min_pwr:float, spindlespeed:int, toolchangexy,endxy,pp,muted,outname:str` | — |
| `TclCommandCopperClear` `['ncc_clear','ncc']` | `name:str` | `tooldia,order,method,connect,contour,rest,ref,box,outname:str, overlap,margin,offset:float, all:int` | `name` |
| `TclCommandCutout` `['cutout','geocutout']` | `name:str` | `type,gaps,outname:str, dia,margin,gapsize:float` | `name` |
| `TclCommandDelete` `['delete','del']` | `name:str` | `f:str` | — |
| `TclCommandDrillcncjob` `['drillcncjob']` *(Signaled)* | `name:str` | `drilled_dias,toolchangexy,endxy,pp,opt_type,muted,outname:str, drillz,dpp,travelz,feedrate_z,feedrate_rapid,toolchangez,startz,endz,dwelltime,las_power,las_min_pwr,diatol:float, spindlespeed:int` | `name` |
| `TclCommandExportDXF` `['export_dxf','edxf']` | `name,filename:str` | — | both |
| `TclCommandExportExcellon` `['export_exc','ee','export_excellon']` | `name,filename:str` | — | both |
| `TclCommandExportGcode` `['export_gcode']` | `name:str` | `preamble,postamble:str` (+positional filename in practice) | `name` |
| `TclCommandExportGerber` `['export_grb','egr','export_gerber']` | `name,filename:str` | — | both |
| `TclCommandExportSVG` `['export_svg']` | `name,filename:str` | `scale_stroke_factor:float` | both |
| `TclCommandExteriors` `['exteriors','ext']` | `name:str` | `outname:str` | `name` |
| `TclCommandFollow` `['follow']` | `name:str` | `outname:str` | `name` |
| `TclCommandGeoCutout` `['geoc']` | `name:str` | `dia,margin,gapsize:float, gaps,outname:str` | `name` |
| `TclCommandGeoUnion` `['geo_union']` | `name:str` | — | `name` |
| `TclCommandGetActive/GetNames/GetPath/ListPP/Quit/SaveSys` | — | — | — |
| `TclCommandGetSys` `['get_sys','getsys']` | `name:str` | — | `name` |
| `TclCommandHelp` `['help']` | `name:str` | — | — |
| `TclCommandImportSvg` `['import_svg']` | `filename:str` | `type,outname:str` | `filename` |
| `TclCommandInteriors` `['interiors']` | `name:str` | `outname:str` | `name` |
| `TclCommandIsolate` `['isolate']` *(Signaled)* | `name:str` | `dia,overlap:float, passes,iso_type:int, combine,outname:str` | `name` |
| `TclCommandJoinExcellon` `['join_excellon','join_excellons']` | — | `outname:str` | — |
| `TclCommandJoinGeometry` `['join_geometries','join_geometry']` | — | `outname:str` | — |
| `TclCommandListSys` `['list_sys','listsys']` | `selection:str` | — | — |
| `TclCommandMillDrills` `['milldrills','milld']` | `name:str` | `milled_dias,outname,use_thread:str, tooldia,diatol:float` | `name` |
| `TclCommandMillSlots` `['millslots','mills']` | `name:str` | `milled_dias,outname,use_thread:str, tooldia,diatol:float` | `name` |
| `TclCommandMirror` `['mirror']` | — | `axis,box,origin:str` | — |
| `TclCommandNew` `['new']` | — | `reset,keep_scripts:str` (bool via `eval`) | — |
| `TclCommandNewExcellon/NewGeometry/NewGerber` | `name:str` | — | — |
| `TclCommandNregions` `['non_copper_regions','ncr']` | `name:str` | `outname,rounded:str, margin:float` | `name` |
| `TclCommandOffset` `['offset']` | — | `x,y:float` | — |
| `TclCommandOpenDXF/OpenSVG` | `filename:str` | `type,outname:str` | `filename` |
| `TclCommandOpenExcellon/OpenGerber/OpenGCode` | `filename:str` | `outname:str` | `filename` |
| `TclCommandOpenFolder` `['open_folder']` | — | `dir:str` | — |
| `TclCommandOpenProject/SaveProject` | `filename:str` | — | `filename` |
| `TclCommandOptions` `['options']` | `name:str` | — | `name` |
| `TclCommandPaint` `['paint']` | `name:str` | `tooldia,order,method,connect,contour,all,single,ref,box,outname:str, overlap,offset:float` | `name` |
| `TclCommandPanelize` `['panelize','pan','panel']` | `name:str` | `rows,columns:int, spacing_columns,spacing_rows:float, box,outname,use_thread:str` | `name` |
| `TclCommandPlotAll` `['plot_all']` | — | `plot_status,use_thread:str` | — |
| `TclCommandPlotObjects` `['plot_objects']` | `names:str` | `plot_status:str` | `names` |
| `TclCommandScale` `['scale']` | — | `factor,x,y:float, origin:str` | `name` *(schema quirk: `name` required but absent from `arg_names`; verify `execute`)* |
| `TclCommandSetActive` `['set_active']` | `name:str` | — | `name` |
| `TclCommandSetOrigin` `['set_origin','origin']` | `loc:str` | `auto,obj_name:str` | — |
| `TclCommandSetPath` `['set_path']` | `path:str` | `create:str` | `path` |
| `TclCommandSetSys` `['set_sys','setsys']` | `name,value:str` | — | both |
| `TclCommandSkew` `['skew']` | — | `x,y,x_dist,y_dist:float, origin:str` | — |
| `TclCommandSplitGeometry` `['split_geometries','split_geometry']` | `source_name:str` | — | `source_name` |
| `TclCommandSubtractPoly/SubtractRectangle` | `name:str` | — | `name` |
| `TclCommandVersion` `['version']` | — | — | — |
| `TclCommandWriteGCode` `['write_gcode']` | `name,filename:str` | `preamble,postamble,muted:str` | both |

**Rust port notes (Tcl):** expose as CLI/JSON with the same `-option` names/aliases; preserve `eval`-of-bool/string and `x,y` tuple parsing hazards as explicit parsers (no `eval`); `-timeout` is runner-level. `scale` has an apparent schema quirk (`name` required but not in `arg_names`) — verify body before porting. Read each class `execute()`/`help` for object-kind constraints and defaults when implementing.

**Critical `execute()` semantics (verified, 2026-05-08) — port as explicit parsers, no `eval`:**

**Additional `execute()` semantics (verified 2026-05-08):**
- **`offset` (`TclCommandOffset.py`):** `arg_names` empty, `required=[]`; targets are `unnamed_args` (space-separated). `-x/-y` default `0.0`; `(0,0)` is a no-op warning. Applies `obj.offset((x,y))` to each object and refreshes `xmin/ymin/xmax/ymax` in `obj_options` (logs/returns fail if bounds fail). No new object.
- **`scale` (`TclCommandScale.py`, confirmed):** `required=['name']` but `arg_names` empty → targets in `unnamed_args`. Requires at least one of `-factor|-x|-y`. Computes combined bounds of all targets. Origin: default/`center` = bounds center, `origin`=`(0,0)`, `min_bounds`=(xmin,ymin), else `eval(str(origin))` expecting a tuple (**`eval` hazard**). `-factor` scales both axes and `continue`s; else `-x` only passes `(fx,0)`, `-y` only `(0,fy)`, both `(fx,fy)` to `obj_to_scale.scale(...)`. Refreshes bounds per object.
- **`exteriors`/`interiors` (Signaled):** Geometry-only (`isinstance Geometry` check, else Tcl error). `outname` defaults `name_exteriors`/`name_interiors`. Calls `obj.get_exteriors()`/`get_interiors()` and registers a new geometry with `solid_geometry` set (plot False). No multigeo/tool stamping.
- **`geocutout`/`geoc` (Signaled, `TclCommandGeoCutout.py`):** different from `cutout`. Builds cutout perimeter from source: if Geometry uses `deepcopy(solid_geometry)`; if Gerber uses `obj.isolation_geometry(dia/2, iso_type=0, corner=2)` (error→fail). Gaps in `none/lr/tb/2lr/2tb/4/8`; `gapsize = gapsize/2 + dia/2`; center/length from bounds+margin. Subtracts axis-aligned gap rectangles by `flatten(pathonly=True)` + `LineString/LinearRing.difference(unary_union(Polygon(pts)))` (other types warn “Not implemented”). Registers geometry (disables source plots), stamps `tools[1]` with `cutout` data (`tool dia/margin/gaps/gapsize`) and deepcopied solid. *Caution:* `cutout_obj` bounds written back to source `obj_options`.
- **`cutout` (Signaled, detail addendum):** `gapsize_par += dia_par` (full dia). `rect` cuts `box(bounds)` flattened; `any` uses flattened `solid_geometry`. Gerber margin: positive → `buffer(margin + |dia/2|).exterior`; negative/zero → `buffer(-margin + |dia/2|).interiors` (**sign/shape nuance; preserve**). Geometry passes struct through. Dispatch `rect_cutout_handler` / `any_cutout_handler` returns `(solid, r_geo)` (rect returns solid); `linemerge` attempted (falls back for polygons). Creates geometry `outname`, `multigeo=True`, `tools[1]` with cutout tool data.
- **`nccoffset` alias:** not found as a Tcl command class/alias in `tclCommands/` (verified by search). NCC offset is handled via `ncc`/`copper_clear` `-offset` + `has_offset` and ToolNCC UI/options `tools_ncc_offset_choice/value`.

- **`isolate` (Signaled, `TclCommandIsolate.py:100+`):** defaults `outname=name+'_iso'`; `combine` parsed via `str(...).capitalize()` + `eval` fallback to `tools_iso_combine_passes` — **bool hazard**. Validates gerber kind, pops `name`, calls `obj.isolate(plot=False, **args)`.
- **`cncjob` (Signaled, `:170+`):** `muted` bool via `capitalize/eval`. Requires `name` in body (schema `required=[]`); defaults `outname=name+'_cnc'`, `dia/z_cut/z_move/feedrate/feedrate_z/rapid/pp/startz/endz` from `tools_mill_*` (laser forces `feedrate_z=feedrate` if absent). `dpp` sets `multidepth` else uses option; `endxy/toolchangexy` strings default to option/`'0,0'`, validated `len(eval(...))==2` (**`eval` hazard**). `spindlespeed`=None or `las_power` for laser; `dwell/dwelltime`, `toolchange/toolchangez` toggled by presence. Refuses any arg `None` (except toolchange_xy/spindlespeed/startz). Single-geo → `obj.generatecncjob(use_thread=False, plot=False, **args)`; multigeo → clones `obj.tools`, stamps `tools_mill_*`/seg/laser fields from args into each `data`, calls `milling_tool.generate_cnc_job_handler(..., tools_in_use=[], use_thread=False, from_tcl=True)`. *Note source quirk:* sets `data['seg_y']=options['geometry_seg_x']` (uses seg_x).
- **`drillcncjob` (Signaled, `:150-500`):** Excellon-only. `drilled_dias` CSV or `'all'` (+`diatol` % closeness) → tool set; `outname=name+'_cnc'`. `dpp`→`multidepth/z_depthpercut`; `toolchangez/toolchangexy` (validated via `eval` len 2) enable toolchange; `endxy` validated via `eval`; `opt_type` default `R`. Populates `cnc_job_obj` (preproc `ppname_e`, depths, feeds, spindle/laser, dwell, coords decimals, bounds, `xy_end=eval(str(xy_end))`, `excellon_optimization_type`). Runs `tcl_gcode_from_excellon_by_tool(obj,tools,is_first=True)` → builds `gcode_parsed` per tool (toolchange vs single/first) via `excellon_tool_gcode_parse`, `create_geometry()`, registers cncjob (no plot).
- **`paint` (`:160+`):** `tooldia` CSV parsed with `eval` (**hazard**); selection exclusive by arg presence: `-single` (x,y via `eval`) / `-box` ref / else `-all`. Builds per-tool `tools_storage` with paint/mill defaults, calls `paint_tool.paint_poly_all|paint_poly|paint_poly_ref(..., run_threaded=False)`. `method/connect/contour/order/offset/overlap/outname` default from options.
- **`ncc`/`copper_clear` (`:80-300`):** `overlap` arg is **percent, divided by 100** here (tool internals expect fraction). `method` maps `standard/seed/lines`→0/1/2 (string else passes option). `connect/contour/rest` bool via `capitalize/eval`. `select` = 0 if `-all` else 2 (box/ref) — needs `-all` or `-box`, else error. Builds `ncc_tools` store, calls `ncclear_tool.clear_copper_tcl(..., run_threaded=False)` with `has_offset`, fraction overlap, method code.
- **`cutout` (`:63+`):** gerber/geometry; `gaps` in `none/tb/lr/2tb/2lr/4/8` (case-insensitive), `type` `rect|any` (default any). Computes `gapsize += dia_par`. `rect` uses `box(bounds)`; gerber margins buffer by `margin ± dia/2` (exterior/interiors), geometry passes struct; dispatches to `cutout_tool.rect_cutout_handler` or `any_cutout_handler(..., gapsize, margin)`, `linemerge`s, builds geometry tool 1 with cutout tool data, registers object.


## 13. Evidence index

Every path is relative to `/home/th3g3ntl3man/Code/analisis`.

```
flatcam_plus/__main__.py
flatcam_plus/launcher.py:448,456,458-478,570,576-584,596,599-601
flatcam_plus/appMain.py:130,138,150-167,297,529,575,584,589,592-594,599,629,745,748-760,766-769,1638,1792,~1965
flatcam_plus/appPreProcessor.py:20,25,28-36,39-81,85,155,158,163
flatcam_plus/preprocessors/default.py:14,16,17,18,20
flatcam_plus/defaults.py:10,43 + AppDefaults.factory_defaults (613 keys)
flatcam_plus/camlib.py:61,65-66,84,91,95,248,263,287,310,333,378,409,456,482,539,606,649,690,742,771,783,898,928,944,968,1009,1112,2655,2671,2722,2728,3040,3104,3324,3486,4017,4482,4723,5301,5358,5623,6396,6844-6985,6987-7233,7400-7412,7917,7942,7988,8014,8054,8061,8099,8124,8452,8462,8496,8504,8579
flatcam_plus/appCommon/Common.py:1 (LoudDict), 707 (travel_coordinates)
flatcam_plus/appObjects/AppObjectTemplate.py:55,67,70,91,92,94
flatcam_plus/appObjects/GerberObject.py:37,57
flatcam_plus/appObjects/ExcellonObject.py:36,54,959,1064,1183,1188
flatcam_plus/appObjects/GeometryObject.py:44,64
flatcam_plus/appObjects/CNCJobObject.py:40,64,744,926,929,947,948,955,962,1094,1133-1170
flatcam_plus/appObjects/ObjectCollection.py:731
flatcam_plus/appObjects/ScriptObject.py:28,42
flatcam_plus/appObjects/DocumentObject.py:28,42
flatcam_plus/appParsers/ParseGerber.py:32,1535-1624,1616,1621-1623,1628
flatcam_plus/appParsers/ParseExcellon.py:31
flatcam_plus/appPlugins/ToolIsolation.py:42,287,1978,1991,2018,2030,2044,2049,2068,2200,2276,2332,2355,2357,2359,2361,2792,3693,3710,3927
flatcam_plus/appPlugins/ToolCutOut.py:42,2366
flatcam_plus/appPlugins/ToolDrilling.py:39,1906,1968-2280,2397
flatcam_plus/appPlugins/ToolMilling.py:84,3308,3530,4283
flatcam_plus/appPlugins/ToolPaint.py:46
flatcam_plus/appPlugins/ToolNCC.py:40,2836,3842,3849
flatcam_plus/appPlugins/ToolPanelize.py:34,1134
flatcam_plus/appPlugins/ToolCopperThieving.py:39,1233
flatcam_plus/appPlugins/ToolFilm.py:47,1271
flatcam_plus/appPlugins/ToolPunchGerber.py:36,1998
flatcam_plus/appPlugins/ToolSolderPaste.py:39,1248
flatcam_plus/appPlugins/ToolRulesCheck.py:34,1191
flatcam_plus/appPlugins/ToolCNCControl.py:43
flatcam_plus/appPlugins/ToolPlotter.py:33
```



## 12.3 Object-level semantics consumed by Tcl (verified entry points)

- **Excellon slot milling command/object path (verified):** `TclCommandMillSlots.py` (`aliases mills/millslots`, `required name`) maps `milled_dias` (CSV or `all`, optional `diatol` % closeness against `obj.tools[tool]['C']`/diameter with decimals formatting) to selected tool keys, removes `milled_dias/diatol/name`, and calls `obj.generate_milling_slots(plot=False, **args)` (threading via `use_thread` bool parsed by `capitalize/eval`). `ExcellonObject.generate_milling_slots` (`:1064+`) selects tools with `slots`, rejects if tool dia `> file tool dia` (decimal-formatted + `0.0001` rounding tolerance), and buffers each slot `LineString(start,stop)` by `file_tool/2 - tool_dia/2 + 0.0001`; if buffer becomes `0` it emits `buffer(0.0000001, geo_steps_per_circle).exterior` (near-zero trace) — preserves source rounding/edge behavior. Creates Geometry `_mill_slot` with `solid_geometry` from slot buffers. **Rust:** separate from `drillcncjob` point emission and from `drill_slots` conversion-to-drills.
- **Thieving fill/PPM (verified, `ToolCopperThieving.py:330+,550+,954+`):** Robber bar: `outline_pol = solid_geometry.envelope` (MultiPolygon fallback), `robber_line = envelope.buffer(rb_margin+rb_thickness/2).exterior`, `robber_geo = robber_line.buffer(rb_thickness/2)`; reuses circular aperture with same `size==rb_thickness` else creates new id (`max+1` or 10), appends `{'solid':robber_geo,'follow':robber_line}`; unions into new Gerber solid and exports `_robber`. Thieving: clearance buffer `resolution=geo_steps_per_circle/4`; fill = `bbox.difference(clearance)` (`itself` box type min/convex/rect+mitre margin; `area` buffers selected polys; `box` ref uses geo bounds/hull/intersection path by ref kind), filters `area >= min_area`. `dot/square`: grid over bbox, translate by `bbox.centroid-grid.centroid`, keep only `element.within(fill_geo)`. `line`: outline around features `buffer(c_val+half_thick, res/4)` + inner box outline + H/V strips stepped `line_size+line_spacing`; each strip `difference(clearance_geometry)` (appended as rest), result is `flatten([outline_geometry, box_outline_geometry, diff_lines])`. Adds thieving into Gerber copy as aperture `0` REG elements `{'solid':poly,'follow':poly.exterior}`, `solid_geometry = MultiPolygon(geo_list).buffer(0.0000001).buffer(-0.0000001)` flattened, exports `_thief`. **PPM:** `geo_choice` 0 both,1 thieving,2 robber; `only_pads` selects SM aperture geometries whose follow is `Point` and has `solid`. `ppm_clearance!=0` buffers `geo_list` and rewrites SM aperture `solid/follow/clear` (clears become `[]` or copied; solids buffered) — *destructive rebuild of aperture geometry when clearance applied*. Adds thieving for choices `b/t` (buffered by `ppm_clearance`, into aperture 0), robber for `b/r` (finds/creates circular aperture sized `rb_thickness+ppm_clearance`, adds buffered robber solid/follow). Plated area accumulates selected parts (mm²→cm² when mm). Output name `base_rpartition('.')` → `_plating_mask.ext`; `export_gerber` source.
- **PunchGerber methods/manual symmetry (verified, `ToolPunchGerber.py:538-1760+`):** auto/manual dispatch same method names (`on_excellon/_manual_method` etc.). Auto ops over selected aperture ids (`ui.apertures_table.selectedItems`); manual over `manual_pads=[{'apid','idx'}]` from `find_pad` (only selected apids, geometry with follow `Point` and pt `within` solid) toggled on left-click/release. `exc`: differences target by exc solid; auto adds clears for drills `within` pad solid per aperture/selected ids; manual only uses exc geos `within` selected pad solids and adds clears similarly. `fixed`: checks per-shape constraints and returns fail with message when punch too big; creates punch `buffer(dia/2)` and warns if result unchanged. `ring`/`prop`: compute dia by type (`C/O/R` rect vs square by rounded width==height, `AM` bounds min axis adjusted), create punching circles, `difference`, add clears with `geo.centroid` grouped by computed `dia` (new aperture ids appended). Manual variants use pad follow points for selected pads (note: circular branch in manual ring/prop checks shape checkbox in some branches while C/fixed manual path still validates by aperture; preserve source conditions). All create `_punched`, copy options/fill/outline/tools, `source_file=export_gerber`. ESC/right-click flow restores grid/plot handlers, runs `on_manual_punch`, clears marking/UI state.
- **DXF units (verified gap):** no `$INSUNITS` handling found in `ParseDXF.py`/`appHandlers/appIO.py`. Unit scaling may be context/app-dependent; INSERT non-uniform scale and header units can break parity. Treat as explicit parity risk; add fixtures before strict port.


- **`GerberObject.isolate` (`GerberObject.py:533+`):** defaults `dia/passes/overlap/combine/milling_type` from `tools_iso_*`; `overlap` arg divided by 100. Names `_ext_iso/_int_iso/_iso` (+ pass index when not combined). `combine=True`: one geometry, `multigeo`, `tools[1]` seeded from `geometry`+`tools_mill` defaults (`offset_type=0, offset_value=0`), accumulates `generate_envelope` per pass (offset `dia*((2*n+1)/2)-(n*overlap*dia)`, direction from `milling_type` `cl`→1) and appends flattened geom; empty → `ValidationError`/fail. `combine=False`: one object per pass with its own `tools[1]`. `generate_envelope` calls `isolation_geometry(offset, iso_type, passes)`; if invert reverses Polygon exteriors/LinearRings (MultiPolygon/Polygon cases) — failure returns `'fail'`.
- **`GeometryObject.generatecncjob` (`GeometryObject.py:1056+`, Tcl single-geo path):** resolves defaults from `tools_mill_*`/`geometry_seg_*`; `endxy/toolchangexy` accept tuple or string cleaned of `()[]` and parsed via `eval`-split (**hazard**). Builds cncjob via `generate_from_geometry_2(...)` (tolerance `global_tolerance/20`, `is_first=True`), assigns `gc_start/source_file`, runs `gcode_parse()`. Threaded path wraps in worker + promise; Tcl calls with `use_thread=False`.
- **`excellon_tool_gcode_gen` body (`camlib.py:3324-3815+`, verified):** no OR-Tools → `M/B` coerced to `R`. Empty points/failed optimization → `'fail'`; `abort_flag` → `GracefulException`. Sets tool/preproc/feeds/spindle/dwell/laser params from tool data; sanitizes `xy_toolchange/xy_end` by stripping `()[]`+`eval` split (**hazard**), validates length 2 else emits error/defaults. Optimization: `M` meta (`excellon_search_time`), `B` basic, `T` salesman, `R` `exc_optimized_rtree(points)`, else raw drill coords. `is_first` emits `start_code`. Toolchange emits `z_feedrate_code` + `toolchange_code`(+`z_feedrate`); else lift/startz (laser skips lift). Emits spindle/dwell. Applies `z_offset = -tools_drill_offset` to `z_cut`; computes `calculate_depths`. **G90 only** (G91 emits error/fail). For each optimized point (T/R/array forms differ) resolves `(locx,locy)`, moves via `exc_areas.travel_coordinates` (may insert intermediate lift/restore `z_move`), then per depth: `down_code`, bookkeeping, retract `up_to_zero` when `f_retract` false and `z_cut<0` else `lift`; restores original `z_cut`. `is_last` emits spindle stop + `end_code`. Returns `(t_gcode, (locx,locy), start_gcode)`.
- **`excellon_tool_gcode_parse` body (`camlib.py:6396-6596+`, verified):** rejects lines containing `%MO/%/MOIN/MOMM` unless `force_parsing` true → `"fail"`. Tracks units `G20/21`. On `Z` change: warns on non-orthogonal motion in most cases (except Roland/hpgl/laser/Line_xyz); flushes path (`LineString`) when `len(path)>1`; if `Z<0` appends a hole as `Point(current X,Y).buffer(dia/2).exterior` kind `['C','F']`. `G0/1` appends line; `G2/3` builds arc from `I,J` center, `arctan2` start/stop, direction cw/ccw, `steps_per_circle`. Kind is travel when `Z>0`, slow when `G>0`. Parses via `codes_split`; returns `list[{geom,kind}]|"fail"`.

- **Excellon slots (verified, split semantics):** Excellon stores `tools[*]['slots']` as `(start,end)` pairs. **Drilling/peck-as-drills path:** `ToolDrilling.process_slot_as_drills` interpolates a `LineString` slot by `target += overlap*tooldia` (`overlap` here = `1 - ui_overlap%/100`, i.e. step fraction) and collects points while `new_pt.within(slot_line)` is true; may append final point if `last_drill_cb`. `create_drill_points` converts slots to drills **only when** per-tool `data['tools_drill_drill_slots']` is true (using UI `drill_overlap_entry`/last-drill setting in that UI path), appending to `points[tool]`. `ToolDrilling.on_generate_cnc_job` then builds cncjob solid geometry: when `drill_slots` false it buffers original slot LineStrings (`nr_slots` counted); when true slots become drills (no slot buffers added for that converted case). **Tcl/camlib drill G-code:** `excellon_tool_gcode_gen` emits rapid+multidepth cycles per optimized **point** location (T `[x,y]`, R `(pt,geo)` coords, others index/coords) — no slot-segment iteration. `tcl_gcode_from_excellon_by_tool` buffers drills+slots into `solid_geometry`/counts but builds optimization `points` from `drills` only; so Tcl drillcncjob emits point-drill G-code from drills, not routed slot cuts. **Milling slots:** `ExcellonObject.generate_milling_slots` (UI `generate_milling_slots_button`) creates a separate Geometry object milling slots as paths (`_mill_slot`) — distinct from drill/peck G-code. *Port rule:* preserve per-mode behavior (slot→drills conversion controlled by tool/UI flag, slot geometry buffering when not converted, drill-point emission in drillcncjob; separate slot-milling geometry route). Read `ExcellonObject.generate_milling_slots` body and ToolDrilling UI-vs-Tcl flag sourcing before slot parity implementation.
- **Excellon parse addendum:** flush on each `Z` change preserves previous path; drill-hole circle is emitted only when `current Z < 0` (uses `Point(X,Y).buffer(dia/2).exterior`). Arcs require `I,J` present (`gobj['I'/'J']`); parse is stateful over `current G/X/Y/Z`.

- **Panelize dispatch/reachability (verified, `ToolPanelize.py:361,510,824,1099+`):** `panel_type` radio/default `gerber`. **Dispatch:** if `panel_source_obj.kind=='excellon'` → `job_init_excellon` (Excellon object). Else if `panel_type=='geometry'` → `job_init_geometry` (Geometry). Else if `panel_type=='gerber'` → `job_init_gerber` (Gerber). `job_init_excellon` copies/translates drills+slots per tool, `create_geometry()`, sets `zeros/units/source_file=export_excellon(local_use)`. `job_init_geometry`: initializes by source kind (Geometry multigeo/tools or Gerber apertures), translates per cell (multigeo tool solids, always `panel_source_obj.solid_geometry`, and Gerber aperture `solid/clear/follow`), optional multigeo snap `tol=1e-7`+`linemerge`+`unary_union`; when **source is Gerber** it enters a conversion block that rebuilds mill-style `new_obj.tools` from aperture `solid` unions + whole `solid_geometry` and **`del new_obj.tools`** (`:824`), then exports DXF. `job_init_gerber`: translates by source kind (Geometry multigeo/tools+solid or Gerber apertures) and always solid; when **source is Geometry** and was multigeo it rebuilds `tools[0]` from follow/solid lists and **`del new_obj.tools`** (`:~1076/1086` region), else builds a single REG tool and **`del new_obj.tools`**; exports Gerber. **Port rule:** preserve exact `panel_type × panel_source_obj.kind` behavior including `del` side effects and export format (DXF from geometry worker, Gerber export from gerber worker); do not normalize until fixtures prove equivalence.


### 12.4 DXF `$INSUNITS` / INSERT fixture spec (3a/3b, documented; not implemented)

**3a — Planned fixtures/tests (when authorized)**
- **Location:** `tests/test_dxf_parse.py`; generate DXF deterministically with `ezdxf>=1.4.5` in `tmp_path`/`tmp_path_factory` (repo pattern: `tests/data/*` samples, `conftest.py` `fc_app`/`DATA_DIR`; HPGL2 writes temp files). Commit `tests/data/*.dxf` only if a stable human-readable sample is needed.
- **API focus:** `ParseDXF.getdxfgeo(ezdxf.readfile(path))` is pure shapely/ezdxf (no `fc_app` needed). Object/import-path tests only if `fc_app` wiring is required.
- **Run:** `uv run --with pytest pytest tests/test_dxf_parse.py -v` then `uv run --with pytest pytest tests -q`.
- **Cases:** DXF-01 `$INSUNITS` mm (`INSUNITS=4`), DXF-02 inches (`1`), DXF-03 none/unset (`0`/absent) — **characterize current behavior first** (FlatCAM does not read `$INSUNITS`); DXF-04 INSERT translate+`base_point`; DXF-05 non-uniform `xscale/yscale`; DXF-06 rotation (`origin=tr` behavior); DXF-07 row/column arrays (`row_count/spacing`, `column_count/spacing`); DXF-08 combined scale+rotate+translate; DXF-09 nested blocks/INSERT; DXF-10 `base_point!=(0,0)`; DXF-11 supported entities (LINE/CIRCLE/ARC/ELLIPSE/LWPOLYLINE/POLYLINE/SOLID/TRACE/SPLINE/POINT/INSERT); DXF-12 unsupported entity (ignore, no crash).
- **Assertion rule:** first produce a golden of observed FlatCAM behavior for units/INSERT quirks; parity Rust can target “compat FlatCAM” (current behavior) vs “DXF-correct”, but the choice must be explicit.

**3b — Verified `ParseDXF.py` semantics (spec for parity)**
- `getdxfgeo(doc)` → `get_geo(doc, doc.modelspace())`. Unsupported entity types are logged/debug-ignored.
- Entity mapping: `POINT` → `Point(...).buffer(0.01)`; `LINE` → 2D `LineString`; `CIRCLE`/`ARC` handle `extrusion!=(0,0,1)` via OCS→WCS (`ARC` adjusts angles/direction per branch); `ELLIPSE/LWPOLYLINE/POLYLINE/SOLID/TRACE/SPLINE` via helpers (SPLINE `spline2Polyline(..., segments=20)`); `INSERT` recurses.
- `get_geo_from_insert` (verified order/quirks): reads `rotation/insert/xscale/yscale/row_count,row_spacing,column_count,column_spacing`; resolves `block.base_point`; for each block geo: translate by `(tr-block_coords)` when `tr!=0`; if `row_count>1` append per-row translated copies using X offset with `yoff=0`, if `column_count>1` append per-column copies using Y offset with `xoff=0` (no row×col grid combination; copies appended without mutating `geo`; the transformed `geo` is also appended at end — **potential duplication/overrepresentation, fixture DXF-07 must characterize**); then `scale(geo,sx,sy)` if non-1; then `rotate(geo,phi,origin=tr_adjusted)` if non-0 (`'c'`→`center`, `Vec3`→list). Arrays are not a pure 2D matrix in source form.
- **Parity risks:** no `$INSUNITS` handling; transform/array order and origin are source-of-truth for strict compatibility even if geometrically odd; POINT is area buffer, not pure point.

