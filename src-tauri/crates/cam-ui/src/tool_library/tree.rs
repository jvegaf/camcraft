use cam_store::tool_edit::rendered_name;
use cam_store::tools::{StoredTool, ToolKind};
use cam_units::Format;

/// A tool's title: `T4 - Flat end mill · 6 mm`, or the name alone when the record has no number.
pub fn tool_title(tool: &StoredTool) -> String {
  let name = rendered_name(tool);
  match tool.cutting.tool_number {
    Some(number) => format!("T{number} - {name}"),
    None => name,
  }
}

/// The row's dim second line: `Ø6 · 2 cut(s)`.
///
/// The two numbers the reference's row carries, and nothing else: the diameter without its unit, because the
/// row is a list and the form is where a number is written with its unit, and the flute count, which the
/// reference calls the number of cuts. A number the library does not hold is a dash, never a zero.
pub fn cut_line(tool: &StoredTool) -> String {
  let diameter = match tool.geometry.diameter {
    Some(diameter) => trim_mm(diameter.mm()),
    None => "—".to_owned(),
  };
  let cuts = match tool.geometry.num_flutes {
    Some(flutes) => flutes.to_string(),
    None => "—".to_owned(),
  };
  format!("Ø{diameter} · {cuts} cut(s)")
}

/// A millimetre value with the zeroes a reader does not need: `6`, `12.7`, `3.175`.
fn trim_mm(millimetres: f64) -> String {
  let text = Format::fixed(3).apply(millimetres);
  let trimmed = text.trim_end_matches('0').trim_end_matches('.');
  if trimmed.is_empty() {
    "0".to_owned()
  } else {
    trimmed.to_owned()
  }
}

/// One row of the master tree, with nothing borrowed: the paint needs no access to the shell's library.
#[derive(Clone, Debug)]
pub(super) struct Row {
  /// The row's title, as reported and painted.
  pub(super) title: String,
  /// The dim second line.
  pub(super) cut: String,
  /// The library's own geometry id: what a selection names.
  pub(super) id: String,
}

/// The tree's rows for a library snapshot, in the library's own order, each with the kind it groups under.
pub(super) fn tree_rows(tools: &[StoredTool]) -> Vec<(ToolKind, Row)> {
  tools
    .iter()
    .map(|tool| {
      (
        ToolKind::of(&tool.geometry),
        Row {
          title: tool_title(tool),
          cut: cut_line(tool),
          id: tool.geometry.id.clone(),
        },
      )
    })
    .collect()
}

/// The tool one selection id names, when the library this window was given still holds it.
pub(super) fn tool_with<'a>(tools: &'a [StoredTool], id: Option<&str>) -> Option<&'a StoredTool> {
  let id = id?;
  tools.iter().find(|tool| tool.geometry.id == id)
}

/// Whether one row survives the search field.
///
/// The title, the id, the kind's own word and the cut line are all searched, because they are all on screen: an
/// operator who reads `Ball nose (3)` and types `ball` is asking for the rows under it, and one who types `Ø6`
/// is asking for the tools that width. Case is folded, because nobody types a capital to find one.
pub(super) fn row_matches(row: &Row, kind: ToolKind, needle: &str) -> bool {
  let needle = needle.trim().to_lowercase();
  if needle.is_empty() {
    return true;
  }
  row.title.to_lowercase().contains(&needle)
    || row.id.to_lowercase().contains(&needle)
    || row.cut.to_lowercase().contains(&needle)
    || kind.label().to_lowercase().contains(&needle)
}
