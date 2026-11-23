//! Generate a preview MoveStream from a document's PCB toolpaths.

use cam_geom::{Point2, TolerancePolicy};
use cam_model::{Document, Operation};
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_ops::unite_copper;
use cam_store::tools::{stored_tool_for_toolpath, StoredTool};
use cam_units::Length;

const PREVIEW_FEED_MM_MIN: f64 = 100.0;
const PREVIEW_PLUNGE_MM_MIN: f64 = 30.0;
const PREVIEW_SPINDLE_RPM: u32 = 18_000;
const PREVIEW_DEFAULT_DIAMETER_MM: f64 = 1.0;

/// Build a MoveStream for the document's PCB toolpaths for preview/simulation.
/// Returns None when there are no supported toolpaths or generation fails.
pub fn build_preview_stream(
  document: &Document,
  tools: &[StoredTool],
) -> Option<(MoveStream, Vec<String>)> {
  let tolerance = TolerancePolicy::default();
  let mut stream = MoveStream::new(
    Length::from_mm(cam_model::DEFAULT_SAFE_Z_MM),
    Point2::new(0.0, 0.0),
  );
  let mut warnings = Vec::new();
  let mut any = false;
  for toolpath in &document.toolpaths {
    let (tool_shape, tool_diameter_mm) = resolve_preview_tool(
      tools,
      toolpath.tool_id.as_deref(),
      toolpath.tool_number,
      &toolpath.name,
      &mut warnings,
    );
    match &toolpath.operation {
      Operation::Isolation(params) => {
        let contours = collect_contours(document, &toolpath.entity_ids);
        if contours.is_empty() {
          continue;
        }
        let Ok(copper) = unite_copper(&contours, tolerance) else {
          continue;
        };
        let Ok(passes) = cam_ops::isolation_passes_with_kind(
          &copper,
          tool_diameter_mm,
          params.passes,
          params.overlap,
          params.direction,
          params.kind,
          params.keep_pours,
          tolerance,
        ) else {
          continue;
        };
        let mut moves = Vec::new();
        for pass in passes.passes {
          for contour in pass.contours {
            push_contour_moves(
              &mut moves,
              &contour,
              params.cut_depth.mm(),
              stream.safe_z.mm(),
            );
          }
        }
        if push_segment(&mut stream, toolpath, tool_shape, moves) {
          any = true;
        }
      }
      Operation::NonCopperClear(params) => {
        let contours = collect_contours(document, &toolpath.entity_ids);
        if contours.is_empty() {
          continue;
        }
        let Ok(moves) = cam_ops::ncc_moves(&contours, params.cut_depth.mm(), &document.board)
        else {
          continue;
        };
        if push_segment(&mut stream, toolpath, tool_shape, moves) {
          any = true;
        }
      }
      Operation::Cutout(params) => {
        let contours = collect_contours(document, &toolpath.entity_ids);
        if contours.is_empty() {
          continue;
        }
        let mut moves = Vec::new();
        for contour in contours {
          push_contour_moves(
            &mut moves,
            &contour,
            params.cut_depth.mm(),
            stream.safe_z.mm(),
          );
        }
        if push_segment(&mut stream, toolpath, tool_shape, moves) {
          any = true;
        }
      }
      Operation::Drilling(params) => {
        let mut moves = Vec::new();
        let safe_z = stream.safe_z.mm();
        for entity_id in &toolpath.entity_ids {
          let Some(entity) = document.entity(entity_id.as_str()) else {
            continue;
          };
          match &entity.kind {
            cam_model::EntityKind::Drill { center, .. } => {
              push_drill(
                &mut moves,
                center.x,
                center.y,
                params.cut_depth.mm(),
                safe_z,
              );
            }
            cam_model::EntityKind::Slot { start, end, .. } => {
              push_slot(
                &mut moves,
                start.x,
                start.y,
                end.x,
                end.y,
                params.cut_depth.mm(),
                safe_z,
              );
            }
            cam_model::EntityKind::Contour { contour } => {
              for v in contour.vertices() {
                push_drill(
                  &mut moves,
                  v.point.x,
                  v.point.y,
                  params.cut_depth.mm(),
                  safe_z,
                );
              }
            }
          }
        }
        if push_segment(&mut stream, toolpath, tool_shape, moves) {
          any = true;
        }
      }
      Operation::Paint(params) => {
        let contours = collect_contours(document, &toolpath.entity_ids);
        if contours.is_empty() {
          continue;
        }
        let mut moves = Vec::new();
        for contour in contours {
          push_contour_moves(
            &mut moves,
            &contour,
            params.cut_depth.mm(),
            stream.safe_z.mm(),
          );
        }
        if push_segment(&mut stream, toolpath, tool_shape, moves) {
          any = true;
        }
      }
      Operation::Panelize(_) => {}
    }
  }
  if any {
    Some((stream, warnings))
  } else {
    None
  }
}

