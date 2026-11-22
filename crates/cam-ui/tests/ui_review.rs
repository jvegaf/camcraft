//! Automatic visual review for the Floating Studio shell (design/camcraft-workbench-v2/02-floating-studio.html).
//!
//! Renders the real egui shell with `egui_kittest` (wgpu) and compares against golden PNGs.
//! The goldens catch visual regressions in implemented UI. They do NOT prove pixel-perfect parity
//! with the HTML artboard: the artboard is the visual spec, and roadmap gaps (E6.x) are documented
//! in `tests/goldens/floating-studio/manifest.json`.
//!
//! Run:    just ui-review
//! Update: UPDATE_UI_GOLDENS=1 just ui-review

use std::path::{Path, PathBuf};

use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{
  Document, Entity, EntityKind, IsolationKind, IsolationParams, Job, Layer, LayerRole, MillingDirection,
  Operation, Provenance, Side, Toolpath,
};
use cam_ui::{RecentProject, Shell};
use cam_units::Length;
use egui_kittest::Harness;

const WIDTH: f32 = 1440.0;
const HEIGHT: f32 = 900.0;

fn golden_dir() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/floating-studio")
}

fn diff_dir() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-review-diffs")
}

fn manifest_path() -> PathBuf {
  golden_dir().join("manifest.json")
}

fn document_for_floating_shell() -> Document {
  let mut document = Document::new(Job::default_sheet());
  let contour = Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(100.0, 0.0),
      Point2::new(100.0, 80.0),
      Point2::new(0.0, 80.0),
    ],
    true,
  )
  .expect("board outline");

  document.layers.push(Layer {
    id: "Top Copper".to_owned(),
    role: LayerRole::Outline,
    side: Side::Top,
    color: "#c47a2c".to_owned(),
    visible: true,
    source: Provenance::default(),
    entities: vec![Entity {
      id: "E-top".to_owned(),
      kind: EntityKind::Contour {
        contour: contour.clone(),
      },
      source: Provenance::default(),
    }],
  });
  document.layers.push(Layer {
    id: "Bottom Copper".to_owned(),
    role: LayerRole::Outline,
    side: Side::Bottom,
    color: "#c47a2c".to_owned(),
    visible: true,
    source: Provenance::default(),
    entities: vec![],
  });
  document.layers.push(Layer {
    id: "Drill Hits".to_owned(),
    role: LayerRole::Drill,
    side: Side::Both,
    color: "#7cc7ff".to_owned(),
    visible: true,
    source: Provenance::default(),
    entities: vec![],
  });
  document.layers.push(Layer {
    id: "Board Outline".to_owned(),
    role: LayerRole::Outline,
    side: Side::Both,
    color: "#ddc9a3".to_owned(),
    visible: true,
    source: Provenance::default(),
    entities: vec![Entity {
      id: "E-outline".to_owned(),
      kind: EntityKind::Contour { contour },
      source: Provenance::default(),
    }],
  });

  document.toolpaths.push(Toolpath {
    id: "TP1".to_owned(),
    name: "Profile outline".to_owned(),
    entity_ids: vec!["E-outline".to_owned()],
    tool_number: 1,
    operation: Operation::Isolation(IsolationParams {
      passes: 1,
      overlap: 0.5,
      direction: MillingDirection::Climb,
      kind: IsolationKind::Full,
      cut_depth: Length::from_mm(1.8),
      keep_pours: false,
    }),
  });
  document.toolpaths.push(Toolpath {
    id: "TP2".to_owned(),
    name: "Drill 0.8 / 1.0".to_owned(),
    entity_ids: vec![],
    tool_number: 1,
    operation: Operation::Isolation(IsolationParams {
      passes: 1,
      overlap: 0.5,
      direction: MillingDirection::Climb,
      kind: IsolationKind::Full,
      cut_depth: Length::from_mm(1.8),
      keep_pours: false,
    }),
  });
  document.toolpaths.push(Toolpath {
    id: "TP3".to_owned(),
    name: "Isolation top".to_owned(),
    entity_ids: vec!["E-top".to_owned()],
    tool_number: 2,
    operation: Operation::Isolation(IsolationParams {
      passes: 2,
      overlap: 0.5,
      direction: MillingDirection::Climb,
      kind: IsolationKind::Full,
      cut_depth: Length::from_mm(0.1),
      keep_pours: false,
    }),
  });
  document
}

fn render(shell: Shell, app: AppState, name: &str) -> image::RgbaImage {
  let mut harness = Harness::builder()
    .with_size(egui::Vec2::new(WIDTH, HEIGHT))
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

  for _ in 0..8 {
    harness.step();
  }
  harness.run();
  harness.step();

  let image = harness.render().unwrap_or_else(|e| panic!("render {name}: {e:?}"));
  println!("ui-review: rendered {name} ({}x{})", image.width(), image.height());
  image
}

fn save_png(path: &Path, image: &image::RgbaImage) {
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).expect("create golden dir");
  }
  image::DynamicImage::ImageRgba8(image.clone())
    .save_with_format(path, image::ImageFormat::Png)
    .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

