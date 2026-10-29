//! Parser for the `.pp` post-processor dialect.

use crate::{BlockKind, EmissionMode, PostError, PostProcessor, PostUnits, VarSpec};

/// Comment markers seen in the dialect: `+` in shipped files, `|` in the versioned store.
fn is_comment(line: &str) -> bool {
  line.starts_with('+') || line.starts_with('|')
}

/// Parse a post-processor.
pub fn parse_post(text: &str) -> Result<PostProcessor, PostError> {
  let mut post = PostProcessor::default();
  let mut current: Option<BlockKind> = None;

  for (index, raw) in text.lines().enumerate() {
    let number = index + 1;
    let line = raw.trim();
    if line.is_empty() || is_comment(line) {
      continue;
    }

    if let Some(name) = strip_keyword(line, "begin") {
      let kind = BlockKind::parse(name).ok_or_else(|| {
        PostError::new(
          number,
          column_of(line, name) + 1,
          format!(
            "unknown output block {name:?}; expected one of {}",
            BlockKind::ALL
              .iter()
              .map(|kind| kind.as_str())
              .collect::<Vec<_>>()
              .join(", ")
          ),
        )
      })?;
      post.blocks.entry(kind).or_default();
      current = Some(kind);
      continue;
    }

    if let Some(body) = strip_keyword(line, "VAR") {
      let declaration = parse_var(body, number, line)?;
      if post.variables.contains_key(&declaration.0) {
        return Err(PostError::new(
          number,
          1,
          format!("duplicate declaration of variable {:?}", declaration.0),
        ));
      }
      post.variables.insert(declaration.0, declaration.1);
      current = None;
      continue;
    }

    if let Some((key, value)) = split_directive(line) {
      apply_directive(&mut post, key, value, number, line)?;
      current = None;
      continue;
    }

    let Some(kind) = current else {
      return Err(PostError::new(
        number,
        1,
        format!("output line {line:?} appears before any begin block"),
      ));
    };
    let text = unquote(line, number)?;
    post.blocks.entry(kind).or_default().push(text);
  }

  Ok(post)
}

/// Split `KEYWORD rest` case-insensitively, returning `rest`.
fn strip_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
  let (head, rest) = line.split_at_checked(keyword.len())?;
  if head.eq_ignore_ascii_case(keyword) {
    let rest = rest.trim_start();
    if rest.len() < line.len() - keyword.len() {
      Some(rest)
    } else {
      None
    }
  } else {
    None
  }
}

/// Split `KEY = value` into its parts.
fn split_directive(line: &str) -> Option<(&str, &str)> {
  let (key, value) = line.split_once('=')?;
  let key = key.trim();
  if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
    return None;
  }
  Some((key, value.trim()))
}

/// Byte column of `needle` within `line`, 0-based.
fn column_of(line: &str, needle: &str) -> usize {
  line.find(needle).unwrap_or(0)
}

fn apply_directive(
  post: &mut PostProcessor,
  key: &str,
  value: &str,
  number: usize,
  line: &str,
) -> Result<(), PostError> {
  let upper = key.to_ascii_uppercase();
  match upper.as_str() {
    "POST_NAME" => post.post_name = unquote(value, number)?,
    "FILE_EXTENSION" => {
      let extension = unquote(value, number)?;
      let extension = extension.trim_start_matches('.').to_owned();
      if extension.is_empty() {
        return Err(PostError::new(
          number,
          column_of(line, "FILE_EXTENSION") + 1,
          "FILE_EXTENSION must not be empty",
        ));
      }
      post.file_extension = extension;
    }
    "UNITS" => {
      let units = unquote(value, number)?;
      post.units = PostUnits::parse(&units).ok_or_else(|| {
        PostError::new(
          number,
          column_of(line, "UNITS") + 1,
          format!("unknown UNITS {units:?}; expected \"MM\" or \"INCHES\""),
        )
      })?;
    }
    "LINE_ENDING" => post.line_ending = unquote(value, number)?,
    "LINE_NUMBER_START" => post.line_number_start = Some(parse_integer(value, number, line, 0)?),
    "LINE_NUMBER_INCREMENT" => {
      post.line_number_increment = Some(parse_integer(value, number, line, 1)?)
    }
    "LINE_NUMBER_MAXIMUM" => {
      post.line_number_maximum = Some(parse_integer(value, number, line, 100_000)?)
    }
    "DIRECT_OUTPUT" => post.direct_output = Some(unquote(value, number)?),
    "SUBSTITUTE" => post.substitute = Some(unquote(value, number)?),
    "ROTARY_WRAP_X" => post.rotary_wrap_x = Some(unquote_or_bare(value, number)?),
    "ROTARY_WRAP_Y" => post.rotary_wrap_y = Some(unquote_or_bare(value, number)?),
    "RAPID_PLUNGE_TO_STARTZ" => post.rapid_plunge_to_start_z = parse_flag(value, number, line, 0)?,
    "PRINT_DIRECT" => post.print_direct = parse_flag(value, number, line, 1)?,
    other => {
      return Err(PostError::new(
        number,
        1,
        format!(
          "unknown directive {other:?}; expected one of POST_NAME, FILE_EXTENSION, UNITS, \
           LINE_ENDING, LINE_NUMBER_START, LINE_NUMBER_INCREMENT, LINE_NUMBER_MAXIMUM, \
           DIRECT_OUTPUT, SUBSTITUTE, ROTARY_WRAP_X, ROTARY_WRAP_Y, RAPID_PLUNGE_TO_STARTZ, \
           PRINT_DIRECT or VAR"
        ),
      ))
    }
  }
  Ok(())
}