fn resolve_preview_tool(
  tools: &[StoredTool],
  tool_id: Option<&str>,
  tool_number: u32,
  toolpath_name: &str,
  warnings: &mut Vec<String>,
) -> (ToolShape, f64) {
  let Some(tool) = stored_tool_for_toolpath(tools, tool_id, tool_number) else {
    warnings.push(format!(
      "toolpath '{toolpath_name}' uses missing tool {tool_number}; preview uses default {}mm cylinder",
      PREVIEW_DEFAULT_DIAMETER_MM
    ));
    return (
      ToolShape::cylinder(Length::from_mm(PREVIEW_DEFAULT_DIAMETER_MM)),
      PREVIEW_DEFAULT_DIAMETER_MM,
    );
  };
  let diameter_mm = tool
    .geometry
    .diameter
    .map(Length::mm)
    .filter(|d| *d > 0.0)
    .unwrap_or(PREVIEW_DEFAULT_DIAMETER_MM);
  let shape = if let Some(angle) = tool.geometry.included_angle {
    ToolShape::v_bit(
      angle,
      tool.geometry.flat_diameter.unwrap_or(Length::from_mm(0.0)),
    )
  } else {
    ToolShape::cylinder(Length::from_mm(diameter_mm))
  };
  (shape, diameter_mm)
}

fn push_segment(
  stream: &mut MoveStream,
  toolpath: &cam_model::Toolpath,
  tool: ToolShape,
  moves: Vec<Move>,
) -> bool {
  if moves.is_empty() {
    return false;
  }
  stream.segments.push(Segment {
    name: toolpath.name.clone(),
    tool_number: toolpath.tool_number,
    tool,
    feed_rate: PREVIEW_FEED_MM_MIN,
    plunge_rate: PREVIEW_PLUNGE_MM_MIN,
    spindle_speed: PREVIEW_SPINDLE_RPM,
    moves,
  });
  true
}

fn push_drill(moves: &mut Vec<Move>, x: f64, y: f64, cut_depth: f64, safe_z: f64) {
  moves.push(Move::Rapid {
    to: Point3::new(x, y, safe_z),
  });
  moves.push(Move::Feed {
    to: Point3::new(x, y, -cut_depth),
  });
  moves.push(Move::Retract {
    to: Point3::new(x, y, safe_z),
  });
}

fn push_slot(
  moves: &mut Vec<Move>,
  sx: f64,
  sy: f64,
  ex: f64,
  ey: f64,
  cut_depth: f64,
  safe_z: f64,
) {
  moves.push(Move::Rapid {
    to: Point3::new(sx, sy, safe_z),
  });
  moves.push(Move::Feed {
    to: Point3::new(sx, sy, -cut_depth),
  });
  moves.push(Move::Feed {
    to: Point3::new(ex, ey, -cut_depth),
  });
  moves.push(Move::Retract {
    to: Point3::new(ex, ey, safe_z),
  });
}

fn collect_contours(document: &Document, entity_ids: &[String]) -> Vec<cam_geom::Contour> {
  let mut out = Vec::new();
  for id in entity_ids {
    let Some(entity) = document.entity(id.as_str()) else {
      continue;
    };
    if let cam_model::EntityKind::Contour { contour } = &entity.kind {
      out.push(contour.clone());
    }
  }
  out
}

fn push_contour_moves(
  moves: &mut Vec<Move>,
  contour: &cam_geom::Contour,
  cut_depth: f64,
  safe_z: f64,
) {
  let vertices = contour.vertices();
  if vertices.is_empty() {
    return;
  }
  let first = vertices[0].point;
  moves.push(Move::Rapid {
    to: Point3::new(first.x, first.y, safe_z),
  });
  moves.push(Move::Feed {
    to: Point3::new(first.x, first.y, -cut_depth),
  });
  for v in vertices.iter().skip(1) {
    let p = v.point;
    moves.push(Move::Feed {
      to: Point3::new(p.x, p.y, -cut_depth),
    });
  }
  if contour.is_closed() {
    moves.push(Move::Feed {
      to: Point3::new(first.x, first.y, -cut_depth),
    });
  }
  moves.push(Move::Retract {
    to: Point3::new(first.x, first.y, safe_z),
  });
}

