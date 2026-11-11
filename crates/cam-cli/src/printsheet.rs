//! The HTML print sheet (M10 T3, S5: "HTML print sheets").
//!
//! One command, `cam-cli report <document> [--out <path>]`, writes a self-contained HTML file a person
//! can print and a shop can read: no external asset, no font, no script, every style inside the file,
//! every byte fixed. A template engine would be a dependency and is refused for that reason
//! (invariant 6), so the sheet is rendered by hand, one line at a time, from the document and nothing
//! else.
//!
//! Three laws hold this file together:
//!
//! 1. **The sheet reports the document, from the document alone**, the discipline `simulate` follows
//!    ("a simulation is a question about the document"). It reads no tool library, runs no
//!    post-processor and simulates nothing, and what it therefore cannot say is listed in its own
//!    `Omitted` section instead of being left out in silence.
//! 2. **The document is escaped, not trusted.** Every id and name it holds is user text, and every one
//!    of them reaches the sheet through [`escape_html`] — a layer called `<b>x</b>` prints as those
//!    characters and never becomes markup.
//! 3. **Two runs are the same bytes.** Nothing is iterated in an order the document does not fix (the
//!    one sorted collection is the tool list, and it is a `BTreeSet`), nothing is timestamped, and
//!    every number goes through `cam-cli`'s `decimal`, so the same document prints the same file
//!    (invariant 5).
//!
//! Utilisation and waste are [`Document::nesting_utilisation`]'s answer, the one arithmetic, never a
//! second: the sheet prints the fraction the document derives and its complement, which is the
//! definition the stored cost record is validated against (`D-M9`).

use std::collections::BTreeSet;
use std::path::PathBuf;

use cam_model::{Document, Operation, OriginCorner, Toolpath};
use cam_units::Angle;

use crate::{decimal, value_at, Failure};

/// Everything `cam-cli report`'s command line holds, parsed and nothing more.
#[derive(Default)]
struct ReportArgs {
  /// The document to read.
  document: Option<PathBuf>,
  /// Where to write the sheet; stdout when absent, exactly like `emit`'s program.
  out: Option<PathBuf>,
}

fn report_args(rest: &[String]) -> Result<ReportArgs, Failure> {
  let mut args = ReportArgs::default();
  let mut index = 0;
  while index < rest.len() {
    match rest[index].as_str() {
      "--out" => {
        args.out = Some(PathBuf::from(value_at(rest, index, "--out")?));
        index += 2;
      }
      other if other.starts_with('-') => {
        return Err(Failure::input(format!("unknown report option {other:?}")))
      }
      other => {
        if args.document.is_some() {
          return Err(Failure::input(
            "report takes exactly one document, before its options",
          ));
        }
        args.document = Some(PathBuf::from(other));
        index += 1;
      }
    }
  }
  Ok(args)
}

/// `cam-cli report <document> [--out <path>]`: render the sheet and write it, or print it.
///
/// The document is opened with [`crate::open`], which is every subcommand's loader and which validates
/// as it reads, so a document that does not validate is refused by `cam-cli`'s own diagnostic before a
/// byte is rendered (invariant 4's counterpart for a reader: the sheet never describes a document the
/// model would refuse).
pub(crate) fn report_command(rest: &[String]) -> Result<(), Failure> {
  let args = report_args(rest)?;
  let path = args
    .document
    .clone()
    .ok_or_else(|| Failure::input("report requires a document path"))?;
  let document = crate::open(&path)?;
  let sheet = print_sheet(&document);

  match &args.out {
    Some(destination) => std::fs::write(destination, &sheet)
      .map_err(|error| Failure::input(format!("{}: {error}", destination.display()))),
    None => {
      print!("{sheet}");
      Ok(())
    }
  }
}

