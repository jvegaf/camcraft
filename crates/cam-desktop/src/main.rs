//! Native window binary: opens the editor with a sample document and its 3D preview.
//!
//! The window itself needs a display, so it is verified by building rather than by automation; the
//! behaviour it exposes lives in `cam-app`, `cam-sim`, `cam-view` and `cam-ui`, all tested headless.
//!
//! The sample preview is a demonstrator, not a feature (M5 V2): a toolpath authored here the way
//! `cam-cli` authors one — the strategy plus its `ToolSpec` — simulated with `cam-sim` and handed to
//! the shell, which only draws it (D-M5-9). Small and deterministic on purpose: the same moves every
//! launch, so the picture a reviewer sees is the picture the tests describe.

use cam_app::AppState;
use cam_geom::{Contour, Point2, Vertex};
use cam_model::{Document, Entity, EntityKind, Job, Layer, PocketParams, PocketStrategy};
use cam_move::{MoveStream, Segment};
use cam_sim::{sweep, Heightfield, DEFAULT_CELL};
use cam_toolpath::{pocket, PocketRequest, ToolSpec};
use cam_ui::Shell;
use cam_units::Length;

fn main() -> eframe::Result<()> {
  let options = eframe::NativeOptions {
    viewport: eframe::egui::ViewportBuilder::default()
      .with_inner_size([1280.0, 800.0])
      .with_min_inner_size([800.0, 600.0])
      .with_title("campilot"),
    ..Default::default()
  };
  eframe::run_native(
    "campilot",
    options,
    Box::new(|_creation| Ok(Box::new(Editor::new()))),
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
    let (field, program) = sample_preview(app.document());
    // The window hands the shell a finished simulation: the preview never authors toolpaths and
    // never touches the document (M5 D-M5-9).
    shell.set_preview(field, program, &app.document().job);
    shell.status =
      "sample document — click an edge to select it, drag the 3D view to orbit".to_owned();
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
    feed_rate: 100.0,
    plunge_rate: 30.0,
    spindle_speed: 18_000,
  }
}

/// The sample program, and the stock it leaves behind: the rectangle pocketed 6 mm deep.
///
/// This is the whole of what the window demonstrates: a toolpath built the way `cam-cli` builds one,
/// simulated with `cam-sim`, and handed to the shell to draw. Deterministic on purpose — same tool,
/// same parameters, same moves on every launch.
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
    tool_diameter: tool.diameter,
    feed_rate: tool.feed_rate,
    plunge_rate: tool.plunge_rate,
    spindle_speed: tool.spindle_speed,
    moves: path.moves,
  });

  // The default cell keeps the demonstrator at the resolution the M5 oracle is written for; the
  // window's cell count is bounded by `cam-sim`'s own cap rather than by this call.
  let mut field = Heightfield::new(&document.job, Length::from_mm(DEFAULT_CELL))
    .expect("the sample stock fits the cell cap");
  sweep::apply(&program, &mut field);
  (field, program)
}

#[cfg(test)]
mod tests {
  use super::{sample_document, sample_preview};

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
  fn the_sample_preview_actually_cuts_the_sample_stock() {
    let document = sample_document();
    let (field, program) = sample_preview(&document);

    assert!(
      program.move_count() > 0,
      "the window must have a program to preview"
    );
    assert!(
      field.volumes().removed > 0.0,
      "a preview of an uncut stock would show nothing in 3D"
    );
    assert_eq!(
      field.thickness(),
      document.job.thickness,
      "the preview is simulated over the document's own stock"
    );
  }
}
