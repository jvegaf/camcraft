//! The stock heightfield: its grid, its removal rule and the volumes it reports (M5 A1, A3, A6).

use cam_model::{Job, OriginCorner};
use cam_sim::{grid_for, GridError, Heightfield, DEFAULT_CELL, MAX_CELLS};
use cam_units::Length;

fn job(width: f64, height: f64, thickness: f64) -> Job {
  Job {
    width: Length::from_mm(width),
    height: Length::from_mm(height),
    thickness: Length::from_mm(thickness),
    origin: OriginCorner::BottomLeft,
    side_up: cam_model::BoardSideUp::Top,
  }
}

fn square(extent: f64, thickness: f64) -> Job {
  job(extent, extent, thickness)
}

#[test]
fn a_default_sheet_at_the_default_cell_stays_under_the_cap() {
  let grid =
    grid_for(&Job::default_sheet(), Length::from_mm(DEFAULT_CELL)).expect("a default sheet fits");
  assert_eq!(grid.columns, 1219);
  assert_eq!(grid.rows, 2438);
  assert!(
    grid.cells() <= MAX_CELLS,
    "{} cells is over the {} cap",
    grid.cells(),
    MAX_CELLS
  );
}

#[test]
fn a_cell_finer_than_the_cap_is_refused_by_name() {
  let error =
    grid_for(&Job::default_sheet(), Length::from_mm(0.1)).expect_err("0.1 mm is too fine");
  match error {
    GridError::TooFine {
      cell,
      columns,
      rows,
      cap,
    } => {
      assert!((cell - 0.1).abs() < 1e-12, "cell {cell}");
      assert_eq!(columns, 12190);
      assert_eq!(rows, 24380);
      assert_eq!(cap, MAX_CELLS);
    }
    other => panic!("expected TooFine, got {other:?}"),
  }
  let message = grid_for(&Job::default_sheet(), Length::from_mm(0.1))
    .expect_err("0.1 mm is too fine")
    .to_string();
  assert!(message.contains("12190"), "{message}");
  assert!(message.contains("24380"), "{message}");
  assert!(message.contains("4000000"), "{message}");
}

#[test]
fn a_zero_sized_job_is_degenerate_not_a_panic() {
  let error = grid_for(&job(0.0, 10.0, 5.0), Length::from_mm(1.0)).expect_err("no width");
  assert!(matches!(error, GridError::Degenerate { .. }), "{error:?}");
  let error = grid_for(&square(10.0, 5.0), Length::from_mm(0.0)).expect_err("no cell");
  assert!(matches!(error, GridError::Degenerate { .. }), "{error:?}");
}

#[test]
fn a_ten_millimetre_square_has_cells_whose_centres_are_half_steps() {
  let field =
    Heightfield::new(&square(10.0, 5.0), Length::from_mm(1.0)).expect("a small sheet fits");
  let grid = field.grid();
  assert_eq!(grid.columns, 10);
  assert_eq!(grid.rows, 10);
  assert_eq!(grid.cells(), 100);
  assert!((grid.cell.mm() - 1.0).abs() < 1e-12);
  let first = field.cell_centre(0, 0);
  let last = field.cell_centre(9, 9);
  assert!(
    (first.x - 0.5).abs() < 1e-12 && (first.y - 0.5).abs() < 1e-12,
    "{first:?}"
  );
  assert!(
    (last.x - 9.5).abs() < 1e-12 && (last.y - 9.5).abs() < 1e-12,
    "{last:?}"
  );
}

#[test]
fn an_untouched_field_is_the_stock_and_has_removed_nothing() {
  let field = Heightfield::new(&square(10.0, 5.0), Length::from_mm(1.0)).expect("fits");
  let volumes = field.volumes();
  assert_eq!(volumes.removed, 0.0);
  assert_eq!(volumes.through_cells, 0);
  assert!(
    (volumes.remaining - 10.0 * 10.0 * 5.0).abs() < 1e-9,
    "{}",
    volumes.remaining
  );
  assert_eq!(field.surface(0, 0), 0.0);
}

#[test]
fn lowering_a_cell_twice_keeps_the_deeper_value() {
  let mut field = Heightfield::new(&square(10.0, 5.0), Length::from_mm(1.0)).expect("fits");
  field.lower(3, 4, -2.0);
  field.lower(3, 4, -1.0);
  assert_eq!(field.surface(3, 4), -2.0);
  field.lower(3, 4, -4.0);
  assert_eq!(field.surface(3, 4), -4.0);
}

#[test]
fn a_cell_never_goes_below_the_stock_bottom() {
  let mut field = Heightfield::new(&square(10.0, 5.0), Length::from_mm(1.0)).expect("fits");
  field.lower(1, 1, -100.0);
  assert_eq!(field.surface(1, 1), -5.0);
  let volumes = field.volumes();
  assert_eq!(volumes.through_cells, 1);
}

#[test]
fn a_field_cut_to_the_bottom_reports_every_cell_through_and_nothing_left() {
  let mut field = Heightfield::new(&square(10.0, 5.0), Length::from_mm(1.0)).expect("fits");
  for column in 0..10 {
    for row in 0..10 {
      field.lower(column, row, -5.0);
    }
  }
  let volumes = field.volumes();
  assert_eq!(volumes.through_cells, 100);
  assert_eq!(volumes.remaining, 0.0);
  assert!(
    (volumes.removed - 500.0).abs() < 1e-9,
    "{}",
    volumes.removed
  );
}

#[test]
fn a_partly_cut_field_reports_the_depth_it_lost() {
  let mut field = Heightfield::new(&square(10.0, 5.0), Length::from_mm(1.0)).expect("fits");
  for column in 0..5 {
    for row in 0..5 {
      field.lower(column, row, -2.0);
    }
  }
  let volumes = field.volumes();
  assert!(
    (volumes.removed - 25.0 * 2.0).abs() < 1e-9,
    "{}",
    volumes.removed
  );
  assert!(
    (volumes.remaining - (500.0 - 50.0)).abs() < 1e-9,
    "{}",
    volumes.remaining
  );
}

#[test]
fn a_grid_that_is_not_a_whole_number_of_cells_covers_the_whole_job() {
  let field = Heightfield::new(&job(10.2, 10.0, 5.0), Length::from_mm(1.0)).expect("fits");
  let grid = field.grid();
  assert_eq!(grid.columns, 11);
  assert_eq!(grid.rows, 10);
  let last = field.cell_centre(10, 0);
  assert!(
    last.x > 10.2,
    "the last cell's centre {last:?} is inside the job"
  );
}

proptest::proptest! {
  /// The two volumes are one volume split in two, whatever sequence of cuts happened (A3).
  #[test]
  fn removed_plus_remaining_is_the_grid_volume(
    cuts in proptest::collection::vec((0usize..9, 0usize..9, -20.0f64..0.0), 0..40)
  ) {
    let mut field = Heightfield::new(&square(9.0, 5.0), Length::from_mm(1.0)).expect("fits");
    for (column, row, z) in cuts {
      if column < field.grid().columns && row < field.grid().rows {
        field.lower(column, row, z);
      }
    }
    let volumes = field.volumes();
    let grid = field.grid();
    let total = grid.cells() as f64 * grid.cell.mm() * grid.cell.mm() * 5.0;
    proptest::prop_assert!((volumes.removed + volumes.remaining - total).abs() < 1e-9,
      "removed {} + remaining {} != {total}", volumes.removed, volumes.remaining);
    proptest::prop_assert!(volumes.removed >= 0.0);
    proptest::prop_assert!(volumes.remaining >= 0.0);
  }
}
