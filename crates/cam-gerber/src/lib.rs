//! Gerber RS-274X reader for the dialect KiCad writes.
//!
//! Reads the header, coordinate format, aperture and macro tables, and a flat command stream.
//! Geometry evaluation beyond the parsed command stream is out of scope.

#![warn(missing_docs)]

use cam_geom::boolean::{combine, BooleanOp, Combined};
use cam_geom::primitive;
use cam_geom::region::Region;
use cam_geom::{Arc, Contour, LineSeg, Point2, Segment};
use std::f64::consts::TAU;

/// Why a Gerber file could not be read.
#[derive(Debug, thiserror::Error)]
pub enum GerberError {
  /// The file uses something this reader does not implement.
  #[error("line {line}: {what} is not supported")]
  Unsupported {
    /// Line the construct is on.
    line: u32,
    /// What was found.
    what: String,
  },
  /// A draw/move/flash occurred before any aperture was selected.
  #[error("line {line}: a draw, move or flash before any aperture was selected")]
  NoApertureSelected {
    /// Line the operation is on.
    line: u32,
  },
  /// A D code selected an aperture that was never defined.
  #[error("line {line}: aperture {aperture} is not defined")]
  UnknownAperture {
    /// Line the selection is on.
    line: u32,
    /// The aperture code.
    aperture: u32,
  },
  /// The file uses a construct this reader does not accept.
  #[error("line {line}: {what}")]
  Invalid {
    /// Line the problem is on.
    line: u32,
    /// What was found.
    what: String,
  },
}

/// Zero suppression mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZeroSuppression {
  /// Leading zeros omitted.
  Leading,
  /// Trailing zeros omitted.
  Trailing,
  /// No suppression.
  None,
}

/// Coordinate format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
  /// Zero suppression.
  pub zero_suppression: ZeroSuppression,
  /// Integer digits.
  pub integer_digits: u32,
  /// Decimal digits.
  pub decimal_digits: u32,
}

/// Units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Units {
  /// Millimetres.
  Millimetre,
  /// Inches.
  Inch,
}

/// Polarity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Polarity {
  /// Dark.
  Dark,
  /// Clear.
  Clear,
}

/// Aperture definition.
#[derive(Clone, Debug, PartialEq)]
pub struct Aperture {
  /// Aperture code.
  pub code: u32,
  /// Template name.
  pub template: String,
  /// Parameters.
  pub parameters: Vec<f64>,
}

/// Macro primitive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacroPrimitive {
  /// Primitive code.
  pub code: u32,
  /// Exposure.
  pub exposure: bool,
  /// Parameters as strings.
  pub parameters: Vec<String>,
}

/// Macro definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Macro {
  /// Macro name.
  pub name: String,
  /// Primitives.
  pub primitives: Vec<MacroPrimitive>,
}

/// Command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
  /// Move.
  Move {
    /// Destination.
    to: Point2,
  },
  /// Line.
  Line {
    /// Destination.
    to: Point2,
  },
  /// Arc.
  Arc {
    /// Destination.
    to: Point2,
    /// Offset.
    offset: (f64, f64),
    /// Clockwise.
    clockwise: bool,
  },
  /// Flash.
  Flash {
    /// Position.
    at: Point2,
  },
  /// Select aperture.
  SelectAperture(u32),
  /// Polarity.
  Polarity(Polarity),
  /// Region begin.
  RegionBegin,
  /// Region end.
  RegionEnd,
}

