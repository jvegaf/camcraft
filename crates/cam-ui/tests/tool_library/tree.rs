use cam_app::AppState;
use cam_store::tools::ToolKind;
use cam_ui::Shell;

use super::support::*;

// ---------------------------------------------------------------------------------------------
// S2: three columns, and the master column's tree
// ---------------------------------------------------------------------------------------------

#[test]
fn the_modal_lays_out_a_master_a_detail_and_a_preview_column_left_to_right() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  assert_eq!(
    opened.drawn.tool_columns,
    vec![300.0, 424.0, 200.0],
    "S2's three columns are mastered, detailed and previewed in that order and those widths"
  );

  // The widths are a claim about the picture, so the picture is asked where it put each column: the tree's
  // rows, the form's boxes and the preview's card, left to right.
  let (_, runs) = frame_with_runs(&ctx, &mut shell, &mut app);
  let row = opened
    .drawn
    .tool_library_hits
    .iter()
    .find(|(word, _)| word == "T2 - Ball nose (6mm)")
    .map(|(_, point)| point.0)
    .expect("the tree drew no row to read the master column from");
  let box_left = box_point(&runs, "Diameter").x;
  let card_left = opened
    .drawn
    .tool_drawing
    .expect("the preview drew no card")
    .0;
  assert!(
    row < box_left && box_left < card_left,
    "the three columns are not the master, the detail and the preview in that order: tree at {row}, \
     form at {box_left}, card at {card_left}"
  );
}

#[test]
fn the_tree_groups_the_library_by_kind_and_puts_the_count_in_the_heading() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  let mut tools = library();
  // A second flat end mill, so one count is a number other than one.
  tools.push(flat_mill("G7", Some(7), 3.175));
  shell.set_tools(tools);
  let opened = open_library(&ctx, &mut shell, &mut app);

  // One heading per kind the library holds, in the combo's own order, each with what sits under it.
  assert_eq!(
    opened.drawn.tool_families,
    vec![
      ("Flat end mill".to_owned(), 2),
      ("Ball nose".to_owned(), 1),
      ("Corner radius".to_owned(), 1),
      ("V-bit".to_owned(), 1),
      ("Engraving".to_owned(), 1),
      ("Drill".to_owned(), 1),
    ],
    "the tree does not group by kind, in one order, skipping the kinds it has no tool for"
  );
  // A list of counts cannot say *which* rows sit together, so the rows are read too.
  let families: Vec<&str> = opened
    .drawn
    .tool_rows
    .iter()
    .map(|(family, _, _)| family.as_str())
    .collect();
  assert_eq!(
    families,
    vec![
      "Flat end mill",
      "Flat end mill",
      "Ball nose",
      "Corner radius",
      "V-bit",
      "Engraving",
      "Drill",
    ]
  );
  let words = joined(&opened.drawn.tool_library);
  for heading in [
    "Flat end mill (2)",
    "Ball nose (1)",
    "Drill (1)",
    "Ø3.175 · 2 cut(s)",
  ] {
    assert!(
      words.contains(heading),
      "the tree never drew {heading:?}: {words}"
    );
  }
  // And the headings are the thing that folds the group, so they are pressable.
  for kind in ToolKind::ALL {
    let heading = format!("{} (", kind.label());
    assert!(
      hits(&opened).iter().any(|word| word.starts_with(&heading)),
      "the {heading} heading is not pressable, so its group cannot fold: {:?}",
      hits(&opened)
    );
  }
}

