use cam_app::AppState;
use cam_ui::{Shell, ShellOutput};

use super::support::*;

// ---------------------------------------------------------------------------------------------
// S2's third column: the preview — the card, one shared scale, and two dimensions
// ---------------------------------------------------------------------------------------------

#[test]
fn the_preview_card_is_the_preview_column_and_cutting_width_follows_the_shank_ratio() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);
  let columns = opened.drawn.tool_columns.clone();

  let (small, _) = click_word(
    &ctx,
    &mut shell,
    &mut app,
    &opened,
    "T1 - Flat end mill (6mm)",
  );
  let small_card = small
    .drawn
    .tool_drawing
    .expect("the flat end mill was not drawn");
  assert!(
    small.drawn.tool_width_px > 0.0,
    "the drawing reported no cutting width"
  );
  assert_eq!(
    (small_card.2, small_card.3),
    (columns[2], 330.0),
    "the preview card is not the preview column's own rectangle"
  );

  // Without a shank diameter the cutting section equals the shank (a fixed body), so two such tools
  // share the same cutting width; a tool with a narrower cutter than its shank draws narrower.
  let (large, _) = click_word(
    &ctx,
    &mut shell,
    &mut app,
    &small,
    "T3 - Corner radius (12mm)",
  );
  let large_card = large
    .drawn
    .tool_drawing
    .expect("the corner radius was not drawn");
  assert_eq!(
    (large_card.2, large_card.3),
    (small_card.2, small_card.3),
    "the tools were drawn in different sized boxes"
  );
  assert!(
    (large.drawn.tool_width_px - small.drawn.tool_width_px).abs() < 0.5,
    "tools with no shank diameter draw the cutting section equal to the shank: {} vs {}",
    large.drawn.tool_width_px,
    small.drawn.tool_width_px
  );

  // A tool whose cutter is narrower than its shank draws the cutting section at the diameter ratio.
  let scratch = Scratch::new("ratio-preview");
  let mut app2 = AppState::new(document_with_toolpath(22));
  let mut shell2 = Shell::new();
  shell2
    .open_library_at(scratch.path())
    .expect("open a fresh factory library");
  shell2.open_tool_library(&app2);
  let _ = frame_with(&ctx, &mut shell2, &mut app2);
  let (t22, _) = frame_with(&ctx, &mut shell2, &mut app2);
  let t22w = t22.drawn.tool_width_px;
  // T22: 1 mm cutter in 3.175 mm shank => cutting narrower than shank and >= 6 px.
  assert!(t22w >= 6.0, "T22 cutting width must be >= 6 px, got {t22w}");
  // Shank width is 0.16 * inner (inner = 200 - 32 = 168 => 26.88); cutting ~8.47.
  assert!(
    t22w < 26.0,
    "T22 cutting section must be narrower than the shank: got {t22w}"
  );
}

#[test]
fn the_callouts_name_the_overall_length_and_the_diameter_and_nothing_the_library_lacks() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(1));
  let mut shell = Shell::new();
  // A record that holds both dimensions, and then one that holds neither.
  shell.set_tools(vec![measured("G1", Some(1))]);
  shell.open_tool_library(&app);
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (drawn, texts) = frame_with(&ctx, &mut shell, &mut app);

  assert!(
    drawn
      .drawn
      .tool_callouts
      .iter()
      .any(|callout| callout == "Overall length 38 mm"),
    "the length was not called out: {:?}",
    drawn.drawn.tool_callouts
  );
  assert!(
    drawn
      .drawn
      .tool_callouts
      .iter()
      .any(|callout| callout == "Diameter Ø6 mm"),
    "the diameter was not called out: {:?}",
    drawn.drawn.tool_callouts
  );
  // A callout is a sentence about a dimension line, and what the card paints is the dimension itself: the two
  // numbers the report's words are built from are on the card.
  let painted = joined(&texts);
  for number in ["38", "Ø6"] {
    assert!(
      painted.contains(number),
      "the callout naming {number} was reported and the card painted no such number: {painted}"
    );
  }

  // A tool the library never measured has no length to call out: a dimension the record does not hold is a
  // dimension the shell invented.
  let mut thin = Shell::new();
  thin.set_tools(vec![flat_mill("G1", Some(1), 6.0)]);
  thin.open_tool_library(&app);
  let _ = frame_with(&ctx, &mut thin, &mut app);
  let (bare, _) = frame_with(&ctx, &mut thin, &mut app);
  assert!(
    !bare
      .drawn
      .tool_callouts
      .iter()
      .any(|callout| callout.starts_with("Overall length")),
    "an unmeasured tool was given a length: {:?}",
    bare.drawn.tool_callouts
  );
  assert!(bare
    .drawn
    .tool_callouts
    .iter()
    .any(|c| c.starts_with("Diameter")));
}