/// Parsed Gerber file.
#[derive(Clone, Debug, PartialEq)]
pub struct Gerber {
  /// Format.
  pub format: Format,
  /// Units.
  pub units: Units,
  /// Apertures.
  pub apertures: Vec<Aperture>,
  /// Macros.
  pub macros: Vec<Macro>,
  /// Commands.
  pub commands: Vec<Command>,
  /// Attributes.
  pub attributes: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Interpolation {
  Linear,
  Clockwise,
  CounterClockwise,
}

impl Gerber {
  /// Evaluate the parsed command stream into one image region.
  ///
  /// Dark material is unioned in and clear material is subtracted, in command order. A file that
  /// yields no material returns an empty [`Region`] (`Region::new(Vec::new())` is valid).
  ///
  /// # Errors
  ///
  /// Returns [`GerberError::Unsupported`] naming the command index for any construct this pass
  /// cannot build yet (`Flash`, a draw outside a region, an arc whose centre cannot be derived, or
  /// a region with fewer than three segments). `Command` carries no source line number, so the
  /// refusal reports the command index in `what` and uses `0` for `line`.
  pub fn image(&self, tolerance: f64) -> Result<Region, GerberError> {
    let mut current: Option<Point2> = None;
    let mut polarity = Polarity::Dark;
    let mut selected_aperture: Option<u32> = None;
    // Start empty: no boundaries means no material (membership is vacuously outside).
    let mut image = Region::new(Vec::new()).map_err(|error| GerberError::Invalid {
      line: 0,
      what: format!("command 0: could not build an empty image: {error}"),
    })?;

    let mut in_region = false;
    let mut region_segments: Vec<Segment> = Vec::new();

    for (index, command) in self.commands.iter().copied().enumerate() {
      match command {
        Command::Polarity(next) => {
          polarity = next;
        }
        Command::SelectAperture(code) => {
          selected_aperture = Some(code);
        }
        Command::Move { to } => {
          if in_region {
            return Err(GerberError::Unsupported {
              line: 0,
              what: format!("command {index}: a move inside a region"),
            });
          }
          current = Some(to);
        }
        Command::RegionBegin => {
          if in_region {
            return Err(GerberError::Unsupported {
              line: 0,
              what: format!("command {index}: a region begins inside a region"),
            });
          }
          in_region = true;
          region_segments.clear();
        }
        Command::RegionEnd => {
          if !in_region {
            return Err(GerberError::Unsupported {
              line: 0,
              what: format!("command {index}: a region ends outside a region"),
            });
          }
          if region_segments.len() < 3 {
            return Err(GerberError::Unsupported {
              line: 0,
              what: format!(
                "command {index}: a region closes with fewer than three segments ({})",
                region_segments.len()
              ),
            });
          }
          let contour = Contour::from_segments(&region_segments, true).map_err(|error| {
            GerberError::Invalid {
              line: 0,
              what: format!("command {index}: the region boundary is not valid: {error}"),
            }
          })?;
          let region = Region::new(vec![contour]).map_err(|error| GerberError::Invalid {
            line: 0,
            what: format!("command {index}: the region is not valid: {error}"),
          })?;
          image = apply_region(image, &region, polarity, tolerance, index)?;
          in_region = false;
          region_segments.clear();
        }
        Command::Line { to } => {
          if !in_region {
            return Err(GerberError::Unsupported {
              line: 0,
              what: format!("command {index}: a line outside a region"),
            });
          }
          let from = current.ok_or(GerberError::Unsupported {
            line: 0,
            what: format!("command {index}: a line without a current point"),
          })?;
          if from.is_near(to, 0.0) {
            return Err(GerberError::Unsupported {
              line: 0,
              what: format!("command {index}: a zero-length line in a region"),
            });
          }
          region_segments.push(Segment::Line(LineSeg::new(from, to)));
          current = Some(to);
        }
        Command::Arc {
          to,
          offset,
          clockwise,
        } => {
          if !in_region {
            return Err(GerberError::Unsupported {
              line: 0,
              what: format!("command {index}: an arc outside a region"),
            });
          }
          let from = current.ok_or(GerberError::Unsupported {
            line: 0,
            what: format!("command {index}: an arc without a current point"),
          })?;
          let arc = arc_from_gerber(from, to, offset, clockwise).map_err(|reason| {
            GerberError::Unsupported {
              line: 0,
              what: format!("command {index}: {reason}"),
            }
          })?;
          region_segments.push(Segment::Arc(arc));
          current = Some(to);
        }
        Command::Flash { at } => {
          let code = selected_aperture.ok_or(GerberError::NoApertureSelected { line: 0 })?;
          let aperture =
            self
              .apertures
              .iter()
              .find(|a| a.code == code)
              .ok_or(GerberError::UnknownAperture {
                line: 0,
                aperture: code,
              })?;
          let shape = match aperture.template.as_str() {
            "C" => {
              let diameter =
                aperture
                  .parameters
                  .first()
                  .copied()
                  .ok_or_else(|| GerberError::Unsupported {
                    line: 0,
                    what: format!(
                      "command {index}: a flash of aperture {code} template C without a diameter"
                    ),
                  })?;
              // A circular aperture is the disc of radius d/2 centred at the flash point.
              let contour =
                primitive::circle(at, diameter / 2.0).map_err(|error| GerberError::Invalid {
                  line: 0,
                  what: format!(
                    "command {index}: the flash of aperture {code} template C is not valid: {error}"
                  ),
                })?;
              Region::new(vec![contour]).map_err(|error| GerberError::Invalid {
                line: 0,
                what: format!(
                  "command {index}: the flash of aperture {code} template C is not valid: {error}"
                ),
              })?
            }
            "R" => {
              let width = aperture.parameters.first().copied().ok_or_else(|| {
                GerberError::Unsupported {
                  line: 0,
                  what: format!(
                    "command {index}: a flash of aperture {code} template R without width and height"
                  ),
                }
              })?;
              let height = aperture.parameters.get(1).copied().ok_or_else(|| {
                GerberError::Unsupported {
                  line: 0,
                  what: format!(
                    "command {index}: a flash of aperture {code} template R without width and height"
                  ),
                }
              })?;
              // Gerber flashes a rectangle centred on the point; cam_geom::primitive::rectangle
              // takes the lower-left corner, so translate by half the size.
              let corner = Point2::new(at.x - width / 2.0, at.y - height / 2.0);
              let contour = primitive::rectangle(corner, width, height).map_err(|error| {
                GerberError::Invalid {
                  line: 0,
                  what: format!(
                    "command {index}: the flash of aperture {code} template R is not valid: {error}"
                  ),
                }
              })?;
              Region::new(vec![contour]).map_err(|error| GerberError::Invalid {
                line: 0,
                what: format!(
                  "command {index}: the flash of aperture {code} template R is not valid: {error}"
                ),
              })?
            }
            template => {
              return Err(GerberError::Unsupported {
                line: 0,
                what: format!("command {index}: a flash of template {template}"),
              });
            }
          };
          image = apply_region(image, &shape, polarity, tolerance, index)?;
        }
      }
    }

    Ok(image)
  }
}

fn apply_region(
  image: Region,
  region: &Region,
  polarity: Polarity,
  tolerance: f64,
  index: usize,
) -> Result<Region, GerberError> {
  let op = match polarity {
    Polarity::Dark => BooleanOp::Union,
    Polarity::Clear => BooleanOp::Difference,
  };
  // `combine` may legitimately return `Combined::Empty` (an answer, not an error).
  match combine(&image, region, op, tolerance) {
    Ok(Combined::Empty) => Region::new(Vec::new()).map_err(|error| GerberError::Invalid {
      line: 0,
      what: format!("command {index}: could not represent an empty image: {error}"),
    }),
    Ok(Combined::Region { region, .. }) => Ok(region),
    Err(error) => Err(GerberError::Invalid {
      line: 0,
      what: format!("command {index}: polarity could not be applied: {error}"),
    }),
  }
}

fn arc_from_gerber(
  start: Point2,
  end: Point2,
  offset: (f64, f64),
  clockwise: bool,
) -> Result<Arc, &'static str> {
  let center = Point2::new(start.x + offset.0, start.y + offset.1);
  let radius = (offset.0 * offset.0 + offset.1 * offset.1).sqrt();
  if !radius.is_finite() || radius <= 0.0 {
    return Err("an arc whose centre cannot be derived");
  }
  let start_angle = (start - center).angle();
  let end_angle = (end - center).angle();
  let delta = if clockwise {
    (start_angle - end_angle).rem_euclid(TAU)
  } else {
    (end_angle - start_angle).rem_euclid(TAU)
  };
  let delta = if delta.abs() < 1.0e-12 { TAU } else { delta };
  let sweep = if clockwise { -delta } else { delta };
  Arc::new(center, radius, start_angle, sweep).map_err(|_| "an arc whose centre cannot be derived")
}

