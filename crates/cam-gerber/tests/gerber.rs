//! Behaviour tests for the Gerber RS-274X reader.

use cam_geom::Point2;
use cam_gerber::{
  parse, Command, Format, GerberError, MacroPrimitive, Polarity, Units, ZeroSuppression,
};

const KICAD_EXCERPT: &str = "\
%TF.GenerationSoftware,KiCad,Pcbnew,10.0.7*%
%FSLAX46Y46*%
G04 Gerber Fmt 4.6, Leading zero omitted, Abs format (unit mm)*
%MOMM*%
%LPD*%
G01*
%AMRoundRect*
0 Rectangle with rounded corners*
0 $1 Rounding radius*
0 $2 $3 $4 $5 $6 $7 $8 $9 X,Y pos of 4 corners*
0 Add a 4 corners polygon primitive as box body*
4,1,4,$2,$3,$4,$5,$6,$7,$8,$9,$2,$3,0*
0 Add four circle primitives for the rounded corners*
1,1,$1+$1,$2,$3*
1,1,$1+$1,$4,$5*
1,1,$1+$1,$6,$7*
1,1,$1+$1,$8,$9*
0 Add four rect primitives between the rounded corners*
20,1,$1+$1,$2,$3,$4,$5,0*
20,1,$1+$1,$4,$5,$6,$7,0*
20,1,$1+$1,$6,$7,$8,$9,0*
20,1,$1+$1,$8,$9,$2,$3,0*%
%ADD10RoundRect,0.250000X-0.574524X-0.097227X-0.097227X-0.574524X0.574524X0.097227X0.097227X0.574524X0*%
%ADD11O,3.000000X7.000000*%
%ADD14C,0.660400*%
D10*
X-11528623Y12798623D03*
X-17267400Y2027700D02*
G75*
G02*
X-796519Y17414890I17240020J-1945160D01*
G01*
G36*
X413825Y19511607D02*
G01*
X1261081Y19474615D01*
X1263790Y19474437D01*
G37*
M02*
";

fn close_point(actual: Point2, expected: (f64, f64)) -> bool {
  (actual.x - expected.0).abs() < 1.0e-9 && (actual.y - expected.1).abs() < 1.0e-9
}

#[test]
fn parses_format_units_first_command_and_dark_polarity() {
  let file = parse(KICAD_EXCERPT).expect("fixture parses");
  assert_eq!(
    file.format,
    Format {
      zero_suppression: ZeroSuppression::Leading,
      integer_digits: 4,
      decimal_digits: 6,
    }
  );
  assert_eq!(file.units, Units::Millimetre);
  // Polarity is state, so it is a command and it comes before the first draw.
  assert_eq!(
    file.commands.first().copied(),
    Some(Command::Polarity(Polarity::Dark)),
    "the polarity command precedes the first draw, because it is state"
  );
  assert_eq!(
    file.commands.get(1).copied(),
    Some(Command::SelectAperture(10))
  );
  // And it is no longer merely text in the attribute stream.
  assert!(
    !file.attributes.iter().any(|a| a.contains("LP")),
    "polarity left the attribute stream: {:?}",
    file.attributes
  );
}

#[test]
fn parses_clear_polarity_as_a_command() {
  let file = parse("%FSLAX46Y46*%\n%MOMM*%\n%LPC*%\nM02*\n").expect("clear polarity parses");
  assert_eq!(
    file.commands.first().copied(),
    Some(Command::Polarity(Polarity::Clear)),
    "clear polarity is the same command carrying the other state"
  );
}

#[test]
fn refuses_a_polarity_it_does_not_implement() {
  // `%LPI*%` is a real Gerber statement (inverted polarity). Naming it is honest;
  // guessing at dark would cut material the file never asked to cut.
  let err = parse("%FSLAX46Y46*%\n%MOMM*%\n%LPI*%\nM02*\n").expect_err("LPI is not supported");
  let text = format!("{err}");
  assert!(
    text.contains("LPI"),
    "the refusal names the statement it cannot honour: {text}"
  );
}

#[test]
fn parses_three_apertures_with_templates_and_parameters() {
  let file = parse(KICAD_EXCERPT).expect("fixture parses");
  assert_eq!(file.apertures.len(), 3);
  let a10 = file
    .apertures
    .iter()
    .find(|a| a.code == 10)
    .expect("aperture 10");
  assert_eq!(a10.template, "RoundRect");
  assert_eq!(a10.parameters.len(), 10);
  assert!((a10.parameters[0] - 0.25).abs() < 1.0e-12);

  let a11 = file
    .apertures
    .iter()
    .find(|a| a.code == 11)
    .expect("aperture 11");
  assert_eq!(a11.template, "O");
  assert_eq!(a11.parameters, vec![3.0, 7.0]);

  let a14 = file
    .apertures
    .iter()
    .find(|a| a.code == 14)
    .expect("aperture 14");
  assert_eq!(a14.template, "C");
  assert_eq!(a14.parameters.len(), 1);
  assert!((a14.parameters[0] - 0.6604).abs() < 1.0e-12);
}

#[test]
fn parses_roundrect_macro_primitives_in_order_without_comment_lines() {
  let file = parse(KICAD_EXCERPT).expect("fixture parses");
  assert_eq!(file.macros.len(), 1);
  let m = &file.macros[0];
  assert_eq!(m.name, "RoundRect");
  let codes: Vec<u32> = m
    .primitives
    .iter()
    .map(|p: &MacroPrimitive| p.code)
    .collect();
  assert_eq!(codes, vec![4, 1, 1, 1, 1, 20, 20, 20, 20]);
}

