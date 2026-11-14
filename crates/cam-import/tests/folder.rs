//! Tests for fabrication folder import (E1.3 / W2c).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use cam_import::{detect_role, import_folder, Detection, ImportOptions};
use cam_model::{LayerRole, Side};

static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn corpus_dir(name: &str) -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../../fixtures/corpus")
    .join(name)
}

fn temp_dir(name: &str) -> PathBuf {
  let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
  let dir = std::env::temp_dir().join(format!(
    "cam-import-folder-{name}-{}-{n}",
    std::process::id()
  ));
  fs::create_dir_all(&dir).expect("create temp dir");
  dir
}

fn gerber_square(x0: i64, y0: i64, x1: i64, y1: i64) -> String {
  format!(
    "%FSLAX36Y36*%\n%MOMM*%\n%LPD*%\n%ADD10C,0.100*%\nD10*\n\
     X{x0:09}Y{y0:09}D02*\nG36*\n\
     X{x1:09}Y{y0:09}D01*\nX{x1:09}Y{y1:09}D01*\n\
     X{x0:09}Y{y1:09}D01*\nX{x0:09}Y{y0:09}D01*\n\
     G37*\nM02*\n"
  )
}

fn simple_drill() -> String {
  "M48\nMETRIC\nT1C0.800\n%\nT1\nX5.000Y5.000\nX15.000Y10.000\nM30\n".to_string()
}

fn write_file(dir: &Path, name: &str, content: &str) {
  fs::write(dir.join(name), content).expect("write file");
}

fn write_simple_kicad_folder(dir: &Path) {
  // 20x15 mm board outline, with geometry inside for each layer.
  write_file(
    dir,
    "outline.gbr",
    &gerber_square(0, 0, 20_000_000, 15_000_000),
  );
  write_file(
    dir,
    "top.gbr",
    &gerber_square(2_000_000, 2_000_000, 18_000_000, 13_000_000),
  );
  write_file(
    dir,
    "bottom.gbr",
    &gerber_square(2_000_000, 2_000_000, 18_000_000, 13_000_000),
  );
  write_file(
    dir,
    "mask-top.gbr",
    &gerber_square(1_000_000, 1_000_000, 19_000_000, 14_000_000),
  );
  write_file(
    dir,
    "mask-bottom.gbr",
    &gerber_square(1_000_000, 1_000_000, 19_000_000, 14_000_000),
  );
  write_file(
    dir,
    "silk-top.gbr",
    &gerber_square(3_000_000, 3_000_000, 8_000_000, 5_000_000),
  );
  write_file(
    dir,
    "silk-bottom.gbr",
    &gerber_square(3_000_000, 3_000_000, 8_000_000, 5_000_000),
  );
  write_file(dir, "drill.drl", &simple_drill());
}

#[test]
fn detect_roles_by_simplified_kicad_names() {
  let cases = [
    ("top.gbr", LayerRole::CopperTop, Side::Top),
    ("bottom.gbr", LayerRole::CopperBottom, Side::Bottom),
    ("mask-top.gbr", LayerRole::Mask, Side::Top),
    ("mask-bottom.gbr", LayerRole::Mask, Side::Bottom),
    ("silk-top.gbr", LayerRole::Silk, Side::Top),
    ("silk-bottom.gbr", LayerRole::Silk, Side::Bottom),
    ("outline.gbr", LayerRole::Outline, Side::Both),
    ("drill.drl", LayerRole::Drill, Side::Both),
  ];
  for (name, role, side) in cases {
    let (detected, how) = detect_role(name, b"");
    let detected = detected.unwrap_or_else(|| panic!("{name} should be recognized"));
    assert_eq!(detected.role, role, "{name} role");
    assert_eq!(detected.side, side, "{name} side");
    assert!(
      matches!(how, Detection::Name | Detection::NameAndContent),
      "{name} should be detected by name, got {how:?}"
    );
  }
}

#[test]
fn detect_roles_by_standard_kicad_names() {
  let cases = [
    ("board-F_Cu.gtl", LayerRole::CopperTop, Side::Top),
    ("board-B_Cu.gbl", LayerRole::CopperBottom, Side::Bottom),
    ("board-F_Mask.gts", LayerRole::Mask, Side::Top),
    ("board-B_Mask.gbs", LayerRole::Mask, Side::Bottom),
    ("board-F_Silkscreen.gto", LayerRole::Silk, Side::Top),
    ("board-B_Silkscreen.gbo", LayerRole::Silk, Side::Bottom),
    ("board-Edge_Cuts.gm1", LayerRole::Outline, Side::Both),
    ("board-PTH.drl", LayerRole::Drill, Side::Both),
  ];
  for (name, role, side) in cases {
    let (detected, _) = detect_role(name, b"");
    let detected = detected.unwrap_or_else(|| panic!("{name} should be recognized"));
    assert_eq!((detected.role, detected.side), (role, side), "{name}");
  }
}

