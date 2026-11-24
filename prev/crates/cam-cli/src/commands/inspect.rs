use crate::{only_path, open, Failure};

pub fn run(rest: &[String]) -> Result<(), Failure> {
  let path = only_path(rest, "inspect")?;
  let document = open(&path)?;
  println!("schema: {}", document.schema);
  println!(
    "job: {:.3} x {:.3} x {:.3} mm, origin {:?}, default safe Z {:.3} mm",
    document.board.width.mm(),
    document.board.height.mm(),
    document.board.thickness.mm(),
    document.board.origin,
    cam_model::DEFAULT_SAFE_Z_MM
  );
  println!("layers: {}", document.layers.len());
  for layer in &document.layers {
    println!(
      "  {}: {} entity(ies), visible {}",
      layer.id,
      layer.entities.len(),
      layer.visible
    );
  }
  println!("toolpaths: {}", document.toolpaths.len());
  let mut total_length = 0.0;
  for entity in document.entities() {
    if let Some(contour) = entity.kind.as_contour() {
      total_length += contour.perimeter();
    }
  }
  println!("geometry length: {total_length:.3} mm");
  Ok(())
}
