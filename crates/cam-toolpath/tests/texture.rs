//! Behaviour tests for texture (M6 A5, D-M6-6).
//!
//! A texture is a pattern of parallel grooves across a closed region, and the material between them
//! stands. Each groove is **one pass** at the authored depth with the tool's centre on it, and the tool
//! retracts to the job's safe Z between grooves (D-M4-6, unchanged by M6). The pattern is not this
//! module's: `TextureParams::grooves` derives it once in `cam-model`, and these tests hold the moves to
//! those vertices rather than to a second derivation.

use cam_geom::{Contour, Point2};
use cam_model::{Job, TextureParams};
use cam_move::{Move, Point3};
use cam_toolpath::{texture, TextureRequest, ToolSpec, ToolpathError};
use cam_units::{Angle, Length};

fn rectangle(width: f64, height: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(width, 0.0),
      Point2::new(width, height),
      Point2::new(0.0, height),
    ],
    true,
  )
  .expect("rectangle")
}

fn open_vector(length: f64) -> Contour {
  Contour::from_points(&[Point2::new(0.0, 0.0), Point2::new(length, 0.0)], false)
    .expect("an open vector")
}

fn job() -> Job {
  Job::default_sheet()
}

fn tool(diameter: f64) -> ToolSpec {
  ToolSpec {
    number: 1,
    diameter: Length::from_mm(diameter),
    included_angle: None,
    feed_rate: 100.0,
    plunge_rate: 30.0,
    spindle_speed: 18_000,
  }
}

fn params(cut_depth: f64, start_depth: f64, spacing: f64, degrees: f64) -> TextureParams {
  TextureParams {
    start_depth: Length::from_mm(start_depth),
    cut_depth: Length::from_mm(cut_depth),
    spacing: Length::from_mm(spacing),
    angle: Angle::from_degrees(degrees),
  }
}

fn request<'a>(
  contours: &'a [&'a Contour],
  params: &'a TextureParams,
  tool: &'a ToolSpec,
  job: &'a Job,
) -> TextureRequest<'a> {
  TextureRequest {
    contours,
    params,
    tool,
    job,
  }
}

#[test]
fn each_groove_is_one_pass_at_the_cut_depth() {
  // A 10 x 10 mm square with a 2 mm spacing at 0 degrees: five grooves at y = 1, 3, 5, 7 and 9, each
  // cut in **one** pass — a rapid over its first vertex at safe Z, one plunge there to `-cut_depth`, one
  // feed to its last vertex at the same Z, and a retract. Twenty moves for five grooves, and no second
  // pass at a deeper Z anywhere: a texture's depth is authored, and a pass law would cut each groove
  // twice.
  let region = rectangle(10.0, 10.0);
  let params = params(3.0, 0.0, 2.0, 0.0);
  let job = job();
  let tool = tool(6.0);
  let contours = [&region];
  let moves = texture(&request(&contours, &params, &tool, &job)).expect("a texture of a square");

  assert_eq!(moves.len(), 20, "five grooves, four moves each: {moves:?}");
  for (index, groove) in moves.chunks(4).enumerate() {
    let y = 1.0 + 2.0 * index as f64;
    assert!(
      matches!(
        groove[0],
        Move::Rapid {
          to: Point3 { x, y: move_y, z }
        } if (x.mm() - 0.0).abs() < 1e-9
          && (move_y.mm() - y).abs() < 1e-9
          && z == job.safe_z
      ),
      "groove {index} opens with a rapid over its first vertex at safe Z: {:?}",
      groove[0]
    );
    assert!(
      matches!(
        groove[1],
        Move::Feed {
          to: Point3 { x, y: move_y, z }
        } if (x.mm() - 0.0).abs() < 1e-9
          && (move_y.mm() - y).abs() < 1e-9
          && (z.mm() + 3.0).abs() < 1e-9
      ),
      "groove {index} plunges in place to -cut_depth: {:?}",
      groove[1]
    );
    assert!(
      matches!(
        groove[2],
        Move::Feed {
          to: Point3 { x, y: move_y, z }
        } if (x.mm() - 10.0).abs() < 1e-9
          && (move_y.mm() - y).abs() < 1e-9
          && (z.mm() + 3.0).abs() < 1e-9
      ),
      "groove {index} is one pass at the one depth: {:?}",
      groove[2]
    );
    assert!(
      matches!(
        groove[3],
        Move::Retract {
          to: Point3 { x, y: move_y, z }
        } if (x.mm() - 10.0).abs() < 1e-9
          && (move_y.mm() - y).abs() < 1e-9
          && z == job.safe_z
      ),
      "groove {index} retracts to safe Z: {:?}",
      groove[3]
    );
  }
}

#[test]
fn the_grooves_are_the_patterns_own_vertices() {
  // The moves are held to `TextureParams::grooves`, the one derivation `cam-sim` authorises along too:
  // each groove's two cutting moves end at its polyline's two vertices, at 45 degrees as well as at 0,
  // so neither the strategy nor the target can hold a second version of the pattern.
  for degrees in [0.0, 45.0, 30.0] {
    let region = rectangle(10.0, 10.0);
    let params = params(3.0, 0.0, 2.0, degrees);
    let job = job();
    let tool = tool(6.0);
    let contours = [&region];
    let pattern = params.grooves(&contours);
    assert!(!pattern.is_empty(), "{degrees} degrees has a pattern");
    let moves = texture(&request(&contours, &params, &tool, &job)).expect("a texture");

    assert_eq!(
      moves.len(),
      pattern.len() * 4,
      "one pass per groove at {degrees} degrees"
    );
    for (groove, moves) in pattern.iter().zip(moves.chunks(4)) {
      let start = moves[1].destination();
      let end = moves[2].destination();
      assert!(
        (start.x.mm() - groove[0].x).abs() < 1e-9 && (start.y.mm() - groove[0].y).abs() < 1e-9,
        "the plunge is on the pattern's first vertex: {start:?} against {:?}",
        groove[0]
      );
      assert!(
        (end.x.mm() - groove[1].x).abs() < 1e-9 && (end.y.mm() - groove[1].y).abs() < 1e-9,
        "and the pass ends on its last: {end:?} against {:?}",
        groove[1]
      );
    }
  }
}

