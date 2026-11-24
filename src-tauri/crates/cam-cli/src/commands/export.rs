use std::io::Write;
use std::path::PathBuf;

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
      // (R4-ATOMIC-OUT). The temporary sits beside the destination so the rename stays on one filesystem.
      let temporary = path.with_extension(format!(
        "{}.tmp",
        path
          .extension()
          .map(|extension| extension.to_string_lossy().into_owned())
          .unwrap_or_default()
      ));
      std::fs::write(&temporary, &bytes)
        .map_err(|error| Failure::input(format!("{}: {error}", temporary.display())))?;
      std::fs::rename(&temporary, &path).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        Failure::input(format!("{}: {error}", path.display()))
      })?;
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
