//! Behaviour tests for the Gerber RS-274X reader.

use cam_geom::Point2;
use cam_gerber::{parse, Command, Format, GerberError, MacroPrimitive, Units, ZeroSuppression};

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
  assert_eq!(
    file.commands.first().copied(),
    Some(Command::SelectAperture(10))
  );
  // Polarity came through as dark in the attribute stream.
  let joined = file.attributes.join("\n");
  assert!(
    joined.contains("LPD") || joined.contains("%LPD"),
    "dark polarity in attribute stream: {:?}",
    file.attributes
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
