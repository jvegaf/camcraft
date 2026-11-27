use std::path::PathBuf;

use cam_model::{LayerRole, Side};

use crate::{number, option_value, Failure};

/// `cam-cli import <input>`: import DXF or SVG into a new document.
pub fn run(rest: &[String]) -> Result<(), Failure> {
  let mut input: Option<PathBuf> = None;
  let mut out: Option<PathBuf> = None;
  let mut layer: Option<String> = None;
  let mut role: Option<LayerRole> = None;
  let mut side: Option<Side> = None;
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
      "--role" => {
        let value = option_value(rest, index, "--role")?;
        role = Some(parse_role(value)?);
        index += 2;
      }
      "--side" => {
        let value = option_value(rest, index, "--side")?;
        side = Some(parse_side(value)?);
        index += 2;
      }
      "--tolerance" => {
        options.tolerance = number(rest, index, "--tolerance")?;
        index += 2;
      }
      other if other.starts_with('-') => {
        return Err(Failure::input(format!("unknown import option {other:?}")));
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
  let document = cam_import::into_document_with(&imported, layer.as_deref(), role, side)
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

fn parse_role(value: &str) -> Result<LayerRole, Failure> {
  let role = match value {
    "copper_top" | "copper-top" => LayerRole::CopperTop,
    "copper_bottom" | "copper-bottom" => LayerRole::CopperBottom,
    "mask" => LayerRole::Mask,
    "silk" => LayerRole::Silk,
    "outline" => LayerRole::Outline,
    "drill" => LayerRole::Drill,
    "keepout" => LayerRole::Keepout,
    _ => {
      return Err(Failure::input(format!(
        "unknown --role {value:?}; expected copper_top, copper_bottom, mask, silk, outline, drill, or keepout"
      )))
    }
  };
  Ok(role)
}

fn parse_side(value: &str) -> Result<Side, Failure> {
  let side = match value {
    "top" => Side::Top,
    "bottom" => Side::Bottom,
    "both" => Side::Both,
    _ => {
      return Err(Failure::input(format!(
        "unknown --side {value:?}; expected top, bottom, or both"
      )))
    }
  };
  Ok(side)
}
