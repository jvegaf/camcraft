use cam_move::Point3;
use egui::Stroke;

use crate::theme::{self, Palette, Role};

use super::Preview3d;

/// How long one arm of the axis triad is, in points.
const TRIAD_LENGTH_PX: f32 = 20.0;

/// How far a triad's label sits beyond its arm, in points, and how far the triad sits above the corner's foot.
const TRIAD_LABEL_PAD: f32 = 10.0;

impl Preview3d {
  /// Paint the axis triad in `corner` and report where each label went (S8).
  ///
  /// The three directions are machine space's: `+Z` is `TOP`, `-Y` is `FRONT` — the face toward the
  /// operator, which is the one the reference labels `FRENTE` — and `+X` is `RIGHT`. They are projected
  /// through the same camera as the picture, so the triad is a *reading* of the view rather than a
  /// decoration beside it: orbit the stock and the triad turns with it, which is the only way it can be
  /// trusted about which way is up.
  pub(super) fn triad(
    &self,
    painter: &egui::Painter,
    corner: egui::Rect,
    palette: &Palette,
  ) -> Vec<(String, (f32, f32))> {
    let base = match self.camera.project(self.camera.target) {
      Some(base) => base,
      None => return Vec::new(),
    };
    let origin = egui::pos2(
      corner.center().x,
      corner.max.y - TRIAD_LENGTH_PX - TRIAD_LABEL_PAD,
    );
    let mut labels = Vec::new();
    for (label, axis, role) in [
      ("TOP", Point3::new(0.0, 0.0, 1.0), Role::Sel),
      ("FRONT", Point3::new(0.0, -1.0, 0.0), Role::Danger),
      ("RIGHT", Point3::new(1.0, 0.0, 0.0), Role::Ok),
    ] {
      let tip = Point3::new(
        self.camera.target.x.mm() + axis.x.mm(),
        self.camera.target.y.mm() + axis.y.mm(),
        self.camera.target.z.mm() + axis.z.mm(),
      );
      let Some(tip) = self.camera.project(tip) else {
        continue;
      };
      let (across, up) = (tip.x - base.x, tip.y - base.y);
      let length = (across * across + up * up).sqrt();
      if !length.is_finite() || length <= 1.0e-9 {
        // An axis pointing straight at the eye has no direction on the screen, and drawing it in some
        // invented one would say the wrong thing about the view.
        continue;
      }
      let unit = egui::vec2((across / length) as f32, (up / length) as f32);
      let end = egui::pos2(
        origin.x + unit.x * TRIAD_LENGTH_PX,
        origin.y + unit.y * TRIAD_LENGTH_PX,
      );
      painter.line_segment([origin, end], Stroke::new(1.4, palette.get(role)));
      let at = egui::pos2(
        end.x + unit.x * TRIAD_LABEL_PAD,
        end.y + unit.y * TRIAD_LABEL_PAD,
      );
      painter.text(
        at,
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(theme::CAPTION),
        palette.get(Role::TextDim),
      );
      labels.push((label.to_owned(), (at.x, at.y)));
    }
    labels
  }
}

/// The pill a chip is drawn in: the selected surface when it is on, the raised one while it is hovered.
fn paint_pill(
  painter: &egui::Painter,
  rect: egui::Rect,
  palette: &Palette,
  on: bool,
  hovered: bool,
) {
  let radius = egui::CornerRadius::same(9);
  if on {
    painter.rect_filled(rect, radius, palette.get(Role::SelSoft));
    painter.rect_stroke(
      rect,
      radius,
      egui::Stroke::new(1.0, palette.get(Role::Sel)),
      egui::StrokeKind::Inside,
    );
  } else if hovered {
    painter.rect_filled(rect, radius, palette.get(Role::Raised));
  }
}

/// The play/pause mark, painted: a triangle while paused and two bars while playing.
pub(super) fn paint_transport(
  painter: &egui::Painter,
  rect: egui::Rect,
  playing: bool,
  palette: &Palette,
) {
  paint_pill(painter, rect, palette, false, false);
  let ink = palette.get(Role::TextHi);
  let centre = rect.center();
  if playing {
    for half in [-1.0_f32, 1.0] {
      let slot = egui::Rect::from_min_size(
        egui::pos2(centre.x + half * 6.0 - 2.0, centre.y - 4.5),
        egui::vec2(3.5, 9.0),
      );
      painter.rect_filled(slot, egui::CornerRadius::same(1), ink);
    }
  } else {
    // A triangle rather than the glyph `▶`: a mark that renders as a box on a font without it would be worse
    // than no mark, which is the rule the rail's caret and the task editor's tick already keep.
    painter.add(egui::Shape::convex_polygon(
      vec![
        egui::pos2(centre.x - 3.5, centre.y - 6.0),
        egui::pos2(centre.x + 5.5, centre.y),
        egui::pos2(centre.x - 3.5, centre.y + 6.0),
      ],
      ink,
      egui::Stroke::NONE,
    ));
  }
}