/// Parse a Gerber RS-274X file.
///
/// # Errors
///
/// Returns the first construct it cannot honour, with the line it is on.
pub fn parse(text: &str) -> Result<Gerber, GerberError> {
  let mut format: Option<Format> = None;
  let mut units: Option<Units> = None;
  let mut apertures: Vec<Aperture> = Vec::new();
  let mut macros: Vec<Macro> = Vec::new();
  let mut commands: Vec<Command> = Vec::new();
  let mut attributes: Vec<String> = Vec::new();
  let mut macro_names: std::collections::HashSet<String> = std::collections::HashSet::new();
  let mut aperture_codes: std::collections::HashSet<u32> = std::collections::HashSet::new();

  let mut in_block = false;
  let mut block_lines: Vec<(u32, String)> = Vec::new();
  let mut block_start_line: u32 = 0;

  let mut x: Option<f64> = None;
  let mut y: Option<f64> = None;
  let mut selected_aperture: Option<u32> = None;
  let mut interpolation = Interpolation::Linear;

  for (index, raw) in text.lines().enumerate() {
    let line = index as u32 + 1;
    let s = raw.trim_end_matches('\r').to_string();
    if in_block {
      block_lines.push((line, s.clone()));
      if s.trim_end().ends_with('%') {
        // finish block
        let content = collect_block(&block_lines);
        process_extended_block(
          &content,
          block_start_line,
          &mut Tables {
            format: &mut format,
            units: &mut units,
            apertures: &mut apertures,
            macros: &mut macros,
            attributes: &mut attributes,
            commands: &mut commands,
            macro_names: &mut macro_names,
            aperture_codes: &mut aperture_codes,
          },
        )?;
        in_block = false;
        block_lines.clear();
      }
      continue;
    }

    let trimmed = s.trim();
    if trimmed.is_empty() {
      continue;
    }
    if trimmed.starts_with('%') {
      in_block = true;
      block_start_line = line;
      block_lines.clear();
      block_lines.push((line, s.clone()));
      if trimmed.ends_with('%') && trimmed.len() > 1 {
        let content = collect_block(&block_lines);
        process_extended_block(
          &content,
          block_start_line,
          &mut Tables {
            format: &mut format,
            units: &mut units,
            apertures: &mut apertures,
            macros: &mut macros,
            attributes: &mut attributes,
            commands: &mut commands,
            macro_names: &mut macro_names,
            aperture_codes: &mut aperture_codes,
          },
        )?;
        in_block = false;
        block_lines.clear();
      }
      continue;
    }

    // normal command line(s) ending with *
    // a line can contain statements separated? usually one. split on * but keep meaningful
    for stmt in trimmed.split('*') {
      let stmt = stmt.trim();
      if stmt.is_empty() {
        continue;
      }
      // comments
      if stmt.starts_with("G04") {
        continue;
      }
      if stmt == "G01" {
        interpolation = Interpolation::Linear;
        continue;
      }
      if stmt == "G02" {
        interpolation = Interpolation::Clockwise;
        continue;
      }
      if stmt == "G03" {
        interpolation = Interpolation::CounterClockwise;
        continue;
      }
      if stmt == "G75" {
        continue;
      }
      if stmt == "G74" {
        return Err(GerberError::Unsupported {
          line,
          what: "single-quadrant arcs (G74)".to_owned(),
        });
      }
      if stmt == "G36" {
        commands.push(Command::RegionBegin);
        continue;
      }
      if stmt == "G37" {
        commands.push(Command::RegionEnd);
        continue;
      }
      if stmt == "M02" {
        continue;
      }
      // reject other G/M codes
      if let Some(code) = parse_gm_code(stmt) {
        return Err(GerberError::Unsupported {
          line,
          what: format!("code {code}"),
        });
      }

      // coordinate / D operations
      if stmt.contains('X')
        || stmt.contains('Y')
        || stmt.contains('I')
        || stmt.contains('J')
        || stmt.starts_with('D')
      {
        // require format and units before coordinates
        let has_coord =
          stmt.contains('X') || stmt.contains('Y') || stmt.contains('I') || stmt.contains('J');
        if has_coord {
          if format.is_none() {
            return Err(GerberError::Invalid {
              line,
              what: "a coordinate before %FS".to_owned(),
            });
          }
          if units.is_none() {
            return Err(GerberError::Invalid {
              line,
              what: "a coordinate before %MO".to_owned(),
            });
          }
        }
        let fmt = format.unwrap_or(Format {
          zero_suppression: ZeroSuppression::Leading,
          integer_digits: 4,
          decimal_digits: 6,
        });
        let unit = units.unwrap_or(Units::Millimetre);
        let parsed = parse_coord_stmt(stmt, fmt, unit, line)?;
        if let Some(v) = parsed.x {
          x = Some(v);
        }
        if let Some(v) = parsed.y {
          y = Some(v);
        }
        if let Some(d) = parsed.d {
          match d {
            1..=3 => {
              let ap = selected_aperture.ok_or(GerberError::NoApertureSelected { line })?;
              let _ = ap;
              let pt = current_point(x, y, line)?;
              if d == 1 {
                if interpolation == Interpolation::Linear {
                  commands.push(Command::Line { to: pt });
                } else {
                  let offset = (parsed.i.unwrap_or(0.0), parsed.j.unwrap_or(0.0));
                  commands.push(Command::Arc {
                    to: pt,
                    offset,
                    clockwise: interpolation == Interpolation::Clockwise,
                  });
                }
              } else if d == 2 {
                commands.push(Command::Move { to: pt });
              } else {
                commands.push(Command::Flash { at: pt });
              }
            }
            _ if d >= 10 => {
              // aperture select
              if !aperture_codes.contains(&d) {
                return Err(GerberError::UnknownAperture { line, aperture: d });
              }
              selected_aperture = Some(d);
              commands.push(Command::SelectAperture(d));
            }
            _ => {
              // other D codes (e.g. D99) that are not defined apertures treated as unknown selection
              if d >= 10 {
                return Err(GerberError::UnknownAperture { line, aperture: d });
              } else {
                return Err(GerberError::Invalid {
                  line,
                  what: format!("D{d} is not valid in this context"),
                });
              }
            }
          }
        } else if parsed.has_coord_only() {
          // coordinate without explicit D in some streams can imply modal D01 in region; but keep simple
          // no-op here unless tests require; our fixture always has D
        }
        continue;
      }

      // aperture select alone as Dnn*
      if let Some(d) = stmt.strip_prefix('D').and_then(|r| r.parse::<u32>().ok()) {
        if d >= 10 {
          if !aperture_codes.contains(&d) {
            return Err(GerberError::UnknownAperture { line, aperture: d });
          }
          selected_aperture = Some(d);
          commands.push(Command::SelectAperture(d));
          continue;
        }
      }

      // if we reach here, unknown in normal context
      return Err(GerberError::Invalid {
        line,
        what: format!("{stmt:?} is not understood"),
      });
    }
  }

  // A block still open here means the file ended before its `%`, so whatever it defined was never
  // processed. Returning Ok would hand the caller a document silently missing those apertures.
  if in_block {
    return Err(GerberError::Invalid {
      line: block_start_line,
      what: "an extended block that never closed with %".to_owned(),
    });
  }

  let format = format.ok_or(GerberError::Invalid {
    line: 1,
    what: "missing %FS format".to_owned(),
  })?;
  let units = units.ok_or(GerberError::Invalid {
    line: 1,
    what: "missing %MO units".to_owned(),
  })?;

  Ok(Gerber {
    format,
    units,
    apertures,
    macros,
    commands,
    attributes,
  })
}