#[test]
fn a_heading_folds_and_unfolds_and_its_rows_and_their_hits_go_with_it() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  let (folded, _) = click_word(&ctx, &mut shell, &mut app, &opened, "Flat end mill (1)");
  assert_eq!(shell.tool_groups_closed, vec![ToolKind::FlatEndMill]);
  assert_eq!(
    folded.drawn.tool_rows.len(),
    5,
    "a folded heading still lists its rows: {:?}",
    folded.drawn.tool_rows
  );
  assert!(
    !folded
      .drawn
      .tool_rows
      .iter()
      .any(|(family, title, _)| family == "Flat end mill" && title.contains("T1")),
    "the folded group's row is still in the tree: {:?}",
    folded.drawn.tool_rows
  );
  // The row and its hit go together: a row folded away that is still pressable would be a control nobody can
  // see with a live hit area, which is the defect L23 names.
  assert!(
    !hits(&folded).contains(&"T1 - Flat end mill (6mm)"),
    "a folded row is still reported as pressable: {:?}",
    hits(&folded)
  );
  // The heading itself stays, so there is a way back.
  assert!(hits(&folded).contains(&"Flat end mill (1)"));

  let (unfolded, _) = click_word(&ctx, &mut shell, &mut app, &folded, "Flat end mill (1)");
  assert!(shell.tool_groups_closed.is_empty());
  assert_eq!(unfolded.drawn.tool_rows.len(), 6);
  assert!(hits(&unfolded).contains(&"T1 - Flat end mill (6mm)"));
}

#[test]
fn the_selected_row_is_the_one_the_report_names() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  // The document's toolpath cuts with T2, so that is the row the modal opens on.
  assert_eq!(opened.drawn.tool_selected.as_deref(), Some("G2"));

  let (chosen, _) = click_word(&ctx, &mut shell, &mut app, &opened, "T4 - V-bit (12.7mm)");
  assert_eq!(
    chosen.drawn.tool_selected.as_deref(),
    Some("G4"),
    "the row the operator pressed is not the tool the modal says it chose"
  );
  assert_eq!(
    chosen
      .drawn
      .tool_rows
      .iter()
      .find(|(_, title, _)| title == "T4 - V-bit (12.7mm)")
      .map(|(family, _, _)| family.as_str()),
    Some("V-bit"),
    "the chosen tool is not the one under the V-bit heading"
  );
  assert!(
    shell.tool_library_selection() == Some("G4"),
    "the shell's own selection disagrees with the report: {:?}",
    shell.tool_library_selection()
  );
}

#[test]
fn the_search_field_filters_the_tree_and_the_counts_name_what_is_left() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(2));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let _ = open_library(&ctx, &mut shell, &mut app);

  // A kind's own word: an operator who reads `Ball nose (1)` and types `ball` is asking for its rows.
  shell.tool_search = "ball".to_owned();
  let (filtered, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(
    filtered.drawn.tool_rows.len(),
    1,
    "the search field filtered nothing: {:?}",
    filtered.drawn.tool_rows
  );
  assert_eq!(filtered.drawn.tool_selected.as_deref(), Some("G2"));
  // The count is the number of rows drawn, not the number the library holds.
  assert_eq!(
    filtered.drawn.tool_families,
    vec![("Ball nose".to_owned(), 1)]
  );

  // A diameter on the cut line, which is how a machinist looks a tool up.
  shell.tool_search = "Ø12.7".to_owned();
  let (by_width, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(
    by_width.drawn.tool_rows,
    vec![(
      "V-bit".to_owned(),
      "T4 - V-bit (12.7mm)".to_owned(),
      "Ø12.7 · 2 cut(s)".to_owned()
    )],
    "the search never looked at the row's own cut line"
  );

  // And a search that matches nothing says so instead of drawing an empty tree.
  shell.tool_search = "spiral flute".to_owned();
  let (none, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert!(none.drawn.tool_rows.is_empty());
  assert!(none
    .drawn
    .tool_library
    .iter()
    .any(|word| word.contains("No tool matches")));
  assert!(joined(&texts).contains("No tool matches"));
}

#[test]
fn a_row_the_scroll_area_hides_is_not_reported_as_pressable() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(1));
  let mut shell = Shell::new();
  // More tools than the master column has room for: the ones below the fold are drawn by egui and clipped
  // away, and reporting their centres would be a promise about a control nobody can see.
  shell.set_tools(
    (1..=24)
      .map(|index| flat_mill(&format!("G{index}"), Some(index), 6.0))
      .collect(),
  );
  shell.open_tool_library(&app);
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (output, _) = frame_with(&ctx, &mut shell, &mut app);

  let rows: Vec<&str> = output
    .drawn
    .tool_rows
    .iter()
    .map(|(_, title, _)| title.as_str())
    .collect();
  assert_eq!(rows.len(), 24, "the tree lists every tool it was given");
  let pressable: Vec<&str> = hits(&output)
    .into_iter()
    .filter(|name| rows.contains(name))
    .collect();
  assert!(
    pressable.len() < rows.len(),
    "every row is reported as pressable, so nothing was clipped: {pressable:?}"
  );
  assert_eq!(
    pressable,
    rows[..pressable.len()],
    "the reported rows are not the ones at the top of the tree"
  );
  assert!(
    !pressable.contains(&"T24 - Flat end mill (6mm)"),
    "a row below the fold was reported as pressable"
  );
  assert!(
    output.drawn.tool_tree_scrollbar,
    "an overflowing tree must report a scrollbar"
  );
  let last = *pressable.last().expect("some row is visible");
  let at = output
    .drawn
    .tool_library_hits
    .iter()
    .find(|(name, _)| name == last)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .expect("the point it reported");
  click_at(&ctx, &mut shell, &mut app, at);
  assert!(
    shell.tool_library_selection().is_some_and(|id| id != "G1"),
    "the last reported row did not select its tool: {:?}",
    shell.tool_library_selection()
  );
}

