//! Behaviour tests for `.cmc` document persistence (A3).

use cam_geom::{Contour, Point2};
use cam_model::{
  Board, BoardSideUp, CutoutParams, Document, Entity, EntityKind, FillMethod, IsolationKind,
  IsolationParams, Layer, LayerRole, MillingDirection, NonCopperClearParams, Operation,
  PaintParams, PanelizeParams, Provenance, Side, Tab, TabKind, Toolpath, DOCUMENT_SCHEMA,
};
use cam_store::document::{load_document, save_document, DocumentFile, DOCUMENT_ENTRY};
use cam_store::StoreError;
use cam_units::Length;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
  let directory =
    std::env::temp_dir().join(format!("campilot-store-{}-{name}", std::process::id()));
  let _ = std::fs::remove_dir_all(&directory);
  std::fs::create_dir_all(&directory).expect("scratch directory");
  directory.join("part.cmc")
}

/// The names in `directory`, sorted, so a test can assert a directory is exactly as it found it.
fn listing(directory: &std::path::Path) -> Vec<String> {
  let mut names: Vec<String> = std::fs::read_dir(directory)
    .expect("the directory lists")
    .filter_map(|entry| entry.ok())
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .collect();
  names.sort();
  names
}

fn sample_document() -> Document {
  let mut document = Document::new(Board::default_board());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    role: LayerRole::Outline,
    side: Side::Top,
    color: "#000000".to_owned(),
    visible: true,
    entities: vec![Entity {
      id: "E1".to_owned(),
      kind: EntityKind::Contour {
        contour: Contour::from_points(
          &[
            Point2::new(0.0, 0.0),
            Point2::new(50.0, 0.0),
            Point2::new(50.0, 25.0),
            Point2::new(0.0, 25.0),
          ],
          true,
        )
        .expect("rectangle"),
      },
    }],
    source: Provenance::default(),
  });
  document
}

#[test]
fn save_then_load_is_lossless() {
  let path = scratch("roundtrip");
  let document = sample_document();
  save_document(&path, &document).expect("saves");
  let loaded = load_document(&path).expect("loads");
  assert_eq!(loaded, document);
  std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}

#[test]
fn the_archive_holds_human_readable_json() {
  let path = scratch("readable");
  save_document(&path, &sample_document()).expect("saves");
  let bytes = std::fs::read(&path).expect("reads");
  let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip");
  let mut entry = archive.by_name(DOCUMENT_ENTRY).expect("document entry");
  let mut text = String::new();
  std::io::Read::read_to_string(&mut entry, &mut text).expect("utf8");
  assert!(
    text.contains("\"schema\": \"camcraft/document@1\""),
    "{text}"
  );
  assert!(text.contains('\n'), "payload should be pretty-printed");
  std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}

#[test]
fn unknown_archive_entries_survive_a_load_and_save_cycle() {
  let path = scratch("extras");
  save_document(&path, &sample_document()).expect("saves");

  // Simulate a newer build having added an asset.
  let mut file = DocumentFile::load(&path).expect("loads");
  file
    .extras
    .insert("assets/logo.png".to_owned(), vec![1, 2, 3, 4]);
  file.save(&path).expect("saves with extras");

  let reloaded = DocumentFile::load(&path).expect("reloads");
  assert_eq!(
    reloaded.extras.get("assets/logo.png"),
    Some(&vec![1, 2, 3, 4])
  );
  assert_eq!(reloaded.document, sample_document());
  std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}

#[test]
fn a_missing_document_entry_names_the_expected_entry() {
  let path = scratch("missing-entry");
  // A valid ZIP that is not a cmc document.
  let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
  writer
    .start_file("notes.txt", zip::write::SimpleFileOptions::default())
    .expect("start file");
  std::io::Write::write_all(&mut writer, b"hello").expect("write");
  let bytes = writer.finish().expect("finish").into_inner();
  std::fs::write(&path, bytes).expect("write file");

  let error = load_document(&path).expect_err("must reject");
  let message = error.to_string();
  assert!(message.contains(DOCUMENT_ENTRY), "{message}");
  assert!(matches!(error, StoreError::MissingEntry { .. }));
  std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}

