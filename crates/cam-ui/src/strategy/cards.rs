use cam_model::{Operation, PocketStrategy};
use egui::Sense;

use crate::theme::{self, Metrics, Palette, Role};
use crate::{Shell, ShellOutput};

use super::{clearing, kind_of, Kind};

impl Kind {
  /// The picture the card carries.
  fn art(self) -> Art {
    match self {
      Self::Profile => Art::Profile,
      Self::Pocket => Art::Pocket,
    }
  }
}

/// The picture a card carries.
///
/// Four diagrams, each painted from the operation's own semantics rather than pasted: a contour with the tool
/// beside it, a region filled with rings, and the two clearing patterns a pocket can use. They are the shell's
/// own art the way [`crate::tool_library`]'s scaled tool drawing is — see this module's header for what a real
/// thumbnail would need.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Art {
  /// A vector and the tool running along its outside.
  Profile,
  /// A region cleared with concentric rings.
  Pocket,
  /// The same region, cleared with rings.
  Offset,
  /// The same region, cleared with parallel rows.
  Raster,
}

impl Shell {
  /// The two kind cards: the strategy's own shape, which is a choice that needs a picture.
  pub(super) fn kind_cards(
    &mut self,
    ui: &mut egui::Ui,
    palette: &Palette,
    metrics: &Metrics,
    output: &mut ShellOutput,
  ) {
    let current = kind_of(&self.strategy.operation);
    let cards: Vec<(String, Art, bool)> = Kind::ALL
      .iter()
      .map(|kind| (kind.word().to_owned(), kind.art(), current == Some(*kind)))
      .collect();
    let mut drawn: Vec<(String, bool, usize)> = Vec::new();
    let pressed = image_cards(
      ui,
      palette,
      metrics,
      &cards,
      &mut drawn,
      &mut output.drawn.strategy_hits,
    );
    if let Some(index) = pressed {
      self.strategy.choose_kind(Kind::ALL[index]);
    }
    // Reported after the press, so the frame that chooses a kind is the frame that reports it chosen (L13).
    let now = kind_of(&self.strategy.operation);
    for (word, _, strokes) in drawn {
      let kind = Kind::ALL.iter().find(|kind| kind.word() == word).copied();
      output
        .drawn
        .strategy_cards
        .push((word, kind.is_some() && kind == now, strokes));
    }
  }

  /// The clearing pattern's two cards, on a pocket only: the other choice that needs a picture.
  pub(super) fn clearing_cards(
    &mut self,
    ui: &mut egui::Ui,
    palette: &Palette,
    metrics: &Metrics,
    output: &mut ShellOutput,
  ) {
    let current = clearing(&self.strategy.operation);
    let cards = vec![
      (
        "Offset".to_owned(),
        Art::Offset,
        current == Some(PocketStrategy::Offset),
      ),
      (
        "Raster".to_owned(),
        Art::Raster,
        current == Some(PocketStrategy::Raster),
      ),
    ];
    let mut drawn: Vec<(String, bool, usize)> = Vec::new();
    let pressed = image_cards(
      ui,
      palette,
      metrics,
      &cards,
      &mut drawn,
      &mut output.drawn.strategy_hits,
    );
    if let Some(index) = pressed {
      if let Operation::Pocket(params) = &mut self.strategy.operation {
        params.strategy = if index == 0 {
          PocketStrategy::Offset
        } else {
          PocketStrategy::Raster
        };
      }
    }
    let now = clearing(&self.strategy.operation);
    for (word, _, strokes) in drawn {
      let chosen = matches!(
        (word.as_str(), now),
        ("Offset", Some(PocketStrategy::Offset)) | ("Raster", Some(PocketStrategy::Raster))
      );
      output.drawn.strategy_cards.push((word, chosen, strokes));
    }
  }
}

