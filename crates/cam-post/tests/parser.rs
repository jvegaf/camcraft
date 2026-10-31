//! Behaviour tests for the post-processor parser (A5, A7).
//!
//! The fixture below is authored for this project: no the vendor `.pp` file is vendored (A7). It covers
//! all thirteen directives and all twenty output blocks — the four helical kinds M6 added included, so
//! `every_block_is_parsed` still means what it says (D-M6-8).

use cam_post::{parse_post, BlockKind, EmissionMode, PostUnits};

const FIXTURE: &str = r#"
+ CamPilot fixture post-processor
+ Covers every directive and every output block of the dialect.
POST_NAME = "Fixture Mach3 Arcs (mm) (*.nc)"
FILE_EXTENSION = "nc"
UNITS = "MM"
LINE_ENDING = "[13][10]"
LINE_NUMBER_START = 0
LINE_NUMBER_INCREMENT = 10
LINE_NUMBER_MAXIMUM = 999999
DIRECT_OUTPUT = "Mach|Mach4.Document"
SUBSTITUTE = "({)}"
ROTARY_WRAP_X = "-A"
ROTARY_WRAP_Y = "-A"
RAPID_PLUNGE_TO_STARTZ = NO
PRINT_DIRECT = "YES"

VAR LINE_NUMBER = [N|A|N|1.0]
VAR SPINDLE_SPEED = [S|A|S|1.0]
VAR TOOL_NUMBER = [T|A||1.0]
VAR FEED_RATE = [F|C|F|1.1]
VAR PLUNGE_RATE = [FP|C|F|1.1]
VAR CUT_RATE = [FC|C|F|1.1|0.0166]
VAR X_POSITION = [X|C|X|1.4]
VAR Y_POSITION = [Y|C|Y|1.4]
VAR Z_POSITION = [Z|C|Z|1.4]
VAR X_HOME_POSITION = [XH|A|X|1.4]
VAR Y_HOME_POSITION = [YH|A|Y|1.4]
VAR Z_HOME_POSITION = [ZH|A|Z|1.4]
VAR SAFE_Z_HEIGHT = [SAFEZ|A|Z|1.4]
VAR ARC_CENTRE_I_INC_POSITION = [I|A|I|1.4]
VAR ARC_CENTRE_J_INC_POSITION = [J|A|J|1.4]
VAR ARC_RADIUS = [Radius|A|U|1.4]
VAR WRAP_DIAMETER = [WRAP_DIA|A||1.4]

begin HEADER
"( [TOOLPATH_NAME] )"
"( [POST_NAME] )"
"[N]G00G17G90G21"
"[N]T[T]M06"
"[N][S]M03"

begin INITIAL_RAPID_MOVE
"[N]G00[X][Y]"

begin FIRST_RAPID_MOVE
"[N]G00[X][Y]"

begin RAPID_MOVE
"[N]G00[X][Y]"

begin PLUNGE_MOVE
"[N]G01[Z][F]"

begin FIRST_FEED_MOVE
"[N]G01[X][Y][Z][F]"

begin FEED_MOVE
"[N]G01[X][Y][Z]"

begin RETRACT_MOVE
"[N]G00[Z]"

begin FIRST_CW_ARC_MOVE
"[N]G02[X][Y][I][J][F]"

begin CW_ARC_MOVE
"[N]G02[X][Y][I][J]"

begin FIRST_CCW_ARC_MOVE
"[N]G03[X][Y][I][J][F]"

begin CCW_ARC_MOVE
"[N]G03[X][Y][I][J]"

begin FIRST_CW_HELIX_MOVE
"[N]G02[X][Y][Z][I][J][F]"

begin CW_HELIX_MOVE
"[N]G02[X][Y][Z][I][J]"

begin FIRST_CCW_HELIX_MOVE
"[N]G03[X][Y][Z][I][J][F]"

begin CCW_HELIX_MOVE
"[N]G03[X][Y][Z][I][J]"

begin FEED_RATE_CHANGE
"[N][F]"

begin NEW_SEGMENT
"[N]G00[Z]"

begin TOOLCHANGE
"[N]M05"
"[N]T[T]M06"

begin FOOTER
"[N]G00[ZH]"
"[N]G00[XH][YH]"
"[N]M05"
"[N]M30"
"#;

#[test]
fn every_directive_is_parsed() {
  let post = parse_post(FIXTURE).expect("fixture parses");
  assert_eq!(post.post_name, "Fixture Mach3 Arcs (mm) (*.nc)");
  assert_eq!(post.file_extension, "nc");
  assert_eq!(post.units, PostUnits::Millimetres);
  assert_eq!(post.line_ending, "[13][10]");
  assert_eq!(post.line_number_start, Some(0));
  assert_eq!(post.line_number_increment, Some(10));
  assert_eq!(post.line_number_maximum, Some(999_999));
  assert_eq!(post.direct_output.as_deref(), Some("Mach|Mach4.Document"));
  assert_eq!(post.substitute.as_deref(), Some("({)}"));
  assert_eq!(post.rotary_wrap_x.as_deref(), Some("-A"));
  assert_eq!(post.rotary_wrap_y.as_deref(), Some("-A"));
  assert!(!post.rapid_plunge_to_start_z);
  assert!(post.print_direct);
}

#[test]
fn every_block_is_parsed() {
  let post = parse_post(FIXTURE).expect("fixture parses");
  assert_eq!(
    post.blocks.len(),
    BlockKind::ALL.len(),
    "the fixture defines every block"
  );
  for kind in BlockKind::ALL {
    assert!(!post.block(kind).is_empty(), "missing block {kind:?}");
  }
  assert_eq!(post.block(BlockKind::Header).len(), 5);
  assert_eq!(post.block(BlockKind::Footer).len(), 4);
}