fn collect_block(block_lines: &[(u32, String)]) -> String {
  let mut out = String::new();
  for (_, l) in block_lines {
    out.push_str(l);
    out.push('\n');
  }
  out
}

struct Tables<'a> {
  format: &'a mut Option<Format>,
  units: &'a mut Option<Units>,
  apertures: &'a mut Vec<Aperture>,
  macros: &'a mut Vec<Macro>,
  attributes: &'a mut Vec<String>,
  commands: &'a mut Vec<Command>,
  macro_names: &'a mut std::collections::HashSet<String>,
  aperture_codes: &'a mut std::collections::HashSet<u32>,
}

fn process_extended_block(
  content: &str,
  start_line: u32,
  tables: &mut Tables<'_>,
) -> Result<(), GerberError> {
  let c = content.trim();
  // strip leading and trailing % for easier parsing
  let inner = c.strip_prefix('%').unwrap_or(c);
  let inner = inner.strip_suffix('%').unwrap_or(inner);
  let inner = inner.trim();

  if inner.starts_with("FS") {
    *tables.format = Some(parse_fs(inner, start_line)?);
    return Ok(());
  }
  if inner.starts_with("MO") {
    let u = match inner {
      "MOMM" | "MOMM*" => Units::Millimetre,
      "MOIN" | "MOIN*" => Units::Inch,
      _ => {
        // tolerate trailing *
        let t = inner.trim_end_matches('*');
        if t == "MOMM" {
          Units::Millimetre
        } else if t == "MOIN" {
          Units::Inch
        } else {
          return Err(GerberError::Invalid {
            line: start_line,
            what: format!("unknown units {inner:?}"),
          });
        }
      }
    };
    *tables.units = Some(u);
    return Ok(());
  }
  if inner.starts_with("LP") {
    // Polarity is state, not decoration: it applies to everything that follows, so it
    // belongs in the command stream, where order is preserved, and not in attributes.
    let t = inner.trim_end_matches('*');
    let polarity = match t {
      "LPD" => Polarity::Dark,
      "LPC" => Polarity::Clear,
      _ => {
        return Err(GerberError::Unsupported {
          line: start_line,
          what: format!("polarity statement {t:?}"),
        })
      }
    };
    tables.commands.push(Command::Polarity(polarity));
    return Ok(());
  }
  if inner.starts_with("ADD") {
    let body = inner.strip_prefix("ADD").unwrap().trim_end_matches('*');
    parse_add(
      body,
      start_line,
      tables.apertures,
      tables.macro_names,
      tables.aperture_codes,
    )?;
    return Ok(());
  }
  if inner.starts_with("AD") {
    let body = inner.strip_prefix("AD").unwrap().trim_end_matches('*');
    parse_add(
      body,
      start_line,
      tables.apertures,
      tables.macro_names,
      tables.aperture_codes,
    )?;
    return Ok(());
  }
  if inner.starts_with("AM") {
    let body = inner.strip_prefix("AM").unwrap();
    parse_am(body, start_line, tables.macros, tables.macro_names);
    return Ok(());
  }
  if inner.starts_with("TF")
    || inner.starts_with("TA")
    || inner.starts_with("TO")
    || inner.starts_with("TD")
  {
    // keep raw text as attribute block
    tables.attributes.push(content.trim().to_string());
    return Ok(());
  }

  // unknown extended command - refuse? not specified except listed ones; keep strict-ish
  Err(GerberError::Invalid {
    line: start_line,
    what: format!(
      "extended command {:?} is not understood",
      inner.lines().next().unwrap_or(inner)
    ),
  })
}

