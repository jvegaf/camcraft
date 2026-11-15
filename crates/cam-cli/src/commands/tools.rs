use std::path::PathBuf;

use cam_store::tools::{CuttingData, ToolGeometry, ToolLibrary};

use crate::Failure;

pub fn run(rest: &[String]) -> Result<(), Failure> {
  let Some(subcommand) = rest.first() else {
    return Err(Failure::input("tools requires a subcommand: list or add"));
  };
  match subcommand.as_str() {
    "list" => {
      let path = crate::only_path(&rest[1..], "tools list")?;
      let library = ToolLibrary::open(&path).map_err(|error| Failure::input(error.to_string()))?;
      for tool in library
        .tools()
        .map_err(|error| Failure::input(error.to_string()))?
      {
        println!(
          "T{} {} diameter {:.3} mm",
          tool.cutting.tool_number.unwrap_or(0),
          cam_store::tools::render_name(&tool.geometry.name_format, "Tool", &tool.geometry)
            .unwrap_or_else(|_| tool.geometry.id.clone()),
          tool.geometry.diameter.unwrap_or_default().mm()
        );
      }
      Ok(())
    }
    "add" => {
      let mut path = None;
      let mut id = None;
      let mut diameter = None;
      let mut number = None;
      let mut name_format = String::from("{Tool Type} ({Diameter|F}{Units Short})");
      let mut kind = cam_store::tools::ToolKind::FlatEndMill;
      let mut kind_explicit = false;
      let mut flutes: u32 = 2;
      let mut included_angle = None;
      let mut flat_diameter = None;
      let mut tip_radius = None;
      let mut flute_length = None;
      let mut shank_diameter = None;
      let mut overall_length = None;
      let mut feed: Option<f64> = Some(100.0);
      let mut plunge: Option<f64> = Some(30.0);
      let mut spindle: Option<u32> = Some(18_000);
      let mut stepdown = Some(cam_units::Length::from_mm(6.0));
      let mut stepover = None;
      let mut index = 0;
      while index < rest.len() - 1 {
        let argument = &rest[index + 1];
        match argument.as_str() {
          "--id" => {
            id = rest.get(index + 2).cloned();
            index += 2;
          }
          "--diameter" => {
            diameter = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok());
            index += 2;
          }
          "--number" => {
            number = rest
              .get(index + 2)
              .and_then(|value| value.parse::<u32>().ok());
            index += 2;
          }
          "--name-format" => {
            if let Some(value) = rest.get(index + 2) {
              name_format = value.clone();
            }
            index += 2;
          }
          "--kind" => {
            let Some(value) = rest.get(index + 2) else {
              return Err(Failure::input("--kind requires a value"));
            };
            kind = cam_store::tools::ToolKind::from_slug(value)
              .ok_or_else(|| Failure::input(format!("unknown tool kind {value:?}")))?;
            kind_explicit = true;
            index += 2;
          }
          "--flutes" => {
            flutes = rest
              .get(index + 2)
              .and_then(|value| value.parse::<u32>().ok())
              .ok_or_else(|| Failure::input("--flutes requires an integer value"))?;
            index += 2;
          }
          "--included-angle" => {
            included_angle = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(cam_units::Angle::from_degrees);
            if included_angle.is_none() {
              return Err(Failure::input("--included-angle requires a number"));
            }
            index += 2;
          }
          "--flat-diameter" => {
            flat_diameter = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(cam_units::Length::from_mm);
            if flat_diameter.is_none() {
              return Err(Failure::input("--flat-diameter requires a number"));
            }
            index += 2;
          }
          "--tip-radius" => {
            tip_radius = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(cam_units::Length::from_mm);
            if tip_radius.is_none() {
              return Err(Failure::input("--tip-radius requires a number"));
            }
            index += 2;
          }
          "--flute-length" => {
            flute_length = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(cam_units::Length::from_mm);
            if flute_length.is_none() {
              return Err(Failure::input("--flute-length requires a number"));
            }
            index += 2;
          }
          "--shank-diameter" => {
            shank_diameter = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(cam_units::Length::from_mm);
            if shank_diameter.is_none() {
              return Err(Failure::input("--shank-diameter requires a number"));
            }
            index += 2;
          }
          "--overall-length" => {
            overall_length = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(cam_units::Length::from_mm);
            if overall_length.is_none() {
              return Err(Failure::input("--overall-length requires a number"));
            }
            index += 2;
          }
          "--feed" => {
            feed = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(Some)
              .ok_or_else(|| Failure::input("--feed requires a number"))?;
            index += 2;
          }
          "--plunge" => {
            plunge = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(Some)
              .ok_or_else(|| Failure::input("--plunge requires a number"))?;
            index += 2;
          }
          "--spindle" => {
            spindle = rest
              .get(index + 2)
              .and_then(|value| value.parse::<u32>().ok())
              .map(Some)
              .ok_or_else(|| Failure::input("--spindle requires an integer value"))?;
            index += 2;
          }
          "--stepdown" => {
            stepdown = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(|value| Some(cam_units::Length::from_mm(value)))
              .ok_or_else(|| Failure::input("--stepdown requires a number"))?;
            index += 2;
          }
          "--stepover" => {
            stepover = rest
              .get(index + 2)
              .and_then(|value| value.parse::<f64>().ok())
              .map(|value| Some(cam_units::Length::from_mm(value)))
              .ok_or_else(|| Failure::input("--stepover requires a number"))?;
            index += 2;
          }
          other if !other.starts_with('-') && path.is_none() => {
            path = Some(PathBuf::from(other));
            index += 1;
          }
          other => {
            return Err(Failure::input(format!(
              "unknown tools add option {other:?}"
            )))
          }
        }
      }
      let path = path.ok_or_else(|| Failure::input("tools add requires a library path"))?;
      let id = id.ok_or_else(|| Failure::input("tools add requires --id"))?;
      let diameter = diameter.ok_or_else(|| Failure::input("tools add requires --diameter"))?;
      let number = number.ok_or_else(|| Failure::input("tools add requires --number"))?;

      let library = ToolLibrary::open(&path).map_err(|error| Failure::input(error.to_string()))?;
      let tool = cam_store::tools::StoredTool {
        geometry: ToolGeometry {
          id: id.clone(),
          name_format,
          tool_type: kind.raw(),
          units: cam_units::UnitSystem::Mm,
          diameter: Some(cam_units::Length::from_mm(diameter)),
          included_angle,
          flat_diameter,
          num_flutes: Some(flutes),
          flute_length,
          tip_radius,
          shank_diameter,
          overall_length,
          custom_attributes: None,
        },
        cutting: CuttingData {
          id: format!("{id}-cut"),
          rate_units: cam_units::UnitSystem::Mm,
          feed_rate: feed,
          plunge_rate: plunge,
          spindle_speed: spindle,
          stepdown,
          stepover,
          clear_stepover: None,
          tool_number: Some(number),
          notes: None,
        },
      };
      library
        .save_tool(&tool)
        .map_err(|error| Failure::input(error.to_string()))?;
      if kind_explicit {
        println!(
          "added {id} as T{number} with a {diameter} mm diameter ({})",
          kind.slug()
        );
      } else {
        println!("added {id} as T{number} with a {diameter} mm diameter");
      }
      Ok(())
    }
    other => Err(Failure::input(format!(
      "unknown tools subcommand {other:?}; expected list or add"
    ))),
  }
}