/// The print sheet of `document`: one self-contained HTML file, deterministic and escaped.
///
/// This is the whole renderer, and the only renderer: the command above is its caller, so a future
/// scripted `report` would call this same function and not a second sheet (M10 invariant 1).
pub(crate) fn print_sheet(document: &Document) -> String {
  let mut lines = vec![
    "<!DOCTYPE html>".to_owned(),
    "<html lang=\"en\">".to_owned(),
    "<head>".to_owned(),
    "<meta charset=\"utf-8\">".to_owned(),
    "<title>Print sheet</title>".to_owned(),
    "<style>".to_owned(),
    "body { margin: 2rem; color: #111; font-family: Georgia, serif; }".to_owned(),
    "h1 { margin: 0 0 0.25rem; font-size: 1.6rem; }".to_owned(),
    "h2 { margin: 2rem 0 0.5rem; font-size: 1.2rem; border-bottom: 1px solid #999; }".to_owned(),
    "h3 { margin: 1rem 0 0.25rem; font-size: 1rem; }".to_owned(),
    "p { margin: 0.25rem 0; }".to_owned(),
    "table { margin: 0.5rem 0; border-collapse: collapse; }".to_owned(),
    "th, td { padding: 0.2rem 0.6rem; border: 1px solid #bbb; text-align: left; }".to_owned(),
    "th { background: #eee; }".to_owned(),
    ".empty { font-style: italic; }".to_owned(),
    "</style>".to_owned(),
    "</head>".to_owned(),
    "<body>".to_owned(),
    "<h1>Print sheet</h1>".to_owned(),
    format!(
      "<p>schema {}; figures in millimetres</p>",
      escape_html(&document.schema)
    ),
  ];
  lines.extend(job_section(document));
  lines.extend(layer_section(document));
  lines.extend(toolpath_section(document));
  lines.extend(tool_section(document));
  lines.extend(nesting_section(document));
  lines.extend(omitted_section());
  lines.push("</body>".to_owned());
  lines.push("</html>".to_owned());

  let mut text = lines.join("\n");
  text.push('\n');
  text
}

/// Escape `text` for HTML text and attribute content: `&`, `<`, `>`, `"` and `'`.
///
/// The document is user text, not markup (invariant 2). Every id and name the sheet prints passes
/// through here, so a layer named `<b>x</b>` or an entity id containing `</script>` prints as the
/// characters the user typed. The five characters are the ones that can open a tag, close one, end an
/// attribute or start an entity reference, which is why all five are escaped rather than only the
/// angle brackets.
fn escape_html(text: &str) -> String {
  let mut escaped = String::with_capacity(text.len());
  for character in text.chars() {
    match character {
      '&' => escaped.push_str("&amp;"),
      '<' => escaped.push_str("&lt;"),
      '>' => escaped.push_str("&gt;"),
      '"' => escaped.push_str("&quot;"),
      '\'' => escaped.push_str("&#39;"),
      other => escaped.push(other),
    }
  }
  escaped
}

/// A table with one header row and one row per entry, its cells already escaped.
fn table(headers: &[&str], rows: &[Vec<String>]) -> Vec<String> {
  let mut lines = vec!["<table>".to_owned()];
  let head: Vec<String> = headers
    .iter()
    .map(|header| format!("<th>{header}</th>"))
    .collect();
  lines.push(format!("<tr>{}</tr>", head.concat()));
  for row in rows {
    let cells: Vec<String> = row.iter().map(|cell| format!("<td>{cell}</td>")).collect();
    lines.push(format!("<tr>{}</tr>", cells.concat()));
  }
  lines.push("</table>".to_owned());
  lines
}