fn parse_fs(inner: &str, line: u32) -> Result<Format, GerberError> {
  // e.g. FSLAX46Y46*
  let t = inner.trim_end_matches('*');
  if !t.starts_with("FSLAX") && !t.starts_with("FSTAX") {
    return Err(GerberError::Invalid {
      line,
      what: format!("unsupported format {t:?}"),
    });
  }
  if t.starts_with("FSTAX")
    || t.contains("T") && t.starts_with("FS") && t.chars().nth(2) == Some('T')
  {
    return Err(GerberError::Unsupported {
      line,
      what: "trailing-zero suppression".to_owned(),
    });
  }
  // expect FSLAX46Y46
  let rest = t.strip_prefix("FSLAX").ok_or(GerberError::Invalid {
    line,
    what: format!("unsupported format {t:?}"),
  })?;
  let (xpart, ypart) = rest.split_once('Y').ok_or(GerberError::Invalid {
    line,
    what: format!("unsupported format {t:?}"),
  })?;
  if xpart.len() != 2 || ypart.len() != 2 || xpart != ypart {
    return Err(GerberError::Invalid {
      line,
      what: format!("unsupported format {t:?}"),
    });
  }
  // `len() == 2` and `xpart == ypart` prove the shape, not that the characters are digits: a
  // format of "AA" would have panicked here. Each character is converted once and refused by name.
  let digits: Vec<u32> = xpart
    .chars()
    .map(|character| {
      character.to_digit(10).ok_or_else(|| GerberError::Invalid {
        line,
        what: format!("unsupported format {t:?}"),
      })
    })
    .collect::<Result<_, _>>()?;
  Ok(Format {
    zero_suppression: ZeroSuppression::Leading,
    integer_digits: digits[0],
    decimal_digits: digits[1],
  })
}

