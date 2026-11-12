use egui::{RichText, Sense};

use crate::help::Command;
use crate::icons::Icon;
use crate::theme::{self, Palette, Role};
use crate::{ShellOutput, Tool};

/// Which glyph a canvas gesture is drawn with.
fn tool_icon(tool: Tool) -> Icon {
  match tool {
    Tool::Select => Icon::Select,
    Tool::Line => Icon::Line,
    Tool::Rectangle => Icon::Rectangle,
    Tool::Polyline => Icon::Polyline,
    Tool::Arc => Icon::Arc,
    Tool::Circle => Icon::Circle,
    Tool::Ellipse => Icon::Ellipse,
    Tool::Polygon => Icon::Polygon,
    Tool::Star => Icon::Star,
  }
}

/// Which glyph a command is drawn with, wherever the frame draws it as an icon.
///
/// A gesture keeps the glyph its tool has always had ([`tool_icon`]); the rest are the symbols the palette
/// and the app bar already used for them.
pub(super) fn command_icon(command: Command) -> Icon {
  if let Some(tool) = command.tool() {
    return tool_icon(tool);
  }
  match command {
    Command::Move => Icon::Move,
    Command::Rotate => Icon::Rotate,
    Command::Scale => Icon::Scale,
    Command::Mirror => Icon::MirrorH,
    Command::Array => Icon::ArrayGrid,
    Command::Offset => Icon::Offset,
    Command::Join => Icon::Union,
    Command::Grid => Icon::Grid,
    Command::Layers => Icon::Layers,
    Command::Help => Icon::Help,
    Command::Shortcuts => Icon::Shortcuts,
    // A script is a program file the engine reads line by line, which is the one glyph in the set that
    // draws a program file; the fallback `Select` would say a script is a click on the sheet.
    Command::Script => Icon::Gcode,
    Command::ToolLibrary => Icon::ToolLibrary,
    Command::Undo => Icon::Undo,
    Command::Redo => Icon::Redo,
    Command::NewProject => Icon::New,
    Command::Open => Icon::Open,
    Command::Save => Icon::Save,
    Command::SaveAs => Icon::SaveAs,
    Command::FitJob => Icon::ZoomFit,
    Command::Import => Icon::ImportVectors,
    // The rest are not drawn as glyphs anywhere in the frame: they are labelled buttons in the Tools tab
    // or reached by a key, so this arm is the honest "no symbol of its own" answer rather than a new
    // symbol nobody has ever seen.
    _ => Icon::Select,
  }
}

/// Record one app-bar button: the word the command table gives it, and the point that presses it.
///
/// Kept for the buttons the bar still owns. Since camui-prancheta T2b no bar button runs a table command —
/// FILE moved to the rail — so [`crate::DrawnState::app_bar`] stays empty and a test asserts it, which is how a
/// duplicate button cannot come back unnoticed.
#[allow(dead_code)]
fn report_app_bar(output: &mut ShellOutput, command: Command, rect: egui::Rect) {
  let centre = rect.center();
  output.drawn.app_bar.push(command.label().to_owned());
  output
    .drawn
    .app_bar_hits
    .push((command.label().to_owned(), (centre.x, centre.y)));
}

