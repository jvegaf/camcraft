#!/usr/bin/env bash
# Regenerate `fixtures/corpus/` from the KiCad demo boards, byte for byte.
#
# The corpus is committed (E0.5): CI never regenerates it, it only judges it. This script exists so
# that the committed artefacts have a provenance anyone can replay, and so that the day a board has
# to be replaced the replacement is the same six roles of the same source, not a hand-picked file.
#
# Two things make a replay land on the same bytes as the committed corpus:
#
#   1. `kicad-cli` is pinned by the version it reports, which the script refuses to run without
#      printing: two major versions of the plotter do not write the same file.
#   2. Every timestamp and generator comment the plotter embeds is stripped. The plotter writes
#      `TF.CreationDate`, `TF.GenerationSoftware` and a `Created by KiCad ... date ...` line into
#      each file, and a corpus whose bytes change with the wall clock cannot be a fixture: a diff
#      would be unreadable and a golden would be pinned to the day it was made.
#
# Usage:
#   scripts/parity/export.sh            # rewrite fixtures/corpus/
#   scripts/parity/export.sh --check    # write to a temp dir and diff against the committed one
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
DEMOS=${KICAD_DEMOS:-/usr/share/kicad/demos}
OUT="$ROOT/fixtures/corpus"
CHECK=0
# The branch below either rewrites the committed corpus or proves it reproduces. Those are different
# acts, so the argument that selects one is matched exactly and anything else is a refusal: a typo like
# `--chek` must not fall through to the rewrite, because the rewrite deletes the corpus first and
# reports success after — the two halves of that sentence are exactly what a silent fall-through turns
# into data loss.
case "$#:${1:-}" in
0:) ;;
1:--check) CHECK=1 ;;
*)
  echo "usage: scripts/parity/export.sh [--check]" >&2
  echo "  no argument rewrites fixtures/corpus/; --check only proves it reproduces" >&2
  exit 2
  ;;
esac

if ! command -v kicad-cli >/dev/null 2>&1; then
  echo "ERROR: kicad-cli is not installed; the corpus cannot be regenerated here." >&2
  echo "       The committed corpus under fixtures/corpus/ is what the parity harness judges." >&2
  exit 1
fi
if [ ! -d "$DEMOS" ]; then
  echo "ERROR: the KiCad demo boards are not at $DEMOS (override with KICAD_DEMOS=...)." >&2
  exit 1
fi

KICAD_VERSION=$(kicad-cli version 2>/dev/null || echo unknown)
echo "kicad-cli $KICAD_VERSION"
echo "demos      $DEMOS"

# slug|source board (relative to $DEMOS)|why this board is in the corpus
#
# S1 names six sources — KiCad 2-layer simple, KiCad 4-layer with pours, EasyEDA, Eagle, Altium and a
# board larger than 100x100 mm with pours. Only KiCad's own demos are available offline here, so four
# of the six are KiCad boards standing in for a provenance the corpus cannot obtain yet. The stand-ins
# are not silently relabelled: each one names, in `provenance.json`, which S1 slot it fills and that it
# is a substitute. A board that pretends to be an Eagle export is worse than no corpus, because the
# day an Eagle file arrives nothing would notice it reads differently.
BOARDS=(
  "kicad-2l-simple|stickhub/StickHub.kicad_pcb|S1 KiCad 2 layers, no pours (6.5 KB of copper, 2 layers)"
  "kicad-4l-pours|tiny_tapeout/tinytapeout-demo.kicad_pcb|S1 KiCad 4 layers with pours (167 zones, In1/In2)"
  "kicad-large-pours|pic_programmer/pic_programmer.kicad_pcb|S1 larger than 100x100 mm with pours (234 x 140 mm, 4 zones)"
  "substitute-easyeda|complex_hierarchy/complex_hierarchy.kicad_pcb|substitute for EasyEDA (KiCad, 2 layers, 100.7 x 80.0 mm)"
  "substitute-eagle|openair-max/One-Air-Max.kicad_pcb|substitute for Eagle (KiCad, 4 layers, 43 zones)"
  "substitute-altium|kit-dev-coldfire-xilinx_5213/kit-dev-coldfire-xilinx_5213.kicad_pcb|substitute for Altium (KiCad, 4 layers, 157 x 91 mm)"
)