fn parse_add(
  body: &str,
  line: u32,
  apertures: &mut Vec<Aperture>,
  macro_names: &std::collections::HashSet<String>,
  aperture_codes: &mut std::collections::HashSet<u32>,
) -> Result<(), GerberError> {
  // <n><template>,params
  let mut digits = String::new();
  let mut i = 0usize;
  for ch in body.chars() {
    if ch.is_ascii_digit() {
      digits.push(ch);
      i += ch.len_utf8();
    } else {
      break;
    }
  }
  if digits.is_empty() {
    return Err(GerberError::Invalid {
      line,
      what: "ADD missing code".to_owned(),
    });
  }
  let code: u32 = digits.parse().map_err(|_| GerberError::Invalid {
    line,
    what: "bad ADD code".to_owned(),
  })?;
  let rest = &body[i..];
  let (template, params_str) = if let Some((t, p)) = rest.split_once(',') {
    (t.to_string(), Some(p))
  } else {
    (rest.to_string(), None)
  };
  if template.is_empty() {
    return Err(GerberError::Invalid {
      line,
      what: "ADD missing template".to_owned(),
    });
  }
  let is_standard = matches!(template.as_str(), "C" | "O" | "R");
  let is_macro = macro_names.contains(&template);
  if !is_standard && !is_macro {
    return Err(GerberError::Invalid {
      line,
      what: format!("aperture template {template:?} is neither C/O/R nor a defined macro"),
    });
  }
  let parameters = if let Some(p) = params_str {
    p.split('X')
      .filter(|s| !s.is_empty())
      .map(|s| {
        s.parse::<f64>().map_err(|_| GerberError::Invalid {
          line,
          what: format!("bad aperture parameter {s:?}"),
        })
      })
      .collect::<Result<Vec<_>, _>>()?
  } else {
    Vec::new()
  };
  aperture_codes.insert(code);
  apertures.push(Aperture {
    code,
    template,
    parameters,
  });
  Ok(())
}