/// One icon-only control: a square target, a hover wash, and the glyph at the set's own weight.
///
/// The glyph keeps its stroke in every state — pressed, hovered, active — because a rail of twenty
/// icons that thicken as the pointer moves reads as flicker, not as feedback.
pub(super) fn icon_button(
  ui: &mut egui::Ui,
  palette: &Palette,
  icon: Icon,
  tooltip: &str,
  active: bool,
  enabled: bool,
  glyph: f32,
) -> egui::Response {
  let target = glyph + 12.0;
  let sense = if enabled {
    Sense::click()
  } else {
    Sense::hover()
  };
  let (rect, response) = ui.allocate_exact_size(egui::vec2(target, target), sense);
  let ink = if !enabled {
    palette.get(Role::TextOff)
  } else if active {
    palette.get(Role::Sel)
  } else if response.hovered() {
    palette.get(Role::TextHi)
  } else {
    palette.get(Role::Text)
  };
  let painter = ui.painter();
  let radius = egui::CornerRadius::same(4);
  if active {
    painter.rect_filled(rect, radius, palette.get(Role::SelSoft));
    painter.rect_stroke(
      rect,
      radius,
      egui::Stroke::new(1.0, palette.get(Role::Sel)),
      egui::StrokeKind::Inside,
    );
  } else if response.hovered() && enabled {
    painter.rect_filled(rect, radius, palette.get(Role::Raised));
  }
  let box_size = glyph.min(rect.width() - 4.0);
  let box_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(box_size, box_size));
  crate::icons::paint(
    painter,
    box_rect,
    icon,
    ink,
    theme::ICON_STROKE * box_size / theme::ICON_SIZE,
  );
  if enabled {
    response.on_hover_text(tooltip)
  } else {
    response
  }
}

/// The product mark: a cube in the selection colour, drawn rather than imported.
pub(super) fn paint_mark(ui: &mut egui::Ui, palette: &Palette, size: f32) {
  let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), Sense::hover());
  crate::icons::paint(
    ui.painter(),
    rect,
    Icon::View3d,
    palette.get(Role::Sel),
    theme::ICON_STROKE * size / theme::ICON_SIZE,
  );
}

/// A chip: a small pill that is either on or off, in the reference's `Ponta | Meio | Centro` strip.
pub(crate) fn chip(ui: &mut egui::Ui, palette: &Palette, label: &str, on: bool) -> egui::Response {
  let font = egui::FontId::proportional(theme::CAPTION);
  let galley = ui
    .painter()
    .layout_no_wrap(label.to_owned(), font.clone(), palette.get(Role::Text));
  let size = egui::vec2(galley.size().x + 14.0, 18.0);
  let (rect, response) = ui.allocate_exact_size(size, Sense::click());
  let radius = egui::CornerRadius::same(9);
  if on {
    ui.painter()
      .rect_filled(rect, radius, palette.get(Role::SelSoft));
    ui.painter().rect_stroke(
      rect,
      radius,
      egui::Stroke::new(1.0, palette.get(Role::Sel)),
      egui::StrokeKind::Inside,
    );
  } else if response.hovered() {
    ui.painter()
      .rect_filled(rect, radius, palette.get(Role::Raised));
  }
  let ink = if on {
    palette.get(Role::Sel)
  } else {
    palette.get(Role::TextDim)
  };
  ui.painter()
    .text(rect.center(), egui::Align2::CENTER_CENTER, label, font, ink);
  response
}

/// One window rectangle as the four numbers [`DrawnState`](crate::DrawnState) reports its regions with.
pub(crate) fn rect_report(rect: egui::Rect) -> (f32, f32, f32, f32) {
  (rect.min.x, rect.min.y, rect.width(), rect.height())
}

/// What a segmented control drew: the cell that was pressed, where each cell was drawn, and the strip.
///
/// The cells are reported for the same reason the palette reports its chips: egui offers no
/// widget-by-text lookup from outside a frame, so a test that has to press `3D` is given the point the
/// cell was drawn at instead of guessing at the control's margins (camui-prancheta S8).
pub(crate) struct Segmented {
  /// The index the operator picked this frame, if any.
  pub picked: Option<usize>,
  /// Each cell's word and the point in the window that presses it, in the order drawn.
  pub cells: Vec<(String, (f32, f32))>,
  /// The strip the whole control occupied.
  pub rect: egui::Rect,
  /// Each cell's rectangle, in the order drawn.
  ///
  /// Kept so the control can be painted a second time later in the same frame: the canvas' `2D | 3D` sits on
  /// the ruler band and has to be painted over it (camui-prancheta S8, R2-002).
  pub cell_rects: Vec<egui::Rect>,
}

