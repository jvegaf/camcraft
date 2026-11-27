#!/usr/bin/env python3
"""Measure one corpus artefact — and optionally one PCB operation over it — with FlatCAM-Plus (E0.6).

This script is run by FlatCAM's *own* interpreter, never by the workspace's, because FlatCAM is a
separate Python environment (`../README.md`). It prints exactly one JSON object on stdout and leaves
everything FlatCAM logs on stderr, so `collect.py` can read it as a measurement instead of as a
transcript.

    flatcam_measure.py <artefact>                        # import: what FlatCAM makes of the file
    flatcam_measure.py <artefact> --operation isolation  # + isolate and post it, count the moves
    flatcam_measure.py <artefact> --operation drill      # + drill it, count the moves

Three things here are not obvious and are the reason this file exists rather than a shell one-liner:

* **The app is not optional.** FlatCAM's geometry classes take an `app`, so a measurement means booting
  the real application; `QT_QPA_PLATFORM=offscreen` gets that without a display. The constructor builds
  a `multiprocessing.Pool`, so this file must be imported under a `__main__` guard — without it the
  forkserver re-imports this module and runs the measurement again inside every worker.
* **Moves are counted on the object, not in the text.** `CNCjob.gcode_parsed` is the list of tool paths
  with a `kind` of `T`/`C` (travel/cut) × `F`/`S` (fast/slow), so the count does not depend on which
  post-processor dialect emitted the G-code, nor on how it formats numbers.
* **A measurement is a property of a version.** `environment()` records the FlatCAM, Python and Shapely
  versions with every result, because a change in any of them changes what the corpus measures.
"""

import argparse
import json
import os
import re
import resource
import sys
import time

# FlatCAM flattens every arc to lines through Shapely, so what it reports as a polygon is already a
# polygon in the linear sense. Nothing in this file re-flattens: the numbers are FlatCAM's own.
MAX_DEPTH = 8


def environment():
    """The versions a golden depends on, read from the environment that is actually running."""
    import shapely

    return {
        "flatcam": _flatcam_version(),
        "python": "%d.%d.%d" % sys.version_info[:3],
        "shapely": shapely.__version__,
        "platform": os.environ.get("QT_QPA_PLATFORM", ""),
    }


def _flatcam_version():
    """The version the running tree reports, without importing the whole application for it."""
    for module in ("flatcam_plus.__version__", "flatcam_plus.version"):
        try:
            value = __import__(module, fromlist=["__version__"]).__version__
        except Exception:
            continue
        if isinstance(value, str) and value:
            return value
    return "unknown"


def boot():
    """Start FlatCAM offscreen, with an empty project, and with networking switched off."""
    from PyQt6 import QtWidgets

    from flatcam_plus import appMain

    # The application schedules an update check 1.8 s after boot; a measuring instrument does not
    # reach the network, and a check that fires today would fire on a machine without a route.
    appMain.App.schedule_startup_update_check = lambda self: None

    app = QtWidgets.QApplication(sys.argv[:1])
    fc = appMain.App(qapp=app, startup_project_request=None)
    # FlatCAM refuses to open anything until a project workspace is active, which the launcher
    # normally does from the GUI. Nothing else about "new project" is needed: the collection starts
    # empty either way, which is what makes one measurement per process reproducible.
    fc.ui.activate_project_workspace()
    return fc


def run_tcl(fc, command):
    """Run one line in FlatCAM's shell, which is the surface FlatCAM documents for scripting."""
    return fc.shell.exec_command(command, no_echo=True)


def tcl_word(value):
    """One argument for the Tcl shell, quoted so it can neither split nor substitute.

    Paths and object names reach the command line, and the shell splits on whitespace and substitutes
    `$`, `[` and `\\` unless they are escaped. An unquoted corpus path with a space in it would open
    nothing and be recorded as a refusal — a measurement that never happened.
    """
    text = str(value).replace("\\", "\\\\").replace('"', '\\"').replace("$", "\\$").replace("[", "\\[")
    return '"' + text + '"'


def safe_name(stem):
    """The object name this measurement uses, whatever the file was called.

    FlatCAM's collection is keyed by name and the name is also reused in later commands, so the stem is
    reduced to characters that survive the round trip instead of being trusted verbatim.
    """
    return re.sub(r"[^A-Za-z0-9_-]", "_", stem)


def object_named(fc, name):
    return fc.collection.get_by_name(name)


def rings_of(geometry, depth=0):
    """Every linear ring of a Shapely geometry tree, as coordinate lists.

    FlatCAM keeps `solid_geometry` as a list of polygons rather than one union, so the walk has to
    follow lists as well as the geometry collection types.
    """
    if geometry is None or depth > MAX_DEPTH:
        return []
    if isinstance(geometry, (list, tuple, set)):
        rings = []
        for item in geometry:
            rings.extend(rings_of(item, depth + 1))
        return rings
    kind = getattr(geometry, "geom_type", None)
    if kind == "Polygon":
        rings = [list(geometry.exterior.coords)]
        rings.extend(list(hole.coords) for hole in geometry.interiors)
        return rings
    if kind in ("MultiPolygon", "GeometryCollection"):
        rings = []
        for item in geometry.geoms:
            rings.extend(rings_of(item, depth + 1))
        return rings
    return []


