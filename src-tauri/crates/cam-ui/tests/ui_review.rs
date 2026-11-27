//! Visual review automático de la UI de camcraft contra referencias canónicas.
//!
//! Renderiza el shell real de egui con `egui_kittest` (wgpu) y compara contra los PNGs de
//! `fixtures/ui/`. Las referencias son la verdad visual: este test NO las sobreescribe.
//!
//! Correr:    just ui-review
//! Diffs:     target/ui-review-diffs/<id>.actual.png y <id>.diff.png
//!
//! Si un cambio visual es intencional y la UI ya coincide con la nueva intención, actualizá la
//! referencia a mano (copiá el actual sobre fixtures/ui/<id>.png) y revisá el diff antes de commitear.

use std::path::{Path, PathBuf};

use cam_app::AppState;
use cam_geom::{Contour, Point2};
use cam_model::{
  Document, Entity, EntityKind, IsolationKind, IsolationParams, Job, Layer, LayerRole,
  MillingDirection, Operation, Provenance, Side, Toolpath,
};
use cam_ui::{RecentProject, Shell};
use cam_units::Length;
use egui_kittest::Harness;

fn repo_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn reference_dir() -> PathBuf {
  repo_root().join("fixtures/ui")
}

fn diff_dir() -> PathBuf {
  repo_root().join("target/ui-review-diffs")
}

fn manifest_path() -> PathBuf {
  reference_dir().join("manifest.json")
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
    tool_id: None,
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
    tool_id: None,
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
    tool_id: None,
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

fn render(shell: Shell, app: AppState, name: &str, width: f32, height: f32) -> image::RgbaImage {
  let mut harness = Harness::builder()
    .with_size(egui::Vec2::new(width, height))
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

  let image = harness
    .render()
    .unwrap_or_else(|e| panic!("render {name}: {e:?}"));
  println!(
    "ui-review: rendered {name} ({}x{})",
    image.width(),
    image.height()
  );
  image
}

fn save_png(path: &Path, image: &image::RgbaImage) {
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).expect("create diff dir");
  }
  image::DynamicImage::ImageRgba8(image.clone())
    .save_with_format(path, image::ImageFormat::Png)
    .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

fn diff_images(
  expected: &image::RgbaImage,
  actual: &image::RgbaImage,
) -> (u64, u64, image::RgbaImage) {
  assert_eq!(
    (expected.width(), expected.height()),
    (actual.width(), actual.height()),
    "size mismatch: expected {}x{}, got {}x{}",
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
      let alpha = (pixel_diff.min(255)) as u8;
      diff.put_pixel(x, y, image::Rgba([255, 0, 0, alpha.max(80)]));
    } else {
      let gray = ((e[0] as u16 + e[1] as u16 + e[2] as u16) / 3) as u8;
      diff.put_pixel(x, y, image::Rgba([gray / 3, gray / 3, gray / 3, 255]));
    }
  }
  (different_pixels, total_channel_diff, diff)
}

fn compare(name: &str, actual: &image::RgbaImage) {
  let reference_path = reference_dir().join(format!("{name}.png"));
  assert!(
    reference_path.exists(),
    "missing reference {}. Las referencias viven en fixtures/ui/ y no se generan automáticamente.",
    reference_path.display()
  );

  let expected = image::open(&reference_path)
    .unwrap_or_else(|e| panic!("read reference {}: {e}", reference_path.display()))
    .to_rgba8();

  let (different_pixels, total_channel_diff, diff) = diff_images(&expected, actual);
  let total_pixels = (expected.width() * expected.height()) as u64;
  let pct = (different_pixels as f64 / total_pixels as f64) * 100.0;

  let diff_path = diff_dir().join(format!("{name}.diff.png"));
  let actual_path = diff_dir().join(format!("{name}.actual.png"));

  if different_pixels == 0 {
    // Clean stale diffs for this scene so the output dir reflects current state.
    let _ = std::fs::remove_file(&diff_path);
    let _ = std::fs::remove_file(&actual_path);
    println!("ui-review: {name}  OK  (match exacto contra {})", reference_path.display());
    return;
  }

  save_png(&diff_path, &diff);
  save_png(&actual_path, actual);

  // Tolerancia pequeña para AA/rasterización de fuentes. Estricta a propósito.
  let max_pct = 0.50;
  let max_channel_diff = total_pixels * 4 * 2; // avg 2 niveles por canal por pixel
  println!(
    "ui-review: {name}  DIFF  {different_pixels}/{total_pixels} px ({pct:.3}%), channel_diff={total_channel_diff}"
  );
  println!("  reference: {}", reference_path.display());
  println!("  actual:    {}", actual_path.display());
  println!("  diff:      {}", diff_path.display());

  assert!(
    pct <= max_pct && total_channel_diff <= max_channel_diff,
    "{name}: regresión visual {pct:.3}% ({different_pixels}/{total_pixels}), channel_diff={total_channel_diff}. \
Revisá {} y {}. Si el cambio es intencional, actualizá la referencia a mano: \
cp {} {}",
    actual_path.display(),
    diff_path.display(),
    actual_path.display(),
    reference_path.display()
  );
  println!("ui-review: {name}  OK  (dentro de tolerancia {pct:.3}%)");
}

fn reference_size(name: &str) -> (f32, f32) {
  let reference_path = reference_dir().join(format!("{name}.png"));
  let img = image::image_dimensions(&reference_path)
    .unwrap_or_else(|e| panic!("read dimensions {}: {e}", reference_path.display()));
  (img.0 as f32, img.1 as f32)
}

fn assert_manifest_exists() {
  let path = manifest_path();
  assert!(
    path.exists(),
    "missing manifest {}: debe documentar escenas, cobertura y gaps",
    path.display()
  );
}

#[test]
#[ignore = "visual review offscreen — correr con `just ui-review`"]
fn start_screen() {
  assert_manifest_exists();
  let (width, height) = reference_size("start-screen");
  let mut shell = Shell::new();
  shell.start_with_no_project();
  shell.set_recents(vec![
    RecentProject::new(
      "controller-board",
      "/tmp/controller-board.cam",
      "Hace 12 min",
    ),
    RecentProject::new("usb-adapter", "/tmp/usb-adapter.cam", "Ayer"),
    RecentProject::new("front-panel", "/tmp/front-panel.cam", "Hace 3 días"),
  ]);
  let app = AppState::new(Document::new(Job::default_sheet()));
  let img = render(shell, app, "start-screen", width, height);
  compare("start-screen", &img);
}

#[test]
#[ignore = "visual review offscreen — correr con `just ui-review`"]
fn floating_shell() {
  assert_manifest_exists();
  let (width, height) = reference_size("floating-shell");
  let mut shell = Shell::new();
  shell.status = "Sin errores".to_owned();
  let app = AppState::new(document_for_floating_shell());
  let img = render(shell, app, "floating-shell", width, height);
  compare("floating-shell", &img);
}

#[test]
#[ignore = "visual review offscreen — correr con `just ui-review`"]
fn settings() {
  assert_manifest_exists();
  let (width, height) = reference_size("settings");
  let mut shell = Shell::new();
  shell.status = "Sin errores".to_owned();
  shell.open_settings();
  let app = AppState::new(document_for_floating_shell());
  let img = render(shell, app, "settings", width, height);
  compare("settings", &img);
}