#[test]
fn the_kinds_own_name_is_the_caption_under_the_card() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  // One row of each of the six kinds: the caption says what the tool is called, so it is the row's own family.
  let mut frame = opened;
  for (family, title, _) in frame.drawn.tool_rows.clone() {
    let (chosen, texts) = click_word(&ctx, &mut shell, &mut app, &frame, &title);
    assert!(
      chosen.drawn.tool_selected.is_some(),
      "the row {title:?} selected nothing"
    );
    assert_eq!(
      chosen.drawn.tool_preview_caption.as_deref(),
      Some(family.as_str()),
      "the caption under the {title:?} card is not the kind's own name"
    );
    assert!(
      joined(&texts).contains(&family),
      "the caption was reported and never painted: {}",
      joined(&texts)
    );
    frame = chosen;
  }
}

#[test]
fn t22_preview_draws_a_narrow_cutting_section_and_keeps_callouts_inside_the_card() {
  let ctx = egui::Context::default();
  let scratch = Scratch::new("t22-preview");
  let mut app = AppState::new(document_with_toolpath(22));
  let mut shell = Shell::new();
  shell
    .open_library_at(scratch.path())
    .expect("open a fresh factory library");
  shell.open_tool_library(&app);
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (drawn, texts) = frame_with(&ctx, &mut shell, &mut app);
  let (_, runs) = frame_with_runs(&ctx, &mut shell, &mut app);

  assert!(
    drawn.drawn.tool_width_px >= 8.0,
    "T22 cutting width must be >= 8 px, got {}",
    drawn.drawn.tool_width_px
  );
  assert!(
    drawn.drawn.tool_width_px >= 6.0,
    "T22 cutting width must be >= 6 px floor, got {}",
    drawn.drawn.tool_width_px
  );
  let card = drawn.drawn.tool_drawing.expect("T22 preview drew no card");
  let card_rect = egui::Rect::from_min_size(egui::pos2(card.0, card.1), egui::vec2(card.2, card.3));
  // All dimension callouts reported and their painted labels sit inside the card.
  for callout in &drawn.drawn.tool_callouts {
    assert!(!callout.is_empty(), "empty callout reported");
  }
  for label in ["Ø1", "Ø3.175"] {
    assert!(
      joined(&texts).contains(label) || drawn.drawn.tool_callouts.iter().any(|c| c.contains(label)),
      "expected dimension label {label} for T22; callouts={:?} texts={}",
      drawn.drawn.tool_callouts,
      joined(&texts)
    );
  }
  for (text, pos, size) in &runs {
    if (text.starts_with('Ø') || text == "38" || text == "7.7" || text == "7,7")
      && pos.x >= card_rect.left() - 1.0
      && pos.y <= card_rect.bottom() + 40.0
      && pos.x <= card_rect.right() + 40.0
    {
      let r = egui::Rect::from_min_size(*pos, *size);
      assert!(
        card_rect.intersects(r) || card_rect.contains(r.center()),
        "dimension text {text:?} painted outside/away from card: {r:?} not in {card_rect:?}"
      );
    }
  }
}