#[test]
fn parses_flash_move_and_arc_with_expected_coordinates() {
  let file = parse(KICAD_EXCERPT).expect("fixture parses");
  // flash at (-11.528623, 12.798623)
  let flash = file
    .commands
    .iter()
    .find_map(|c| match *c {
      Command::Flash { at } => Some(at),
      _ => None,
    })
    .expect("flash");
  assert!(
    close_point(flash, (-11.528623, 12.798623)),
    "flash {flash:?}"
  );
  // move at (-17.2674, 2.0277)
  let moves: Vec<Point2> = file
    .commands
    .iter()
    .filter_map(|c| match *c {
      Command::Move { to } => Some(to),
      _ => None,
    })
    .collect();
  assert!(
    moves.iter().any(|p| close_point(*p, (-17.2674, 2.0277))),
    "moves {moves:?}"
  );
  // arc ends at (-0.796519, 17.41489) with offset (17.24002, -1.94516) clockwise true
  let arc = file
    .commands
    .iter()
    .find_map(|c| match *c {
      Command::Arc {
        to,
        offset,
        clockwise,
      } => Some((to, offset, clockwise)),
      _ => None,
    })
    .expect("arc");
  assert!(
    close_point(arc.0, (-0.796519, 17.41489)),
    "arc to {:?}",
    arc.0
  );
  assert!(
    (arc.1 .0 - 17.24002).abs() < 1.0e-9 && (arc.1 .1 - (-1.94516)).abs() < 1.0e-9,
    "arc offset {:?}",
    arc.1
  );
  assert!(arc.2, "arc clockwise");
}

#[test]
fn parses_region_with_exactly_two_lines_inside() {
  let file = parse(KICAD_EXCERPT).expect("fixture parses");
  let mut in_region = false;
  let mut region_lines: Vec<Point2> = Vec::new();
  let mut region_count = 0;
  for cmd in &file.commands {
    match *cmd {
      Command::RegionBegin => {
        in_region = true;
        region_count += 1;
      }
      Command::RegionEnd => {
        in_region = false;
      }
      Command::Line { to } if in_region => region_lines.push(to),
      _ => {}
    }
  }
  assert_eq!(region_count, 1);
  assert_eq!(region_lines.len(), 2, "region lines: {region_lines:?}");
  assert!(
    close_point(region_lines[0], (1.261081, 19.474615)),
    "{:?}",
    region_lines[0]
  );
  assert!(
    close_point(region_lines[1], (1.26379, 19.474437)),
    "{:?}",
    region_lines[1]
  );
}

#[test]
fn refuses_trailing_zero_suppression_naming_its_line() {
  let err = parse("%FSTAX24Y24*%\n").expect_err("trailing zeros refused");
  assert!(
    matches!(err, GerberError::Unsupported { line: 1, .. }),
    "{err:?}"
  );
  let msg = err.to_string();
  assert!(msg.contains("1"), "{msg}");
}

#[test]
fn refuses_g74_single_quadrant_naming_its_line() {
  let err = parse("%FSLAX46Y46*%\n%MOMM*%\nG74*\n").expect_err("G74 refused");
  assert!(
    matches!(err, GerberError::Unsupported { line: 3, .. }),
    "{err:?}"
  );
  assert!(err.to_string().contains("G74"), "{err}");
}

#[test]
fn refuses_d03_with_no_aperture_selected_naming_its_line() {
  let err = parse("%FSLAX46Y46*%\n%MOMM*%\nX0Y0D03*\n").expect_err("no aperture");
  assert!(
    matches!(err, GerberError::NoApertureSelected { line: 3 }),
    "{err:?}"
  );
}

#[test]
fn refuses_d99_undefined_aperture_naming_its_line() {
  let text = "%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.0*%\nD99*\n";
  let err = parse(text).expect_err("undefined aperture");
  assert!(
    matches!(
      err,
      GerberError::UnknownAperture {
        line: 4,
        aperture: 99
      }
    ),
    "{err:?}"
  );
  assert!(err.to_string().contains("99"), "{err}");
}

#[test]
fn a_format_whose_digits_are_not_digits_is_refused_rather_than_panicking() {
  // "AA" has the right length and matches itself, so the shape checks pass; the characters are not
  // digits. Before this test the reader unwrapped that conversion and panicked on untrusted input.
  let error = parse("%FSLAXAAYAA*%\n%MOMM*%\nM02*\n").expect_err("AA is not a format");
  assert!(matches!(error, GerberError::Invalid { .. }), "{error:?}");
  assert!(error.to_string().contains("FSLAXAAYAA"), "{error}");
}

#[test]
fn a_file_that_ends_inside_an_extended_block_is_refused() {
  // The second block never closes, so its contents were never processed. Returning Ok here would
  // hand the caller a document missing whatever that block defined, with no diagnostic at all.
  let error = parse("%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.25*%\n%ADD11C,0.5")
    .expect_err("the block never closes");
  assert!(matches!(error, GerberError::Invalid { .. }), "{error:?}");
  assert!(error.to_string().contains("never closed"), "{error}");
}

const TOLERANCE: f64 = 1.0e-6;