#[test]
fn detect_drill_by_excellon_content() {
  let content = b"M48\nMETRIC,TZ\nT1C0.80\n%\nT1\nX010Y010\nM30\n";
  let (detected, how) = detect_role("unknown_file.txt", content);
  let detected = detected.expect("excellon content should be recognized as drill");
  assert_eq!(detected.role, LayerRole::Drill);
  assert_eq!(detected.side, Side::Both);
  assert_eq!(how, Detection::Content);
}

#[test]
fn detect_gerber_role_by_filefunction_content() {
  let top_cu = b"%TF.FileFunction,Copper,L1,Top*%\n%FSLAX46Y46*%\nM02*\n";
  let (detected, how) = detect_role("mystery.gbr", top_cu);
  let detected = detected.expect("FileFunction Copper Top should be recognized");
  assert_eq!(detected.role, LayerRole::CopperTop);
  assert_eq!(detected.side, Side::Top);
  assert_eq!(how, Detection::Content);

  let outline = b"%TF.FileFunction,Profile,NP*%\n%FSLAX46Y46*%\nM02*\n";
  let (detected, _) = detect_role("mystery.gbr", outline);
  let detected = detected.expect("FileFunction Profile should be recognized");
  assert_eq!(detected.role, LayerRole::Outline);

  let mask_bot = b"%TF.FileFunction,Soldermask,Bot*%\nM02*\n";
  let (detected, _) = detect_role("mystery.gbr", mask_bot);
  let detected = detected.expect("Soldermask Bot should be recognized");
  assert_eq!(
    (detected.role, detected.side),
    (LayerRole::Mask, Side::Bottom)
  );

  let silk_top = b"%TF.FileFunction,Legend,Top*%\nM02*\n";
  let (detected, _) = detect_role("mystery.gbr", silk_top);
  let detected = detected.expect("Legend Top should be recognized");
  assert_eq!((detected.role, detected.side), (LayerRole::Silk, Side::Top));

  let copper_bot = b"%TF.FileFunction,Copper,L2,Bot*%\nM02*\n";
  let (detected, _) = detect_role("mystery.gbr", copper_bot);
  let detected = detected.expect("Copper Bot should be recognized");
  assert_eq!(
    (detected.role, detected.side),
    (LayerRole::CopperBottom, Side::Bottom)
  );
}

#[test]
fn unrecognized_files_are_not_assigned_a_role() {
  let (detected, how) = detect_role("readme.txt", b"hello world");
  assert!(detected.is_none());
  assert_eq!(how, Detection::Unrecognized);
}

#[test]
fn a_recognized_file_that_fails_to_parse_is_warned_but_not_imported() {
  let dir = temp_dir("failed-layer");
  // The outline imports, so the folder as a whole succeeds.
  write_file(
    &dir,
    "outline.gbr",
    &gerber_square(0, 0, 20_000_000, 15_000_000),
  );
  // Recognized as top copper by name, but the content is not a Gerber file.
  write_file(&dir, "top.gbr", "this is not a gerber file\n");

  let result = import_folder(&dir, &ImportOptions::default()).expect("import folder");

  assert!(
    result
      .warnings
      .iter()
      .any(|warning| warning.contains("top.gbr")),
    "the failed layer is reported as a warning: {:?}",
    result.warnings
  );
  assert!(
    result
      .imported
      .iter()
      .all(|file| file.file_name != "top.gbr"),
    "a layer that failed to parse is not reported as imported: {:?}",
    result
      .imported
      .iter()
      .map(|file| &file.file_name)
      .collect::<Vec<_>>()
  );
  assert!(
    result
      .imported
      .iter()
      .any(|file| file.file_name == "outline.gbr"),
    "the layer that parsed is reported as imported: {:?}",
    result
      .imported
      .iter()
      .map(|file| &file.file_name)
      .collect::<Vec<_>>()
  );

  let _ = fs::remove_dir_all(&dir);
}