/// The modal, opened on the factory tool whose number is `number`, with its library seeded.
fn on_factory_tool(ctx: &egui::Context, number: u32) -> (AppState, Shell, ShellOutput) {
  let scratch = Scratch::new("preview-geometry");
  let mut app = AppState::new(document_with_toolpath(number));
  let mut shell = Shell::new();
  shell
    .open_library_at(scratch.path())
    .expect("the application's library opens");
  shell.open_tool_library(&app);
  let _ = frame_with(ctx, &mut shell, &mut app);
  let (output, _) = frame_with(ctx, &mut shell, &mut app);
  (app, shell, output)
}

fn rect_of(pixel: (f32, f32, f32, f32)) -> egui::Rect {
  egui::Rect::from_min_max(egui::pos2(pixel.0, pixel.1), egui::pos2(pixel.2, pixel.3))
}

#[test]
fn no_dimension_label_touches_its_line_the_tool_or_another_label() {
  let ctx = egui::Context::default();
  // A flat end mill (T22, with a shank and both lengths) and a V-bit (T4, with an angle and a flat) cover both
  // drawing paths: the rectangle and the cone.
  for number in [22, 4] {
    let (_app, _shell, output) = on_factory_tool(&ctx, number);
    let dimensions = &output.drawn.tool_preview_dimensions;
    assert!(
      dimensions.len() >= 3,
      "T{number} drew {} dimensions, too few to prove anything: {dimensions:?}",
      dimensions.len()
    );
    // Exactly one entry per painted dimension: a report that names the same dimension twice is a report a
    // consumer cannot deduplicate, and it is how this block shipped a duplicate before the review found it.
    let mut labels: Vec<&String> = dimensions.iter().map(|(label, _, _)| label).collect();
    labels.sort();
    let total = labels.len();
    labels.dedup();
    assert_eq!(
      labels.len(),
      total,
      "T{number}: a dimension was reported twice: {:?}",
      dimensions
        .iter()
        .map(|(label, _, _)| label)
        .collect::<Vec<_>>()
    );
    let body: Vec<egui::Rect> = output
      .drawn
      .tool_preview_body
      .iter()
      .map(|r| rect_of(*r))
      .collect();
    for (label, line, box_pixel) in dimensions {
      let line = rect_of(*line);
      let label_rect = rect_of(*box_pixel);
      assert!(
        !label_rect.shrink(0.5).intersects(line),
        "T{number}: the label {label:?} sits on its own line: label {label_rect:?} line {line:?}"
      );
      for part in &body {
        assert!(
          !label_rect.shrink(0.5).intersects(*part),
          "T{number}: the label {label:?} sits on the tool: label {label_rect:?} body {part:?}"
        );
      }
      for (other, _, other_box) in dimensions {
        if other == label {
          continue;
        }
        let other_rect = rect_of(*other_box);
        if other_rect == label_rect {
          continue;
        }
        assert!(
          !label_rect.shrink(0.5).intersects(other_rect),
          "T{number}: the labels {label:?} and {other:?} overlap: {label_rect:?} {other_rect:?}"
        );
      }
    }
  }
}

#[test]
fn the_v_bit_is_drawn_as_its_cone_and_not_as_a_second_rectangle() {
  let ctx = egui::Context::default();
  let (_app, _shell, output) = on_factory_tool(&ctx, 4);
  let (top, tip, height) = output
    .drawn
    .tool_preview_cone
    .expect("a V-bit draws its cone: the angle is what makes it a V-bit");

  // T4 is 90° with a 0.2 mm flat in a 12.7 mm body: the taper's height is `half_width / tan(45°)`, so the
  // height of a right isosceles triangle whose base is the body.
  assert!(top > tip, "the cone must taper: top {top} tip {tip}");
  let expected = (top - tip) / 2.0;
  assert!(
    (height - expected).abs() <= 1.0,
    "the 90° cone's height should be half its taper ({expected:.1} px), got {height:.1} px"
  );

  // And the flat is the tip's width, not the body's.
  let flat = 0.2_f32 / 12.7 * top;
  assert!(
    (tip - flat).abs() <= 1.0,
    "the tip must be the 0.2 mm flat ({flat:.1} px), got {tip:.1} px"
  );
}