#[test]
fn a_region_square_becomes_one_closed_boundary_of_four_lines_with_expected_bounds() {
  // A G36/G37 square drawn as four D01 lines is one boundary, closed, and line-only.
  // FSLAX36Y36 => 3 integer and 6 decimal digits, so 10.0 mm is 010000000.
  let gerber = parse(
    "%FSLAX36Y36*%\n%MOMM*%\n%ADD10C,1.0*%\nD10*\n\
     X000000000Y000000000D02*\nG36*\n\
     X010000000Y000000000D01*\nX010000000Y010000000D01*\n\
     X000000000Y010000000D01*\nX000000000Y000000000D01*\n\
     G37*\nM02*\n",
  )
  .expect("fixture parses");
  let image = gerber.image(TOLERANCE).expect("image builds");
  assert_eq!(image.boundaries.len(), 1, "one boundary");
  let boundary = &image.boundaries[0];
  assert!(boundary.is_closed(), "the boundary is closed");
  let segments = boundary.segments();
  assert_eq!(segments.len(), 4, "four segments");
  assert!(
    segments
      .iter()
      .all(|segment| matches!(segment, cam_geom::Segment::Line(_))),
    "every segment is a line: {segments:?}"
  );
  let bounds = boundary.bounds();
  assert!((bounds.min.x - 0.0).abs() < 1.0e-9, "min x {:?}", bounds);
  assert!((bounds.min.y - 0.0).abs() < 1.0e-9, "min y {:?}", bounds);
  assert!((bounds.max.x - 10.0).abs() < 1.0e-9, "max x {:?}", bounds);
  assert!((bounds.max.y - 10.0).abs() < 1.0e-9, "max y {:?}", bounds);
}

#[test]
fn a_dark_square_minus_a_smaller_clear_square_yields_outer_and_inner_hole_boundaries() {
  // Polarity is applied at RegionEnd: dark unions material, clear subtracts it, leaving a hole.
  // FSLAX36Y36 => 3 integer and 6 decimal digits.
  let gerber = parse(
    "%FSLAX36Y36*%\n%MOMM*%\n%ADD10C,1.0*%\nD10*\n\
     %LPD*%\nX000000000Y000000000D02*\nG36*\n\
     X020000000Y000000000D01*\nX020000000Y020000000D01*\n\
     X000000000Y020000000D01*\nX000000000Y000000000D01*\nG37*\n\
     %LPC*%\nX005000000Y005000000D02*\nG36*\n\
     X015000000Y005000000D01*\nX015000000Y015000000D01*\n\
     X005000000Y015000000D01*\nX005000000Y005000000D01*\nG37*\n\
     M02*\n",
  )
  .expect("fixture parses");
  let image = gerber.image(TOLERANCE).expect("image builds");
  assert_eq!(
    image.boundaries.len(),
    2,
    "the outer boundary plus the inner hole: {:?}",
    image.boundaries
  );
  let outer = image
    .boundaries
    .iter()
    .find(|boundary| (boundary.bounds().max.x - 20.0).abs() < 1.0e-9)
    .expect("outer boundary present");
  let hole = image
    .boundaries
    .iter()
    .find(|boundary| (boundary.bounds().max.x - 15.0).abs() < 1.0e-9)
    .expect("inner hole boundary present");
  let hole_bounds = hole.bounds();
  assert!((hole_bounds.min.x - 5.0).abs() < 1.0e-9, "hole min x");
  assert!((hole_bounds.min.y - 5.0).abs() < 1.0e-9, "hole min y");
  assert!((hole_bounds.max.y - 15.0).abs() < 1.0e-9, "hole max y");
  assert!(
    !image.contains(cam_geom::Point2::new(10.0, 10.0), TOLERANCE),
    "the centre of the clear window is not material"
  );
  assert!(
    image.contains(cam_geom::Point2::new(2.0, 10.0), TOLERANCE),
    "a point inside the outer ring is material"
  );
  assert!(
    !image.contains(cam_geom::Point2::new(25.0, 10.0), TOLERANCE),
    "outside the outer boundary is not material"
  );
  let _ = outer;
}

#[test]
fn a_region_arc_is_kept_as_a_segment_arc_not_flattened() {
  // S4: an arc of Gerber enters the region as an arc, never sampled into lines.
  // A quarter circle: centre (0,0), radius 10, from (10,0) to (0,10) counter-clockwise (G03).
  // I,J are offsets from start to centre: (-10, 0). FSLAX36Y36 => 10.0 mm is 010000000.
  let gerber = parse(
    "%FSLAX36Y36*%\n%MOMM*%\n%ADD10C,1.0*%\nD10*\n\
     X010000000Y000000000D02*\nG36*\n\
     G03*\nX000000000Y010000000I-010000000J000000000D01*\n\
     G01*\nX000000000Y000000000D01*\nX010000000Y000000000D01*\n\
     G37*\nM02*\n",
  )
  .expect("fixture parses");
  let image = gerber.image(TOLERANCE).expect("image builds");
  assert_eq!(image.boundaries.len(), 1);
  let segments = image.boundaries[0].segments();
  let arcs: Vec<_> = segments
    .iter()
    .filter_map(|segment| match segment {
      cam_geom::Segment::Arc(arc) => Some(arc),
      _ => None,
    })
    .collect();
  assert_eq!(
    arcs.len(),
    1,
    "exactly one arc segment survives: {segments:?}"
  );
  let arc = arcs[0];
  assert!(
    (arc.radius() - 10.0).abs() < 1.0e-6,
    "radius {:?}",
    arc.radius()
  );
  assert!(
    (arc.sweep().abs() - std::f64::consts::FRAC_PI_2).abs() < 1.0e-6,
    "quarter-turn sweep {:?}",
    arc.sweep()
  );
  assert!(
    arc.is_ccw(),
    "counter-clockwise arc, got sweep {:?}",
    arc.sweep()
  );
}