/// Lay a segmented control out and report it, without painting it.
///
/// The canvas' `2D | 3D` needs this: it is laid out before the frame knows whether it splits (L13's rule) but it
/// has to be *painted* after the ruler band it sits on (R2-002), so the two halves of the widget are separate
/// calls and [`paint_segmented`] is the only painter for both.
pub(super) fn segmented_cells(ui: &mut egui::Ui, palette: &Palette, labels: &[&str]) -> Segmented {
  let font = egui::FontId::proportional(theme::CAPTION);
  let mut widths = Vec::with_capacity(labels.len());
  for label in labels {
    let galley =
      ui.painter()
        .layout_no_wrap((*label).to_owned(), font.clone(), palette.get(Role::Text));
    widths.push(galley.size().x + 18.0);
  }
  let total: f32 = widths.iter().sum();
  let (rect, response) = ui.allocate_exact_size(egui::vec2(total, 22.0), Sense::click());
  let mut left = rect.left();
  let mut cell_rects = Vec::with_capacity(labels.len());
  for width in &widths {
    cell_rects.push(egui::Rect::from_min_size(
      egui::Pos2::new(left, rect.top()),
      egui::vec2(*width, rect.height()),
    ));
    left += *width;
  }
  let mut clicked = None;
  let mut cells = Vec::with_capacity(labels.len());
  for (index, label) in labels.iter().enumerate() {
    let cell = cell_rects[index];
    if response.clicked()
      && ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|p| cell.contains(p))
    {
      clicked = Some(index);
    }
    let centre = cell.center();
    cells.push(((*label).to_owned(), (centre.x, centre.y)));
  }
  Segmented {
    picked: clicked,
    cells,
    rect,
    cell_rects,
  }
}

/// Paint a segmented control: the strip, the chosen cell and every word.
///
/// Separate from [`segmented_cells`] so it can be called a **second** time, later in the frame. The canvas'
/// `2D | 3D` sits on the ruler band; the band is painted after the widget is laid out, and a later painter
/// wins — which is how the control ended up under the band's own fill and numbers, invisible in 2D mode
/// (R2-002 in the T6 review). One function for both paints, so the two cannot drift apart.
pub(crate) fn paint_segmented(
  painter: &egui::Painter,
  rect: egui::Rect,
  cell_rects: &[egui::Rect],
  labels: &[&str],
  active: usize,
  palette: &Palette,
) {
  let radius = egui::CornerRadius::same(11);
  painter.rect_filled(rect, radius, palette.get(Role::Raised));
  painter.rect_stroke(
    rect,
    radius,
    egui::Stroke::new(1.0, palette.get(Role::Border)),
    egui::StrokeKind::Inside,
  );
  let font = egui::FontId::proportional(theme::CAPTION);
  for (index, label) in labels.iter().enumerate() {
    let Some(cell) = cell_rects.get(index).copied() else {
      continue;
    };
    let on = index == active;
    if on {
      painter.rect_filled(cell, radius, palette.get(Role::SelSoft));
      painter.rect_stroke(
        cell,
        radius,
        egui::Stroke::new(1.0, palette.get(Role::Sel)),
        egui::StrokeKind::Inside,
      );
    }
    let ink = if on {
      palette.get(Role::Sel)
    } else {
      palette.get(Role::TextDim)
    };
    painter.text(
      cell.center(),
      egui::Align2::CENTER_CENTER,
      *label,
      font.clone(),
      ink,
    );
  }
}

/// One labelled value box: the label above, the number inside a bordered field.
///
/// Read-only on purpose, and that is the one thing T12 changed about it: a job's three measurements are
/// defined once, in [`Shell::material_dialog`](crate::Shell::material_dialog), and a box in a 288 px
/// inspector is where they are *reported* afterwards. Typing a size into two of these in place would let a
/// job exist in a state nobody drew a plan for, so the dialog is the only writer and this box is the receipt.
pub(crate) fn field(ui: &mut egui::Ui, palette: &Palette, label: &str, value: &str) {
  ui.vertical(|ui| {
    ui.label(
      RichText::new(label)
        .size(theme::CAPTION)
        .color(palette.get(Role::TextDim)),
    );
    let (rect, _) = ui.allocate_exact_size(egui::vec2(122.0, 24.0), Sense::hover());
    let radius = egui::CornerRadius::same(4);
    ui.painter()
      .rect_filled(rect, radius, palette.get(Role::Raised));
    ui.painter().rect_stroke(
      rect,
      radius,
      egui::Stroke::new(1.0, palette.get(Role::Border)),
      egui::StrokeKind::Inside,
    );
    ui.painter().text(
      rect.right_center() - egui::vec2(8.0, 0.0),
      egui::Align2::RIGHT_CENTER,
      value,
      egui::FontId::monospace(theme::SMALL),
      palette.get(Role::TextHi),
    );
  });
}

