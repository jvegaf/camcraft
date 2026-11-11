//! CSV part lists **in** and the cut list **out** (M10 T2b, S3: "Import y export").
//!
//! Two commands, and each is a caller: `parts` reads a part list with [`cam_import::csv`] and authors
//! one closed rectangle contour and one [`NestingPart`] per row, while `cutlist` writes one row per
//! placement M9 stored. `cam-cli parts`/`cam-cli cutlist` and the script language's `parts`/`cutlist`
//! call the same functions in here, so neither route can drift from the other (M10 invariant 1).
//!
//! # Two formats, not one schema
//!
//! A part list and a cut list are two documents about one job, and forcing them into one schema would
//! make one of them lie:
//!
//! * **In** (`name,width,height,quantity`) **asks for parts**. One row is one part: its name, its
//!   size, and how many copies the nesting should place. It carries no position and no sheet, because
//!   nothing has been placed yet. That is [`cam_import::csv::COLUMNS`], with its fixed header order.
//! * **Out** (`part,sheet,x,y,rotation,width,height`) **records placements**. One row is one instance
//!   already on a board: which part, which sheet, where its lower-left corner sits, how far it is
//!   turned, and the part's own unturned size. It has `sheet`, `x`, `y` and `rotation` columns a part
//!   list cannot mean, and it has no `quantity`, because every placed copy is its own row.
//!
//! So `cam_import::csv::import` deliberately does **not** read a cut list back: its header and its
//! four fields are the part list's, and a parser that accepted both shapes would have to guess which
//! one it was handed. The two formats share the dialect — comma-separated fields, a required header
//! row, `\n` line ends — and nothing else, because the shape a job is ordered in is not the shape a
//! shop reads off the bench.
//!
//! # The cut list's formatting
//!
//! One header row, then one row per placement in the order the document stores them. `x`, `y` and the
//! part's own `width` and `height` are millimetres, `rotation` is counter-clockwise degrees, and each
//! number is written with **three decimals** through `cam-cli`'s `decimal` (locale-free): a placement
//! is `0.000`, a quarter turn is `90.000`, and a 120 mm side is `120.000`. The file ends with exactly
//! one `\n` after its last row. Two runs over the same document produce the same bytes (M10 invariant
//! 5) — no map is iterated and no number is formatted twice.

use std::io::Write;
use std::path::{Path, PathBuf};

use cam_geom::{Contour, Point2};
use cam_model::{Document, Entity, EntityKind, Layer, Nesting, NestingPart};

use crate::{decimal, fresh_part_id, value_at, Failure};

/// The layer a part list lands in when the caller names none.
const DEFAULT_LAYER: &str = "Parts";

/// The cut list's one header row: the seven columns every row carries, in order.
const CUT_LIST_HEADER: &str = "part,sheet,x,y,rotation,width,height";

/// Everything `cam-cli parts`'s command line holds, parsed and nothing more.
///
/// The scripted `parts` command hands its tokens to this same parser (M10 invariant 1), so a flag
/// means one thing in both places.
#[derive(Default)]
pub(crate) struct PartsArgs {
  pub(crate) document: Option<PathBuf>,
  pub(crate) csv: Option<PathBuf>,
  pub(crate) layer: Option<String>,
  pub(crate) out: Option<PathBuf>,
}

pub(crate) fn parts_args(rest: &[String]) -> Result<PartsArgs, Failure> {
  let mut args = PartsArgs::default();
  let mut index = 0;
  while index < rest.len() {
    let flag = rest[index].as_str();
    // Every option takes exactly one value.
    let take = |what: &str| value_at(rest, index, what);
    match flag {
      "--csv" => args.csv = Some(PathBuf::from(take("--csv")?)),
      "--layer" => args.layer = Some(take("--layer")?),
      "--out" => args.out = Some(PathBuf::from(take("--out")?)),
      other if other.starts_with('-') => {
        return Err(Failure::input(format!("unknown parts option {other:?}")))
      }
      other => {
        if args.document.is_some() {
          return Err(Failure::input(
            "parts takes exactly one document, before its options",
          ));
        }
        args.document = Some(PathBuf::from(other));
        index += 1;
        continue;
      }
    }
    index += 2;
  }
  Ok(args)
}

/// `cam-cli parts <document> --csv <file>`: author a part per row, then write the document.
pub(crate) fn parts_command(rest: &[String]) -> Result<(), Failure> {
  let args = parts_args(rest)?;
  let path = args
    .document
    .clone()
    .ok_or_else(|| Failure::input("parts requires a document path"))?;
  require_csv(&args)?;
  let mut document = crate::open(&path)?;
  let report = author_parts(&mut document, &args)?;

  let destination = args.out.clone().unwrap_or(path);
  cam_store::document::save_document(&destination, &document)
    .map_err(|error| Failure::input(format!("{}: {error}", destination.display())))?;
  eprint!("{}", save_report(&document, &destination));
  print!("{report}");
  Ok(())
}