#[test]
fn a_multi_quadrant_arc_sweeps_the_long_way_round() {
  // The dialect is G75 multi-quadrant: the arc travels in the commanded direction,
  // so it may sweep more than half a turn. Centre (0,0), radius 10, from (0,10)
  // clockwise to (-10,0) covers 270 degrees. A reader that took the shorter way
  // between the same endpoints would produce 90 and still look plausible.
  let gerber = parse(
    "%FSLAX36Y36*%\n%MOMM*%\n%ADD10C,1.0*%\nD10*\n\
     X000000000Y010000000D02*\nG36*\n\
     G02*\nX-010000000Y000000000I000000000J-010000000D01*\n\
     G01*\nX000000000Y000000000D01*\nX000000000Y010000000D01*\n\
     G37*\nM02*\n",
  )
  .expect("fixture parses");
  let image = gerber.image(TOLERANCE).expect("image builds");
  let segments = image.boundaries[0].segments();
  let arcs: Vec<_> = segments
    .iter()
    .filter_map(|segment| match segment {
      cam_geom::Segment::Arc(arc) => Some(*arc),
      _ => None,
    })
    .collect();
  assert_eq!(arcs.len(), 1, "one arc segment: {segments:?}");
  let arc = arcs[0];
  let three_quarters = 3.0 * std::f64::consts::FRAC_PI_2;
  assert!(
    (arc.sweep().abs() - three_quarters).abs() < 1.0e-6,
    "a 270 degree sweep, not the 90 degree shortcut: {:?}",
    arc.sweep()
  );
  // The contour is canonicalised to a counter-clockwise outer boundary, so the stored arc
  // runs the other way round: assert the curve, not the direction flag. A 90 degree
  // shortcut would keep the same endpoints and put the midpoint on the opposite side.
  let midpoint = arc.point_at_fraction(0.5);
  let diagonal = 10.0 / std::f64::consts::SQRT_2;
  assert!(
    (midpoint.x - diagonal).abs() < 1.0e-6 && (midpoint.y + diagonal).abs() < 1.0e-6,
    "the arc rounds the far side through (7.07, -7.07), not the near side: {midpoint:?}"
  );
}

#[test]
fn a_dark_c_flash_of_diameter_two_at_five_five_gives_bounds_one_millimetre_round_the_point() {
  // Command stream: [SelectAperture(10)=0, Flash=1]. Diameter 2 => radius 1, centred at (5,5).
  let gerber =
    parse("%FSLAX36Y36*%\n%MOMM*%\n%ADD10C,2.0*%\nD10*\nX005000000Y005000000D03*\nM02*\n")
      .expect("fixture parses");
  let image = gerber.image(TOLERANCE).expect("image builds");
  assert_eq!(
    image.boundaries.len(),
    1,
    "one boundary: {:?}",
    image.boundaries
  );
  let boundary = &image.boundaries[0];
  assert!(
    boundary
      .segments()
      .iter()
      .all(|segment| matches!(segment, cam_geom::Segment::Arc(_))),
    "a circular aperture is a circle, not a polygon: {:?}",
    boundary.segments()
  );
  let bounds = boundary.bounds();
  assert!((bounds.min.x - 4.0).abs() < 1.0e-6, "min x {bounds:?}");
  assert!((bounds.min.y - 4.0).abs() < 1.0e-6, "min y {bounds:?}");
  assert!((bounds.max.x - 6.0).abs() < 1.0e-6, "max x {bounds:?}");
  assert!((bounds.max.y - 6.0).abs() < 1.0e-6, "max y {bounds:?}");
  let area: f64 = image
    .boundaries
    .iter()
    .map(|boundary| boundary.signed_area().abs())
    .sum();
  assert!(
    (area - std::f64::consts::PI).abs() < 1.0e-9,
    "area is the exact circle area pi, got {area}"
  );
  assert!(
    image.contains(cam_geom::Point2::new(5.0, 5.0), TOLERANCE),
    "the flash point is material"
  );
  assert!(
    !image.contains(cam_geom::Point2::new(5.0, 6.5), TOLERANCE),
    "beyond the radius is not material"
  );
}

#[test]
fn a_dark_r_flash_of_four_by_two_at_five_five_is_centred_on_the_flash_point() {
  // A Gerber R flash is centred on the point: 4 wide by 2 high at (5,5) spans (3,4)-(7,6),
  // which cam_geom::primitive::rectangle would place corner-anchored unless translated.
  let gerber =
    parse("%FSLAX36Y36*%\n%MOMM*%\n%ADD10R,4.0X2.0*%\nD10*\nX005000000Y005000000D03*\nM02*\n")
      .expect("fixture parses");
  let image = gerber.image(TOLERANCE).expect("image builds");
  assert_eq!(
    image.boundaries.len(),
    1,
    "one boundary: {:?}",
    image.boundaries
  );
  let bounds = image.boundaries[0].bounds();
  assert!((bounds.min.x - 3.0).abs() < 1.0e-9, "min x {bounds:?}");
  assert!((bounds.min.y - 4.0).abs() < 1.0e-9, "min y {bounds:?}");
  assert!((bounds.max.x - 7.0).abs() < 1.0e-9, "max x {bounds:?}");
  assert!((bounds.max.y - 6.0).abs() < 1.0e-9, "max y {bounds:?}");
  let cx = (bounds.min.x + bounds.max.x) / 2.0;
  let cy = (bounds.min.y + bounds.max.y) / 2.0;
  assert!((cx - 5.0).abs() < 1.0e-9, "centred on x: {cx}");
  assert!((cy - 5.0).abs() < 1.0e-9, "centred on y: {cy}");
}