#[test]
fn a_file_that_is_not_a_zip_is_rejected() {
  let path = scratch("not-a-zip");
  std::fs::write(&path, b"this is not an archive").expect("write file");
  let error = load_document(&path).expect_err("must reject");
  assert!(matches!(error, StoreError::Archive { .. }), "{error:?}");
  std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}

#[test]
fn saving_an_invalid_document_writes_nothing() {
  let path = scratch("invalid");
  let mut document = sample_document();
  document.schema = "c2d/document@3".to_owned();
  let error = save_document(&path, &document).expect_err("must refuse");
  assert!(matches!(error, StoreError::Invalid { .. }), "{error:?}");
  assert!(
    !path.exists(),
    "no file may be created for an invalid document"
  );
  std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}

#[test]
fn saving_over_an_existing_document_replaces_it_without_a_temporary_leftover() {
  let path = scratch("overwrite");
  save_document(&path, &sample_document()).expect("first save");
  let mut second = sample_document();
  second.board.width = cam_units::Length::from_mm(600.0);
  save_document(&path, &second).expect("second save");

  let loaded = load_document(&path).expect("loads");
  assert_eq!(loaded.board.width, cam_units::Length::from_mm(600.0));

  let leftovers: Vec<_> = std::fs::read_dir(path.parent().expect("parent"))
    .expect("list")
    .filter_map(|entry| entry.ok())
    .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
    .collect();
  assert!(leftovers.is_empty(), "temporary file left behind");
  std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}