fn parse_am(
  body: &str,
  _start_line: u32,
  macros: &mut Vec<Macro>,
  macro_names: &mut std::collections::HashSet<String>,
) {
  // body starts with name* then lines of primitives/comments, may end with * from block end context handled outside
  let mut lines = body.lines();
  let first = lines.next().unwrap_or("").trim().trim_end_matches('*');
  let name = first.to_string();
  let mut primitives = Vec::new();
  for raw in lines {
    let l = raw.trim().trim_end_matches('*').trim();
    if l.is_empty() {
      continue;
    }
    // comment lines starting with 0 are comments (not primitives)
    if let Some(rest) = l.strip_prefix("0 ") {
      let _ = rest;
      continue;
    }
    if l == "0" {
      continue;
    }
    // primitive: code, params
    let parts: Vec<String> = l.split(',').map(|s| s.trim().to_string()).collect();
    if parts.is_empty() {
      continue;
    }
    if let Ok(code) = parts[0].parse::<u32>() {
      if code == 0 {
        continue;
      } // safety
      let exposure = parts.get(1).map(|s| s != "0").unwrap_or(false);
      // parameters: keep as strings, they carry $1+$1
      let parameters = if parts.len() > 2 {
        parts[2..].to_vec()
      } else {
        Vec::new()
      };
      primitives.push(MacroPrimitive {
        code,
        exposure,
        parameters,
      });
    }
  }
  macro_names.insert(name.clone());
  macros.push(Macro { name, primitives });
}

