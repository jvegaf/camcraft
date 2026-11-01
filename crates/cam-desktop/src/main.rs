//! Native window binary: opens the editor with a sample document and plays its 3D preview.
//!
//! The window itself needs a display, so it is verified by building rather than by automation; the
//! behaviour it exposes lives in `cam-app`, `cam-sim`, `cam-view` and `cam-ui`, all tested headless.
//!
//! The sample preview is a demonstrator, not a feature (M5 V3): a toolpath authored here the way
//! `cam-cli` authors one — the strategy plus its `ToolSpec` — handed to the shell as the stock it has
//! not cut yet plus the program that cuts it. The shell plays it, which is what makes the stock
//! disappear as it cuts (A10); the window never authors a toolpath and never touches the document
//! (D-M5-9). Small and deterministic on purpose: the same moves every launch, so the picture a
//! reviewer sees is the picture the tests describe.
//!
//! The window opens **playing** from the first move: same start, same speed and same moves on every
//! launch, so "deterministic" survives the animation. [`cam_ui::Preview3d`] itself opens paused,
//! which is what a test wants; the demonstrator opts into motion explicitly with
//! [`cam_ui::Preview3d::set_playing`].

use cam_app::AppState;
use cam_geom::{Contour, Point2, Vertex};
use cam_model::{Document, Entity, EntityKind, Job, Layer, PocketParams, PocketStrategy};
use cam_move::{MoveStream, Segment, ToolShape};
use cam_sim::{Heightfield, DEFAULT_CELL};
use cam_toolpath::{pocket, PocketRequest, ToolSpec};
use cam_ui::Shell;
use cam_units::Length;

fn main() -> eframe::Result<()> {
  let options = eframe::NativeOptions {
    viewport: eframe::egui::ViewportBuilder::default()
      .with_inner_size([1440.0, 900.0])
      .with_min_inner_size([800.0, 600.0])
      .with_title("campilot"),
    ..Default::default()
  };
  eframe::run_native(
    "campilot",
    options,
    Box::new(|creation| {
      let mut editor = Editor::new();
      // Publish the tokens before the first frame: this host hands the shell a `Ui` it built itself,
      // so the only moment the theme can reach every widget of frame one is here, on the context.
      editor.shell.publish_theme(&creation.egui_ctx);
      Ok(Box::new(editor))
    }),
  )
}

/// The editor: application state plus the shell, wired together once per frame.
struct Editor {
  app: AppState,
  shell: Shell,
}

impl Editor {
  fn new() -> Self {
    let mut shell = Shell::new();
    let app = AppState::new(sample_document());
    let (stock, program) = sample_preview(app.document());
    // The window hands the shell the stock and the program: the preview plays them, and never
    // authors a toolpath or touches the document (M5 D-M5-9).
    shell.set_preview(stock, program, &app.document().job);
    if let Some(preview) = shell.preview_mut() {
      // Open playing from the first move: the sample cuts itself, which is the point of A10. The
      // start state is the same on every launch, so the animation is as deterministic as the moves.
      preview.set_playing(true);
    }
    shell.status =
      "sample document — click an edge to select it, drag the 3D view to orbit, pause to inspect"
        .to_owned();
    Self { app, shell }
  }
}

impl eframe::App for Editor {
  fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
    let output = self.shell.show_in(&mut self.app, ui);
    // The shell never mutates the document itself; its actions are applied here, and a refusal
    // becomes a status-bar message instead of a silent no-op.
    for action in output.actions {
      // Through the shell, not the bare handler: the shell turns the command's outcome into the
      // sentence the user reads, including created counts, an ellipse's radial error and how many
      // arcs a non-uniform scale cost (M2 A5, D-M2-2).
      self.shell.apply_action(&mut self.app, &action);
    }
  }
}

/// A small document so the window shows something useful on first launch.
fn sample_document() -> Document {
  let mut document = Document::new(Job::default_sheet());

  let rectangle = Contour::from_points(
    &[
      Point2::new(200.0, 200.0),
      Point2::new(700.0, 200.0),
      Point2::new(700.0, 500.0),
      Point2::new(200.0, 500.0),
    ],
    true,
  )
  .expect("rectangle");

  let circle = Contour::new(
    vec![
      Vertex::new(Point2::new(1000.0, 350.0), 1.0),
      Vertex::new(Point2::new(800.0, 350.0), 1.0),
    ],
    true,
  )
  .expect("circle");

  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Sample".to_owned(),
    visible: true,
    entities: vec![
      Entity {
        id: "E1".to_owned(),
        kind: EntityKind::Contour { contour: rectangle },
      },
      Entity {
        id: "E2".to_owned(),
        kind: EntityKind::Contour { contour: circle },
      },
    ],
  });

  document
}

