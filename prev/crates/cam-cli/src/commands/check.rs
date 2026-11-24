use crate::{only_path, open, Failure};

pub fn run(rest: &[String]) -> Result<(), Failure> {
  let path = only_path(rest, "check")?;
  let document = open(&path)?;

  // Findings are collected with the entity id and the entity's own issue order, then sorted by
  // id, so the report is stable whatever order the layers happen to be in.
  let mut findings: Vec<(String, usize, String)> = Vec::new();
  for entity in document.entities() {
    let Some(contour) = entity.kind.as_contour() else {
      continue;
    };
    let issues = cam_geom::validate::inspect(contour, cam_geom::TolerancePolicy::standard());
    let mut order = 0;
    if let Some(gap) = issues.open {
      findings.push((
        entity.id.clone(),
        order,
        format!("{} open: gap {gap:.4} mm", entity.id),
      ));
      order += 1;
    }
    for intersection in &issues.intersections {
      findings.push((
        entity.id.clone(),
        order,
        format!(
          "{} self-intersection at {:.3}, {:.3} (segments {}, {})",
          entity.id,
          intersection.point.x,
          intersection.point.y,
          intersection.first,
          intersection.second
        ),
      ));
      order += 1;
    }
    for index in &issues.degenerate {
      findings.push((
        entity.id.clone(),
        order,
        format!("{} degenerate segment {index}", entity.id),
      ));
      order += 1;
    }
  }

  if findings.is_empty() {
    println!("all vectors valid");
    return Ok(());
  }

  findings.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
  for (_, _, line) in &findings {
    println!("{line}");
  }
  Err(Failure::vectors(format!(
    "{} vector issue(s) found",
    findings.len()
  )))
}
