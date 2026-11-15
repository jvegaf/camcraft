use std::path::PathBuf;

use crate::{number, option_value, Failure};

/// `cam-cli import <input>`: import DXF or SVG into a new document.
pub fn run(rest: &[String]) -> Result<(), Failure> {
  let mut input: Option<PathBuf> = None;
  let mut out: Option<PathBuf> = None;
  let mut layer: Option<String> = None;
  let mut options = cam_import::ImportOptions::default();

  let mut index = 0;
  while index < rest.len() {
    match rest[index].as_str() {
      "--out" => {
        out = Some(PathBuf::from(option_value(rest, index, "--out")?));
        index += 2;
      }
      "--layer" => {
        layer = Some(option_value(rest, index, "--layer")?.to_string());
        index += 2;
      }
      "--tolerance" => {
        options.tolerance = number(rest, index, "--tolerance")?;
        index += 2;
      }
      other if other.starts_with('-') => {
        return Err(Failure::input(format!("unknown import option {other:?}")))
      }
      other => {
        if input.is_some() {
          return Err(Failure::input("import takes exactly one input file"));
        }
        input = Some(PathBuf::from(other));
        index += 1;
      }
    }
  }

  let input = input.ok_or_else(|| Failure::input("import requires an input file"))?;
  if options.tolerance <= 0.0 {
    return Err(Failure::input(
      "--tolerance must be a positive length in mm",
    ));
  }

  let imported = cam_import::import_path(&input, &options)
    .map_err(|error| Failure::input(format!("{}: {error}", input.display())))?;
  let document = cam_import::into_document(&imported, layer.as_deref())
    .map_err(|error| Failure::input(format!("{}: {error}", input.display())))?;

  if let Some(path) = &out {
    cam_store::document::save_document(path, &document)
      .map_err(|error| Failure::input(format!("{}: {error}", path.display())))?;
    eprintln!(
      "wrote {} layer(s), {} entity(ies) to {}",
      document.layers.len(),
      document.entities().count(),
      path.display()
    );
  }

  print!("{}", imported.report.render());
  Ok(())
}
