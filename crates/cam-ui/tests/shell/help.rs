use super::support::*;
use cam_app::AppState;
use cam_ui::help::{Command as HelpCommand, Group};
use cam_ui::Shell;

#[test]
fn every_command_has_a_what_and_a_how() {
  for command in HelpCommand::ALL {
    let help = command.help();
    assert!(
      !help.what.trim().is_empty(),
      "{command:?} has no consequence to state"
    );
    assert!(
      !help.how.trim().is_empty(),
      "{command:?} has no instruction to give"
    );
  }
}

#[test]
fn a_what_is_one_sentence() {
  // The design's first rule: one sentence of consequence, before any mechanics. A paragraph here is a
  // manual nobody reads, and a second line is a sentence the operator has to pick from.
  for command in HelpCommand::ALL {
    let what = command.help().what;
    assert!(
      !what.contains('\n'),
      "{command:?} says {what:?}, which is more than one line"
    );
    assert!(
      what.ends_with('.'),
      "{command:?} says {what:?}, which is not a sentence"
    );
    assert!(
      !what.contains('!'),
      "{command:?} says {what:?}, and the shell has no business shouting"
    );
    assert!(
      what.chars().count() <= 140,
      "{command:?} says {what:?} in {} characters, and one sentence is at most 140",
      what.chars().count()
    );
  }
}

#[test]
fn every_related_command_is_a_real_command() {
  for command in HelpCommand::ALL {
    for related in command.help().related {
      assert!(
        HelpCommand::ALL.contains(related),
        "{command:?} points at {related:?}, which this shell does not offer"
      );
    }
  }
}

#[test]
fn the_palette_and_the_table_agree_on_labels() {
  // The rail's own literals are the truth; the table follows them. Without this the help would slowly
  // become a second vocabulary for the same command.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, _) = frame(&mut shell, &mut app);
  let offered = output.drawn.palette;
  for command in [
    HelpCommand::Move,
    HelpCommand::Rotate,
    HelpCommand::Scale,
    HelpCommand::Mirror,
    HelpCommand::Array,
    HelpCommand::Offset,
    HelpCommand::Join,
  ] {
    assert!(
      offered.contains(&command.label().to_owned()),
      "the palette offers {offered:?} but the table calls this command {:?}",
      command.label()
    );
  }
}

#[test]
fn f1_opens_the_help_panel_and_it_names_every_group() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);

  let _ = press(&ctx, &mut shell, &mut app, egui::Key::F1);
  assert!(shell.show_help, "F1 must open the help panel");
  // One more frame: the panel's own size settles on the second pass, exactly as a real window does.
  let (output, texts) = frame_with(&ctx, &mut shell, &mut app);

  for group in Group::ALL {
    assert!(
      output.drawn.help.contains(&group.label().to_owned()),
      "the panel must name the {} group: {:?}",
      group.label(),
      output.drawn.help
    );
  }
  for command in HelpCommand::ALL {
    assert!(
      output.drawn.help.contains(&command.label().to_owned()),
      "the panel must list {:?}: {:?}",
      command.label(),
      output.drawn.help
    );
  }
  assert!(
    joined(&texts).contains("Help"),
    "the window must say what it is: {}",
    joined(&texts)
  );
  // The panel has no scroll area, so a row that does not fit is a command nobody can read about. These
  // three words are painted by the panel and by nothing else in the frame, and they are at the bottom.
  for alone in ["Icon set", "Shortcuts", "Fit the job"] {
    assert!(
      joined(&texts).contains(alone),
      "{alone:?} is below the fold of the help panel: {}",
      joined(&texts)
    );
  }
}

#[test]
fn the_help_panel_says_what_a_command_does_and_how() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let _ = press(&ctx, &mut shell, &mut app, egui::Key::F1);
  let (output, texts) = frame_with(&ctx, &mut shell, &mut app);

  let focused = HelpCommand::ALL[0];
  assert_eq!(
    shell.help_focused(),
    focused,
    "the panel opens on the first command"
  );
  let help = focused.help();
  let all = joined(&texts);
  assert!(
    all.contains(help.what),
    "the panel must state what {focused:?} gives: {all}"
  );
  assert!(
    all.contains(help.how),
    "the panel must say how to run {focused:?}: {all}"
  );
  assert!(
    output.drawn.help.contains(&help.what.to_owned())
      && output.drawn.help.contains(&help.how.to_owned()),
    "the panel must report the two sentences it drew: {:?}",
    output.drawn.help
  );
}

#[test]
fn escape_closes_the_help_panel() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let _ = press(&ctx, &mut shell, &mut app, egui::Key::F1);
  assert!(shell.show_help);

  let _ = press(&ctx, &mut shell, &mut app, egui::Key::Escape);
  assert!(!shell.show_help, "Esc must close the help panel");
  let (output, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    output.drawn.help.is_empty(),
    "a closed panel draws no rows: {:?}",
    output.drawn.help
  );
  assert!(
    !joined(&texts).contains(HelpCommand::ALL[0].help().what),
    "a closed panel teaches nothing: {}",
    joined(&texts)
  );
}

// ---- T8.2: the palette and the app bar read the command table --------------------------------

#[test]
fn the_help_button_opens_the_help_panel() {
  // "Help" means the F1 panel now: the panel teaches every command, so a help button that opened a
  // list of chords instead would be answering a smaller question than the one asked.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let (output, _) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    output.drawn.help.is_empty(),
    "no panel is open before the click: {:?}",
    output.drawn.help
  );

  let _ = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Help.label());
  assert!(shell.show_help, "the VIEW Help chip must open the F1 panel");

  let (output, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    !output.drawn.help.is_empty(),
    "the panel must draw its table"
  );
  assert!(
    output
      .drawn
      .help
      .contains(&HelpCommand::Help.label().to_owned()),
    "the panel must list the command that opened it: {:?}",
    output.drawn.help
  );
  assert!(
    !joined(&texts).contains("Zoom about the pointer"),
    "the Help chip must not open the shortcuts sheet: {}",
    joined(&texts)
  );
}

#[test]
fn the_shortcuts_chip_still_opens_the_shortcuts_sheet() {
  // The sheet keeps a mouse path of its own, so rewiring Help does not make the keys unreachable.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);

  let _ = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Shortcuts.label());
  let (_, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    joined(&texts).contains("Zoom about the pointer"),
    "the Shortcuts chip must open the shortcuts sheet: {}",
    joined(&texts)
  );
  assert!(
    !shell.show_help,
    "the Shortcuts chip must not open the help panel"
  );
}

#[test]
fn the_app_bar_labels_match_the_table() {
  // The bar holds no table command since camui-prancheta T2b: FILE moved to the rail. The report is empty,
  // and a button that came back — under any label — would have to come back here.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output.drawn.app_bar.is_empty(),
    "the app bar must draw no command button: {:?}",
    output.drawn.app_bar
  );
}
