//! Excellon drill files: the tool table, the coordinates and the slots.
//!
//! An Excellon file is a list of holes and routed slots addressed by tool number. This reads the
//! dialect KiCad, Altium and Eagle write — a header between `M48` and `%`, then commands — and
//! produces an [`Excellon`] that names every feature with the tool that cuts it, in millimetres.
//!
//! What it refuses matters as much as what it reads. A coordinate before the units are declared and
//! an integer coordinate in a file that never declares its digit format are both diagnosed with their
//! line rather than guessed at, because a guessed scale puts the drill in the wrong place on the
//! board. Both coordinate modes are read: `G91` is a step from where the tool already is.

use cam_geom::Point2;
use cam_units::Length;

/// The unit an Excellon file's coordinates and diameters are written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Units {
  /// Millimetres, declared `METRIC` or `M71`.
  Metric,
  /// Inches, declared `INCH` or `M72`. Converted to millimetres on read.
  Inch,
}

impl Units {
  /// `value` in this unit, as millimetres.
  fn to_mm(self, value: f64) -> f64 {
    match self {
      Self::Metric => value,
      Self::Inch => value * 25.4,
    }
  }
}

/// One entry of the file's tool table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tool {
  /// The `T` number the body selects with.
  pub number: u32,
  /// Cutting diameter, in millimetres.
  pub diameter: Length,
}

/// One thing the file asks the machine to cut.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Feature {
  /// One plunge at a point.
  Hole {
    /// Tool that drills it.
    tool: u32,
    /// Centre, in millimetres from the file's origin.
    at: Point2,
  },
  /// One routed slot, cut with the tool down from `from` to `to`.
  Slot {
    /// Tool that cuts it.
    tool: u32,
    /// Where the tool went down.
    from: Point2,
    /// Where it came up.
    to: Point2,
  },
}

/// Everything a file says.
#[derive(Clone, Debug, PartialEq)]
pub struct Excellon {
  /// The unit the file declared.
  pub units: Units,
  /// The tool table, in the order the file lists it.
  pub tools: Vec<Tool>,
  /// Holes and slots, in the order the file cuts them.
  pub features: Vec<Feature>,
}

/// Why a file could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ExcellonError {
  /// No `METRIC` or `INCH` was declared before a value that depends on it.
  #[error("line {line}: no METRIC or INCH is declared, so this value has no scale")]
  MissingUnits {
    /// Line the offending value is on.
    line: u32,
  },
  /// The file uses something this reader does not implement.
  #[error("line {line}: {what} is not supported")]
  Unsupported {
    /// Line the construct is on.
    line: u32,
    /// What was found.
    what: String,
  },
  /// A feature arrived before any tool was selected.
  #[error("line {line}: a hole or slot before any tool was selected")]
  NoToolSelected {
    /// Line the feature is on.
    line: u32,
  },
  /// The body selected a tool the header never defined.
  #[error("line {line}: tool {tool} is not in this file's table")]
  UnknownTool {
    /// Line the selection is on.
    line: u32,
    /// The tool number selected.
    tool: u32,
  },
  /// A line that is neither a coordinate nor a command this reader knows.
  #[error("line {line}: {word:?} is not a command or a coordinate")]
  UnknownWord {
    /// Line the word is on.
    line: u32,
    /// The word itself.
    word: String,
  },
  /// A tool table entry that does not read as `TnCdiameter`.
  #[error("line {line}: {entry:?} is not a tool definition")]
  MalformedTool {
    /// Line the entry is on.
    line: u32,
    /// The entry itself.
    entry: String,
  },
  /// A spindle command with nothing to act on.
  #[error("line {line}: the tool went down at no position")]
  NoPosition {
    /// Line the command is on.
    line: u32,
  },
}

/// The two command shapes a coordinate line can carry.
#[derive(Clone, Copy)]
enum Command {
  /// `G00`: move with the tool up.
  Rapid,
  /// `G01`: cut with the tool down.
  Feed,
}

