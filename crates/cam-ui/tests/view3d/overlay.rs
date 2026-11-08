use super::support::*;

#[test]
fn the_overlay_row_names_the_four_layers_and_every_chip_hides_exactly_its_own() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  let words: Vec<&str> = harness
    .output
    .drawn
    .layer_chips
    .iter()
    .map(|(word, _)| word.as_str())
    .collect();
  assert_eq!(
    words,
    [
      "Material",
      "Vectors",
      "Toolpaths",
      "Tool",
      "Simulate again",
      "Centre",
      "?"
    ],
    "S8's overlay row, in its own order, with M13 S3's `Simulate again` before `Centre`"
  );
  let showing: Vec<(String, bool)> = vec![
    ("Material".to_owned(), true),
    ("Vectors".to_owned(), true),
    ("Toolpaths".to_owned(), true),
    ("Tool".to_owned(), true),
  ];
  assert_eq!(
    harness.output.drawn.scene_layers, showing,
    "every layer starts visible"
  );

  // The fixture document holds one closed four-point rectangle and the preview one segment of four drawn
  // edges, drawn with a tool marker of an axis and a diameter.
  let counts = |harness: &Harness| {
    let drawn = &harness.output.drawn;
    (
      drawn.painted_material_lines,
      drawn.painted_vector_lines,
      drawn.painted_program_lines,
      drawn.painted_tool_lines,
    )
  };
  let whole = [12, 4, 4, 2];
  assert_eq!(
    counts(&harness),
    (whole[0], whole[1], whole[2], whole[3]),
    "twelve stock edges, four vectors, four program edges and the tool's axis and diameter"
  );

  for (word, hidden) in [
    ("Material", 0usize),
    ("Vectors", 1),
    ("Toolpaths", 2),
    ("Tool", 3),
  ] {
    let at = hit(&harness.output.drawn.layer_chips, word);
    harness.click(at);
    let after = counts(&harness);
    if word == "Material" {
      // Hiding the material hides the surface as well as the box: the triangles are the material, and a checkbox
      // that left them on would be half a control.
      assert_eq!(
        harness.output.drawn.painted_triangles, 0,
        "`Material` must take the simulated surface with it"
      );
    }
    for (index, expected) in whole.iter().enumerate() {
      let drawn = [after.0, after.1, after.2, after.3][index];
      if index == hidden {
        assert_eq!(drawn, 0, "`{word}` must hide its own layer");
      } else {
        assert_eq!(drawn, *expected, "`{word}` must leave layer {index} alone");
      }
    }
    assert!(
      !harness.output.drawn.scene_layers[hidden].1,
      "the row must say `{word}` is off in the frame it is pressed"
    );
    // Back on, so the next chip is measured against the whole picture.
    harness.click(at);
    assert_eq!(
      counts(&harness),
      (whole[0], whole[1], whole[2], whole[3]),
      "`{word}` comes back"
    );
    assert!(
      harness.output.drawn.painted_triangles > 0,
      "and so does the surface, with the material"
    );
  }
}

#[test]
fn the_centre_chip_reframes_the_stock_at_the_angle_the_eye_is_at() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  let framed = *harness.preview().home_camera();

  let (_, three_d) = harness.halves();
  harness.wheel(three_d.center(), 120.0);
  let from = three_d.center();
  harness.drag(from, egui::pos2(from.x + 80.0, from.y + 40.0));
  let away = *harness.preview().camera();
  assert!(
    (away.distance.mm() - framed.distance.mm()).abs() > 1.0e-6
      && (away.yaw - framed.yaw).abs() > 1.0e-6,
    "the fixture must have moved the eye before `Centre` is asked to put it back"
  );

  harness.click(hit(&harness.output.drawn.layer_chips, "Centre"));
  let back = *harness.preview().camera();
  assert_eq!(
    back.target, framed.target,
    "`Centre` puts the target back on the stock's centre"
  );
  assert!(
    (back.distance.mm() - framed.distance.mm()).abs() < 1.0e-9,
    "and the distance back to the one that frames it: {} against {}",
    back.distance.mm(),
    framed.distance.mm()
  );
  assert!(
    (back.yaw - away.yaw).abs() < 1.0e-9,
    "`Centre` keeps the angle the eye is at — that is what makes it not the home button"
  );
}

#[test]
fn the_question_mark_opens_the_help_panel() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  assert!(!harness.shell.show_help, "the panel starts closed");

  harness.click(hit(&harness.output.drawn.layer_chips, "?"));
  assert!(
    harness.shell.show_help,
    "`?` must do something: it opens the F1 panel, which teaches every command"
  );
}
