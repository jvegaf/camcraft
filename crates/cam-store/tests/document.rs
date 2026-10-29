//! Behaviour tests for `.c2d` document persistence (A3).

use cam_geom::{Contour, Point2};
use cam_model::{Document, Entity, EntityKind, Job, Layer, DOCUMENT_SCHEMA};
use cam_store::document::{load_document, save_document, DocumentFile, DOCUMENT_ENTRY};
use cam_store::StoreError;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
  let directory =
    std::env::temp_dir().join(format!("campilot-store-{}-{name}", std::process::id()));
  let _ = std::fs::remove_dir_all(&directory);
  std::fs::create_dir_all(&directory).expect("scratch directory");
  directory.join("part.c2d")
}

fn sample_document() -> Document {
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(Layer {
    id: "L1".to_owned(),
    name: "Part".to_owned(),
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
  assert!(text.contains("\"schema\": \"c2d/document@2\""), "{text}");
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
  // A valid ZIP that is not a c2d document.
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
  second.job.width = cam_units::Length::from_mm(600.0);
  save_document(&path, &second).expect("second save");

  let loaded = load_document(&path).expect("loads");
  assert_eq!(loaded.job.width, cam_units::Length::from_mm(600.0));

  let leftovers: Vec<_> = std::fs::read_dir(path.parent().expect("parent"))
    .expect("list")
    .filter_map(|entry| entry.ok())
    .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
    .collect();
  assert!(leftovers.is_empty(), "temporary file left behind");
  std::fs::remove_dir_all(path.parent().expect("parent")).ok();
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
fn a_legacy_document_is_upgraded_on_read_and_written_as_the_current_schema() {
  // A `c2d/document@1` file is what M0–M4 wrote: no `island_ids` field at all. Reading it must not
  // fail, and saving it back must produce the current tag so a later build cannot mistake it for a
  // document that predates islands.
  let path = scratch("legacy");
  let document = sample_document();
  let legacy_json = serde_json::to_string(&document)
    .expect("serialises")
    .replace("\"c2d/document@2\"", "\"c2d/document@1\"");
  let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
  writer
    .start_file(DOCUMENT_ENTRY, zip::write::SimpleFileOptions::default())
    .expect("start entry");
  use std::io::Write as _;
  writer
    .write_all(legacy_json.as_bytes())
    .expect("write payload");
  std::fs::write(&path, writer.finish().expect("finish").into_inner()).expect("write archive");

  let loaded = load_document(&path).expect("a legacy document loads");
  assert_eq!(loaded.schema, DOCUMENT_SCHEMA);
  assert_eq!(
    loaded.toolpaths, document.toolpaths,
    "the migration changes no toolpath"
  );
  assert!(loaded
    .toolpaths
    .iter()
    .all(|toolpath| toolpath.island_ids.is_empty()));

  save_document(&path, &loaded).expect("saves");
  let raw = std::fs::read(&path).expect("read");
  let mut archive = zip::ZipArchive::new(std::io::Cursor::new(raw)).expect("zip");
  let mut payload = String::new();
  std::io::Read::read_to_string(
    &mut archive.by_name(DOCUMENT_ENTRY).expect("entry"),
    &mut payload,
  )
  .expect("utf8");
  assert!(
    payload.contains("\"schema\": \"c2d/document@2\""),
    "{payload}"
  );
  std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}