/// Read an Excellon file.
///
/// # Errors
///
/// Returns the first construct it cannot honour, with the line it is on. A file is read whole or not
/// at all: a partially understood drill program is worse than a refusal, because the holes it does
/// not mention are the ones nobody notices are missing.
pub fn parse(text: &str) -> Result<Excellon, ExcellonError> {
  let mut units: Option<Units> = None;
  // The digit split an integer coordinate is read with, declared either by a `;FILE_FORMAT=i:d`
  // comment or by a zero pattern such as `METRIC,000.000`.
  let mut format: Option<(u32, u32)> = None;
  let mut raw_tools: Vec<(u32, f64)> = Vec::new();
  let mut features: Vec<Feature> = Vec::new();
  let mut in_header = false;
  let mut selected: Option<u32> = None;
  let mut position = Point2::new(0.0, 0.0);
  let mut have_position = false;
  let mut rapid_from: Option<Point2> = None;
  let mut tool_down = false;
  // `G91` makes every coordinate a step from the current position; `G90` returns to absolute.
  let mut relative = false;

  for (index, raw) in text.lines().enumerate() {
    let line = index as u32 + 1;
    // Everything after `;` is a comment, and KiCad writes the digit format there, so read it
    // before the comment is dropped.
    if let Some(declared) = file_format_of(raw) {
      format = Some(declared);
    }
    let code = raw.split(';').next().unwrap_or("").trim();
    if code.is_empty() {
      continue;
    }
    if code == "M48" {
      in_header = true;
      continue;
    }
    if code == "%" {
      in_header = false;
      continue;
    }
    if code == "M30" {
      break;
    }

    if in_header {
      if code == "FMAT,2" {
        continue;
      }
      if let Some(rest) = code
        .strip_prefix("METRIC")
        .or_else(|| code.strip_prefix("INCH"))
      {
        units = Some(if code.starts_with("METRIC") {
          Units::Metric
        } else {
          Units::Inch
        });
        // A modifier is either a suppression flag, which says nothing about how many digits there
        // are, or a zero pattern, which does: `METRIC,000.000` is three integer digits and three
        // decimals. Only the second one can be read as a scale.
        if let Some(modifier) = rest.strip_prefix(',') {
          if let Some(declared) = digits_of_pattern(modifier.trim()) {
            format = Some(declared);
          }
        }
        continue;
      }
      if let Some((number, diameter)) = tool_definition(code) {
        raw_tools.push((number, diameter));
        continue;
      }
      return Err(ExcellonError::Unsupported {
        line,
        what: format!("the header entry {code:?}"),
      });
    }

    match code {
      "G90" => {
        relative = false;
        continue;
      }
      "G05" | "G81" => continue,
      "G91" => {
        relative = true;
        continue;
      }
      "M71" => {
        units = Some(Units::Metric);
        continue;
      }
      "M72" => {
        units = Some(Units::Inch);
        continue;
      }
      "M15" => {
        if !have_position {
          return Err(ExcellonError::NoPosition { line });
        }
        tool_down = true;
        rapid_from = Some(position);
        continue;
      }
      "M16" => {
        tool_down = false;
        rapid_from = None;
        continue;
      }
      _ => {}
    }

    if let Some(rest) = code.strip_prefix('T') {
      if rest.chars().all(|character| character.is_ascii_digit()) && !rest.is_empty() {
        let number: u32 = rest.parse().map_err(|_| ExcellonError::MalformedTool {
          line,
          entry: code.to_owned(),
        })?;
        if !raw_tools.iter().any(|(known, _)| *known == number) {
          return Err(ExcellonError::UnknownTool { line, tool: number });
        }
        selected = Some(number);
        continue;
      }
      return Err(ExcellonError::MalformedTool {
        line,
        entry: code.to_owned(),
      });
    }

    // A routed slot written inline as `X..Y..G85X..Y..`: move to the first point, cut to the
    // second. It is one feature, and the tool ends at the second point.
    if let Some((before, after)) = code.split_once("G85") {
      let Some(units) = units else {
        return Err(ExcellonError::MissingUnits { line });
      };
      let Some(tool) = selected else {
        return Err(ExcellonError::NoToolSelected { line });
      };
      let (from, have_from) = apply_coordinates(
        before,
        position,
        have_position,
        relative,
        units,
        format,
        line,
      )?;
      let (to, _) = apply_coordinates(after, from, have_from, relative, units, format, line)?;
      features.push(Feature::Slot { tool, from, to });
      position = to;
      have_position = true;
      rapid_from = Some(position);
      tool_down = false;
      continue;
    }

    // Three shapes remain: a rapid, a feed, and a bare coordinate — which is a hole in drill mode,
    // or the next leg of a slot while the tool is down.
    let command = if code.starts_with("G00") {
      Some(Command::Rapid)
    } else if code.starts_with("G01") {
      Some(Command::Feed)
    } else if code.starts_with('X') || code.starts_with('Y') {
      None
    } else {
      return Err(ExcellonError::UnknownWord {
        line,
        word: code.to_owned(),
      });
    };

    let body = code
      .strip_prefix("G00")
      .or_else(|| code.strip_prefix("G01"))
      .unwrap_or(code);
    let Some(units) = units else {
      return Err(ExcellonError::MissingUnits { line });
    };
    let (position_new, _) =
      apply_coordinates(body, position, have_position, relative, units, format, line)?;
    position = position_new;
    have_position = true;

    if matches!(command, Some(Command::Rapid)) {
      rapid_from = Some(position);
      continue;
    }

    let Some(tool) = selected else {
      return Err(ExcellonError::NoToolSelected { line });
    };
    if matches!(command, Some(Command::Feed)) {
      let Some(from) = rapid_from else {
        return Err(ExcellonError::Unsupported {
          line,
          what: "a feed with the tool up".to_owned(),
        });
      };
      features.push(Feature::Slot {
        tool,
        from,
        to: position,
      });
      continue;
    }
    if tool_down {
      let from = rapid_from.unwrap_or(position);
      features.push(Feature::Slot {
        tool,
        from,
        to: position,
      });
      rapid_from = Some(position);
      continue;
    }
    features.push(Feature::Hole { tool, at: position });
  }

  let Some(units) = units else {
    return Err(ExcellonError::MissingUnits { line: 0 });
  };
  let tools = raw_tools
    .into_iter()
    .map(|(number, diameter)| Tool {
      number,
      diameter: Length::from_mm(units.to_mm(diameter)),
    })
    .collect();
  Ok(Excellon {
    units,
    tools,
    features,
  })
}