/// A palette or inspector section label: small, uppercase, dim, with a hairline under it.
///
/// The reference app labels every group this way and it is the single cheapest thing that made a wall of
/// commands readable: the word is already in the operator's vocabulary, and it costs one line.
pub(crate) fn section(ui: &mut egui::Ui, palette: &Palette, label: &str) {
  ui.add_space(4.0);
  ui.label(
    RichText::new(label)
      .size(theme::CAPTION)
      .color(palette.get(Role::TextDim)),
  );
  let width = ui.available_width();
  let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), Sense::hover());
  ui.painter()
    .rect_filled(rect, 0.0, palette.get(Role::Hairline));
}

/// A palette heading that folds its group.
///
/// The same word, weight, colour and hairline as [`section`], plus a caret and a click target: this is the
/// reference's collapsible group, and the whole row is the control rather than a triangle the operator has to
/// hit. The caret is painted as a triangle instead of written as a glyph because `▾` is a font bet — the
/// bundled faces may or may not carry it — and a group whose affordance renders as a box is worse than no
/// affordance at all.
pub(crate) fn section_toggle(
  ui: &mut egui::Ui,
  palette: &Palette,
  label: &str,
  open: bool,
) -> egui::Response {
  ui.add_space(4.0);
  // The row is allocated as a widget before anything is painted into it, because that is what makes it
  // clickable: a response reused from a child layout answers no click, and this heading is the one control
  // in the rail that is drawn without a widget of its own.
  let width = ui.available_width();
  let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 14.0), Sense::click());
  let colour = palette.get(Role::TextDim);
  let (cx, cy) = (rect.left() + 5.0, rect.center().y);
  let points = if open {
    vec![
      egui::pos2(cx - 3.0, cy - 2.0),
      egui::pos2(cx + 3.0, cy - 2.0),
      egui::pos2(cx, cy + 3.0),
    ]
  } else {
    vec![
      egui::pos2(cx - 2.0, cy - 3.0),
      egui::pos2(cx - 2.0, cy + 3.0),
      egui::pos2(cx + 3.0, cy),
    ]
  };
  ui.painter().add(egui::Shape::convex_polygon(
    points,
    colour,
    egui::Stroke::NONE,
  ));
  ui.painter().text(
    egui::pos2(cx + 8.0, cy),
    egui::Align2::LEFT_CENTER,
    label,
    egui::FontId::proportional(theme::CAPTION),
    colour,
  );
  let width = ui.available_width();
  let (hairline, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), Sense::hover());
  ui.painter()
    .rect_filled(hairline, 0.0, palette.get(Role::Hairline));
  response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// One dimension line: its two end ticks, the line between them, and its number.
pub(crate) fn dimension_line(
  painter: &egui::Painter,
  palette: &Palette,
  from: egui::Pos2,
  to: egui::Pos2,
  label: &str,
  label_offset: egui::Vec2,
  align: egui::Align2,
) {
  let stroke = egui::Stroke::new(1.0, palette.get(Role::TextDim));
  painter.line_segment([from, to], stroke);
  let tick = (to - from).normalized().rot90() * 3.5;
  painter.line_segment([from - tick, from + tick], stroke);
  painter.line_segment([to - tick, to + tick], stroke);
  let mid = from + (to - from) / 2.0 + label_offset;
  painter.text(
    mid,
    align,
    label,
    egui::FontId::monospace(theme::CAPTION),
    palette.get(Role::TextHi),
  );
}