def signed_area(coords):
    """The shoelace area of one ring, signed: positive for the winding this order describes."""
    total = 0.0
    for index in range(len(coords) - 1):
        x1, y1 = coords[index]
        x2, y2 = coords[index + 1]
        total += x1 * y2 - x2 * y1
    return total / 2.0


def geometry_metrics(geometry):
    """The three geometric counts, defined exactly as the parity harness defines them.

    `polygons` is the number of rings (a hole is its own ring, so an outline reads as two boundaries);
    `contour_area_mm2` is the **sum of the absolute ring areas**, which is that same harness' rule and
    not the material area; `vertices` is how many vertices FlatCAM's own flattening produced. That last
    one is recorded and deliberately never compared with our `points` — FlatCAM flattens arcs in
    Shapely and the workspace preserves them, so the two count different things (see
    `docs/ROADMAP_CAMCRAFT.md`, "Nota de tolerancia de arcos").
    """
    rings = rings_of(geometry)
    return {
        "polygons": len(rings),
        "contour_area_mm2": sum(abs(signed_area(ring)) for ring in rings),
        "vertices": sum(len(ring) for ring in rings),
    }


def drill_metrics(obj):
    """How many tools and how many features FlatCAM found, counted the way our reader counts them."""
    tools = getattr(obj, "tools", None) or {}
    features = 0
    for tool in tools.values():
        features += len(tool.get("drills") or [])
        features += len(tool.get("slots") or [])
    return {"tools": len(tools), "features": features}


def peak_rss_kb():
    return resource.getrusage(resource.RUSAGE_SELF).ru_maxrss


def moves_of(cnc):
    """The move count of a CNC job: every parsed tool path, split by what it is."""
    parsed = getattr(cnc, "gcode_parsed", None)
    if not parsed:
        parsed = []
        for tool in (getattr(cnc, "tools", None) or {}).values():
            parsed = parsed + list(tool.get("gcode_parsed") or [])
    cut = travel = 0
    path_mm = 0.0
    for move in parsed:
        kind = "".join(move.get("kind") or [])
        if "C" in kind:
            cut += 1
        elif "T" in kind:
            travel += 1
        geometry = move.get("geom")
        if geometry is not None:
            path_mm += geometry.length
    return {
        "moves": len(parsed),
        "cut_moves": cut,
        "travel_moves": travel,
        "path_mm": path_mm,
    }


def zero_geometry(status):
    return {
        "status": status,
        "polygons": 0,
        "contour_area_mm2": 0.0,
        "vertices": 0,
    }


def zero_drill(status):
    return {"status": status, "tools": 0, "features": 0}


def zero_operation(status):
    return {
        "status": status,
        "moves": 0,
        "cut_moves": 0,
        "travel_moves": 0,
        "path_mm": 0.0,
    }


def open_artefact(fc, path, stem):
    """Open one artefact and time it. Returns the object, or None when FlatCAM refused the file."""
    command = "open_excellon %s -outname %s" if path.endswith(".drl") else "open_gerber %s -outname %s"
    started = time.perf_counter()
    run_tcl(fc, command % (tcl_word(path), tcl_word(stem)))
    seconds = time.perf_counter() - started
    return object_named(fc, stem), seconds


def measure_import(fc, path, stem):
    obj, seconds = open_artefact(fc, path, stem)
    if obj is None:
        # FlatCAM reports a file it cannot read as a message and no object at all; there is no
        # partial object to measure, and inventing one would hide the refusal.
        refused = zero_drill("refused") if path.endswith(".drl") else zero_geometry("refused")
        return {**refused, "seconds": seconds, "peak_rss_kb": peak_rss_kb()}
    metrics = drill_metrics(obj) if path.endswith(".drl") else geometry_metrics(obj.solid_geometry)
    return {
        "status": "parsed",
        **metrics,
        "seconds": seconds,
        "peak_rss_kb": peak_rss_kb(),
    }