/// Parse `NAME = [source|mode|prefix|decimals]` (optionally with a fifth scale field).
fn parse_var(body: &str, number: usize, line: &str) -> Result<(String, VarSpec), PostError> {
  let (name, value) = body.split_once('=').ok_or_else(|| {
    PostError::new(
      number,
      1,
      format!(
        "VAR declaration {line:?} is missing `=`; expected `VAR NAME = [src|mode|prefix|decimals]`"
      ),
    )
  })?;
  let name = name.trim().to_ascii_uppercase();
  if name.is_empty() {
    return Err(PostError::new(number, 1, "VAR declaration has no name"));
  }
  let value = value.trim();
  let inner = value
    .strip_prefix('[')
    .and_then(|rest| rest.strip_suffix(']'));
  let Some(inner) = inner else {
    return Err(PostError::new(
      number,
      column_of(line, "[").max(1),
      format!("VAR {name} value {value:?} must be bracketed, as in [X|C|X|1.4]"),
    ));
  };

  let fields: Vec<&str> = inner.split('|').collect();
  if fields.len() < 4 || fields.len() > 5 {
    return Err(PostError::new(
      number,
      column_of(line, "[") + 1,
      format!(
        "VAR {name} has {} field(s) in [{}]; expected 4 ([source|mode|prefix|decimals]) or 5 with a scale factor",
        fields.len(),
        inner
      ),
    ));
  }

  let source = fields[0].trim().to_owned();
  if source.is_empty() {
    return Err(PostError::new(
      number,
      column_of(line, "[") + 1,
      format!("VAR {name} has an empty source field"),
    ));
  }

  let mode = match fields[1].trim().to_ascii_uppercase().as_str() {
    "A" | "ALWAYS" => EmissionMode::Always,
    "C" | "ONCHANGE" | "ON_CHANGE" => EmissionMode::OnChange,
    other => {
      return Err(PostError::new(
        number,
        column_of(line, "[") + 1,
        format!("VAR {name} mode {other:?} is invalid; expected A (always) or C (on change)"),
      ))
    }
  };

  let prefix = fields[2].trim().to_owned();
  for character in prefix.chars() {
    if !character.is_ascii_alphabetic() {
      return Err(PostError::new(
        number,
        column_of(line, "[") + 1,
        format!("VAR {name} prefix {prefix:?} must be letters only, such as X or none"),
      ));
    }
  }

  let decimals = parse_decimals(fields[3].trim(), &name, number, line)?;
  let scale = match fields.get(4) {
    None => 1.0,
    Some(raw) => raw.trim().parse::<f64>().map_err(|_| {
      PostError::new(
        number,
        column_of(line, "[") + 1,
        format!(
          "VAR {name} scale {:?} is not a number; expected a multiplier such as 0.0166",
          raw.trim()
        ),
      )
    })?,
  };
  if scale == 0.0 || !scale.is_finite() {
    return Err(PostError::new(
      number,
      column_of(line, "[") + 1,
      format!("VAR {name} scale must be finite and non-zero"),
    ));
  }

  Ok((
    name,
    VarSpec {
      source,
      mode,
      prefix,
      decimals,
      scale,
    },
  ))
}

/// Read the dialect's decimal field.
///
/// The dialect writes `1.4` to mean **four** decimals: the integer part is a fixed marker and the
/// fractional part carries the count. A bare integer is also accepted.
fn parse_decimals(raw: &str, name: &str, number: usize, line: &str) -> Result<u8, PostError> {
  let digits = match raw.split_once('.') {
    Some((_, fraction)) => fraction,
    None => raw,
  };
  if digits.is_empty() {
    return Err(PostError::new(
      number,
      column_of(line, "[") + 1,
      format!("VAR {name} decimal field {raw:?} carries no digit count"),
    ));
  }
  digits.parse::<u8>().map_err(|_| {
    PostError::new(
      number,
      column_of(line, "[") + 1,
      format!("VAR {name} decimal field {raw:?} is not a digit count; expected forms like 1.4"),
    )
  })
}

fn parse_integer(value: &str, number: usize, line: &str, _default: i64) -> Result<i64, PostError> {
  let value = unquote_or_bare(value, number)?;
  value.parse::<i64>().map_err(|_| {
    PostError::new(
      number,
      column_of(line, "=") + 2,
      format!("expected an integer, found {value:?}"),
    )
  })
}

fn parse_flag(value: &str, number: usize, line: &str, _default: u8) -> Result<bool, PostError> {
  let value = unquote_or_bare(value, number)?;
  match value.to_ascii_uppercase().as_str() {
    "YES" | "TRUE" | "1" => Ok(true),
    "NO" | "FALSE" | "0" => Ok(false),
    other => Err(PostError::new(
      number,
      column_of(line, "=") + 2,
      format!("expected YES or NO, found {other:?}"),
    )),
  }
}

fn unquote(value: &str, number: usize) -> Result<String, PostError> {
  let value = value.trim();
  match value
    .strip_prefix('"')
    .and_then(|rest| rest.strip_suffix('"'))
  {
    Some(inner) => Ok(inner.to_owned()),
    None => Err(PostError::new(
      number,
      1,
      format!("expected a quoted value, found {value:?}"),
    )),
  }
}

fn unquote_or_bare(value: &str, number: usize) -> Result<String, PostError> {
  let value = value.trim();
  if value.starts_with('"') {
    unquote(value, number)
  } else if value.is_empty() {
    Err(PostError::new(number, 1, "expected a value, found nothing"))
  } else {
    Ok(value.to_owned())
  }
}
