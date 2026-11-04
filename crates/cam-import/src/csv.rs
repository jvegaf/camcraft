//! CSV part lists: the columns, the dialect and the row type the caller turns into a part.
//!
//! A part list is one row per part, in millimetres, and this module's job ends at parsing: turning a
//! [`PartRow`] into a `cam_model::NestingPart` and its geometry is the caller's task, because that is
//! where the document, the entity ids and the validation live.
//!
//! # The dialect
//!
//! Four fields per row, separated by commas, in the order [`COLUMNS`] names them: `name`, `width`,
//! `height` and `quantity`. Fields are trimmed of surrounding spaces, so `bracket, 120, 80, 4` and
//! `bracket,120,80,4` are the same row.
//!
//! * **The header row is required** and it is the first line that is neither blank nor a comment. It
//!   must name the four columns **in that exact order**, matched case-insensitively; a header whose
//!   `width` and `height` are swapped is refused at its own line and field instead of silently
//!   producing rotated parts. There is no header-less form: the column order is never guessed.
//! * **Blank lines and `#` comments are allowed anywhere** and skipped, the convention the script DSL
//!   uses. A line is a comment when its first non-blank character is `#`. There are no trailing
//!   comments — a `#` after a field is part of that field's value — so a part may be named `part#1`.
//! * **Quoted fields are refused by name**, never guessed at: a `"` anywhere in a field is a located
//!   error, so a file exported with quoting is refused instead of being cut into a wrong name.
//! * **A duplicate part name is an error**, not a sum: `quantity` is already a column, so summing
//!   would silently change the quantity the list asked for and hide the copy-paste that made the
//!   row. Names are compared exactly, case included.
//! * **A byte order mark and CRLF line endings are tolerated**, because a part list is authored in a
//!   spreadsheet; the file itself must be UTF-8, and a byte that is not is refused with its line.
//!
//! # Refusals
//!
//! Every refusal names the line and, where one field is at fault, the field, its column and the
//! value that was read: `csv at line 3, field 2 ("width"): expected a positive finite number of
//! millimetres, found "0"`. A list with no rows at all is refused as nothing imported rather than
//! handed back empty, so a mistyped path cannot look like an empty job.

use crate::ImportError;

/// The format name every refusal in this module carries.
const FORMAT: &str = "csv";

/// The four columns, in the order the header row must name them.
pub const COLUMNS: [&str; 4] = ["name", "width", "height", "quantity"];

/// The header row, as the refusal message spells it.
const HEADER: &str = "name,width,height,quantity";

/// How many fields a row must have.
const FIELDS: usize = COLUMNS.len();

/// One part list row, as read.
#[derive(Debug, Clone, PartialEq)]
pub struct PartRow {
  /// 1-based line the row came from, so a later refusal can point at it.
  pub line: usize,
  /// Part name, trimmed; never empty.
  pub name: String,
  /// Width in millimetres; positive and finite.
  pub width: f64,
  /// Height in millimetres; positive and finite.
  pub height: f64,
  /// How many copies of the part; at least one.
  pub quantity: usize,
}

/// A part list, and what reading it did.
#[derive(Debug, Clone, PartialEq)]
pub struct PartList {
  /// The parts, in file order.
  pub parts: Vec<PartRow>,
  /// What reading the file did.
  pub report: Report,
}

/// What reading a part list did, in the terms a reviewer checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
  /// Parts read.
  pub parts: usize,
  /// Line the header row was on.
  pub header_line: usize,
  /// Comment lines skipped.
  pub comments: usize,
  /// Blank lines skipped.
  pub blank_lines: usize,
}

impl Report {
  /// Render the report as stable text.
  ///
  /// Determinism matters here: `cam-cli` prints this, and a test asserts it whole.
  pub fn render(&self) -> String {
    format!(
      "format: {FORMAT}\nheader: line {}\nparts: {}\ncomments: {}\nblank lines: {}\n",
      self.header_line, self.parts, self.comments, self.blank_lines
    )
  }
}