#[test]
fn a_clear_c_flash_cuts_a_hole_through_a_dark_r_region() {
  // Dark 10x10 region, then a clear circular flash of diameter 2 at its centre (5,5): polarity
  // applies to flashes the same way as to region blocks, so the circle is subtracted as a hole.
  let gerber = parse(
    "%FSLAX36Y36*%\n%MOMM*%\n%ADD10C,2.0*%\nD10*\n\
     %LPD*%\nX000000000Y000000000D02*\nG36*\n\
     X010000000Y000000000D01*\nX010000000Y010000000D01*\n\
     X000000000Y010000000D01*\nX000000000Y000000000D01*\nG37*\n\
     %LPC*%\nX005000000Y005000000D03*\nM02*\n",
  )
  .expect("fixture parses");
  let image = gerber.image(TOLERANCE).expect("image builds");
  assert_eq!(
    image.boundaries.len(),
    2,
    "outer boundary plus inner hole: {:?}",
    image.boundaries
  );
  let outer = image
    .boundaries
    .iter()
    .find(|boundary| (boundary.bounds().max.x - 10.0).abs() < 1.0e-6)
    .expect("outer boundary present");
  let hole = image
    .boundaries
    .iter()
    .find(|boundary| (boundary.bounds().max.x - 6.0).abs() < 1.0e-6)
    .expect("inner hole boundary present");
  let hole_bounds = hole.bounds();
  assert!(
    (hole_bounds.min.x - 4.0).abs() < 1.0e-6,
    "hole min x {hole_bounds:?}"
  );
  assert!(
    (hole_bounds.min.y - 4.0).abs() < 1.0e-6,
    "hole min y {hole_bounds:?}"
  );
  assert!(
    !image.contains(cam_geom::Point2::new(5.0, 5.0), TOLERANCE),
    "the flashed centre is cleared"
  );
  assert!(
    image.contains(cam_geom::Point2::new(2.0, 5.0), TOLERANCE),
    "material remains around the hole"
  );
  assert!(
    !image.contains(cam_geom::Point2::new(12.0, 5.0), TOLERANCE),
    "outside the region is not material"
  );
  assert!(
    hole
      .segments()
      .iter()
      .all(|segment| matches!(segment, cam_geom::Segment::Arc(_))),
    "the cleared hole is a circle, not a polygon: {:?}",
    hole.segments()
  );
  // material area = outer minus hole
  let outer_area = outer.signed_area().abs();
  let hole_area = hole.signed_area().abs();
  assert!(
    ((outer_area - hole_area) - (100.0 - std::f64::consts::PI)).abs() < 1.0e-9,
    "remaining material is 100-pi, got {}",
    outer_area - hole_area
  );
  let _ = outer;
}

#[test]
fn an_o_aperture_flash_is_refused_naming_the_command_index_and_the_template() {
  // Command stream: [SelectAperture(10)=0, Flash=1]. O (obround) has no primitive constructor.
  let gerber =
    parse("%FSLAX36Y36*%\n%MOMM*%\n%ADD10O,2.0X1.0*%\nD10*\nX000000000Y000000000D03*\nM02*\n")
      .expect("fixture parses");
  let error = gerber
    .image(TOLERANCE)
    .expect_err("an O flash is not supported");
  assert!(
    matches!(error, GerberError::Unsupported { line: 0, .. }),
    "refused as unsupported at line 0: {error:?}"
  );
  let message = error.to_string();
  assert!(
    message.contains("command 1"),
    "names the command index: {message}"
  );
  assert!(
    message.contains("template O") || (message.contains('O') && message.contains("template")),
    "names the template: {message}"
  );
}

/// The endpoints of a drawn stroke's straight sides, in micrometres, sorted.
fn line_ends(image: &cam_geom::region::Region) -> Vec<(i64, i64)> {
  let mut ends: Vec<(i64, i64)> = image
    .boundaries
    .iter()
    .flat_map(|contour| contour.segments())
    .filter(|segment| matches!(segment, cam_geom::Segment::Line(_)))
    .flat_map(|segment| [segment.start_point(), segment.end_point()])
    .map(|point| {
      (
        (point.x * 1.0e6).round() as i64,
        (point.y * 1.0e6).round() as i64,
      )
    })
    .collect();
  ends.sort_unstable();
  ends
}

fn caps(image: &cam_geom::region::Region) -> Vec<cam_geom::Arc> {
  image
    .boundaries
    .iter()
    .flat_map(|contour| contour.segments())
    .filter_map(|segment| match segment {
      cam_geom::Segment::Arc(arc) => Some(arc),
      _ => None,
    })
    .collect()
}

#[test]
fn a_circular_draw_is_the_aperture_swept_along_the_segment() {
  // S6: a draw outside a region is the aperture dragged along the segment. Diameter 2 from
  // (0,0) to (10,0) gives a capsule: two straight sides at y = +-1 and two half-turn caps.
  let gerber = parse(
    "%FSLAX36Y36*%\n%MOMM*%\n%ADD10C,2.0*%\nD10*\n\
     X000000000Y000000000D02*\nG01*\nX010000000Y000000000D01*\nM02*\n",
  )
  .expect("fixture parses");
  let image = gerber.image(TOLERANCE).expect("image builds");
  assert_eq!(image.boundaries.len(), 1, "a capsule is one boundary");
  assert_eq!(
    line_ends(&image),
    vec![
      (0, -1_000_000),
      (0, 1_000_000),
      (10_000_000, -1_000_000),
      (10_000_000, 1_000_000),
    ],
    "the sides run the segment's length at the aperture's half width"
  );
  // The arrangement may split an arc where a seam lands on the body's edge, so count the turning
  // rather than the pieces: two round caps together are one full turn, at the aperture's radius.
  let caps = caps(&image);
  assert!(!caps.is_empty(), "a capsule has round ends");
  let turned: f64 = caps.iter().map(|cap| cap.sweep().abs()).sum();
  assert!(
    (turned - std::f64::consts::TAU).abs() < 1.0e-6,
    "two half-turn caps are one full turn, got {turned}"
  );
  for cap in &caps {
    assert!(
      (cap.radius() - 1.0).abs() < 1.0e-6,
      "cap radius {:?}",
      cap.radius()
    );
  }
}