#[test]
fn import_simplified_kicad_folder_as_complete_board_with_correct_roles() {
  let dir = temp_dir("simple-kicad");
  write_simple_kicad_folder(&dir);

  let result = import_folder(&dir, &ImportOptions::default()).expect("import folder");
  let doc = &result.document;

  let has = |role: LayerRole, side: Side| {
    doc
      .layers
      .iter()
      .any(|l| l.role == role && l.side == side && !l.entities.is_empty())
  };
  assert!(
    has(LayerRole::CopperTop, Side::Top),
    "missing top copper: warnings={:?} layers={:?}",
    result.warnings,
    layer_summary(doc)
  );
  assert!(
    has(LayerRole::CopperBottom, Side::Bottom),
    "missing bottom copper: warnings={:?} layers={:?}",
    result.warnings,
    layer_summary(doc)
  );
  assert!(
    has(LayerRole::Mask, Side::Top),
    "missing top mask: warnings={:?} layers={:?}",
    result.warnings,
    layer_summary(doc)
  );
  assert!(
    has(LayerRole::Mask, Side::Bottom),
    "missing bottom mask: warnings={:?} layers={:?}",
    result.warnings,
    layer_summary(doc)
  );
  assert!(
    has(LayerRole::Silk, Side::Top),
    "missing top silk: warnings={:?} layers={:?}",
    result.warnings,
    layer_summary(doc)
  );
  assert!(
    has(LayerRole::Silk, Side::Bottom),
    "missing bottom silk: warnings={:?} layers={:?}",
    result.warnings,
    layer_summary(doc)
  );
  assert!(
    has(LayerRole::Outline, Side::Both),
    "missing outline: warnings={:?} layers={:?}",
    result.warnings,
    layer_summary(doc)
  );
  assert!(
    has(LayerRole::Drill, Side::Both),
    "missing drill: warnings={:?} layers={:?}",
    result.warnings,
    layer_summary(doc)
  );

  assert!(doc.board.width.mm() > 0.0);
  assert!(doc.board.height.mm() > 0.0);
  assert_eq!(
    result.imported.len(),
    8,
    "all 8 fabrication files imported: {:?}",
    result
      .imported
      .iter()
      .map(|f| &f.file_name)
      .collect::<Vec<_>>()
  );
  assert!(
    result.unrecognized.is_empty(),
    "no unrecognized files expected: {:?}",
    result.unrecognized
  );

  let _ = fs::remove_dir_all(&dir);
}

#[test]
fn import_folder_groups_by_content_when_names_are_generic() {
  let dir = temp_dir("by-content");
  // Generic names, roles determined by content (FileFunction / Excellon signature).
  let outline = format!(
    "%TF.FileFunction,Profile,NP*%\n{}",
    gerber_square(0, 0, 20_000_000, 15_000_000)
  );
  let top = format!(
    "%TF.FileFunction,Copper,L1,Top*%\n{}",
    gerber_square(2_000_000, 2_000_000, 18_000_000, 13_000_000)
  );
  let bottom = format!(
    "%TF.FileFunction,Copper,L2,Bot*%\n{}",
    gerber_square(2_000_000, 2_000_000, 18_000_000, 13_000_000)
  );
  let mask_t = format!(
    "%TF.FileFunction,Soldermask,Top*%\n{}",
    gerber_square(1_000_000, 1_000_000, 19_000_000, 14_000_000)
  );
  let mask_b = format!(
    "%TF.FileFunction,Soldermask,Bot*%\n{}",
    gerber_square(1_000_000, 1_000_000, 19_000_000, 14_000_000)
  );
  let silk_t = format!(
    "%TF.FileFunction,Legend,Top*%\n{}",
    gerber_square(3_000_000, 3_000_000, 8_000_000, 5_000_000)
  );
  let silk_b = format!(
    "%TF.FileFunction,Legend,Bot*%\n{}",
    gerber_square(3_000_000, 3_000_000, 8_000_000, 5_000_000)
  );
  let drill = "M48\nMETRIC,TZ\nT1C0.8\n%\nT1\nX5.000Y5.000\nM30\n".to_string();

  write_file(&dir, "layer_a.gbr", &top);
  write_file(&dir, "layer_b.gbr", &bottom);
  write_file(&dir, "layer_c.gbr", &mask_t);
  write_file(&dir, "layer_d.gbr", &mask_b);
  write_file(&dir, "layer_e.gbr", &silk_t);
  write_file(&dir, "layer_f.gbr", &silk_b);
  write_file(&dir, "layer_g.gbr", &outline);
  write_file(&dir, "layer_h.txt", &drill);
  // an unrelated file should be reported as unrecognized
  write_file(&dir, "notes.txt", "this is not a fabrication file\n");

  let result = import_folder(&dir, &ImportOptions::default()).expect("import folder");
  let doc = &result.document;

  let has = |role: LayerRole, side: Side| {
    doc
      .layers
      .iter()
      .any(|l| l.role == role && l.side == side && !l.entities.is_empty())
  };
  assert!(
    has(LayerRole::CopperTop, Side::Top),
    "top by content: {:?}",
    layer_summary(doc)
  );
  assert!(
    has(LayerRole::CopperBottom, Side::Bottom),
    "bottom by content: {:?}",
    layer_summary(doc)
  );
  assert!(
    has(LayerRole::Mask, Side::Top),
    "mask top by content: {:?}",
    layer_summary(doc)
  );
  assert!(
    has(LayerRole::Mask, Side::Bottom),
    "mask bottom by content: {:?}",
    layer_summary(doc)
  );
  assert!(
    has(LayerRole::Silk, Side::Top),
    "silk top by content: {:?}",
    layer_summary(doc)
  );
  assert!(
    has(LayerRole::Silk, Side::Bottom),
    "silk bottom by content: {:?}",
    layer_summary(doc)
  );
  assert!(
    has(LayerRole::Outline, Side::Both),
    "outline by content: {:?}",
    layer_summary(doc)
  );
  assert!(
    has(LayerRole::Drill, Side::Both),
    "drill by content: {:?}",
    layer_summary(doc)
  );

  // layer_h.txt was recognized as drill by content; notes.txt is unrecognized
  assert!(
    result
      .unrecognized
      .iter()
      .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("notes.txt")),
    "notes.txt should be unrecognized: {:?}",
    result.unrecognized
  );
  assert!(
    result
      .imported
      .iter()
      .any(|f| f.detection == Detection::Content),
    "at least some files should be detected by content: {:?}",
    result
      .imported
      .iter()
      .map(|f| (&f.file_name, f.detection))
      .collect::<Vec<_>>()
  );

  let _ = fs::remove_dir_all(&dir);
}

