fn base_tool(id: &str, number: Option<u32>, diameter: Option<f64>) -> StoredTool {
  StoredTool {
    geometry: ToolGeometry {
      id: id.to_owned(),
      name_format: "{Tool Type} ({Diameter|F}{Units Short})".to_owned(),
      tool_type: 0,
      units: UnitSystem::Mm,
      diameter: diameter.map(Length::from_mm),
      included_angle: None,
      flat_diameter: None,
      num_flutes: Some(2),
      flute_length: None,
      tip_radius: None,
      shank_diameter: None,
      overall_length: None,
      custom_attributes: None,
    },
    cutting: CuttingData {
      id: format!("{id}-cut"),
      rate_units: UnitSystem::Mm,
      feed_rate: Some(100.0),
      plunge_rate: Some(30.0),
      spindle_speed: Some(18_000),
      stepdown: None,
      stepover: None,
      clear_stepover: None,
      tool_number: number,
      notes: None,
    },
  }
}

fn default_tools() -> Vec<StoredTool> {
  let g1 = base_tool("G1", Some(1), Some(6.0));
  let g5 = base_tool("G5", Some(5), Some(3.175));
  let mut g2 = base_tool("G2", Some(2), Some(6.0));
  g2.geometry.tip_radius = Some(Length::from_mm(3.0));
  let mut g3 = base_tool("G3", Some(3), Some(12.7));
  g3.geometry.included_angle = Some(Angle::from_degrees(90.0));
  g3.geometry.flat_diameter = Some(Length::from_mm(0.2));
  vec![g1, g5, g2, g3]
}

use cam_app::AppState;
use cam_store::tools::{stored_tool_for_number, CuttingData, StoredTool, ToolGeometry};
use cam_units::{Angle, Length, UnitSystem};
use egui::RichText;

use crate::theme;
use crate::{Shell, ShellOutput};

impl Shell {
  /// Open the tool library for the toolpath the current-tool card names.
  ///
  /// The card describes the **selected** toolpath, or the document's first when nothing is selected (M13 S4),
  /// and that is what this dialog retools — one resolver, so the card and the dialog cannot disagree about
  /// which toolpath `Choose another tool…` is about. The selection starts on the tool that toolpath already
  /// cuts with, because the first thing an operator wants from `Choose another tool…` is to see what the tool
  /// is now.
  ///
  /// Public for the reason [`Shell::open_material`] is: a modal whose opening cannot be driven from outside a
  /// frame is a modal only a mouse could test.
  pub fn open_tool_library(&mut self, app: &AppState) {
    if self.tools.is_empty() {
      self.tools = default_tools();
    }
    let target = self.described_toolpath(app).map(|toolpath| {
      (
        toolpath.id.clone(),
        stored_tool_for_number(&self.tools, toolpath.tool_number)
          .map(|tool| tool.geometry.id.clone()),
      )
    });
    self.tool_library_target = target.as_ref().map(|(id, _)| id.clone());
    self.tool_library_selected = target.and_then(|(_, selected)| selected);
    self.tool_search.clear();
    self.show_tool_library = true;
  }

  /// Leave the library without writing anything.
  ///
  /// The drafts go with it: a search left behind would filter the next opening for a reason the operator has
  /// already forgotten, and a selection left behind would be a choice nobody made in this visit.
  pub fn close_tool_library(&mut self) {
    self.show_tool_library = false;
    self.tool_library_target = None;
    self.tool_library_selected = None;
    self.tool_library_for_draft = false;
    self.tool_search.clear();
  }

  /// Open the library to choose the tool a **new** strategy will cut with (camui-prancheta S10).
  ///
  /// The same modal and the same commit, with the one difference the subject forces: the strategy editor is
  /// drafting a toolpath that does not exist yet, so `Use this tool` has no document row to write and writes the
  /// draft's tool number instead. Opening it from the card keeps the modal's own promise — the selection starts
  /// on the tool the draft already names, or on nothing when it names none.
  pub(crate) fn open_tool_library_for_draft(&mut self) {
    self.tool_library_target = None;
    self.tool_library_selected = self.strategy.tool_number.and_then(|number| {
      stored_tool_for_number(&self.tools, number).map(|tool| tool.geometry.id.clone())
    });
    self.tool_library_for_draft = true;
    self.tool_search.clear();
    self.show_tool_library = true;
  }

  /// The toolpath `Use this tool` would retool, by id, while the modal is open.
  pub fn tool_library_target(&self) -> Option<&str> {
    self.tool_library_target.as_deref()
  }

  /// The tool the detail side is showing, by the library's own geometry id.
  pub fn tool_library_selection(&self) -> Option<&str> {
    self.tool_library_selected.as_deref()
  }

  /// The current-tool card's `Choose another tool…` row: the one affordance that opens the library.
  ///
  /// Its words are reported apart from [`crate::DrawnState::tool_card`] on purpose: the card is a reading of
  /// the tool a toolpath cuts with, and it grew a control, not a fourth field.
  pub(crate) fn choose_tool_row(
    &mut self,
    app: &AppState,
    ui: &mut egui::Ui,
    output: &mut ShellOutput,
  ) {
    let response = ui.button(RichText::new(CHOOSE).size(theme::SMALL));
    let centre = response.rect.center();
    output
      .drawn
      .tool_card_hits
      .push((CHOOSE.to_owned(), (centre.x, centre.y)));
    if response.clicked() {
      // The subject decides what the choice can write (S10): a toolpath on screen is retooled, a draft is
      // filled in for `Generate toolpath` to write.
      if app.document().toolpaths.is_empty() {
        self.open_tool_library_for_draft();
      } else {
        self.open_tool_library(app);
      }
    }
  }
}

/// The word on the card's opener, spelled the way the reference frame spells it.
pub(crate) const CHOOSE: &str = "Choose another tool…";
