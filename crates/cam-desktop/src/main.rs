//! Native window binary: opens the editor with a sample document.
//!
//! The window itself needs a display, so it is verified by building rather than by automation; the
//! behaviour it exposes lives in `cam-app`, `cam-view` and `cam-ui`, all tested headless.

use cam_app::AppState;
use cam_geom::{Contour, Point2, Vertex};
use cam_model::{Document, Entity, EntityKind, Job, Layer};
use cam_ui::Shell;

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
    shell.status = "sample document — click an edge to select it".to_owned();
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

#[cfg(test)]
mod tests {
  use super::sample_document;

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
}
