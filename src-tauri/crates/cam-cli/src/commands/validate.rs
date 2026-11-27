use cam_model::Document;

use crate::{only_path, open, Failure};

pub fn run(rest: &[String]) -> Result<(), Failure> {
  let path = only_path(rest, "validate")?;
  let document = open(&path)?;
  print!("{}", report(&document));
  Ok(())
}

/// The report `cam-cli validate` prints for a document that loaded.
///
/// The script's `validate` command prints this same report for the document it holds, so a scripted
/// run and a command-line run describe a document in one sentence, not two (M10 invariant 1).
pub fn report(document: &Document) -> String {
  format!(
    "valid: {} layer(s), {} entity(ies), {} toolpath(s)\n",
    document.layers.len(),
    document.entities().count(),
    document.toolpaths.len()
  )
}