#[test]
fn the_tree_rows_are_46_px_tall_with_non_intersecting_glyph_title_and_subtitle() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_toolpath(1));
  let mut shell = Shell::new();
  shell.set_tools(library());
  let opened = open_library(&ctx, &mut shell, &mut app);

  assert!(
    !opened.drawn.tool_tree_rows.is_empty(),
    "the tree reported no row geometry: {:?}",
    opened.drawn.tool_rows
  );
  for (index, (row, glyph, title, subtitle)) in opened.drawn.tool_tree_rows.iter().enumerate() {
    let (rx, ry, _rw, rh) = *row;
    let (gx, gy, gw, gh) = *glyph;
    let (tx, ty, tw, th) = *title;
    let (sx, sy, sw, sh) = *subtitle;
    assert!(
      (rh - 46.0).abs() < 0.5,
      "row {index} height is {rh}, expected 46"
    );
    if index + 1 < opened.drawn.tool_tree_rows.len() {
      let (nrx, nry, _, _) = opened.drawn.tool_tree_rows[index + 1].0;
      if (nrx - rx).abs() < 0.5 {
        let gap = nry - (ry + rh);
        // Adjacent rows in the same group are 48 px apart (46 px row + 2 px gap).
        if gap < 20.0 {
          assert!(
            (gap - 2.0).abs() < 0.5,
            "row {index} gap to next is {gap}, expected 2"
          );
        }
      }
    }
    assert!(
      (gx - (rx + 6.0)).abs() < 0.5 && (gw - 24.0).abs() < 0.5 && (gh - 24.0).abs() < 0.5,
      "row {index} glyph slot wrong: glyph={glyph:?} row={row:?}"
    );
    let glyph_cy = gy + gh / 2.0;
    let row_cy = ry + rh / 2.0;
    assert!(
      (glyph_cy - row_cy).abs() < 1.0,
      "row {index} glyph not vertically centred: glyph cy {glyph_cy} row cy {row_cy}"
    );
    assert!(
      (tx - (rx + 34.0)).abs() < 0.5 && (ty - (ry + 6.0)).abs() < 0.5,
      "row {index} title position wrong: title={title:?} row={row:?}"
    );
    assert!(
      (sx - (rx + 34.0)).abs() < 0.5 && (sy - (ry + 24.0)).abs() < 0.5,
      "row {index} subtitle position wrong: subtitle={subtitle:?} row={row:?}"
    );
    let title_r = egui::Rect::from_min_size(egui::pos2(tx, ty), egui::vec2(tw, th));
    let sub_r = egui::Rect::from_min_size(egui::pos2(sx, sy), egui::vec2(sw, sh));
    let glyph_r = egui::Rect::from_min_size(egui::pos2(gx, gy), egui::vec2(gw, gh));
    assert!(
      !title_r.intersects(sub_r),
      "row {index} title and subtitle intersect: {title_r:?} vs {sub_r:?}"
    );
    assert!(
      !glyph_r.intersects(title_r),
      "row {index} glyph and title intersect: {glyph_r:?} vs {title_r:?}"
    );
  }
}
