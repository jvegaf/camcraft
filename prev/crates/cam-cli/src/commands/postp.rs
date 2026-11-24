use cam_post::parse_post;

use crate::{only_path, Failure};

pub fn run(rest: &[String]) -> Result<(), Failure> {
  let Some(subcommand) = rest.first() else {
    return Err(Failure::input("postp requires a subcommand: validate"));
  };
  if subcommand != "validate" {
    return Err(Failure::input(format!(
      "unknown postp subcommand {subcommand:?}; expected validate"
    )));
  }
  let path = only_path(&rest[1..], "postp validate")?;
  let text = std::fs::read_to_string(&path)
    .map_err(|error| Failure::input(format!("{}: {error}", path.display())))?;
  let post =
    parse_post(&text).map_err(|error| Failure::post(format!("{}: {error}", path.display())))?;
  println!("post: {}", post.post_name);
  println!("extension: {}", post.file_extension);
  println!("units: {:?}", post.units);
  println!("variables: {}", post.variables.len());
  println!("blocks: {}", post.blocks.len());
  Ok(())
}
