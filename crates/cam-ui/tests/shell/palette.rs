use super::support::*;
use cam_app::{AppState, Transform};
use cam_geom::{Point2, Vec2};
use cam_ui::help::{Command as HelpCommand, Group};
use cam_ui::{Shell, Tab, Tool, UiAction, DEFAULT_TOLERANCE_MM};

#[test]
fn the_palette_labels_its_groups() {
  // The reference app's lesson: a labelled group is what keeps twenty commands findable, and an
  // unlabelled rail is what made the seven Edit commands nameless. The five words are the reference's own
  // (camui-prancheta S4).
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (_, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  for label in ["DRAW", "EDIT", "ORGANISE", "SHOW AND SELECT"] {
    assert!(
      all.contains(label),
      "the rail must label the {label} run: {all}"
    );
  }
}

/// The groups the palette shows, SHOW AND SELECT last: the four runs that hold palette commands.
///
/// FILE is absent on purpose and not by accident: every file command still lives in the app bar, and a
/// run the rail draws with no chips in it would be a heading over nothing (camui-prancheta T2a).
fn palette_groups() -> Vec<Group> {
  vec![
    Group::File,
    Group::Draw,
    Group::Edit,
    Group::Organise,
    Group::ShowAndSelect,
  ]
}

/// The words the palette must offer, in the table's own order.
///
/// Derived from [`HelpCommand::ALL`] and not from a list of expectations: the point of the slice is
/// that the table is the only list, so a test that wrote its own second list would not test that.
fn palette_words() -> Vec<String> {
  let groups = palette_groups();
  HelpCommand::ALL
    .iter()
    .filter(|command| groups.contains(&command.group()) && command.in_palette())
    .map(|command| command.label().to_owned())
    .collect()
}

#[test]
fn every_palette_entry_comes_from_the_table() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();

  let (output, _) = frame_with(&ctx, &mut shell, &mut app);
  let offered = output.drawn.palette.clone();
  assert_eq!(
    offered,
    palette_words(),
    "the palette must be the table's own run, in its order"
  );
  for word in &offered {
    assert!(
      HelpCommand::ALL
        .iter()
        .any(|command| command.label() == word),
      "{word:?} is on the palette but no command in the table answers to it: {offered:?}"
    );
  }
}

#[test]
fn the_palette_shows_all_five_runs() {
  // The rail is the whole command table now, FILE included: `Open`, `Save`, `Save as`, `Import`, `Fit the
  // job`, `Undo` and `Redo` moved out of the app bar in camui-prancheta T2b (S4).
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();

  let (output, texts) = frame_with(&ctx, &mut shell, &mut app);
  let all = joined(&texts);
  for group in palette_groups() {
    assert!(
      all.contains(group.label()),
      "the palette must label the {} run: {all}",
      group.label()
    );
    assert!(
      output
        .drawn
        .palette_headings
        .iter()
        .any(|(label, _)| label == group.label()),
      "the {} heading must be drawn as a control: {:?}",
      group.label(),
      output.drawn.palette_headings
    );
  }
}

#[test]
fn the_palette_chips_still_arm_toggle_and_run() {
  // PRESERVE (T8.2): the palette is built from the table now, and every chip has to do exactly what it
  // did when the list was hand-written — arm a gesture, flip a view flag, or push the Tools tab's
  // numbers as an action.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);

  let _ = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Line.label());
  assert_eq!(
    shell.active_tool,
    Tool::Line,
    "a DRAW chip arms its gesture"
  );

  let _ = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Grid.label());
  assert!(!shell.snapping, "the Grid chip toggles snapping off");

  let _ = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Layers.label());
  assert!(
    !shell.inspector_open(),
    "the Layers chip collapses the inspector"
  );

  let (output, _) = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Move.label());
  match output.actions.as_slice() {
    [UiAction::Transform(Transform::Translate { delta })] => assert_eq!(
      *delta,
      Vec2::new(10.0, 0.0),
      "the Move chip reads the Tools tab's offset"
    ),
    other => panic!("the Move chip must push one Translate action, got {other:?}"),
  }
}

#[test]
fn the_palette_runs_the_array_grid_its_own_form_can_type() {
  // The grid count is the one number with a range, and only the Tools tab's widget applies it: the field
  // itself can hold anything, and the palette hands those numbers straight to the command. A 0-row array
  // is refused downstream with a sentence, so the chip has to run what its own form can type instead of
  // letting the two disagree.
  //
  // The inspector is on Selection, not Tools, because a visible `DragValue` clamps its own field on the
  // frame it is drawn: with the form on screen the number is already legal and the disagreement is
  // invisible. The Tools tab being hidden is exactly the case a number that no widget guards can reach.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  shell.set_tab(Tab::Selection);
  shell.array_grid = (0, 99);
  let _ = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(
    shell.array_grid,
    (0, 99),
    "nothing on screen may quietly rewrite the field this test just set"
  );

  let (output, _) = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Array.label());
  match output.actions.as_slice() {
    [UiAction::Array { rows, columns, .. }] => {
      assert_eq!(
        (*rows, *columns),
        (1, 64),
        "the Array chip must run a grid inside the form's range: 1..=64"
      );
    }
    other => panic!("the Array chip must push one Array action, got {other:?}"),
  }
  assert_eq!(
    shell.array_grid,
    (0, 99),
    "the clamp belongs to the action, not to what the operator typed"
  );
}