/// Read a part list from CSV text.
///
/// Nothing is written and no document is touched: the rows come back typed, or the first thing wrong
/// comes back located.
pub fn import(bytes: &[u8]) -> Result<PartList, ImportError> {
  let text = decode(bytes)?;
  let text = text.strip_prefix('\u{feff}').unwrap_or(text);

  let mut report = Report {
    parts: 0,
    header_line: 0,
    comments: 0,
    blank_lines: 0,
  };
  let mut parts: Vec<PartRow> = Vec::new();
  let mut header_line: Option<usize> = None;

  for (index, raw) in text.lines().enumerate() {
    let number = index + 1;
    let line = raw.trim();
    if line.is_empty() {
      report.blank_lines += 1;
      continue;
    }
    if line.starts_with('#') {
      report.comments += 1;
      continue;
    }

    let fields = split(line, number)?;
    match header_line {
      None => {
        check_header(&fields, number)?;
        header_line = Some(number);
      }
      Some(_) => parts.push(read_row(&fields, number, &parts)?),
    }
  }

  let Some(header_line) = header_line else {
    return Err(ImportError::NothingImported {
      format: FORMAT,
      reason: "no header row: every line was blank or a comment".to_string(),
    });
  };
  if parts.is_empty() {
    return Err(ImportError::NothingImported {
      format: FORMAT,
      reason: "the header row is not followed by any part row".to_string(),
    });
  }

  report.parts = parts.len();
  report.header_line = header_line;
  Ok(PartList { parts, report })
}

/// The file as text, or a refusal naming the line the first bad byte is on.
fn decode(bytes: &[u8]) -> Result<&str, ImportError> {
  std::str::from_utf8(bytes).map_err(|error| {
    let prefix = &bytes[..error.valid_up_to()];
    let line = 1 + prefix.iter().filter(|byte| **byte == b'\n').count();
    ImportError::parse(
      FORMAT,
      format!(
        "the file is not valid UTF-8 (byte {} is not part of a character)",
        error.valid_up_to()
      ),
      format!("line {line}"),
    )
  })
}

/// Split a data line into its trimmed fields, refusing a wrong field count and any quoting.
fn split(line: &str, number: usize) -> Result<Vec<&str>, ImportError> {
  let fields: Vec<&str> = line.split(',').map(str::trim).collect();

  if fields.len() != FIELDS {
    return Err(refusal(
      number,
      format!(
        "expected {FIELDS} fields ({HEADER}), found {}",
        fields.len()
      ),
    ));
  }

  for (index, field) in fields.iter().enumerate() {
    if field.contains('"') {
      return Err(field_refusal(
        number,
        index,
        format!("quoted fields are not supported; found '\"' in {field:?}"),
      ));
    }
  }

  Ok(fields)
}

/// Refuse a header that does not name the four columns in the fixed order.
fn check_header(fields: &[&str], number: usize) -> Result<(), ImportError> {
  for (index, (field, column)) in fields.iter().zip(COLUMNS).enumerate() {
    if !field.eq_ignore_ascii_case(column) {
      return Err(field_refusal(
        number,
        index,
        format!("expected the header row {HEADER:?}, found {field:?}"),
      ));
    }
  }
  Ok(())
}

/// Read one data row, refusing each field that is not what its column asks for.
fn read_row(fields: &[&str], number: usize, parts: &[PartRow]) -> Result<PartRow, ImportError> {
  let name = fields[0];
  if name.is_empty() {
    return Err(field_refusal(
      number,
      0,
      format!("expected a non-empty part name, found {name:?}"),
    ));
  }

  let width = dimension(fields[1], number, 1)?;
  let height = dimension(fields[2], number, 2)?;
  let quantity = quantity(fields[3], number)?;

  if let Some(previous) = parts.iter().find(|part| part.name == name) {
    return Err(field_refusal(
      number,
      0,
      format!(
        "duplicate part name {name:?}: already named on line {}",
        previous.line
      ),
    ));
  }

  Ok(PartRow {
    line: number,
    name: name.to_string(),
    width,
    height,
    quantity,
  })
}

/// A width or a height: a positive, finite number of millimetres.
fn dimension(value: &str, number: usize, index: usize) -> Result<f64, ImportError> {
  match value.parse::<f64>() {
    Ok(parsed) if parsed.is_finite() && parsed > 0.0 => Ok(parsed),
    _ => Err(field_refusal(
      number,
      index,
      format!("expected a positive finite number of millimetres, found {value:?}"),
    )),
  }
}

/// A quantity: a positive whole number of copies.
fn quantity(value: &str, number: usize) -> Result<usize, ImportError> {
  match value.parse::<usize>() {
    Ok(parsed) if parsed > 0 => Ok(parsed),
    _ => Err(field_refusal(
      number,
      3,
      format!("expected a positive integer, found {value:?}"),
    )),
  }
}

/// A refusal that names the line, for a fault that is the whole line's.
fn refusal(line: usize, message: String) -> ImportError {
  ImportError::Parse {
    format: FORMAT,
    message,
    location: Some(format!("line {line}")),
  }
}

/// A refusal that names the line, the field and the column that field belongs to.
fn field_refusal(line: usize, index: usize, message: String) -> ImportError {
  ImportError::Parse {
    format: FORMAT,
    message,
    location: Some(format!(
      "line {line}, field {} (\"{}\")",
      index + 1,
      COLUMNS[index]
    )),
  }
}
