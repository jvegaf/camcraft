//! Regression: a stroked outline and a filled region of the same shape, applied
//! in sequence (dark polarity), must combine without an unchainable arrangement.
//!
//! Repro from `fixtures/corpus/kicad-2l-simple/mask-top.gbr`: the 5th pad draws
//! a closed pentagon outline (stroked, D01 with a circular aperture) and then a
//! G36 filled region tracing the same pentagon boundary. Unioning the fill with
//! the accumulated image that already contains the stroked ring previously failed
//! with "the arrangement revisits a piece, so its pieces cannot be chained
//! closed" because a near-boundary probe misclassified the outer join arc. The
//! fix must keep the result as a single solid pad.

use cam_geom::TolerancePolicy;
use cam_gerber::parse;

#[test]
fn stroked_outline_then_fill_of_same_shape_unions() {
  // Four pads (each: stroked square outline + filled square region), then a
  // fifth pad whose outline is a pentagon stroked with D10 (0.2 mm circle)
  // followed by a G36 fill of the same pentagon. Before the fix, imaging this
  // failed at command 63 with an unchainable arrangement.
  let gerber = r#"%FSLAX46Y46*%
%MOMM*%
%LPD*%
G01*
G04 APERTURE LIST*
%ADD10C,0.200000*%
G04 APERTURE END LIST*
D10*
X158000000Y-94750000D02*
X157000000Y-94750000D01*
X157000000Y-93750000D01*
X158000000Y-93750000D01*
X158000000Y-94750000D01*
G36*
X158000000Y-94750000D02*
G01*
X157000000Y-94750000D01*
X157000000Y-93750000D01*
X158000000Y-93750000D01*
X158000000Y-94750000D01*
G37*
X143000000Y-94750000D02*
X142000000Y-94750000D01*
X142000000Y-93750000D01*
X143000000Y-93750000D01*
X143000000Y-94750000D01*
G36*
X143000000Y-94750000D02*
G01*
X142000000Y-94750000D01*
X142000000Y-93750000D01*
X143000000Y-93750000D01*
X143000000Y-94750000D01*
G37*
X143000000Y-87750000D02*
X142000000Y-87750000D01*
X142000000Y-86750000D01*
X143000000Y-86750000D01*
X143000000Y-87750000D01*
G36*
X143000000Y-87750000D02*
G01*
X142000000Y-87750000D01*
X142000000Y-86750000D01*
X143000000Y-86750000D01*
X143000000Y-87750000D01*
G37*
X158000000Y-101750000D02*
X157000000Y-101750000D01*
X157000000Y-100750000D01*
X158000000Y-100750000D01*
X158000000Y-101750000D01*
G36*
X158000000Y-101750000D02*
G01*
X157000000Y-101750000D01*
X157000000Y-100750000D01*
X158000000Y-100750000D01*
X158000000Y-101750000D01*
G37*
X157750000Y-108000000D02*
X157250000Y-108250000D01*
X157000000Y-108250000D01*
X157000000Y-107500000D01*
X158000000Y-107500000D01*
X157750000Y-108000000D01*
G36*
X157750000Y-108000000D02*
G01*
X157250000Y-108250000D01*
X157000000Y-108250000D01*
X157000000Y-107500000D01*
X158000000Y-107500000D01*
X157750000Y-108000000D01*
G37*
M02*
"#;

  let parsed = parse(gerber).expect("gerber parses");
  let (image, skipped) = parsed
    .image_reporting(TolerancePolicy::default())
    .expect("image should succeed: stroked outline + fill of same shape must union");

  assert!(
    skipped.is_empty(),
    "no constructs should be skipped, got: {skipped:?}"
  );
  // 4 earlier pads + the 5th pad combined into one solid shape.
  assert_eq!(
    image.boundaries.len(),
    5,
    "expected 4 pads plus 1 combined 5th pad, got {} boundaries",
    image.boundaries.len()
  );
}