/// The one sentence a written document is reported with: what landed, and where.
///
/// Both writers of a document say it — the subcommand, which prints it on stderr once it saved, and
/// the script's `save`, which hands it to the sink its caller chose (M10 T4b). One function, so a
/// captured `save` and a printed one cannot word it differently.
pub(crate) fn save_report(document: &Document, path: &Path) -> String {
  format!(
    "wrote {} layer(s), {} entity(ies) to {}\n",
    document.layers.len(),
    document.entities().count(),
    path.display()
  )
}

/// The part list path, or the refusal that says the command needs one.
fn require_csv(args: &PartsArgs) -> Result<&Path, Failure> {
  args
    .csv
    .as_deref()
    .ok_or_else(|| Failure::input("parts requires --csv <file>"))
}

/// Author one closed rectangle contour and one nesting part per row of a part list, and report what
/// was read.
///
/// The one authoring function of both routes: the subcommand hands it the document it opened and the
/// script hands it the document `open` read (M10 invariant 1). `target` is replaced only when every
/// row was authored **and** the result validates, so a refusal leaves the caller's document — and,
/// because neither caller writes before this returns, the file on disk — exactly as it was found (M10
/// invariant 4).
///
/// A row's own `name` is the authored entity's id, because a name is the one thing a part list carries
/// that this document model can hold — `Entity` and `NestingPart` have no name field, and an id is what
/// a part's selection is made of. The part itself is named by `fresh_part_id`'s scan (`P1`, `P2`, ...),
/// the rule the scripted `part` command already uses, and never by the CSV name: a collision is
/// refused by the row's line rather than renamed away.
pub(crate) fn author_parts(target: &mut Document, args: &PartsArgs) -> Result<String, Failure> {
  let csv = require_csv(args)?;
  let bytes =
    std::fs::read(csv).map_err(|error| Failure::input(format!("{}: {error}", csv.display())))?;
  let list = cam_import::csv::import(&bytes)
    .map_err(|error| Failure::input(format!("{}: {error}", csv.display())))?;

  let layer_name = args
    .layer
    .clone()
    .unwrap_or_else(|| DEFAULT_LAYER.to_owned());
  let mut document = target.clone();
  document.layers.push(Layer {
    id: fresh_layer_id(&document),
    name: layer_name,
    visible: true,
    role: cam_model::LayerRole::Geometry,
    entities: Vec::new(),
  });
  let layer = document.layers.len() - 1;

  // The part id and entity id each row authored, with the line it came from, so a refusal the model
  // makes after the loop can name the row that produced it.
  let mut authored: Vec<(String, String, usize)> = Vec::new();
  for row in &list.parts {
    if document.entity(&row.name).is_some() {
      return Err(Failure::input(format!(
        "{}: csv at line {}, field 1 (\"name\"): the document already holds a part named {:?}",
        csv.display(),
        row.line,
        row.name
      )));
    }
    document.layers[layer].entities.push(Entity {
      id: row.name.clone(),
      kind: EntityKind::Contour {
        contour: rectangle(row.width, row.height)?,
      },
    });
    let nesting = document.nesting.get_or_insert_with(Nesting::default);
    let id = fresh_part_id(&nesting.parts);
    nesting.parts.push(NestingPart {
      id: id.clone(),
      entity_ids: vec![row.name.clone()],
      instances: row.quantity,
    });
    authored.push((id, row.name.clone(), row.line));
  }

  document.validate().map_err(|error| {
    let message = error.to_string();
    let line = authored
      .iter()
      .rev()
      .find(|(part, entity, _)| {
        message.contains(&format!("{part:?}")) || message.contains(&format!("{entity:?}"))
      })
      .map(|(_, _, line)| *line);
    match line {
      Some(line) => Failure::input(format!("{}: csv at line {line}: {message}", csv.display())),
      // A refusal about the document the import was handed rather than one of its rows: located at
      // the file, never at a row that did not cause it.
      None => Failure::input(format!("{}: csv: {message}", csv.display())),
    }
  })?;

  *target = document;
  Ok(list.report.render())
}

/// A closed rectangle from the origin to `width` × `height`, the geometry a part-list row describes.
fn rectangle(width: f64, height: f64) -> Result<Contour, Failure> {
  Contour::from_points(
    &[
      Point2::new(0.0, 0.0),
      Point2::new(width, 0.0),
      Point2::new(width, height),
      Point2::new(0.0, height),
    ],
    true,
  )
  .map_err(|error| {
    Failure::input(format!(
      "a {width} x {height} mm part is not a contour: {error}"
    ))
  })
}