#[test]
fn a_diagonal_draw_sweeps_the_same_capsule() {
  // The aperture's half width is offset along the segment's normal, so a stroke at 45 degrees
  // is not the axis-aligned capsule moved about: an implementation that only swept along x or y
  // would land on different corners.
  let gerber = parse(
    "%FSLAX36Y36*%\n%MOMM*%\n%ADD10C,2.0*%\nD10*\n\
     X000000000Y000000000D02*\nG01*\nX006000000Y008000000D01*\nM02*\n",
  )
  .expect("fixture parses");
  let image = gerber.image(TOLERANCE).expect("image builds");
  assert_eq!(
    line_ends(&image),
    vec![
      (-800_000, 600_000),
      (800_000, -600_000),
      (5_200_000, 8_600_000),
      (6_800_000, 7_400_000),
    ],
    "the normal of a (6,8) segment is (-0.8, 0.6), not an axis"
  );
  let turned: f64 = caps(&image).iter().map(|cap| cap.sweep().abs()).sum();
  assert!(
    (turned - std::f64::consts::TAU).abs() < 1.0e-6,
    "the caps stay round at any angle, got {turned}"
  );
}

#[test]
fn a_clear_draw_cuts_a_track_out_of_a_dark_region() {
  // The stroke goes through the polarity accumulator like everything else: clear subtracts.
  let gerber = parse(
    "%FSLAX36Y36*%\n%MOMM*%\n%ADD10C,2.0*%\n%LPD*%\nD10*\n\
     X000000000Y000000000D02*\nG36*\n\
     X020000000Y000000000D01*\nX020000000Y020000000D01*\n\
     X000000000Y020000000D01*\nX000000000Y000000000D01*\n\
     G37*\n%LPC*%\n\
     X-005000000Y010000000D02*\nG01*\nX025000000Y010000000D01*\nM02*\n",
  )
  .expect("fixture parses");
  let image = gerber.image(TOLERANCE).expect("image builds");
  assert_eq!(
    image.boundaries.len(),
    2,
    "a clear track across the square leaves two pieces"
  );
}

/// How many distinct directions a shape's straight sides point in.
fn line_directions(image: &cam_geom::region::Region) -> usize {
  let mut directions: Vec<(i64, i64)> = image
    .boundaries
    .iter()
    .flat_map(|contour| contour.segments())
    .filter(|segment| matches!(segment, cam_geom::Segment::Line(_)))
    .map(|segment| (segment.start_point(), segment.end_point()))
    .filter_map(|(start, end)| {
      let (dx, dy) = (end.x - start.x, end.y - start.y);
      let length = (dx * dx + dy * dy).sqrt();
      if length <= 0.0 {
        return None;
      }
      let unit = (
        (dx / length * 1.0e6).round() as i64,
        (dy / length * 1.0e6).round() as i64,
      );
      // An edge and its reverse are one direction.
      Some(if unit < (0, 0) {
        (-unit.0, -unit.1)
      } else {
        unit
      })
    })
    .collect();
  directions.sort_unstable();
  directions.dedup();
  directions.len()
}

/// The box a shape's straight sides span, in micrometres.
fn line_bounds(image: &cam_geom::region::Region) -> (i64, i64, i64, i64) {
  let ends = line_ends(image);
  ends.iter().fold(
    (i64::MAX, i64::MAX, i64::MIN, i64::MIN),
    |(min_x, min_y, max_x, max_y), (x, y)| {
      (min_x.min(*x), min_y.min(*y), max_x.max(*x), max_y.max(*y))
    },
  )
}

#[test]
fn a_rectangular_draw_sweeps_the_aperture_into_a_rectangle() {
  // S6: 2 by 4 from (0,0) to (10,0) is the Minkowski sum of the aperture and the segment: the
  // aperture at each end plus the swept sides, which here is the 12 by 4 rectangle. No arcs.
  let gerber = parse(
    "%FSLAX36Y36*%\n%MOMM*%\n%ADD10R,2.0X4.0*%\nD10*\n\
     X000000000Y000000000D02*\nG01*\nX010000000Y000000000D01*\nM02*\n",
  )
  .expect("fixture parses");
  let image = gerber.image(TOLERANCE).expect("image builds");
  assert!(
    caps(&image).is_empty(),
    "a rectangular sweep has straight sides only"
  );
  // The arrangement splits a side wherever a seam lands on it, so assert the shape, not the pieces.
  assert_eq!(
    line_bounds(&image),
    (-1_000_000, -2_000_000, 11_000_000, 2_000_000),
    "two wide plus ten long, four tall, centred on the segment"
  );
  assert_eq!(
    image.boundaries.len(),
    1,
    "one rectangle, however it is cut"
  );
}