#[cfg(test)]
mod tests {
  use super::*;
  use cam_geom::{Contour, Point2};
  use cam_model::{
    Board, BoardSideUp, Document, Entity, EntityKind, IsolationKind, IsolationParams, Layer,
    LayerRole, MillingDirection, Operation, Provenance, Side, Toolpath,
  };
  use cam_store::tools::{CuttingData, StoredTool, ToolGeometry, ToolKind};
  use cam_units::{Length, UnitSystem};

  fn doc_with_isolation() -> Document {
    let mut doc = Document::new(Board {
      width: Length::from_mm(100.0),
      height: Length::from_mm(80.0),
      thickness: Length::from_mm(1.6),
      origin: cam_model::OriginCorner::BottomLeft,
      side_up: BoardSideUp::Top,
    });
    let contour = Contour::from_points(
      &[
        Point2::new(10.0, 10.0),
        Point2::new(30.0, 10.0),
        Point2::new(30.0, 20.0),
        Point2::new(10.0, 20.0),
      ],
      true,
    )
    .unwrap();
    doc.layers.push(Layer {
      id: "L1".into(),
      role: LayerRole::CopperTop,
      side: Side::Top,
      color: "#0f0".into(),
      visible: true,
      source: Provenance::default(),
      entities: vec![Entity {
        id: "E1".into(),
        kind: EntityKind::Contour { contour },
        source: Provenance::default(),
      }],
    });
    doc.toolpaths.push(Toolpath {
      id: "T1".into(),
      name: "Iso".into(),
      entity_ids: vec!["E1".into()],
      tool_id: Some("G1".into()),
      tool_number: 7,
      operation: Operation::Isolation(IsolationParams {
        passes: 1,
        overlap: 0.5,
        direction: MillingDirection::Climb,
        kind: IsolationKind::Full,
        cut_depth: Length::from_mm(0.1),
        keep_pours: false,
      }),
    });
    doc
  }

  fn tool(id: &str, number: u32, diameter_mm: f64) -> StoredTool {
    StoredTool {
      geometry: ToolGeometry {
        id: id.into(),
        name_format: "{Diameter}".into(),
        tool_type: ToolKind::IsolationMill.raw(),
        units: UnitSystem::Mm,
        diameter: Some(Length::from_mm(diameter_mm)),
        included_angle: None,
        flat_diameter: None,
        num_flutes: Some(1),
        flute_length: Some(Length::from_mm(5.0)),
        tip_radius: None,
        shank_diameter: Some(Length::from_mm(3.175)),
        overall_length: Some(Length::from_mm(20.0)),
        custom_attributes: None,
      },
      cutting: CuttingData {
        id: format!("C{id}"),
        rate_units: UnitSystem::Mm,
        feed_rate: Some(100.0),
        plunge_rate: Some(30.0),
        spindle_speed: Some(18_000),
        stepdown: Some(Length::from_mm(0.1)),
        stepover: Some(Length::from_mm(0.2)),
        clear_stepover: Some(Length::from_mm(0.2)),
        tool_number: Some(number),
        notes: None,
      },
    }
  }

  #[test]
  fn build_preview_stream_uses_bound_tool_diameter_and_shape() {
    let doc = doc_with_isolation();
    let (stream, warnings) = build_preview_stream(&doc, &[tool("G1", 7, 2.0)]).expect("stream");
    assert!(warnings.is_empty());
    let seg = &stream.segments[0];
    assert_eq!(seg.tool_number, 7);
    match seg.tool {
      ToolShape::Cylinder { diameter } => assert!((diameter.mm() - 2.0).abs() < 1e-9),
      _ => panic!("expected cylinder"),
    }
    assert!(seg.moves.len() > 1);
  }

  #[test]
  fn build_preview_stream_reports_none_when_no_supported_ops() {
    let mut doc = doc_with_isolation();
    doc.toolpaths.clear();
    assert!(build_preview_stream(&doc, &[]).is_none());
  }

  #[test]
  fn build_preview_stream_warns_when_bound_tool_is_missing() {
    let doc = doc_with_isolation();
    let (stream, warnings) = build_preview_stream(&doc, &[]).expect("stream");
    assert_eq!(stream.segments.len(), 1);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("missing tool"));
  }
}