/// The tool the sample program cuts with: a 6 mm end mill, the only tool this demonstrator needs.
fn sample_tool() -> ToolSpec {
  ToolSpec {
    number: 1,
    diameter: Length::from_mm(6.0),
    included_angle: None,
    feed_rate: 100.0,
    plunge_rate: 30.0,
    spindle_speed: 18_000,
  }
}

/// The sample's stock and program: the stock untouched, and the moves that pocket it 6 mm deep.
///
/// This is the whole of what the window demonstrates: a toolpath built the way `cam-cli` builds one,
/// handed to the shell with the stock it has not cut yet. The shell plays it (M5 V3), and because
/// playback applies whole moves as the cursor crosses them, the surface on screen ends up cell for
/// cell the field `sweep::apply` gives the report (A10) — which is the demo's own test below.
///
/// The `expect`s are this file's own contract: the sample geometry, tool and parameters are
/// constants, so a failure here is a defect in the demonstrator rather than a user's mistake (the
/// CLI, which takes a user's document, reports the same failures as messages instead).
fn sample_preview(document: &Document) -> (Heightfield, MoveStream) {
  let tool = sample_tool();
  let contour = document.layers[0].entities[0]
    .kind
    .as_contour()
    .expect("the sample's first entity is a contour");
  let params = PocketParams {
    strategy: PocketStrategy::Offset,
    allowance: Length::from_mm(0.5),
    stepover: Length::from_mm(4.0),
    pass_depth: Length::from_mm(6.0),
    start_depth: Length::ZERO,
    cut_depth: Length::from_mm(6.0),
  };
  let path = pocket(&PocketRequest {
    contour,
    islands: &[],
    params: &params,
    tool: &tool,
    job: &document.job,
  })
  .expect("the sample pocket computes");

  let mut program = MoveStream::new(document.job.safe_z, Point2::new(0.0, 0.0));
  program.segments.push(Segment {
    name: "sample pocket".to_owned(),
    tool_number: tool.number,
    tool: ToolShape::cylinder(tool.diameter),
    feed_rate: tool.feed_rate,
    plunge_rate: tool.plunge_rate,
    spindle_speed: tool.spindle_speed,
    moves: path.moves,
  });

  // The default cell keeps the demonstrator at the resolution the M5 oracle is written for; the
  // window's cell count is bounded by `cam-sim`'s own cap rather than by this call. The field is the
  // untouched stock: the preview cuts it as it plays, and `reset` returns to exactly this.
  let stock = Heightfield::new(&document.job, Length::from_mm(DEFAULT_CELL))
    .expect("the sample stock fits the cell cap");
  (stock, program)
}

#[cfg(test)]
mod tests {
  use super::{sample_document, sample_preview};
  use cam_sim::{sweep, Heightfield, DEFAULT_CELL};
  use cam_units::Length;

  #[test]
  fn the_sample_document_is_valid_and_worth_looking_at() {
    let document = sample_document();
    document
      .validate()
      .expect("the sample must be a valid document");
    assert_eq!(document.entities().count(), 2);
    let layer = &document.layers[0];
    assert_eq!(layer.name, "Sample");
    assert!(layer.entities.iter().all(|entity| entity
      .kind
      .as_contour()
      .map(cam_geom::Contour::is_closed)
      .unwrap_or(false)));
  }

  #[test]
  fn the_sample_stock_is_untouched_and_the_preview_that_plays_it_cuts_it() {
    let document = sample_document();
    let (stock, program) = sample_preview(&document);

    assert!(
      program.move_count() > 0,
      "the window must have a program to preview"
    );
    assert_eq!(
      stock.volumes().removed,
      0.0,
      "the window hands the shell the stock before the first move"
    );
    assert_eq!(
      stock.thickness(),
      document.job.thickness,
      "the preview is simulated over the document's own stock"
    );

    // Playing the sample is what cuts it (A10).
    let mut preview = cam_ui::Preview3d::new(stock, program.clone(), &document.job);
    preview.set_playing(true);
    preview.advance(3_600.0);
    assert!(
      preview.finished(),
      "the sample program must play to its end"
    );
    assert!(
      preview.field().volumes().removed > 0.0,
      "a preview of an uncut stock would show nothing in 3D"
    );

    // And the animation cannot disagree with the report: the preview's end state is the
    // whole-program simulation, cell for cell.
    let mut expected = Heightfield::new(&document.job, Length::from_mm(DEFAULT_CELL))
      .expect("the sample stock fits the cell cap");
    sweep::apply(&program, &mut expected);
    let grid = expected.grid();
    for row in 0..grid.rows {
      for column in 0..grid.columns {
        assert_eq!(
          preview.field().surface(column, row),
          expected.surface(column, row),
          "cell ({column}, {row}) must match the simulation the report is written from"
        );
      }
    }
  }
}