/// One row of image cards: the choice a picture makes shorter than a sentence.
///
/// Each card is a picture and its word. The picture is painted from [`Art`], the card is filled and stroked by
/// the palette's own roles, and the number of strokes it painted is reported beside its word so a test can tell
/// a card with a diagram from an empty box (L23: a headless test reads shapes, and a box is a shape).
fn image_cards(
  ui: &mut egui::Ui,
  palette: &Palette,
  metrics: &Metrics,
  cards: &[(String, Art, bool)],
  drawn: &mut Vec<(String, bool, usize)>,
  hits: &mut Vec<(String, (f32, f32))>,
) -> Option<usize> {
  let mut pressed = None;
  let gap = metrics.space(1);
  let width = ((ui.available_width() - gap * (cards.len().saturating_sub(1) as f32))
    / cards.len().max(1) as f32)
    .max(40.0);
  let height = 58.0;
  ui.horizontal(|ui| {
    ui.spacing_mut().item_spacing.x = gap;
    for (index, (word, art, selected)) in cards.iter().enumerate() {
      let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
      let radius = egui::CornerRadius::same(metrics.radius_control as u8);
      let fill = if *selected {
        palette.get(Role::SelSoft)
      } else if response.hovered() {
        palette.get(Role::Raised)
      } else {
        palette.get(Role::Panel)
      };
      ui.painter().rect_filled(rect, radius, fill);
      ui.painter().rect_stroke(
        rect,
        radius,
        egui::Stroke::new(
          if *selected { 1.5 } else { 1.0 },
          palette.get(if *selected { Role::Sel } else { Role::Border }),
        ),
        egui::StrokeKind::Inside,
      );
      let picture = egui::Rect::from_min_size(
        rect.min + egui::vec2(6.0, 5.0),
        egui::vec2(rect.width() - 12.0, height - 26.0),
      );
      let strokes = paint_art(ui.painter(), picture, palette, *art);
      ui.painter().text(
        egui::pos2(rect.left() + 6.0, rect.bottom() - 5.0),
        egui::Align2::LEFT_BOTTOM,
        word,
        egui::FontId::proportional(theme::SMALL),
        palette.get(if *selected { Role::TextHi } else { Role::Text }),
      );
      let centre = rect.center();
      hits.push((word.clone(), (centre.x, centre.y)));
      if response.clicked() {
        pressed = Some(index);
      }
      drawn.push((word.clone(), *selected, strokes));
    }
  });
  pressed
}

/// Paint one card's diagram and report how many strokes it took.
fn paint_art(painter: &egui::Painter, rect: egui::Rect, palette: &Palette, art: Art) -> usize {
  let ink = palette.get(Role::Text);
  let cut = palette.get(Role::Cut);
  let hair = egui::Stroke::new(1.0, ink);
  let cut_stroke = egui::Stroke::new(1.2, cut);
  let mut strokes = 0;
  let ring = |rect: egui::Rect, stroke: egui::Stroke, strokes: &mut usize| {
    painter.rect_stroke(
      rect,
      egui::CornerRadius::same(2),
      stroke,
      egui::StrokeKind::Inside,
    );
    *strokes += 1;
  };
  match art {
    Art::Profile => {
      // The vector, then the tool's own circle sitting on a pass beside it: a profile cuts along the shape, and
      // the picture says "one line, with the bit on it" rather than "a region".
      let vector = rect.shrink(6.0);
      ring(vector, hair, &mut strokes);
      let pass = vector.expand(4.0);
      ring(pass, cut_stroke, &mut strokes);
      let tip = egui::pos2(pass.left() + pass.width() * 0.25, pass.bottom());
      painter.circle_filled(tip, 3.0, palette.get(Role::TextHi));
    }
    Art::Pocket => {
      // The boundary and the rings a clearing pass leaves inside it.
      let mut inside = rect.shrink(5.0);
      ring(inside, hair, &mut strokes);
      for _ in 0..3 {
        inside = inside.shrink(4.0);
        if inside.width() <= 4.0 || inside.height() <= 4.0 {
          break;
        }
        ring(inside, cut_stroke, &mut strokes);
      }
    }
    Art::Offset => {
      // The same rings, without the pocket's boundary stroke: the pattern itself.
      let mut inside = rect.shrink(3.0);
      for _ in 0..4 {
        ring(inside, cut_stroke, &mut strokes);
        inside = inside.shrink(4.0);
        if inside.width() <= 4.0 || inside.height() <= 4.0 {
          break;
        }
      }
    }
    Art::Raster => {
      // Parallel rows: the other clearing pattern, drawn as the rows it cuts.
      let rows = 4;
      let step = rect.height() / (rows + 1) as f32;
      for row in 1..=rows {
        let y = rect.top() + step * row as f32;
        painter.line_segment(
          [
            egui::pos2(rect.left() + 3.0, y),
            egui::pos2(rect.right() - 3.0, y),
          ],
          cut_stroke,
        );
        strokes += 1;
      }
    }
  }
  strokes
}