/// `TnCdiameter`, the header's tool table entry.
fn tool_definition(code: &str) -> Option<(u32, f64)> {
  let rest = code.strip_prefix('T')?;
  let (number, diameter) = rest.split_once('C')?;
  Some((number.parse().ok()?, diameter.parse().ok()?))
}

/// The digit format a `;FILE_FORMAT=i:d` comment declares.
fn file_format_of(line: &str) -> Option<(u32, u32)> {
  let marker = line.find("FILE_FORMAT=")?;
  let rest = &line[marker + "FILE_FORMAT=".len()..];
  let end = rest
    .char_indices()
    .find(|(_, character)| !character.is_ascii_digit() && *character != ':')
    .map(|(index, _)| index)
    .unwrap_or(rest.len());
  let (integer, decimal) = rest[..end].split_once(':')?;
  Some((integer.parse().ok()?, decimal.parse().ok()?))
}

/// The digit format a zero pattern such as `000.000` declares: three and three.
fn digits_of_pattern(pattern: &str) -> Option<(u32, u32)> {
  let (integer, decimal) = pattern.split_once('.')?;
  if integer.is_empty()
    || decimal.is_empty()
    || !integer.chars().all(|character| character == '0')
    || !decimal.chars().all(|character| character == '0')
  {
    return None;
  }
  Some((integer.len() as u32, decimal.len() as u32))
}

/// Apply the `X`/`Y` values in `body` to `position`, returning the new position.
///
/// `relative` makes each axis a step from the current value. A body that carries neither axis is
/// refused, because it names no place for the tool to go.
fn apply_coordinates(
  body: &str,
  mut position: Point2,
  _have_position: bool,
  relative: bool,
  units: Units,
  format: Option<(u32, u32)>,
  line: u32,
) -> Result<(Point2, bool), ExcellonError> {
  let (x, y) = coordinates(body, units, format, line)?;
  if x.is_none() && y.is_none() {
    return Err(ExcellonError::UnknownWord {
      line,
      word: body.to_owned(),
    });
  }
  if let Some(x) = x {
    position = Point2::new(if relative { position.x + x } else { x }, position.y);
  }
  if let Some(y) = y {
    position = Point2::new(position.x, if relative { position.y + y } else { y });
  }
  Ok((position, true))
}

/// The `X` and `Y` a coordinate line carries, in millimetres.
///
/// An axis the line omits keeps its previous value, which the caller applies; both absent is an
/// error the caller reports as an unknown word.
fn coordinates(
  body: &str,
  units: Units,
  format: Option<(u32, u32)>,
  line: u32,
) -> Result<(Option<f64>, Option<f64>), ExcellonError> {
  let mut x = None;
  let mut y = None;
  let mut rest = body;
  while let Some(character) = rest.chars().next() {
    if character != 'X' && character != 'Y' {
      return Err(ExcellonError::UnknownWord {
        line,
        word: rest.to_owned(),
      });
    }
    rest = &rest[1..];
    let end = rest
      .char_indices()
      .find(|(_, character)| *character == 'X' || *character == 'Y' || *character == ' ')
      .map(|(index, _)| index)
      .unwrap_or(rest.len());
    let number = &rest[..end];
    // A file that writes a decimal point states its own scale. One that does not is written in the
    // fixed digit split its header declares, and reading those digits as millimetres would put the
    // hole a factor of the decimal count away, so a missing declaration is refused, not assumed.
    let raw_value: f64 = if number.contains('.') {
      number.parse().map_err(|_| ExcellonError::UnknownWord {
        line,
        word: number.to_owned(),
      })?
    } else {
      let digits: f64 = number.parse().map_err(|_| ExcellonError::UnknownWord {
        line,
        word: number.to_owned(),
      })?;
      // Zero is zero at any scale, so it is the one integer that needs no declaration - and `Y0`
      // is how an ordinary drill line writes a hole on the axis.
      if digits == 0.0 {
        0.0
      } else {
        let Some((_, decimal)) = format else {
          return Err(ExcellonError::Unsupported {
            line,
            what: format!(
              "the coordinate {number:?} has no decimal point and the file never declares its digit format"
            ),
          });
        };
        digits / 10f64.powi(decimal as i32)
      }
    };
    let value = units.to_mm(raw_value);
    if character == 'X' {
      x = Some(value);
    } else {
      y = Some(value);
    }
    rest = &rest[end..];
  }
  Ok((x, y))
}
