//! The document container: a ZIP archive holding `document.json`.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use cam_model::Document;
use zip::write::SimpleFileOptions;

use crate::StoreError;

/// The archive entry that holds the document payload.
pub const DOCUMENT_ENTRY: &str = "document.json";

/// A loaded document plus every archive entry this build does not interpret.
///
/// Keeping the extra entries means a load/save cycle never silently drops data that a later build
/// will understand, which is the failure mode that makes users distrust a file format.
#[derive(Clone, Debug)]
pub struct DocumentFile {
  /// The parsed document.
  pub document: Document,
  /// Uninterpreted entries, preserved byte for byte on save.
  pub extras: BTreeMap<String, Vec<u8>>,
}

impl DocumentFile {
  /// Read and validate a document file.
  pub fn load(path: impl AsRef<Path>) -> Result<Self, StoreError> {
    let path = path.as_ref();
    let display = path.display().to_string();
    let bytes = std::fs::read(path).map_err(|source| StoreError::Io {
      path: display.clone(),
      source,
    })?;
    let mut archive =
      zip::ZipArchive::new(Cursor::new(bytes)).map_err(|source| StoreError::Archive {
        path: display.clone(),
        source,
      })?;

    let mut payload = None;
    let mut extras = BTreeMap::new();
    for index in 0..archive.len() {
      let mut entry = archive
        .by_index(index)
        .map_err(|source| StoreError::Archive {
          path: display.clone(),
          source,
        })?;
      if entry.is_dir() {
        continue;
      }
      let name = entry.name().to_owned();
      let mut buffer = Vec::new();
      entry
        .read_to_end(&mut buffer)
        .map_err(|source| StoreError::Io {
          path: display.clone(),
          source,
        })?;
      if name == DOCUMENT_ENTRY {
        payload = Some(buffer);
      } else {
        extras.insert(name, buffer);
      }
    }

    let payload = payload.ok_or_else(|| StoreError::MissingEntry {
      path: display.clone(),
      entry: DOCUMENT_ENTRY.to_owned(),
    })?;
    let document: Document =
      serde_json::from_slice(&payload).map_err(|source| StoreError::Json {
        path: display.clone(),
        source,
      })?;
    document.validate().map_err(|source| StoreError::Invalid {
      path: display,
      source,
    })?;

    Ok(Self { document, extras })
  }

  /// Write the document atomically.
  ///
  /// The bytes are written to a sibling temporary file and renamed into place, so an interrupted
  /// save leaves the previous file intact instead of a truncated archive.
  pub fn save(&self, path: impl AsRef<Path>) -> Result<(), StoreError> {
    let path = path.as_ref();
    let display = path.display().to_string();
    self
      .document
      .validate()
      .map_err(|source| StoreError::Invalid {
        path: display,
        source,
      })?;
    write_atomically(path, &self.to_bytes()?)
  }

  /// Serialise the archive in memory.
  ///
  /// Exposed for tests and for callers that stream the result somewhere else.
  pub fn to_bytes(&self) -> Result<Vec<u8>, StoreError> {
    let path = "<memory>".to_owned();
    let json = serde_json::to_vec_pretty(&self.document).map_err(|source| StoreError::Json {
      path: path.clone(),
      source,
    })?;

    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    writer
      .start_file(DOCUMENT_ENTRY, options)
      .map_err(|source| StoreError::Archive {
        path: path.clone(),
        source,
      })?;
    writer.write_all(&json).map_err(|source| StoreError::Io {
      path: path.clone(),
      source,
    })?;
    for (name, bytes) in &self.extras {
      writer
        .start_file(name.as_str(), options)
        .map_err(|source| StoreError::Archive {
          path: path.clone(),
          source,
        })?;
      writer.write_all(bytes).map_err(|source| StoreError::Io {
        path: path.clone(),
        source,
      })?;
    }
    let cursor = writer
      .finish()
      .map_err(|source| StoreError::Archive { path, source })?;
    Ok(cursor.into_inner())
  }
}

/// Save a document, validating it first so an invalid document never reaches disk.
pub fn save_document(path: impl AsRef<Path>, document: &Document) -> Result<(), StoreError> {
  let path = path.as_ref();
  let display = path.display().to_string();
  document.validate().map_err(|source| StoreError::Invalid {
    path: display,
    source,
  })?;
  let file = DocumentFile {
    document: document.clone(),
    extras: BTreeMap::new(),
  };
  write_atomically(path, &file.to_bytes()?)
}

/// Load a document, discarding uninterpreted archive entries.
pub fn load_document(path: impl AsRef<Path>) -> Result<Document, StoreError> {
  Ok(DocumentFile::load(path)?.document)
}

/// Write bytes to `path` through a sibling temporary file and an atomic rename.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
  let display = path.display().to_string();
  let temporary = temporary_path(path);
  if let Some(parent) = path.parent() {
    if !parent.as_os_str().is_empty() {
      std::fs::create_dir_all(parent).map_err(|source| StoreError::Io {
        path: parent.display().to_string(),
        source,
      })?;
    }
  }
  std::fs::write(&temporary, bytes).map_err(|source| StoreError::Io {
    path: temporary.display().to_string(),
    source,
  })?;
  if let Err(source) = std::fs::rename(&temporary, path) {
    // A failed write must leave the directory exactly as it found it, so the temporary file — which is
    // the writer's own, never the caller's — is removed before the error comes back. The rename is what
    // failed and the error the caller sees is the rename's: a failure to clean up afterwards is not
    // allowed to replace the reason the save did not happen.
    let _ = std::fs::remove_file(&temporary);
    return Err(StoreError::Io {
      path: display,
      source,
    });
  }
  Ok(())
}

/// The sibling temporary path used while writing `path`.
fn temporary_path(path: &Path) -> PathBuf {
  let name = path
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_else(|| "document".to_owned());
  path.with_file_name(format!(".{name}.tmp"))
}