def measure_isolation(fc, path, stem, options):
    obj, _ = open_artefact(fc, path, stem)
    if obj is None:
        return zero_operation("refused")
    before = set(fc.collection.get_names())
    started = time.perf_counter()
    # `-overlap` is a percentage of the tool diameter in this command (`25` means 25 %), and `-combine`
    # folds the passes into one geometry, which is the isolation job a person actually sends.
    run_tcl(
        fc,
        "isolate %s -dia %s -passes %d -overlap %s -iso_type 2 -combine True -outname %s"
        % (
            tcl_word(stem),
            options["dia"],
            options["passes"],
            options["overlap"],
            tcl_word(safe_name(stem) + "_iso"),
        ),
    )
    # FlatCAM names the isolation object itself (it appends a suffix when the name is taken), so the
    # result is read from the collection's own diff instead of from the name that was asked for.
    created = [
        (name, object_named(fc, name))
        for name in fc.collection.get_names()
        if name not in before
    ]
    geometries = [(name, obj) for name, obj in created if obj is not None and getattr(obj, "kind", "") == "geometry"]
    if not geometries:
        return {**zero_operation("refused"), "seconds": time.perf_counter() - started}
    totals = {"moves": 0, "cut_moves": 0, "travel_moves": 0, "path_mm": 0.0}
    for index, (origin, _geometry) in enumerate(geometries):
        job = safe_name("%s_iso%d_cnc" % (stem, index))
        run_tcl(
            fc,
            "cncjob %s -dia %s -z_cut %s -z_move %s -feedrate %s -outname %s"
            % (
                tcl_word(origin),
                options["dia"],
                options["z_cut"],
                options["z_move"],
                options["feedrate"],
                tcl_word(job),
            ),
        )
        cnc = object_named(fc, job)
        if cnc is None:
            # A CNC job that was asked for and did not appear is a failed measurement, not a zero one:
            # skipping it would put a total nobody produced into the golden and call it `measured`.
            return {**zero_operation("error"), "seconds": time.perf_counter() - started}
        counted = moves_of(cnc)
        totals["moves"] += counted["moves"]
        totals["cut_moves"] += counted["cut_moves"]
        totals["travel_moves"] += counted["travel_moves"]
        totals["path_mm"] += counted["path_mm"]
    return {"status": "measured", **totals, "seconds": time.perf_counter() - started}


def measure_drill(fc, path, stem, options):
    obj, _ = open_artefact(fc, path, stem)
    if obj is None:
        return zero_operation("refused")
    started = time.perf_counter()
    # The toolchange is what makes this a real drill program: without it FlatCAM drills every hole with
    # the first tool in the file ("use the first tool in the selection as the tool that we are going to
    # use"), which would record a move count no machine would ever run for a multi-tool board.
    run_tcl(
        fc,
        "drillcncjob %s -drillz %s -travelz %s -feedrate_z %s -toolchangez %s -toolchangexy 0,0 -outname %s"
        % (
            tcl_word(stem),
            options["drill_z"],
            options["travel_z"],
            options["feedrate_z"],
            options["toolchange_z"],
            tcl_word(safe_name(stem) + "_cnc"),
        ),
    )
    cnc = object_named(fc, "%s_cnc" % stem)
    if cnc is None:
        # Same rule as isolation: a drill job that was asked for and did not appear is not a zero.
        return {**zero_operation("error"), "seconds": time.perf_counter() - started}
    return {"status": "measured", **moves_of(cnc), "seconds": time.perf_counter() - started}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artefact", help="the corpus file to measure")
    parser.add_argument("--operation", choices=("isolation", "drill"), default=None)
    parser.add_argument("--dia", type=float, default=0.2)
    parser.add_argument("--passes", type=int, default=2)
    parser.add_argument("--overlap", type=float, default=15.0, help="percent of the tool diameter")
    parser.add_argument("--z-cut", type=float, default=-0.1)
    parser.add_argument("--z-move", type=float, default=2.0)
    parser.add_argument("--feedrate", type=float, default=100.0)
    parser.add_argument("--drill-z", type=float, default=-1.8)
    parser.add_argument("--travel-z", type=float, default=2.0)
    parser.add_argument("--feedrate-z", type=float, default=60.0)
    parser.add_argument("--toolchange-z", type=float, default=30.0)
    options = parser.parse_args()

    path = os.path.abspath(options.artefact)
    if not os.path.isfile(path):
        print(json.dumps({"status": "error", "reason": "no such artefact: %s" % path}))
        return 2
    stem = safe_name(os.path.splitext(os.path.basename(path))[0])
    mill = {
        "dia": options.dia,
        "passes": options.passes,
        "overlap": options.overlap,
        "z_cut": options.z_cut,
        "z_move": options.z_move,
        "feedrate": options.feedrate,
    }
    drilled = {
        "drill_z": options.drill_z,
        "travel_z": options.travel_z,
        "feedrate_z": options.feedrate_z,
        "toolchange_z": options.toolchange_z,
    }

    boot_started = time.perf_counter()
    fc = boot()
    boot_seconds = time.perf_counter() - boot_started
    try:
        if options.operation == "isolation":
            measured = measure_isolation(fc, path, stem, mill)
        elif options.operation == "drill":
            measured = measure_drill(fc, path, stem, drilled)
        else:
            measured = measure_import(fc, path, stem)
        measured["environment"] = environment()
        measured["boot_seconds"] = boot_seconds
        print(json.dumps(measured, sort_keys=True))
    finally:
        fc.quit_application()
    return 0


if __name__ == "__main__":
    sys.exit(main())