/// The next free `L{n}` layer id: the `fresh_part_id`/`fresh_sheet_id` scan in `script.rs`, so the
/// import names its layer with the rule the rest of the document already uses.
fn fresh_layer_id(document: &Document) -> String {
  let mut number = document.layers.len() + 1;
  loop {
    let id = format!("L{number}");
    if document.layers.iter().all(|layer| layer.id != id) {
      return id;
    }
    number += 1;
  }
}

/// Everything `cam-cli cutlist`'s command line holds, parsed and nothing more.
///
/// The scripted `cutlist` command hands its tokens to this same parser (M10 invariant 1).
#[derive(Default)]
pub(crate) struct CutListArgs {
  pub(crate) document: Option<PathBuf>,
  pub(crate) out: Option<PathBuf>,
}

pub(crate) fn cutlist_args(rest: &[String]) -> Result<CutListArgs, Failure> {
  let mut args = CutListArgs::default();
  let mut index = 0;
  while index < rest.len() {
    let flag = rest[index].as_str();
    match flag {
      "--out" => args.out = Some(PathBuf::from(value_at(rest, index, "--out")?)),
      other if other.starts_with('-') => {
        return Err(Failure::input(format!("unknown cutlist option {other:?}")))
      }
      other => {
        if args.document.is_some() {
          return Err(Failure::input(
            "cutlist takes exactly one document, before its options",
          ));
        }
        args.document = Some(PathBuf::from(other));
        index += 1;
        continue;
      }
    }
    index += 2;
  }
  Ok(args)
}

/// `cam-cli cutlist <document>`: read the document and write its cut list, or print it.
pub(crate) fn cutlist_command(rest: &[String]) -> Result<(), Failure> {
  let args = cutlist_args(rest)?;
  let path = args
    .document
    .clone()
    .ok_or_else(|| Failure::input("cutlist requires a document path"))?;
  let document = crate::open(&path)?;
  cut_list_document(&document, &args, &mut std::io::stdout())
}

/// Write the cut list of `document` to `sink`, or to `--out`, exactly as `emit` writes its program.
///
/// The one writer of both routes (M10 invariant 1). The cut list is an **output**, not the document,
/// which is why a script may name its own `--out` here while a scripted document destination is
/// refused. Without `--out` the rows go to the writer the caller chose — the subcommand passes stdout
/// and a script passes the sink its own caller chose — so a script that lists a cut list without
/// `--out` no longer prints past a capturing caller (M10 T4b).
pub(crate) fn cut_list_document(
  document: &Document,
  args: &CutListArgs,
  sink: &mut dyn Write,
) -> Result<(), Failure> {
  let text = cut_list(document)?;
  match &args.out {
    Some(path) => std::fs::write(path, &text)
      .map_err(|error| Failure::input(format!("{}: {error}", path.display()))),
    None => crate::write_report(sink, &text),
  }
}

/// The cut list of a document, as bytes-ready text, or the refusal that names the state it is in.
///
/// A document with no nesting section has never had a nesting authored; one whose section holds no
/// placement has authored a request that has not been nested. Both are refused by name instead of
/// written as an empty file, so a mistyped path or a forgotten `nest` cannot look like an empty job.
fn cut_list(document: &Document) -> Result<String, Failure> {
  let Some(nesting) = &document.nesting else {
    return Err(Failure::input(
      "cutlist: the document has no nesting section, so there are no placements to list: nest a \
       document that authors sheets and parts first",
    ));
  };
  if nesting.placements.is_empty() {
    return Err(Failure::input(format!(
      "cutlist: the document's nesting section has no placements ({} part(s), 0 placement(s)), so \
       nothing has been nested yet",
      nesting.parts.len()
    )));
  }

  let mut lines = vec![CUT_LIST_HEADER.to_owned()];
  for placement in &nesting.placements {
    let part = nesting.part(&placement.part_id).ok_or_else(|| {
      Failure::input(format!(
        "cutlist: placement names unknown part {:?}",
        placement.part_id
      ))
    })?;
    let bounds = part.bounds(document).ok_or_else(|| {
      Failure::input(format!(
        "cutlist: part {:?} has no entity bounds to measure",
        part.id
      ))
    })?;
    lines.push(format!(
      "{},{},{},{},{},{},{}",
      placement.part_id,
      placement.sheet_id,
      decimal(placement.position.x),
      decimal(placement.position.y),
      decimal(placement.rotation.degrees()),
      decimal(bounds.width()),
      decimal(bounds.height()),
    ));
  }

  let mut text = lines.join("\n");
  text.push('\n');
  Ok(text)
}