#[test]
fn the_palette_and_the_tools_tab_build_the_same_seven_actions() {
  // This is the test the fold exists for: the two surfaces that run a parameterised command used to build
  // the payload themselves, and the palette path also skipped the array grid's range. Pinning the exact
  // `UiAction` each route emits — not merely that the two agree — is what makes a route that reads the
  // wrong field visible, so the numbers below are deliberately not the defaults.
  let mut app = AppState::new(document_with_rectangle());
  let mut framing = Shell::new();
  framing.open_task(cam_ui::Category::Toolpaths);
  let _ = frame_with(&egui::Context::default(), &mut framing, &mut app);
  // Every `about` and `origin` names the view centre, and the shell frames the drawing on its first frame:
  // the point a payload carries is a measurement of the camera, not the origin a default happens to hold.
  let centre = app.camera().center();
  assert!(
    !centre.is_near(Point2::ORIGIN, cam_geom::TolerancePolicy::uniform(1.0e-9)),
    "the payload's point must be the framed view centre, not the origin: {centre:?}"
  );

  let mut numbers = Shell::new();
  // The authoring buttons live in the `TOOLPATHS` task since camui-prancheta S6, so both routes are measured
  // with that task open — the palette chip and the task's button are the same command either way.
  numbers.open_task(cam_ui::Category::Toolpaths);
  numbers.move_delta = (3.5, -2.0);
  numbers.rotate_degrees = 42.0;
  numbers.scale_factors = (1.5, 0.5);
  numbers.array_grid = (3, 4);
  numbers.array_spacing = (12.0, 6.0);
  numbers.offset_distance = 4.25;

  let cases = [
    (
      HelpCommand::Move,
      UiAction::Transform(Transform::Translate {
        delta: Vec2::new(3.5, -2.0),
      }),
    ),
    (
      HelpCommand::Rotate,
      UiAction::Transform(Transform::Rotate {
        radians: 42.0_f64.to_radians(),
        about: centre,
      }),
    ),
    (
      HelpCommand::Scale,
      UiAction::Transform(Transform::Scale {
        sx: 1.5,
        sy: 0.5,
        about: centre,
        tolerance: DEFAULT_TOLERANCE_MM,
      }),
    ),
    (
      HelpCommand::Mirror,
      UiAction::Transform(Transform::Mirror {
        point: centre,
        direction: Vec2::Y_AXIS,
      }),
    ),
    (
      HelpCommand::Array,
      UiAction::Array {
        rows: 3,
        columns: 4,
        dx: 12.0,
        dy: 6.0,
      },
    ),
    (HelpCommand::Offset, UiAction::Offset { distance: 4.25 }),
  ];

  for (command, expected) in cases {
    let mut palette_shell = numbers.clone();
    let palette_ctx = egui::Context::default();
    let _ = frame_with(&palette_ctx, &mut palette_shell, &mut app);
    let (from_palette, _) =
      click_palette_chip(&palette_ctx, &mut palette_shell, &mut app, command.label());
    assert_eq!(
      from_palette.actions,
      vec![expected.clone()],
      "the palette's {} chip",
      command.label()
    );

    let mut tab_shell = numbers.clone();
    let tab_ctx = egui::Context::default();
    let from_tab = click_authoring_button(&tab_ctx, &mut tab_shell, &mut app, command.label());
    assert_eq!(
      from_tab.actions,
      vec![expected],
      "the Tools tab's {} button",
      command.label()
    );
  }
}

#[test]
fn every_palette_chip_is_named_by_its_tooltip() {
  // A headless frame reads neither the pixels nor the hover card, so the closest thing it can assert is
  // that the words a chip shows on hover contain the word the chip itself stands for.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();

  let (output, _) = frame_with(&ctx, &mut shell, &mut app);
  for word in &output.drawn.palette {
    let command = HelpCommand::ALL
      .iter()
      .copied()
      .find(|command| command.label() == word)
      .unwrap_or_else(|| panic!("{word:?} is drawn on the palette but no command answers to it"));
    let tooltip = cam_ui::chrome::palette_tooltip(command);
    assert!(
      tooltip.contains(word),
      "the {word:?} chip's tooltip is {tooltip:?}, which never names the chip"
    );
  }
}

// ---------------------------------------------------------------------------------------------
// T12 — the material dialog: the first thing a job defines (S2 S7 S8).
//
// The three measurements used to be read-only boxes, and the only way to size a job was `cam-cli`. The
// dialog the reference draws (`03-26-19`) is now the shell's one writer of `document.board`, and what these
// tests pin is behaviour: which numbers it shows, which it writes, and what a refused number leaves behind.
// ---------------------------------------------------------------------------------------------