#[test]
fn a_rectangular_draw_at_an_angle_is_a_hexagon_not_a_rotated_rectangle() {
  // The same aperture swept at an angle cannot be a rotated rectangle: the sum is a hexagon.
  // Asserting only the bounds would pass for a rotated rectangle too, so the corner count is
  // what tells them apart.
  let gerber = parse(
    "%FSLAX36Y36*%\n%MOMM*%\n%ADD10R,2.0X2.0*%\nD10*\n\
     X000000000Y000000000D02*\nG01*\nX006000000Y008000000D01*\nM02*\n",
  )
  .expect("fixture parses");
  let image = gerber.image(TOLERANCE).expect("image builds");
  assert!(caps(&image).is_empty(), "no arcs: this is a polygon");
  assert_eq!(
    line_bounds(&image),
    (-1_000_000, -1_000_000, 7_000_000, 9_000_000)
  );
  // A rectangle has two side directions; a hexagon has three. Bounds alone would pass for a
  // rotated rectangle, so this is what tells them apart.
  let directions = line_directions(&image);
  assert!(
    directions >= 3,
    "a swept square is a hexagon, so its sides point three ways, got {directions}"
  );
}

#[test]
fn an_obround_aperture_draw_is_refused_naming_the_template() {
  // S6: an O aperture is a capsule already, so sweeping it is a different construction. Refusing
  // it by name beats pretending the rectangular path covers it.
  let error = parse(
    "%FSLAX36Y36*%\n%MOMM*%\n%ADD10O,2.0X4.0*%\nD10*\n\
     X000000000Y000000000D02*\nG01*\nX010000000Y000000000D01*\nM02*\n",
  )
  .expect("fixture parses")
  .image(TOLERANCE)
  .expect_err("an obround stroke is not built yet");
  let text = format!("{error}");
  assert!(
    text.contains("template O"),
    "the refusal names the template: {text}"
  );
}

#[test]
fn a_move_inside_a_region_starts_another_contour() {
  // S7: a D02 between G36 and G37 closes the contour drawn so far and starts another one of the
  // same region. Two separate squares are two boundaries, and both are dark.
  let gerber = parse(
    "%FSLAX36Y36*%\n%MOMM*%\n%ADD10C,0.100*%\n%LPD*%\nD10*\nG36*\n\
     X000000000Y000000000D02*\n\
     X010000000Y000000000D01*\nX010000000Y010000000D01*\n\
     X000000000Y010000000D01*\nX000000000Y000000000D01*\n\
     X020000000Y000000000D02*\n\
     X030000000Y000000000D01*\nX030000000Y010000000D01*\n\
     X020000000Y010000000D01*\nX020000000Y000000000D01*\n\
     G37*\nM02*\n",
  )
  .expect("fixture parses");
  let image = gerber.image(TOLERANCE).expect("image builds");
  assert_eq!(
    image.boundaries.len(),
    2,
    "the move closed one square and opened the next"
  );
}

#[test]
fn a_contour_with_fewer_than_three_segments_is_refused() {
  // A move one segment into a contour cannot close a shape, so it is refused by name rather than
  // handed to the kernel to fail in its own words.
  let error = parse(
    "%FSLAX36Y36*%\n%MOMM*%\n%ADD10C,0.100*%\n%LPD*%\nD10*\nG36*\n\
     X000000000Y000000000D02*\nX010000000Y000000000D01*\n\
     X000000000Y010000000D02*\nG37*\nM02*\n",
  )
  .expect("fixture parses")
  .image(TOLERANCE)
  .expect_err("a one-segment contour cannot close");
  let text = format!("{error}");
  assert!(
    text.contains("fewer than three"),
    "the refusal says why: {text}"
  );
}

#[test]
fn a_macro_parameter_expression_is_evaluated_with_the_apertures_modifiers() {
  // E3b: the dialect writes `$1+$1` for a width of twice the first modifier, and mixes x and X
  // with the usual arithmetic signs. Multiplication binds tighter than addition.
  assert_eq!(
    cam_gerber::evaluate_parameter("$1+$1", &[2.5]).expect("evaluates"),
    5.0
  );
  assert_eq!(
    cam_gerber::evaluate_parameter("$1x$2/2", &[3.0, 4.0]).expect("evaluates"),
    6.0
  );
  assert_eq!(
    cam_gerber::evaluate_parameter("-$1+1", &[0.25]).expect("evaluates"),
    0.75
  );
  assert_eq!(
    cam_gerber::evaluate_parameter("2X3-1", &[]).expect("evaluates"),
    5.0
  );
}

#[test]
fn a_parameter_the_aperture_does_not_supply_is_refused_by_name() {
  let error = cam_gerber::evaluate_parameter("$3+1", &[2.0, 4.0])
    .expect_err("the aperture supplies two modifiers");
  let text = format!("{error}");
  assert!(
    text.contains("$3") && text.contains('2'),
    "the refusal names the parameter and what was supplied: {text}"
  );
}

#[test]
fn a_parameter_expression_outside_the_grammar_is_refused() {
  // Parentheses are not part of what this reader evaluates, and guessing would silently change
  // the aperture's size, so the refusal names the character.
  let error = cam_gerber::evaluate_parameter("(1+2)", &[]).expect_err("parentheses are refused");
  let text = format!("{error}");
  assert!(
    text.contains('('),
    "the refusal names what it could not read: {text}"
  );

  let error =
    cam_gerber::evaluate_parameter("$1+", &[1.0]).expect_err("a trailing sign is refused");
  assert!(
    format!("{error}").contains("operator"),
    "the refusal says what is missing: {error}"
  );
}

fn macro_shape(source: &str, modifiers: &[f64]) -> cam_geom::region::Region {
  let file = parse(source).expect("fixture parses");
  let definition = file.macros.first().expect("one macro is defined");
  cam_gerber::macro_region(definition, modifiers, TOLERANCE, 0).expect("the macro builds")
}