fn diff_images(expected: &image::RgbaImage, actual: &image::RgbaImage) -> (u64, u64, image::RgbaImage) {
  assert_eq!(
    (expected.width(), expected.height()),
    (actual.width(), actual.height()),
    "golden size mismatch: expected {}x{}, got {}x{}",
    expected.width(),
    expected.height(),
    actual.width(),
    actual.height()
  );
  let mut diff = image::RgbaImage::new(expected.width(), expected.height());
  let mut different_pixels: u64 = 0;
  let mut total_channel_diff: u64 = 0;
  for (x, y, p) in expected.enumerate_pixels() {
    let a = actual.get_pixel(x, y).0;
    let e = p.0;
    let pixel_diff: i32 = e
      .iter()
      .zip(a.iter())
      .take(4)
      .map(|(pe, pa)| (*pe as i32 - *pa as i32).abs())
      .sum();
    total_channel_diff += pixel_diff as u64;
    if pixel_diff > 0 {
      different_pixels += 1;
      // highlight diff in red, keep some context
      let alpha = (pixel_diff.min(255)) as u8;
      diff.put_pixel(x, y, image::Rgba([255, 0, 0, alpha.max(80)]));
    } else {
      let gray = ((e[0] as u16 + e[1] as u16 + e[2] as u16) / 3) as u8;
      diff.put_pixel(x, y, image::Rgba([gray / 3, gray / 3, gray / 3, 255]));
    }
  }
  (different_pixels, total_channel_diff, diff)
}

fn compare_or_update(name: &str, actual: &image::RgbaImage) {
  let golden_path = golden_dir().join(format!("{name}.png"));
  let update = std::env::var_os("UPDATE_UI_GOLDENS").is_some();

  if update || !golden_path.exists() {
    save_png(&golden_path, actual);
    println!(
      "ui-review: {} golden {}",
      if update { "updated" } else { "created" },
      golden_path.display()
    );
    if !update {
      println!("ui-review: golden was missing; created it. Re-run to verify, or commit it if it matches the artboard.");
    }
    return;
  }

  let expected = image::open(&golden_path)
    .unwrap_or_else(|e| panic!("read golden {}: {e}", golden_path.display()))
    .to_rgba8();
  let (different_pixels, total_channel_diff, diff) = diff_images(&expected, actual);
  let total_pixels = (expected.width() * expected.height()) as u64;
  let pct = (different_pixels as f64 / total_pixels as f64) * 100.0;

  if different_pixels == 0 {
    println!("ui-review: {name} matches golden exactly");
    return;
  }

  let diff_path = diff_dir().join(format!("{name}.diff.png"));
  let actual_path = diff_dir().join(format!("{name}.actual.png"));
  save_png(&diff_path, &diff);
  save_png(&actual_path, actual);

  // Small AA/font rasterization tolerance. The threshold is intentionally strict; raise only
  // with evidence from cross-platform runs, not to hide real regressions.
  let max_pct = 0.50;
  let max_channel_diff = total_pixels * 4 * 2; // avg 2 levels per channel per pixel
  println!(
    "ui-review: {name} differs: {different_pixels}/{total_pixels} pixels ({pct:.3}%), channel_diff={total_channel_diff}, diff={}, actual={}",
    diff_path.display(),
    actual_path.display()
  );
  assert!(
    pct <= max_pct && total_channel_diff <= max_channel_diff,
    "{name} visual regression: {pct:.3}% pixels differ ({different_pixels}/{total_pixels}), channel_diff={total_channel_diff}. \
See {} and {}. If the change is intentional, run UPDATE_UI_GOLDENS=1 just ui-review and review the new golden against {}.",
    diff_path.display(),
    actual_path.display(),
    manifest_path().display()
  );
  println!("ui-review: {name} within tolerance ({pct:.3}%)");
}

fn assert_manifest_exists() {
  let path = manifest_path();
  assert!(
    path.exists(),
    "missing coverage manifest {}: it must document artboard coverage and roadmap gaps",
    path.display()
  );
}

#[test]
#[ignore = "offscreen UI visual review — run with `just ui-review`, update with `UPDATE_UI_GOLDENS=1 just ui-review`"]
fn floating_studio_start_screen() {
  assert_manifest_exists();
  let mut shell = Shell::new();
  shell.start_with_no_project();
  shell.set_recents(vec![
    RecentProject::new("controller-board", "/tmp/controller-board.cam", "Hace 12 min"),
    RecentProject::new("usb-adapter", "/tmp/usb-adapter.cam", "Ayer"),
    RecentProject::new("front-panel", "/tmp/front-panel.cam", "Hace 3 días"),
  ]);
  let app = AppState::new(Document::new(Job::default_sheet()));
  let img = render(shell, app, "start-screen");
  compare_or_update("start-screen", &img);
}

#[test]
#[ignore = "offscreen UI visual review — run with `just ui-review`, update with `UPDATE_UI_GOLDENS=1 just ui-review`"]
fn floating_studio_shell() {
  assert_manifest_exists();
  let mut shell = Shell::new();
  // project_open defaults true; ensure start screen is not shown.
  shell.status = "Sin errores".to_owned();
  let app = AppState::new(document_for_floating_shell());
  let img = render(shell, app, "floating-shell");
  compare_or_update("floating-shell", &img);
}
