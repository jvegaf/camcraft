//! Generate a preview MoveStream from a document's PCB toolpaths.

use cam_geom::{Point2, TolerancePolicy};
use cam_model::{Document, Operation};
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_ops::unite_copper;
use cam_units::Length;

/// Build a MoveStream for the document's PCB toolpaths for preview/simulation.
/// Returns None when there are no supported toolpaths or generation fails.
pub fn build_preview_stream(document: &Document) -> Option<MoveStream> {
  let tolerance = TolerancePolicy::default();
  let mut stream = MoveStream::new(
    Length::from_mm(cam_model::DEFAULT_SAFE_Z_MM),
    Point2::new(0.0, 0.0),
  );
  let mut any = false;
  for toolpath in &document.toolpaths {
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
          1.0,
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
        if !moves.is_empty() {
          stream.segments.push(Segment {
            name: toolpath.name.clone(),
            tool_number: toolpath.tool_number,
            tool: ToolShape::cylinder(Length::from_mm(1.0)),
            feed_rate: 100.0,
            plunge_rate: 30.0,
            spindle_speed: 18_000,
            moves,
          });
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
        if !moves.is_empty() {
          stream.segments.push(Segment {
            name: toolpath.name.clone(),
            tool_number: toolpath.tool_number,
            tool: ToolShape::cylinder(Length::from_mm(1.0)),
            feed_rate: 100.0,
            plunge_rate: 30.0,
            spindle_speed: 18_000,
            moves,
          });
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
        if !moves.is_empty() {
          stream.segments.push(Segment {
            name: toolpath.name.clone(),
            tool_number: toolpath.tool_number,
            tool: ToolShape::cylinder(Length::from_mm(1.0)),
            feed_rate: 100.0,
            plunge_rate: 30.0,
            spindle_speed: 18_000,
            moves,
          });
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
              let x = center.x;
              let y = center.y;
              moves.push(Move::Rapid {
                to: Point3::new(x, y, safe_z),
              });
              moves.push(Move::Feed {
                to: Point3::new(x, y, -params.cut_depth.mm()),
              });
              moves.push(Move::Retract {
                to: Point3::new(x, y, safe_z),
              });
            }
            cam_model::EntityKind::Slot { start, end, .. } => {
              let sx = start.x;
              let sy = start.y;
              let ex = end.x;
              let ey = end.y;
              moves.push(Move::Rapid {
                to: Point3::new(sx, sy, safe_z),
              });
              moves.push(Move::Feed {
                to: Point3::new(sx, sy, -params.cut_depth.mm()),
              });
              moves.push(Move::Feed {
                to: Point3::new(ex, ey, -params.cut_depth.mm()),
              });
              moves.push(Move::Retract {
                to: Point3::new(ex, ey, safe_z),
              });
            }
            cam_model::EntityKind::Contour { contour } => {
              for v in contour.vertices() {
                let x = v.point.x;
                let y = v.point.y;
                moves.push(Move::Rapid {
                  to: Point3::new(x, y, safe_z),
                });
                moves.push(Move::Feed {
                  to: Point3::new(x, y, -params.cut_depth.mm()),
                });
                moves.push(Move::Retract {
                  to: Point3::new(x, y, safe_z),
                });
              }
            }
          }
        }
        if !moves.is_empty() {
          stream.segments.push(Segment {
            name: toolpath.name.clone(),
            tool_number: toolpath.tool_number,
            tool: ToolShape::cylinder(Length::from_mm(1.0)),
            feed_rate: 100.0,
            plunge_rate: 30.0,
            spindle_speed: 18_000,
            moves,
          });
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
        if !moves.is_empty() {
          stream.segments.push(Segment {
            name: toolpath.name.clone(),
            tool_number: toolpath.tool_number,
            tool: ToolShape::cylinder(Length::from_mm(1.0)),
            feed_rate: 100.0,
            plunge_rate: 30.0,
            spindle_speed: 18_000,
            moves,
          });
          any = true;
        }
      }
      Operation::Panelize(_) => {}
    }
  }
  if any {
    Some(stream)
  } else {
    None
  }
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