/// A rename that fails must not leave the temporary file it wrote beside the target.
///
/// A directory where the document should go is the one target the atomic writer cannot rename onto, so
/// it is the error path that can leave a write half-done: the bytes are written to a sibling `.name.tmp`
/// and the rename then fails. A save that fails changes nothing (S5), which includes the directory it
/// was writing into — the temporary file is the writer's own and goes with the error.
#[test]
fn a_save_that_cannot_be_renamed_leaves_the_directory_exactly_as_it_found_it() {
  let path = scratch("rename-fails");
  let directory = path.parent().expect("parent").to_path_buf();
  std::fs::create_dir(&path).expect("a directory named like a document");
  let before = listing(&directory);

  let error = save_document(&path, &sample_document()).expect_err("a directory cannot be replaced");

  assert!(matches!(error, StoreError::Io { .. }), "{error:?}");
  assert!(
    path.is_dir(),
    "the target the save could not replace must be untouched"
  );
  assert_eq!(
    listing(&directory),
    before,
    "a failed save must leave the directory exactly as it found it, with no temporary sibling behind"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_truncated_archive_is_rejected_rather_than_half_read() {
  let path = scratch("truncated");
  save_document(&path, &sample_document()).expect("saves");
  let bytes = std::fs::read(&path).expect("reads");
  std::fs::write(&path, &bytes[..bytes.len() / 2]).expect("truncate");
  assert!(load_document(&path).is_err());
  std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}

#[test]
fn a_legacy_schema_document_is_refused_naming_the_schema() {
  // A `c2d/document@1` file is a campilot document; it must be rejected as an unknown schema, never migrated.
  let path = scratch("legacy-refused");
  let document = sample_document();
  let legacy_json = serde_json::to_string(&document)
    .expect("serialises")
    .replace("\"camcraft/document@1\"", "\"c2d/document@1\"");
  let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
  writer
    .start_file(DOCUMENT_ENTRY, zip::write::SimpleFileOptions::default())
    .expect("start entry");
  use std::io::Write as _;
  writer
    .write_all(legacy_json.as_bytes())
    .expect("write payload");
  std::fs::write(&path, writer.finish().expect("finish").into_inner()).expect("write archive");

  let error = load_document(&path).expect_err("a legacy schema must be refused");
  let message = error.to_string();
  assert!(message.contains("c2d/document@1"), "{message}");
  assert!(message.contains(DOCUMENT_SCHEMA), "{message}");
  assert!(matches!(error, StoreError::Invalid { .. }), "{error:?}");
  std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}

#[test]
fn a_legacy_schema_cannot_be_written_through_to_bytes() {
  // Three doors write an archive: `save`, `save_document` and `to_bytes`. The first two validated and
  // the third did not, so the schema rule could be bypassed by loading a good file, changing its tag
  // and serialising the bytes for someone else to write.
  let path = scratch("legacy-to-bytes");
  save_document(&path, &sample_document()).expect("saves");
  let mut file = DocumentFile::load(&path).expect("loads");
  file.document.schema = "c2d/document@1".to_owned();

  let error = file
    .to_bytes()
    .expect_err("a legacy schema must be refused by the byte door too");
  assert!(error.to_string().contains("c2d/document@1"), "{error}");
  assert!(
    file.save(&path).is_err(),
    "and the file door agrees with it"
  );
  std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}

/// A board that uses every layer role and every operation kind.
///
/// The point is coverage of the format, not a real design.
fn full_board() -> Document {
  let mut document = Document::new(Board {
    width: Length::from_mm(60.0),
    height: Length::from_mm(40.0),
    thickness: Length::from_mm(1.6),
    origin: cam_model::OriginCorner::BottomLeft,
    side_up: BoardSideUp::Top,
  });
  let layers = [
    (LayerRole::CopperTop, Side::Top),
    (LayerRole::CopperBottom, Side::Bottom),
    (LayerRole::Mask, Side::Top),
    (LayerRole::Mask, Side::Bottom),
    (LayerRole::Silk, Side::Top),
    (LayerRole::Silk, Side::Bottom),
    (LayerRole::Outline, Side::Both),
    (LayerRole::Drill, Side::Both),
    (LayerRole::Keepout, Side::Top),
  ];
  for (index, (role, side)) in layers.iter().enumerate() {
    let id = format!("L{}", index + 1);
    document.layers.push(Layer {
      id: id.clone(),
      role: *role,
      side: *side,
      color: "#000000".to_owned(),
      visible: true,
      entities: vec![Entity {
        id: format!("E{}", index + 1),
        kind: EntityKind::Contour {
          contour: Contour::from_points(
            &[
              Point2::new(0.0, 0.0),
              Point2::new(20.0, 0.0),
              Point2::new(20.0, 15.0),
              Point2::new(0.0, 15.0),
            ],
            true,
          )
          .expect("rectangle"),
        },
      }],
      source: Provenance::default(),
    });
  }

  let cut = Length::from_mm(0.05);
  let operations = [
    Operation::Isolation(IsolationParams {
      passes: 2,
      overlap: 0.5,
      direction: MillingDirection::Climb,
      kind: IsolationKind::Full,
      cut_depth: cut,
      keep_pours: false,
    }),
    Operation::NonCopperClear(NonCopperClearParams {
      overlap: 0.4,
      margin: Length::from_mm(1.0),
      method: FillMethod::Standard,
      connect: true,
      contour: true,
      copper_offset: Some(Length::from_mm(0.2)),
      direction: MillingDirection::Climb,
      cut_depth: cut,
    }),
    Operation::Cutout(CutoutParams {
      margin: Length::from_mm(0.1),
      tabs: vec![Tab {
        position: 0.25,
        width: Length::from_mm(4.0),
        height: Length::from_mm(0.5),
      }],
      tab_kind: TabKind::Straight,
      pass_depth: Length::from_mm(0.6),
      cut_depth: Length::from_mm(1.8),
      cut_interiors: true,
    }),
    Operation::Paint(PaintParams {
      overlap: 0.4,
      offset: Length::from_mm(0.1),
      method: FillMethod::Lines,
      connect: true,
      contour: false,
      cut_depth: cut,
    }),
    Operation::Panelize(PanelizeParams {
      rows: 2,
      columns: 3,
      spacing_x: Length::from_mm(2.0),
      spacing_y: Length::from_mm(2.0),
      constrain_x: None,
      constrain_y: None,
    }),
  ];
  for (index, operation) in operations.into_iter().enumerate() {
    document.toolpaths.push(Toolpath {
      id: format!("T{}", index + 1),
      name: format!("operation {}", index + 1),
      entity_ids: vec![format!("E{}", index + 1)],
      tool_number: index as u32 + 1,
      operation,
    });
  }
  document
}

#[test]
fn a_full_board_survives_the_archive_unchanged() {
  let path = scratch("full-board");
  let board = full_board();
  board
    .validate()
    .expect("a board with every role and every operation is valid");
  save_document(&path, &board).expect("saves");
  let loaded = load_document(&path).expect("loads");
  assert_eq!(loaded, board, "the board comes back exactly as it went in");
  std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}