#[test]
fn a_macro_primitive_one_is_a_circle() {
  // E3b/T9: `1,exposure,diameter,cx,cy` is a circle, and it stays arcs rather than a polygon.
  let region = macro_shape(
    "%FSLAX36Y36*%\n%MOMM*%\n%AMDisc*\n1,1,2.0,0,0*%\nM02*\n",
    &[],
  );
  assert_eq!(region.boundaries.len(), 1);
  let caps = caps(&region);
  assert!(!caps.is_empty(), "a circle is arcs");
  assert!(
    caps.iter().all(|cap| (cap.radius() - 1.0).abs() < 1.0e-6),
    "diameter 2 is radius 1"
  );
  let turned: f64 = caps.iter().map(|cap| cap.sweep().abs()).sum();
  assert!(
    (turned - std::f64::consts::TAU).abs() < 1.0e-6,
    "a full turn, got {turned}"
  );
}

#[test]
fn an_unexposed_macro_primitive_takes_material_away() {
  // The second primitive's exposure field is 0, which means clear: a hole in the disc.
  let region = macro_shape(
    "%FSLAX36Y36*%\n%MOMM*%\n%AMRing*\n1,1,10.0,0,0*\n1,0,4.0,0,0*%\nM02*\n",
    &[],
  );
  assert_eq!(
    region.boundaries.len(),
    2,
    "the outer disc plus the cleared hole"
  );
}

#[test]
fn a_macro_primitive_reads_the_apertures_modifiers() {
  // `$1` is the first modifier and `$1+$1` is twice it: a vector line of width 2 from (0,0) to
  // (4,0), which is the same capsule a circular draw makes.
  let region = macro_shape(
    "%FSLAX36Y36*%\n%MOMM*%\n%AMTrack*\n20,1,$1,0,0,$1+$1,0*%\nM02*\n",
    &[2.0],
  );
  // The body's straight sides span the segment at the half width; the round ends beyond x=0 and
  // x=4 are arcs, so they are pinned by the turn rather than by the line bounds.
  assert_eq!(
    line_bounds(&region),
    (0, -1_000_000, 4_000_000, 1_000_000),
    "width 2 swept from 0 to 4"
  );
  let turned: f64 = caps(&region).iter().map(|cap| cap.sweep().abs()).sum();
  assert!(
    (turned - std::f64::consts::TAU).abs() < 1.0e-6,
    "the ends are round: {turned}"
  );
}

#[test]
fn a_rotated_macro_primitive_is_refused_naming_the_angle() {
  // Drawing it unrotated would put the pad in the wrong place without saying so.
  let file = parse("%FSLAX36Y36*%\n%MOMM*%\n%AMTurned*\n1,1,2.0,0,0,45*%\nM02*\n").expect("parses");
  let error = cam_gerber::macro_region(file.macros.first().expect("one macro"), &[], TOLERANCE, 0)
    .expect_err("a rotated primitive is not applied");
  let text = format!("{error}");
  assert!(
    text.contains("45") && text.contains("rotated"),
    "the refusal names the angle: {text}"
  );
}

#[test]
fn a_macro_primitive_this_reader_does_not_build_is_refused_by_number() {
  // Primitive 5 is a regular polygon in Gerber. Refusing it by number beats drawing something else
  // and calling it a pad.
  let file = parse("%FSLAX36Y36*%\n%MOMM*%\n%AMGon*\n5,1,4,10.0,0,0*%\nM02*\n").expect("parses");
  let error = cam_gerber::macro_region(file.macros.first().expect("one macro"), &[], TOLERANCE, 0)
    .expect_err("primitive 5 is not built");
  let text = format!("{error}");
  assert!(
    text.contains("primitive 5"),
    "the refusal names the primitive: {text}"
  );
}

#[test]
fn a_flash_of_a_macro_aperture_is_placed_at_the_flash_point() {
  // E3b/T10: the macro's coordinates are relative to its own origin, so a flash has to move them.
  // The disc's arcs are centred on the flash point only if the translation happened.
  let gerber = parse(
    "%FSLAX36Y36*%\n%MOMM*%\n%AMPad*\n1,1,2.0,0,0*%\n%ADD10Pad*%\nD10*\n\
     X005000000Y005000000D03*\nM02*\n",
  )
  .expect("fixture parses");
  let image = gerber.image(TOLERANCE).expect("image builds");
  let caps = caps(&image);
  assert!(!caps.is_empty(), "the macro is a circle of arcs");
  assert!(
    caps
      .iter()
      .all(|cap| (cap.center().x - 5.0).abs() < 1.0e-6 && (cap.center().y - 5.0).abs() < 1.0e-6),
    "the pad sits where the flash put it: {:?}",
    caps.iter().map(|cap| cap.center()).collect::<Vec<_>>()
  );
  assert!(
    caps.iter().all(|cap| (cap.radius() - 1.0).abs() < 1.0e-6),
    "diameter 2 is radius 1"
  );
}

#[test]
fn a_macro_aperture_draw_is_refused_rather_than_swept() {
  // Sweeping an arbitrary macro along a segment is a different construction from the capsule and
  // the rectangle this reader builds, so saying so beats guessing at a shape.
  let error = parse(
    "%FSLAX36Y36*%\n%MOMM*%\n%AMPad*\n1,1,2.0,0,0*%\n%ADD10Pad*%\nD10*\n\
     X000000000Y000000000D02*\nG01*\nX010000000Y000000000D01*\nM02*\n",
  )
  .expect("fixture parses")
  .image(TOLERANCE)
  .expect_err("a macro stroke is not built");
  let text = format!("{error}");
  assert!(
    text.contains("Pad") && text.contains("not a stroke"),
    "the refusal names the aperture template: {text}"
  );
}