# KiCad's plotter names its output after the board and picks the extension from the layer, and the
# extension is the only part of the name that is stable: a board loaded from a legacy project keeps its
# old layer *names* (`Top_layer`, `bottom_layer`) and is plotted as `-Top_layer.gtl` while a modern one
# is `-F_Cu.gtl`. Mapping by extension is therefore the rule that holds for both.
role_of() {
  case "$1" in
  *.gtl) echo "top" ;;
  *.gbl) echo "bottom" ;;
  *.g1) echo "inner1" ;;
  *.g2) echo "inner2" ;;
  *.g3) echo "inner3" ;;
  *.g4) echo "inner4" ;;
  *.gts) echo "mask-top" ;;
  *.gbs) echo "mask-bottom" ;;
  *.gto) echo "silk-top" ;;
  *.gbo) echo "silk-bottom" ;;
  *.gm[0-9]*) echo "outline" ;;
  *) echo "" ;;
  esac
}

# Drop every line that records when the file was made or which build made it. What remains is the
# plotter's geometric output, which is what the parser under judgement should see.
normalize() {
  sed -E \
    -e '/^G04 #@! TF\./d' \
    -e '/^G04 Created by KiCad/d' \
    -e '/^; #@! TF\./d' \
    -e '/^; DRILL file KiCad/d' \
    "$1"
}

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/corpus"

for entry in "${BOARDS[@]}"; do
  IFS='|' read -r slug source why <<<"$entry"
  if [ ! -f "$DEMOS/$source" ]; then
    echo "ERROR: $DEMOS/$source is missing; the corpus cannot be regenerated as recorded." >&2
    exit 1
  fi
  stage="$WORK/$slug"
  mkdir -p "$stage"
  # The plotter names its files after the board, so a copy with a stable name keeps the output names
  # stable across machines and across the demo's own directory layout.
  cp "$DEMOS/$source" "$stage/board.kicad_pcb"
  kicad-cli pcb export gerbers --output "$stage/" --no-x2 --no-netlist "$stage/board.kicad_pcb" >/dev/null
  kicad-cli pcb export drill --output "$stage/" --format excellon --excellon-units mm \
    "$stage/board.kicad_pcb" >/dev/null

  target="$WORK/corpus/$slug"
  mkdir -p "$target"
  found=0
  for plotted in "$stage"/board-*; do
    role=$(role_of "$(basename "$plotted")")
    [ -z "$role" ] && continue
    normalize "$plotted" >"$target/$role.gbr"
    found=$((found + 1))
  done
  for drilled in "$stage"/board*.drl; do
    [ -f "$drilled" ] || continue
    normalize "$drilled" >"$target/drill.drl"
    found=$((found + 1))
  done
  for required in top bottom mask-top mask-bottom silk-top silk-bottom outline drill; do
    if [ ! -f "$target/$required.gbr" ] && [ ! -f "$target/$required.drl" ]; then
      echo "ERROR: $slug has no $required artefact; the corpus promises all six roles." >&2
      exit 1
    fi
  done
  board_sha=$(sha256sum "$DEMOS/$source" | cut -d' ' -f1)
  python3 - "$target/provenance.json" "$slug" "$source" "$board_sha" "$why" "$KICAD_VERSION" <<'PY'
import json, sys
path, slug, source, sha, why, version = sys.argv[1:7]
record = {
    "slug": slug,
    "source": f"kicad-demos/{source}",
    "source_sha256": sha,
    "exported_with": f"kicad-cli {version}",
    "fills": why,
    "is_substitute": "substitute" in slug,
    "artefacts": ["top", "bottom", "mask-top", "mask-bottom", "silk-top", "silk-bottom", "outline", "drill"],
    "note": (
        "Regenerate with scripts/parity/export.sh. Timestamps and generator comments are stripped so "
        "a replay lands on the same bytes; the geometry is the plotter's own output."
    ),
}
open(path, "w", encoding="utf-8").write(json.dumps(record, indent=2) + "\n")
PY
  echo "  $slug: $found artefacts"
done

if [ "$CHECK" = "1" ]; then
  # `parity.json` is the golden, produced by `run.sh --update` from what this script plotted: it is not
  # the plotter's output, so a corpus that is correct always differs from `$WORK/corpus` by exactly those
  # files. Excluding them is what makes this check about reproducibility instead of about which files
  # happen to live in the directory.
  if diff -r -x parity.json "$WORK/corpus" "$OUT" >"$WORK/diff" 2>&1; then
    echo "corpus reproduces byte for byte"
  else
    echo "ERROR: the committed corpus differs from what the plotter produces now:" >&2
    head -40 "$WORK/diff" >&2
    exit 1
  fi
  exit 0
fi

rm -rf "$OUT"
mkdir -p "$OUT"
cp -a "$WORK/corpus/." "$OUT/"
echo "wrote $OUT"