#[test]
fn kicad_corpus_folder_groups_all_fabrication_roles_by_name() {
  // Real KiCad corpus folders may contain gerber/excellon constructs the current parsers cannot yet
  // fully evaluate (parsing content depends on W2a/W2b; see waves.md). Grouping/detection is the
  // core of E1.3 and must correctly identify every fabrication role in a KiCad folder.
  let dir = corpus_dir("kicad-2l-simple");
  if !dir.is_dir() {
    return;
  }
  let options = ImportOptions::default();
  let mut entries: Vec<_> = fs::read_dir(&dir)
    .unwrap()
    .map(|e| e.unwrap().path())
    .collect();
  entries.sort();
  let mut roles: Vec<(LayerRole, Side)> = Vec::new();
  for path in entries {
    if !path.is_file() {
      continue;
    }
    let name = path.file_name().unwrap().to_str().unwrap().to_string();
    if name == "parity.json" || name == "provenance.json" {
      continue;
    }
    let bytes = fs::read(&path).unwrap();
    let (role, _) = detect_role(&name, &bytes);
    if let Some(r) = role {
      roles.push((r.role, r.side));
    }
  }
  let has = |role: LayerRole, side: Side| roles.iter().any(|&(ro, si)| ro == role && si == side);
  assert!(
    has(LayerRole::CopperTop, Side::Top),
    "top copper not detected in corpus folder: {roles:?}"
  );
  assert!(
    has(LayerRole::CopperBottom, Side::Bottom),
    "bottom copper not detected: {roles:?}"
  );
  assert!(
    has(LayerRole::Mask, Side::Top),
    "mask top not detected: {roles:?}"
  );
  assert!(
    has(LayerRole::Mask, Side::Bottom),
    "mask bottom not detected: {roles:?}"
  );
  assert!(
    has(LayerRole::Silk, Side::Top),
    "silk top not detected: {roles:?}"
  );
  assert!(
    has(LayerRole::Silk, Side::Bottom),
    "silk bottom not detected: {roles:?}"
  );
  assert!(
    has(LayerRole::Outline, Side::Both),
    "outline not detected: {roles:?}"
  );
  assert!(
    has(LayerRole::Drill, Side::Both),
    "drill not detected: {roles:?}"
  );
  let _ = options;
}

fn layer_summary(doc: &cam_model::Document) -> Vec<(String, LayerRole, Side, usize)> {
  doc
    .layers
    .iter()
    .map(|l| (l.id.clone(), l.role, l.side, l.entities.len()))
    .collect()
}