#[test]
fn the_dialect_decimal_spelling_is_decoded() {
  let post = parse_post(FIXTURE).expect("fixture parses");
  // "1.4" means four decimals; the integer part is a fixed marker, not a value.
  assert_eq!(post.variables["X_POSITION"].decimals, 4);
  assert_eq!(post.variables["FEED_RATE"].decimals, 1);
  assert_eq!(post.variables["LINE_NUMBER"].decimals, 0);
}

#[test]
fn variable_modes_and_prefixes_are_kept() {
  let post = parse_post(FIXTURE).expect("fixture parses");
  assert_eq!(post.variables["FEED_RATE"].mode, EmissionMode::OnChange);
  assert_eq!(post.variables["LINE_NUMBER"].mode, EmissionMode::Always);
  assert_eq!(post.variables["X_POSITION"].prefix, "X");
  assert_eq!(post.variables["WRAP_DIAMETER"].prefix, "");
}

#[test]
fn the_optional_scale_field_is_parsed() {
  let post = parse_post(FIXTURE).expect("fixture parses");
  let spec = &post.variables["CUT_RATE"];
  assert!((spec.scale - 0.0166).abs() < 1e-12, "{}", spec.scale);
  assert_eq!(post.variables["FEED_RATE"].scale, 1.0);
}

#[test]
fn comments_and_blank_lines_are_ignored() {
  let post = parse_post("\n+\n| a pipe comment\nPOST_NAME = \"x\"\n\n").expect("parses");
  assert_eq!(post.post_name, "x");
  assert!(post.blocks.is_empty());
}

#[test]
fn an_unknown_directive_reports_its_line() {
  let text = "POST_NAME = \"x\"\nFILE_EXTENSION = \"nc\"\nNOT_A_DIRECTIVE = \"y\"\n";
  let error = parse_post(text).expect_err("must reject");
  assert_eq!(error.line, 3, "{error}");
  assert!(error.message.contains("NOT_A_DIRECTIVE"), "{error}");
  assert!(error.message.contains("POST_NAME"), "{error}");
}

#[test]
fn an_unknown_block_reports_its_line() {
  let text = "POST_NAME = \"x\"\nbegin NOT_A_BLOCK\n";
  let error = parse_post(text).expect_err("must reject");
  assert_eq!(error.line, 2);
  assert!(error.message.contains("NOT_A_BLOCK"), "{error}");
  assert!(error.message.contains("HEADER"), "{error}");
}

#[test]
fn a_var_with_wrong_arity_is_rejected() {
  let error = parse_post("VAR X_POSITION = [X|C|X]\n").expect_err("three fields");
  assert!(error.line == 1);
  assert!(error.message.contains("4"), "{error}");

  let error = parse_post("VAR X_POSITION = [X|C|X|1.4|1|9]\n").expect_err("six fields");
  assert!(error.message.contains("6 field"), "{error}");
}

#[test]
fn an_invalid_mode_is_rejected() {
  let error = parse_post("VAR X_POSITION = [X|Z|X|1.4]\n").expect_err("bad mode");
  assert!(error.message.contains("expected A"), "{error}");
  assert!(error.message.contains("\"Z\""), "{error}");
}

#[test]
fn a_non_alphabetic_prefix_is_rejected() {
  let error = parse_post("VAR X_POSITION = [X|C|1X|1.4]\n").expect_err("bad prefix");
  assert!(error.message.contains("letters only"), "{error}");
}

#[test]
fn a_non_numeric_decimal_field_is_rejected() {
  let error = parse_post("VAR X_POSITION = [X|C|X|1.x]\n").expect_err("bad decimals");
  assert!(error.message.contains("digit count"), "{error}");
}

#[test]
fn a_duplicate_variable_is_rejected_instead_of_silently_overwritten() {
  let text = "VAR X_POSITION = [X|C|X|1.4]\nVAR X_POSITION = [X|A|X|1.3]\n";
  let error = parse_post(text).expect_err("must reject");
  assert_eq!(error.line, 2);
  assert!(error.message.contains("duplicate"), "{error}");
}

#[test]
fn an_output_line_outside_a_block_is_rejected() {
  let error = parse_post("POST_NAME = \"x\"\n\"G00X0\"\n").expect_err("must reject");
  assert_eq!(error.line, 2);
  assert!(error.message.contains("begin block"), "{error}");
}

#[test]
fn an_unquoted_directive_value_is_rejected() {
  let error = parse_post("POST_NAME = Mach3\n").expect_err("must reject");
  assert!(error.message.contains("quoted"), "{error}");
}

#[test]
fn an_unknown_units_value_is_rejected() {
  let error = parse_post("UNITS = \"FURLONGS\"\n").expect_err("must reject");
  assert!(error.message.contains("FURLONGS"), "{error}");
  let post = parse_post("UNITS = \"INCHES\"\n").expect("inches accepted");
  assert_eq!(post.units, PostUnits::Inches);
}

#[test]
fn flags_accept_both_yes_no_and_quoted_forms() {
  let post = parse_post("RAPID_PLUNGE_TO_STARTZ = YES\nPRINT_DIRECT = \"NO\"\n").expect("parses");
  assert!(post.rapid_plunge_to_start_z);
  assert!(!post.print_direct);
}

#[test]
fn a_block_content_line_is_unquoted() {
  let post = parse_post("begin HEADER\n\"G00 X0 Y0\"\n").expect("parses");
  assert_eq!(post.block(BlockKind::Header), ["G00 X0 Y0".to_owned()]);
}
