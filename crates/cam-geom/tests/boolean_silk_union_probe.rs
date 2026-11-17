//! Regression: unioning successive dark silkscreen strokes must not drop a
//! boundary piece when probes disagree at different distances.
//!
//! Minimal repro derived from `fixtures/corpus/substitute-easyeda/silk-top.gbr`
//! (the 24-line prefix). Before the fix, a midpoint probe at a middle distance
//! crossed a thin nearby feature while closer and farther probes classified the
//! piece correctly; taking the transient answer dropped the piece and left the
//! arrangement open ("the arrangement revisits a piece, so its pieces cannot be
//! chained closed").

use cam_geom::TolerancePolicy;
use cam_gerber::parse;

#[test]
fn silk_stroke_union_keeps_gap_bridging_piece() {
  let gerber = r#"%FSLAX46Y46*%
G04 Gerber Fmt 4.6, Leading zero omitted, Abs format (unit mm)*
%MOMM*%
%LPD*%
G01*
G04 APERTURE LIST*
%ADD10C,0.150000*%
%ADD11C,0.120000*%
G04 APERTURE END LIST*
D10*
X132999952Y-57695580D02*
X132952333Y-57743200D01*
X132952333Y-57743200D02*
X132809476Y-57790819D01*
X132809476Y-57790819D02*
X132714238Y-57790819D01*
X132714238Y-57790819D02*
X132571381Y-57743200D01*
X132571381Y-57743200D02*
X132476143Y-57647961D01*
X132476143Y-57647961D02*
X132428524Y-57552723D01*
X132428524Y-57552723D02*
X132380905Y-57362247D01*
M02*
"#;

  let parsed = parse(gerber).expect("gerber parses");
  let (image, skipped) = parsed
    .image_reporting(TolerancePolicy::default())
    .expect("image should succeed: union must not drop a boundary piece");

  assert!(
    skipped.is_empty(),
    "no constructs should be skipped, got: {skipped:?}"
  );
  assert_eq!(
    image.boundaries.len(),
    1,
    "expected one combined polygon, got {} boundaries",
    image.boundaries.len()
  );
}