/// A two-column table whose first column is the field's name, its values already escaped.
fn field_table(rows: &[(&'static str, String)]) -> Vec<String> {
  let mut lines = vec!["<table>".to_owned()];
  for (field, value) in rows {
    lines.push(format!("<tr><th>{field}</th><td>{value}</td></tr>"));
  }
  lines.push("</table>".to_owned());
  lines
}

/// The job: the stock and coordinate setup the program is cut in.
fn job_section(document: &Document) -> Vec<String> {
  let job = &document.job;
  let mut lines = vec!["<h2>Job</h2>".to_owned()];
  lines.extend(field_table(&[
    ("width", millimetres(job.width.mm())),
    ("height", millimetres(job.height.mm())),
    ("thickness", millimetres(job.thickness.mm())),
    ("origin", escape_html(origin_label(job.origin))),
    ("safe Z", millimetres(job.safe_z.mm())),
  ]));
  lines
}

/// The layers and how many entities each holds.
fn layer_section(document: &Document) -> Vec<String> {
  let mut lines = vec!["<h2>Layers</h2>".to_owned()];
  if document.layers.is_empty() {
    lines.push("<p class=\"empty\">no layer: the document holds no geometry.</p>".to_owned());
    return lines;
  }
  let rows: Vec<Vec<String>> = document
    .layers
    .iter()
    .map(|layer| {
      vec![
        escape_html(&layer.id),
        escape_html(&layer.name),
        if layer.visible { "yes" } else { "no" }.to_owned(),
        layer.entities.len().to_string(),
      ]
    })
    .collect();
  lines.extend(table(&["layer", "name", "visible", "entities"], &rows));
  lines
}

/// Every toolpath: what it is, the tool it names, the geometry it selects and its authored parameters.
fn toolpath_section(document: &Document) -> Vec<String> {
  let mut lines = vec!["<h2>Toolpaths</h2>".to_owned()];
  if document.toolpaths.is_empty() {
    lines.push(
      "<p class=\"empty\">no toolpath: the document holds none, so there is nothing to cut.</p>"
        .to_owned(),
    );
    return lines;
  }

  for toolpath in &document.toolpaths {
    lines.push(format!(
      "<h3>{} — {}</h3>",
      escape_html(&toolpath.id),
      escape_html(&toolpath.name)
    ));
    let mut rows = vec![
      (
        "operation",
        escape_html(crate::operation_label(&toolpath.operation)),
      ),
      ("tool", format!("T{}", toolpath.tool_number)),
      ("entities", id_list(&toolpath.entity_ids)),
    ];
    if !toolpath.island_ids.is_empty() {
      rows.push(("islands", id_list(&toolpath.island_ids)));
    }
    rows.extend(parameter_rows(toolpath));
    lines.extend(field_table(&rows));
  }
  lines
}

/// The parameters the toolpath's operation authored, in the same order for every document.
fn parameter_rows(toolpath: &Toolpath) -> Vec<(&'static str, String)> {
  match &toolpath.operation {
    Operation::Isolation(params) => vec![
      ("passes", params.passes.to_string()),
      ("overlap", decimal(params.overlap)),
      (
        "direction",
        format!("{:?}", params.direction).to_lowercase(),
      ),
      ("kind", format!("{:?}", params.kind).to_lowercase()),
      ("cut depth", millimetres(params.cut_depth.mm())),
      ("keep pours", params.keep_pours.to_string()),
    ],
    Operation::NonCopperClear(params) => vec![
      ("overlap", decimal(params.overlap)),
      ("margin", millimetres(params.margin.mm())),
      ("method", format!("{:?}", params.method).to_lowercase()),
      ("connect", params.connect.to_string()),
      ("contour", params.contour.to_string()),
      (
        "copper offset",
        params
          .copper_offset
          .map(|v| millimetres(v.mm()))
          .unwrap_or_else(|| "none".to_owned()),
      ),
      (
        "direction",
        format!("{:?}", params.direction).to_lowercase(),
      ),
      ("cut depth", millimetres(params.cut_depth.mm())),
    ],
    Operation::Cutout(params) => vec![
      ("margin", millimetres(params.margin.mm())),
      ("tabs", params.tabs.len().to_string()),
      ("tab kind", format!("{:?}", params.tab_kind).to_lowercase()),
      ("pass depth", millimetres(params.pass_depth.mm())),
      ("cut depth", millimetres(params.cut_depth.mm())),
      ("cut interiors", params.cut_interiors.to_string()),
    ],
    Operation::Paint(params) => vec![
      ("overlap", decimal(params.overlap)),
      ("offset", millimetres(params.offset.mm())),
      ("method", format!("{:?}", params.method).to_lowercase()),
      ("connect", params.connect.to_string()),
      ("contour", params.contour.to_string()),
      ("cut depth", millimetres(params.cut_depth.mm())),
    ],
    Operation::Panelize(params) => {
      let mut rows = vec![
        ("rows", params.rows.to_string()),
        ("columns", params.columns.to_string()),
        ("spacing x", millimetres(params.spacing_x.mm())),
        ("spacing y", millimetres(params.spacing_y.mm())),
      ];
      if let Some(v) = params.constrain_x {
        rows.push(("constrain x", millimetres(v.mm())));
      }
      if let Some(v) = params.constrain_y {
        rows.push(("constrain y", millimetres(v.mm())));
      }
      rows
    }
  }
}

/// The tools the toolpaths name, each with how many toolpaths name it.
///
/// A tool number is all the document holds about a tool, and a `BTreeSet` gives the list one fixed
/// order that no document iteration order can change (invariant 5, rule 1). What the number does not
/// say — diameter, feeds and speeds — is said to be omitted, because it lives in a library the sheet
/// does not read.
fn tool_section(document: &Document) -> Vec<String> {
  let mut lines = vec!["<h2>Tools</h2>".to_owned()];
  let numbers: BTreeSet<u32> = document
    .toolpaths
    .iter()
    .map(|toolpath| toolpath.tool_number)
    .collect();

  if numbers.is_empty() {
    lines.push(
      "<p class=\"empty\">no tool: the document holds no toolpath, so none names a tool.</p>"
        .to_owned(),
    );
  } else {
    let rows: Vec<Vec<String>> = numbers
      .iter()
      .map(|number| {
        let count = document
          .toolpaths
          .iter()
          .filter(|toolpath| toolpath.tool_number == *number)
          .count();
        vec![format!("T{number}"), count.to_string()]
      })
      .collect();
    lines.extend(table(&["tool", "toolpaths"], &rows));
  }

  lines.push(
    "<p>diameter, feeds and speeds: omitted, because the document names a tool number and the tool \
     library is not part of the document.</p>"
      .to_owned(),
  );
  if let Some(reference) = &document.tools_ref {
    lines.push(format!(
      "<p>tool library: {} (named by the document, not read by this sheet)</p>",
      escape_html(reference)
    ));
  }
  lines
}

/// The nesting: its sheets, parts and placements, and the utilisation and waste the document derives.
fn nesting_section(document: &Document) -> Vec<String> {
  let mut lines = vec!["<h2>Nesting</h2>".to_owned()];
  let Some(nesting) = &document.nesting else {
    lines.push(
      "<p class=\"empty\">no nesting: the document holds no nesting section, so it authors no sheet, \
       part or placement.</p>"
        .to_owned(),
    );
    return lines;
  };

  if nesting.sheets.is_empty() {
    lines.push("<p class=\"empty\">no sheet: the nesting section authors none.</p>".to_owned());
  } else {
    let rows: Vec<Vec<String>> = nesting
      .sheets
      .iter()
      .map(|sheet| {
        let placements = nesting
          .placements
          .iter()
          .filter(|placement| placement.sheet_id == sheet.id)
          .count();
        vec![
          escape_html(&sheet.id),
          millimetres(sheet.width.mm()),
          millimetres(sheet.height.mm()),
          placements.to_string(),
        ]
      })
      .collect();
    lines.extend(table(&["sheet", "width", "height", "placements"], &rows));
  }

  if nesting.parts.is_empty() {
    lines.push("<p class=\"empty\">no part: the nesting section authors none.</p>".to_owned());
  } else {
    let rows: Vec<Vec<String>> = nesting
      .parts
      .iter()
      .map(|part| {
        vec![
          escape_html(&part.id),
          id_list(&part.entity_ids),
          part.instances.to_string(),
        ]
      })
      .collect();
    lines.extend(table(&["part", "entities", "instances"], &rows));
  }

  if nesting.placements.is_empty() {
    lines.push(
      "<p class=\"empty\">no placement: the document authors a nesting that has not been nested.</p>"
        .to_owned(),
    );
  } else {
    let rows: Vec<Vec<String>> = nesting
      .placements
      .iter()
      .map(|placement| {
        vec![
          escape_html(&placement.part_id),
          escape_html(&placement.sheet_id),
          millimetres(placement.position.x),
          millimetres(placement.position.y),
          degrees(placement.rotation),
        ]
      })
      .collect();
    lines.extend(table(&["part", "sheet", "x", "y", "rotation"], &rows));
  }

  match document.nesting_utilisation() {
    Some(utilisation) => lines.push(format!(
      "<p>utilisation: {} of the used sheets' area; waste: {}</p>",
      decimal(utilisation),
      decimal(1.0 - utilisation)
    )),
    None => lines.push(
      "<p>utilisation: omitted, because no placement names a sheet, so there is no used area to \
       measure a fraction of.</p>"
        .to_owned(),
    ),
  }
  lines
}

/// What the sheet cannot report, said on the sheet rather than left out in silence.
fn omitted_section() -> Vec<String> {
  vec![
    "<h2>Omitted</h2>".to_owned(),
    "<p>the program: omitted, because a sheet reads the document and runs no post-processor.</p>"
      .to_owned(),
    "<p>the simulation: omitted, because a sheet reports the document and simulates nothing.</p>"
      .to_owned(),
    "<p>the file name: omitted, because the document does not record one, so two copies of one \
     document print one sheet.</p>"
      .to_owned(),
  ]
}

/// `ids` as escaped text, or `none` when the selection is empty.
fn id_list(ids: &[String]) -> String {
  if ids.is_empty() {
    return "none".to_owned();
  }
  ids
    .iter()
    .map(|id| escape_html(id))
    .collect::<Vec<_>>()
    .join(", ")
}

/// A length in millimetres with `cam-cli`'s report precision, `0.000 mm`.
fn millimetres(value: f64) -> String {
  format!("{} mm", decimal(value))
}

/// An angle in degrees with `cam-cli`'s report precision, `0.000 deg`.
fn degrees(angle: Angle) -> String {
  format!("{} deg", decimal(angle.degrees()))
}

/// The job's origin corner, in the words a person reads on a sheet.
fn origin_label(origin: OriginCorner) -> &'static str {
  match origin {
    OriginCorner::BottomLeft => "bottom left",
    OriginCorner::TopLeft => "top left",
    OriginCorner::Center => "centre",
  }
}