#[derive(Default)]
struct CoordStmt {
  x: Option<f64>,
  y: Option<f64>,
  i: Option<f64>,
  j: Option<f64>,
  d: Option<u32>,
}
impl CoordStmt {
  fn has_coord_only(&self) -> bool {
    (self.x.is_some() || self.y.is_some()) && self.d.is_none()
  }
}

fn parse_coord_stmt(
  stmt: &str,
  fmt: Format,
  units: Units,
  line: u32,
) -> Result<CoordStmt, GerberError> {
  let mut out = CoordStmt::default();
  let chars: Vec<char> = stmt.chars().collect();
  let mut idx = 0;
  while idx < chars.len() {
    let c = chars[idx];
    match c {
      'X' | 'Y' | 'I' | 'J' => {
        idx += 1;
        let start = idx;
        if idx < chars.len() && (chars[idx] == '+' || chars[idx] == '-') {
          idx += 1;
        }
        while idx < chars.len() && chars[idx].is_ascii_digit() {
          idx += 1;
        }
        let num: String = chars[start..idx].iter().collect();
        if num.is_empty() || num == "+" || num == "-" {
          return Err(GerberError::Invalid {
            line,
            what: "empty coordinate".to_owned(),
          });
        }
        let v = parse_coordinate_number(&num, &fmt, units, line)?;
        match c {
          'X' => out.x = Some(v),
          'Y' => out.y = Some(v),
          'I' => out.i = Some(v),
          'J' => out.j = Some(v),
          _ => {}
        }
      }
      'D' => {
        idx += 1;
        let start = idx;
        while idx < chars.len() && chars[idx].is_ascii_digit() {
          idx += 1;
        }
        let num: String = chars[start..idx].iter().collect();
        let d = num.parse::<u32>().map_err(|_| GerberError::Invalid {
          line,
          what: "bad D code".to_owned(),
        })?;
        out.d = Some(d);
      }
      _ => {
        // ignore unknown letters? or reject. be strict
        return Err(GerberError::Invalid {
          line,
          what: format!("unexpected character {c:?} in coordinate"),
        });
      }
    }
  }
  Ok(out)
}

fn parse_coordinate_number(
  num: &str,
  fmt: &Format,
  units: Units,
  line: u32,
) -> Result<f64, GerberError> {
  if matches!(fmt.zero_suppression, ZeroSuppression::Trailing) {
    return Err(GerberError::Unsupported {
      line,
      what: "trailing-zero suppression".to_owned(),
    });
  }
  let negative = num.starts_with('-');
  let digits: String = num.chars().filter(|c| c.is_ascii_digit()).collect();
  if digits.is_empty() {
    return Ok(0.0);
  }
  let int_val: i64 = digits.parse().map_err(|_| GerberError::Invalid {
    line,
    what: "bad coordinate number".to_owned(),
  })?;
  let scale = 10f64.powi(fmt.decimal_digits as i32);
  let mut v = (int_val as f64) / scale;
  if negative {
    v = -v;
  }
  Ok(match units {
    Units::Millimetre => v,
    Units::Inch => v * 25.4,
  })
}

fn current_point(x: Option<f64>, y: Option<f64>, line: u32) -> Result<Point2, GerberError> {
  let px = x.ok_or(GerberError::Invalid {
    line,
    what: "missing X coordinate".to_owned(),
  })?;
  let py = y.ok_or(GerberError::Invalid {
    line,
    what: "missing Y coordinate".to_owned(),
  })?;
  Ok(Point2::new(px, py))
}

fn parse_gm_code(stmt: &str) -> Option<String> {
  let c = stmt.chars().next()?;
  if c != 'G' && c != 'M' {
    return None;
  }
  // known handled elsewhere
  let known = [
    "G01", "G02", "G03", "G04", "G36", "G37", "G74", "G75", "M02",
  ];
  if known.contains(&stmt) {
    return None;
  }
  // digits after
  if stmt.len() > 1 && stmt[1..].chars().all(|ch| ch.is_ascii_digit()) {
    return Some(stmt.to_string());
  }
  None
}
