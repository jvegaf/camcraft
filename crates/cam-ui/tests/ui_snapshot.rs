//! Offscreen render harness for the tool-library modal (tool-library-app tooling).
//!
//! `#[ignore]`d so the normal suite is unaffected. Regenerate every shot with:
//!
//! ```sh
//! cargo test -p cam-ui --test ui_snapshot -- --ignored --nocapture
//! ```
//!
//! PNGs are written to `target/ui-snapshots/<name>.png` at 1240 x 1000 logical pixels
//! (1:1 pixels-per-point), which is the size the reference frame was captured at.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{
  Document, Entity, EntityKind, Job, Layer, LayerRole, Operation, ProfileParams, Toolpath,
};
use cam_store::tools::{stored_tool_for_number, StoredTool};
use cam_ui::{Shell, UiAction};
use egui_kittest::Harness;

static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Where this test writes: a fresh directory in the system temp directory, removed when dropped.
///
/// Outside the worktree and never the developer's real data directory (`CAM_PILOT_DATA_DIR` is
/// unset by the caller that builds the shell).
struct Scratch {
  path: PathBuf,
}

impl Scratch {
  fn new(name: &str) -> Self {
    let n = SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
      "campilot-ui-snapshot-{}-{name}-{n}",
      std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("scratch directory");
    Self { path }
  }

  fn path(&self) -> &Path {
    &self.path
  }
}

impl Drop for Scratch {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.path);
  }
}

fn document_with_toolpath(tool_number: u32) -> Document {
  let mut document = Document::new(Job::default_sheet());
  let contour = Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(100.0, 0.0),
      Point2::new(100.0, 60.0),
      Point2::new(0.0, 60.0),
    ],
    true,
  )
  .expect("rectangle");
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
    role: LayerRole::Geometry,
    visible: true,
    entities: vec![Entity {
      id: "E1".to_owned(),
      kind: EntityKind::Contour { contour },
    }],
  });
  document.toolpaths.push(Toolpath {
    id: "P1".to_owned(),
    name: "Perfil 01".to_owned(),
    entity_ids: vec!["E1".to_owned()],
    island_ids: Vec::new(),
    tool_number,
    operation: Operation::Profile(ProfileParams::default()),
  });
  document
}

fn out_dir() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots")
}

/// Drive the shell/app pair for a few frames so egui settles, render, and write `name`.png.
/// Prints the absolute path, byte size and pixel dimensions.
fn render_shot(shell: Shell, app: AppState, name: &str) -> (PathBuf, u64, (u32, u32)) {
  let mut harness = Harness::builder()
    .with_size(egui::Vec2::new(1240.0, 1000.0))
    .with_pixels_per_point(1.0)
    .with_accessibility_check(false)
    .allow_missing_glyphs()
    .with_context_setup(|ctx| {
      ctx.set_fonts(egui::FontDefinitions::default());
    })
    .wgpu()
    .build_ui_state(
      |ui, (shell, app)| {
        shell.show_in(app, ui);
      },
      (shell, app),
    );

  // Run several steps so layout, fonts and the modal settle.
  for _ in 0..8 {
    harness.step();
  }
  harness.run();
  // A final step after run to ensure the settled frame is rendered.
  harness.step();

  let image = harness
    .render()
    .expect("render the tool-library modal to an image");

  let dir = out_dir();
  std::fs::create_dir_all(&dir).expect("create target/ui-snapshots");
  let path = dir.join(format!("{name}.png"));
  image::DynamicImage::ImageRgba8(image.clone())
    .save_with_format(&path, image::ImageFormat::Png)
    .expect("write snapshot PNG");
  let abs = std::fs::canonicalize(&path).expect("absolute snapshot path");
  let bytes = std::fs::metadata(&path).expect("snapshot metadata").len();
  let dims = image.dimensions();
  println!(
    "ui-snapshot: {} ({} bytes, {}x{} px)",
    abs.display(),
    bytes,
    dims.0,
    dims.1
  );
  (abs, bytes, dims)
}

/// Open a fresh factory library in `scratch` and the modal for a document whose toolpath cuts
/// with `tool_number`, so the footer can offer `Use this tool` and the selection starts on it.
fn open_modal(tool_number: u32) -> (Shell, AppState, Scratch) {
  // The harness must not depend on the developer's real data directory.
  std::env::remove_var("CAM_PILOT_DATA_DIR");
  let scratch = Scratch::new("shot");
  let mut app = AppState::new(document_with_toolpath(tool_number));
  let mut shell = Shell::new();
  shell
    .open_library_at(scratch.path())
    .expect("open a fresh factory library in the scratch data directory");
  shell.open_tool_library(&app);
  let _ = &mut app;
  (shell, app, scratch)
}

#[test]
#[ignore = "offscreen UI snapshot — regenerate with `cargo test -p cam-ui --test ui_snapshot -- --ignored --nocapture`"]
fn tool_library_t22() {
  let (shell, app, _scratch) = open_modal(22);
  let (_path, bytes, dims) = render_shot(shell, app, "tool-library-t22");
  assert!(
    bytes > 1024,
    "T22 snapshot is unexpectedly small: {bytes} bytes"
  );
  assert_eq!(dims, (1240, 1000), "T22 snapshot dimensions");
}

#[test]
#[ignore = "offscreen UI snapshot — regenerate with `cargo test -p cam-ui --test ui_snapshot -- --ignored --nocapture`"]
fn tool_library_vbit() {
  // T4 is "V-bit · 90° · 12.7 mm" in the factory catalogue.
  let (shell, app, _scratch) = open_modal(4);
  let (_path, bytes, dims) = render_shot(shell, app, "tool-library-vbit");
  assert!(
    bytes > 1024,
    "V-bit snapshot is unexpectedly small: {bytes} bytes"
  );
  assert_eq!(dims, (1240, 1000), "V-bit snapshot dimensions");
}

#[test]
#[ignore = "offscreen UI snapshot — regenerate with `cargo test -p cam-ui --test ui_snapshot -- --ignored --nocapture`"]
fn tool_library_import_question() {
  std::env::remove_var("CAM_PILOT_DATA_DIR");
  let scratch = Scratch::new("import-shot");
  let mut app = AppState::new(document_with_toolpath(1));
  let mut shell = Shell::new();
  shell
    .open_library_at(scratch.path())
    .expect("open a fresh factory library in the scratch data directory");
  shell.open_tool_library(&app);

  // Stage a valid tool-library import so the question opens over the modal, with no answer.
  let sample: Vec<StoredTool> = vec![{
    // Reuse the factory T22 as a valid, self-contained tool payload.
    let tools = cam_store::tools::ToolLibrary::open_read_only(scratch.path().join("tools.sqlite"))
      .expect("read factory library")
      .tools()
      .expect("list tools");
    stored_tool_for_number(&tools, 22)
      .cloned()
      .expect("factory T22 present")
  }];
  let json = cam_store::tool_file::to_json(&sample).expect("serialize import JSON");
  let import_path = scratch.path().join("import.json");
  std::fs::write(&import_path, json).expect("write import file");

  shell.apply_action(&mut app, &UiAction::ImportTools { path: import_path });

  let (_path, bytes, dims) = render_shot(shell, app, "tool-library-import-question");
  assert!(
    bytes > 1024,
    "import-question snapshot is unexpectedly small: {bytes} bytes"
  );
  assert_eq!(dims, (1240, 1000), "import-question snapshot dimensions");
}
