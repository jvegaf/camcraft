use std::io::Write;
use std::path::{Path, PathBuf};

use crate::{open, option_value, Failure};

/// Write the document's geometry back out as DXF or SVG.
///
/// The inverse of `import`, and deliberately as small: it reads one document, asks `cam-export` for the
/// bytes and writes them, or prints them. It takes no layer, no tolerance and no toolpath flag, because
/// what goes out is what the document holds (M12 S2, S5). What it *does* refuse, and the writer with it:
/// a format this build does not write (named, never guessed from an extension), a DXF layer name the
/// reader would silently alter or drop (empty, padded, carrying a control character, duplicated) and an
/// SVG of a document with no geometry to derive a page from. `--out` may not be the document being read,
/// and the write lands whole or not at all.
pub fn run(rest: &[String]) -> Result<(), Failure> {
  let mut document_path: Option<PathBuf> = None;
  let mut format: Option<cam_export::Format> = None;
  let mut out: Option<PathBuf> = None;

  let mut index = 0;
  while index < rest.len() {
    match rest[index].as_str() {
      "--format" => {
        let value = option_value(rest, index, "--format")?;
        format = Some(
          cam_export::Format::from_name(value)
            .ok_or_else(|| Failure::input(format!("export writes dxf or svg, not {value:?}")))?,
        );
        index += 2;
      }
      "--out" => {
        out = Some(PathBuf::from(option_value(rest, index, "--out")?));
        index += 2;
      }
      other if other.starts_with('-') => {
        return Err(Failure::input(format!("unknown export option {other:?}")))
      }
      other => {
        if document_path.is_some() {
          return Err(Failure::input("export takes exactly one document"));
        }
        document_path = Some(PathBuf::from(other));
        index += 1;
      }
    }
  }

  let document_path =
    document_path.ok_or_else(|| Failure::input("export requires a document path"))?;
  let format = format.ok_or_else(|| Failure::input("export requires --format dxf or svg"))?;
  let document = open(&document_path)?;
  let bytes = cam_export::export_bytes(&document, format)
    .map_err(|error| Failure::input(format!("{}: {error}", document_path.display())))?;

  match out {
    Some(path) => {
      // Writing over the document we just read would destroy it and report success, so the two paths are
      // compared canonically rather than by spelling (review-a7e624d841d3d0c6's R4-SAME-PATH, R3-001).
      let destination = path.canonicalize().unwrap_or_else(|_| path.clone());
      if destination
        == document_path
          .canonicalize()
          .unwrap_or_else(|_| document_path.clone())
      {
        return Err(Failure::input(format!(
          "export --out must not be the document it reads: {}",
          path.display()
        )));
      }
      // Whole or not at all: a truncated `.dxf` beside a live document is worse than a refusal
      // (R4-ATOMIC-OUT), and the temporary's name is unpredictable so a symlink left at the name a
      // derived one would have cannot redirect the write (R1-001).
      write_atomically(&path, &bytes)?;
      eprintln!("wrote {} ({} bytes)", path.display(), bytes.len());
    }
    None => {
      // `print!` panics on a closed pipe, which turns `export | head` into exit 101 and a backtrace
      // instead of a failure a script can read (R4-STDOUT-PANIC).
      let mut stdout = std::io::stdout().lock();
      stdout
        .write_all(&bytes)
        .and_then(|()| stdout.flush())
        .map_err(|error| Failure::input(format!("cannot write to stdout: {error}")))?;
    }
  }
  Ok(())
}

/// Write `bytes` to `path` whole or not at all.
///
/// The bytes go to a temporary in the destination's own directory, so the rename that puts them in
/// place stays on one filesystem and is atomic: a destination that never receives a partial file is
/// what keeps a truncated `.dxf` from reading as a drawing (R4-ATOMIC-OUT). `drill --out` reuses this,
/// because the same argument holds for a `.nc` that a controller would run short.
///
/// Its name carries the process id, a counter and the clock rather than being derived from the
/// destination. A derived name is a write primitive aimed wherever a pre-created symlink points —
/// `fs::write` follows one, so the run reports success over somebody else's file (R1-001) — and
/// `create_new` refuses an existing entry outright instead of following it, which closes the same
/// door for a name that is guessed rather than derived.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), Failure> {
  let temporary = unique_temporary_beside(path);
  let mut file = std::fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .open(&temporary)
    .map_err(|error| Failure::input(format!("{}: {error}", temporary.display())))?;
  let written = file.write_all(bytes).and_then(|()| file.flush());
  // Closed before the rename, so the file the rename publishes is the file the bytes were flushed to.
  drop(file);
  if let Err(error) = written {
    let _ = std::fs::remove_file(&temporary);
    return Err(Failure::input(format!("{}: {error}", temporary.display())));
  }
  std::fs::rename(&temporary, path).map_err(|error| {
    let _ = std::fs::remove_file(&temporary);
    Failure::input(format!("{}: {error}", path.display()))
  })
}

/// A temporary path in `destination`'s own directory whose name nothing can predict.
fn unique_temporary_beside(destination: &Path) -> PathBuf {
  use std::sync::atomic::{AtomicU64, Ordering};
  static SEQUENCE: AtomicU64 = AtomicU64::new(0);
  let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
  let nanos = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|elapsed| elapsed.subsec_nanos())
    .unwrap_or(0);
  let mut name = std::ffi::OsString::from(".");
  name.push(
    destination
      .file_name()
      .unwrap_or_else(|| std::ffi::OsStr::new("out")),
  );
  name.push(format!(".{}-{sequence}-{nanos}.tmp", std::process::id()));
  destination.with_file_name(name)
}