#[test]
fn every_groove_retracts_to_safe_z_before_the_next_one_starts() {
  // D-M4-6's linking rule, unchanged by M6: staying down between grooves would need a gouge-free link,
  // and the material between grooves is meant to stand — the tool would drag through it. So the last
  // move of one groove is a retract to `safe_z` and the first move of the next is a rapid at `safe_z`.
  let region = rectangle(40.0, 30.0);
  let params = params(2.0, 0.0, 5.0, 0.0);
  let job = Job::default_sheet();
  let tool = tool(6.0);
  let contours = [&region];
  let moves = texture(&request(&contours, &params, &tool, &job)).expect("a texture");

  let grooves = moves.len() / 4;
  assert!(grooves > 1, "the fixture has several grooves: {moves:?}");
  for index in 1..grooves {
    let previous_end = moves[index * 4 - 1];
    let next_start = moves[index * 4];
    assert!(
      matches!(previous_end, Move::Retract { to } if to.z == job.safe_z),
      "groove {} ends clear of the material: {previous_end:?}",
      index - 1
    );
    assert!(
      matches!(next_start, Move::Rapid { to } if to.z == job.safe_z),
      "groove {index} starts clear of it: {next_start:?}"
    );
  }
}

#[test]
fn an_open_contour_is_refused_by_name() {
  // A pattern is a region's, and an open vector bounds none. The refusal is
  // `ToolpathError::OpenContour`, the sentence every closed-geometry strategy already speaks.
  let line = open_vector(10.0);
  let params = params(3.0, 0.0, 2.0, 0.0);
  let job = job();
  let tool = tool(6.0);
  let contours = [&line];
  let error = texture(&request(&contours, &params, &tool, &job)).expect_err("must refuse");
  assert_eq!(error, ToolpathError::OpenContour);
}

#[test]
fn a_non_positive_spacing_is_refused_by_name_rather_than_cut_as_an_empty_pattern() {
  // A spacing with no distance behind it is an input error, not a pattern that happens to be empty:
  // an empty program would look like a texture that ran and found nothing to do (D-M6-6).
  let region = rectangle(10.0, 10.0);
  let job = job();
  let tool = tool(6.0);
  let contours = [&region];
  for spacing in [0.0, -2.0] {
    let params = params(3.0, 0.0, spacing, 0.0);
    let error = texture(&request(&contours, &params, &tool, &job)).expect_err("must refuse");
    assert_eq!(
      error,
      ToolpathError::InvalidSpacing { spacing },
      "{error:?}"
    );
    assert!(
      error.to_string().contains(&spacing.to_string()),
      "the refusal names the value: {error}"
    );
  }
}

#[test]
fn a_spacing_with_no_groove_across_the_region_is_refused_by_name() {
  // A 10 mm square with a 100 mm spacing has no groove in it: the lattice's first row is half a spacing
  // in, which is 50 mm past the region. Refused by name, with both numbers, rather than emitted as a
  // program with no moves.
  let region = rectangle(10.0, 10.0);
  let params = params(3.0, 0.0, 100.0, 0.0);
  let job = job();
  let tool = tool(6.0);
  let contours = [&region];
  let error = texture(&request(&contours, &params, &tool, &job)).expect_err("must refuse");
  assert_eq!(
    error,
    ToolpathError::NoGroove {
      spacing: 100.0,
      angle: 0.0,
    }
  );
  assert!(
    error.to_string().contains("100") && error.to_string().contains("0 degrees"),
    "the refusal names the spacing and the angle: {error}"
  );
}

#[test]
fn a_cut_depth_at_or_above_the_start_depth_is_refused_by_name() {
  // The rule an engrave and a flute inherit from the profile: a pass whose cut depth does not extend
  // below its start depth has no pass behind it. Checked before the pattern so the number named is the
  // depth and not a groove.
  let region = rectangle(10.0, 10.0);
  let job = job();
  let tool = tool(6.0);
  let contours = [&region];
  for (cut, start) in [(3.0, 3.0), (2.0, 3.0)] {
    let params = params(cut, start, 2.0, 0.0);
    let error = texture(&request(&contours, &params, &tool, &job)).expect_err("must refuse");
    assert_eq!(
      error,
      ToolpathError::InvalidDepthRange {
        cut_depth: cut,
        start_depth: start,
      }
    );
  }
}

#[test]
fn two_calls_for_one_request_emit_the_same_moves() {
  // S6: nothing on this path iterates a map or a set, and the pattern is a plain vector in a fixed
  // order, so the second call is the first one.
  let region = rectangle(10.0, 10.0);
  let params = params(3.0, 0.0, 2.0, 45.0);
  let job = job();
  let tool = tool(6.0);
  let contours = [&region];
  let first = texture(&request(&contours, &params, &tool, &job)).expect("a texture");
  let second = texture(&request(&contours, &params, &tool, &job)).expect("a texture");
  assert_eq!(first, second);
}