/// The house the home button carries.
///
/// Drawn from five segments instead of added to the icon set: the set is a design artifact with its own sheet
/// and its own count, and one button does not earn a seventy-ninth glyph (`Icon` has no house).
pub(super) fn paint_home(painter: &egui::Painter, rect: egui::Rect, palette: &Palette) {
  paint_pill(painter, rect, palette, false, false);
  let stroke = egui::Stroke::new(1.5, palette.get(Role::TextHi));
  let (left, right) = (rect.min.x + 5.0, rect.max.x - 5.0);
  let (top, bottom) = (rect.min.y + 5.0, rect.max.y - 5.0);
  let eave = top + (bottom - top) * 0.45;
  let apex = egui::pos2((left + right) * 0.5, top);
  painter.line_segment([egui::pos2(left, eave), apex], stroke);
  painter.line_segment([apex, egui::pos2(right, eave)], stroke);
  painter.line_segment(
    [egui::pos2(left + 1.0, eave), egui::pos2(left + 1.0, bottom)],
    stroke,
  );
  painter.line_segment(
    [
      egui::pos2(right - 1.0, eave),
      egui::pos2(right - 1.0, bottom),
    ],
    stroke,
  );
  painter.line_segment(
    [
      egui::pos2(left + 1.0, bottom),
      egui::pos2(right - 1.0, bottom),
    ],
    stroke,
  );
}

/// The playback bar's track and its fill.
///
/// The trough is [`Role::Canvas`] and the fill is [`Role::Cut`] — the same amber the cutting moves are painted
/// in, so the bar measures the cut and nothing else. `tests/view3d.rs` finds both rectangles by role, which is
/// what keeps a bar painted with a literal from passing.
pub(super) fn paint_scrubber(
  painter: &egui::Painter,
  track: egui::Rect,
  fill: f32,
  palette: &Palette,
) {
  let radius = egui::CornerRadius::same(4);
  painter.rect_filled(track, radius, palette.get(Role::Canvas));
  let filled = egui::Rect::from_min_size(
    track.min,
    egui::vec2(track.width() * fill.clamp(0.0, 1.0), track.height()),
  );
  painter.rect_filled(filled, radius, palette.get(Role::Cut));
}

/// How wide a chip is before it is drawn, so the row can be laid out and allocated before it is painted.
///
/// A checkbox carries its own box; a plain chip is a word in a pill. Measuring first is what lets the row be
/// allocated in one pass and painted in another, which is the only way a press in the row can hide a layer in
/// the same frame's picture (L13).
pub(super) fn chip_size(
  painter: &egui::Painter,
  palette: &Palette,
  label: &str,
  checkbox: bool,
) -> egui::Vec2 {
  let galley = painter.layout_no_wrap(
    label.to_owned(),
    egui::FontId::proportional(theme::CAPTION),
    palette.get(Role::Text),
  );
  if checkbox {
    egui::vec2(11.0 + 6.0 + galley.size().x + 10.0, 18.0)
  } else {
    egui::vec2(galley.size().x + 14.0, 18.0)
  }
}

/// One of the overlay row's four checkboxes, painted into the rectangle it was allocated.
///
/// The tick is **painted**, not typeset, for the reason the rail's caret and the task editor's `✓` are (L8,
/// L13): a glyph that renders as a tofu box would be worse than no affordance, and the box has to say which of
/// its two states it is in at a glance.
pub(super) fn paint_layer_chip(
  painter: &egui::Painter,
  rect: egui::Rect,
  palette: &Palette,
  label: &str,
  on: bool,
) {
  let font = egui::FontId::proportional(theme::CAPTION);
  paint_pill(painter, rect, palette, on, false);
  let box_size = 11.0;
  let mark = egui::Rect::from_center_size(
    egui::pos2(rect.min.x + 5.0 + box_size * 0.5, rect.center().y),
    egui::vec2(box_size, box_size),
  );
  let ink = if on {
    palette.get(Role::Sel)
  } else {
    palette.get(Role::TextDim)
  };
  painter.rect_stroke(
    mark,
    egui::CornerRadius::same(2),
    egui::Stroke::new(1.0, ink),
    egui::StrokeKind::Inside,
  );
  if on {
    let tick = egui::Stroke::new(1.6, ink);
    let elbow = egui::pos2(mark.center().x - 0.5, mark.max.y - 3.0);
    painter.line_segment([egui::pos2(mark.min.x + 2.5, mark.center().y), elbow], tick);
    painter.line_segment(
      [elbow, egui::pos2(mark.max.x - 2.5, mark.min.y + 2.5)],
      tick,
    );
  }
  painter.text(
    egui::pos2(mark.max.x + 6.0, rect.center().y),
    egui::Align2::LEFT_CENTER,
    label,
    font,
    if on {
      palette.get(Role::TextHi)
    } else {
      palette.get(Role::TextDim)
    },
  );
}

/// A plain chip of the playback bar: a raised pill with its word in it.
pub(super) fn paint_plain_chip(
  painter: &egui::Painter,
  rect: egui::Rect,
  palette: &Palette,
  label: &str,
) {
  painter.rect_filled(rect, egui::CornerRadius::same(9), palette.get(Role::Raised));
  painter.text(
    rect.center(),
    egui::Align2::CENTER_CENTER,
    label,
    egui::FontId::proportional(theme::CAPTION),
    palette.get(Role::TextDim),
  );
}
