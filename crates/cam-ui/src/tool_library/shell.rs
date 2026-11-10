use std::path::Path;

use cam_app::AppState;
use cam_store::tool_edit::{duplicate_tool, factory_tools, new_tool, next_tool_number};
use cam_store::tool_file;
use cam_store::tools::{stored_tool_for_number, StoredTool, ToolLibrary};
use egui::RichText;

use crate::actions::file_label;
use crate::theme;
use crate::{ActionReport, Shell, ShellOutput};

use super::draft::ToolDraft;
use super::tree::tool_with;

/// The name a `New` tool starts with: a starting point rather than a suggestion, like every number in it.
pub(super) const NEW_TOOL_NAME: &str = "New tool";

/// One row of the master column's library actions that the window can honour itself (S3, S10).
///
/// `Export…` and `Import…` are deliberately not here: their answer is a path only the host's file dialog can
/// supply, so they are [`UiAction`](crate::UiAction)s that leave the window and come back as a path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ToolLibraryRequest {
  /// Append a tool with a fresh id and the lowest free tool number, and select it.
  New,
  /// Copy the selected tool as a new one, and select the copy.
  Duplicate,
  /// Remove the selected tool.
  Delete,
  /// Install the factory set.
  RestoreDefaults,
}

impl Shell {
  /// Apply one of the master column's library actions, and report it (S10).
  ///
  /// The ids of what the file now holds differently are reported in `tool_saved`, and a refusal is the
  /// sentence the modal draws: `WriteToolError`'s own words, unchanged, so the modal and the store cannot
  /// describe one failure two ways.
  pub(crate) fn apply_tool_library_request(
    &mut self,
    request: ToolLibraryRequest,
    output: &mut ShellOutput,
  ) {
    match self.library_action(request) {
      Ok(written) => {
        self.tool_error = None;
        output.drawn.tool_saved.extend(written);
      }
      Err(sentence) => self.tool_error = Some(sentence),
    }
  }

  /// Run one library action: the file first, then the window's snapshot.
  ///
  /// That order is the whole failure rule (S10). The snapshot is mutated **after** the write returns, so the
  /// one outcome this avoids is the pair disagreeing: a library that refused a tool never ends up described by
  /// a tree that lists it. With no library file there is nothing to write and the snapshot is the only copy,
  /// which is why the same code path serves both — [`Shell::write_tool_library`] is a no-op when the window
  /// knows no file. A `Delete` or a `Duplicate` with no selected tool writes nothing and reports nothing: the
  /// rows are drawn dark in that state, and this is the guard behind the picture rather than in place of it.
  fn library_action(&mut self, request: ToolLibraryRequest) -> Result<Vec<String>, String> {
    match request {
      ToolLibraryRequest::New => {
        let id = fresh_tool_id(&self.tools);
        let tool = new_tool(&id, NEW_TOOL_NAME, next_tool_number(&self.tools));
        self.write_tool_library(|library| {
          library.save_tool(&tool).map_err(|error| error.to_string())
        })?;
        let selected = tool.geometry.id.clone();
        self.tools.push(tool.clone());
        self.tool_library_selected = Some(selected.clone());
        self.tool_draft = Some(ToolDraft::of(&tool));
        Ok(vec![selected])
      }
      ToolLibraryRequest::Duplicate => {
        let selected = self.tool_library_selected.as_deref();
        let Some(original) = tool_with(&self.tools, selected).cloned() else {
          return Ok(Vec::new());
        };
        let id = fresh_tool_id(&self.tools);
        let copy = duplicate_tool(&original, &id, next_tool_number(&self.tools));
        self.write_tool_library(|library| {
          library.save_tool(&copy).map_err(|error| error.to_string())
        })?;
        self.tools.push(copy.clone());
        self.tool_library_selected = Some(id.clone());
        self.tool_draft = Some(ToolDraft::of(&copy));
        Ok(vec![id])
      }
      ToolLibraryRequest::Delete => {
        let Some(id) = self.tool_library_selected.clone() else {
          return Ok(Vec::new());
        };
        self.write_tool_library(|library| {
          // The row is the claim; whether the file still held one is not this window's to report. A file
          // that had already lost it leaves the snapshot and the file agreeing either way.
          library
            .remove_tool(&id)
            .map(|_| ())
            .map_err(|error| error.to_string())
        })?;
        self.tools.retain(|tool| tool.geometry.id != id);
        self.tool_library_selected = None;
        self.tool_draft = None;
        Ok(vec![id])
      }
      ToolLibraryRequest::RestoreDefaults => {
        let tools = factory_tools();
        self.write_tool_library(|library| {
          library
            .replace_tools(&tools)
            .map_err(|error| error.to_string())
        })?;
        let written = tools.iter().map(|tool| tool.geometry.id.clone()).collect();
        self.tools = tools;
        self.tool_library_selected = None;
        self.tool_draft = None;
        Ok(written)
      }
    }
  }

  /// Write the library through `write`, when this window knows which file holds it.
  ///
  /// `Ok(())` with no library file is not a success that wrote nothing: it is the state a snapshot handed in by
  /// a host leaves the window in ([`Shell::set_tools`]), where the snapshot is the only copy of the library and
  /// the four rows are drawn dark because no file can take the write.
  fn write_tool_library(
    &self,
    write: impl FnOnce(&ToolLibrary) -> Result<(), String>,
  ) -> Result<(), String> {
    match self.tool_library_path.as_deref() {
      Some(path) => {
        let library = ToolLibrary::open(path).map_err(|error| error.to_string())?;
        write(&library)
      }
      None => Ok(()),
    }
  }

  /// `Export…`: write this window's library to `path` as the file format's own JSON (S11).
  ///
  /// The path is the one the host's dialog returned and nothing else is read or written: a refusal is the
  /// writer's own sentence, kept in [`Shell::tool_error`] so the modal goes on showing why, and the report
  /// records the ids the file now holds so a test can tell this apart from an export nobody ran.
  pub(crate) fn export_tools(
    &mut self,
    label: &'static str,
    path: &Path,
  ) -> Result<ActionReport, String> {
    let written = tool_file::to_json(&self.tools)
      .map_err(|error| error.to_string())
      .and_then(|json| std::fs::write(path, json).map_err(|error| error.to_string()));
    match written {
      Ok(()) => {
        let ids: Vec<String> = self
          .tools
          .iter()
          .map(|tool| tool.geometry.id.clone())
          .collect();
        self.tool_error = None;
        self.tool_saved_pending = ids.clone();
        Ok(ActionReport {
          label,
          note: Some(format!(
            "{} tool(s) written to {}",
            ids.len(),
            file_label(path)
          )),
          ..ActionReport::default()
        })
      }
      Err(sentence) => {
        self.tool_error = Some(sentence.clone());
        Err(sentence)
      }
    }
  }

  /// `Import…`: read `path` and make its tools this window's library (S11).
  ///
  /// The file is read and validated by [`cam_store::tool_file::from_json`] **before** anything is written, so a
  /// file this build cannot use changes neither the library file nor the snapshot. On success the library file
  /// beside the document is replaced first and the snapshot second, the order every write in this module
  /// follows, and with no library file the snapshot is simply replaced.
  pub(crate) fn import_tools(
    &mut self,
    label: &'static str,
    path: &Path,
  ) -> Result<ActionReport, String> {
    let read = std::fs::read_to_string(path)
      .map_err(|error| error.to_string())
      .and_then(|text| tool_file::from_json(&text).map_err(|error| error.to_string()));
    let tools = match read {
      Ok(tools) => tools,
      Err(sentence) => {
        self.tool_error = Some(sentence.clone());
        return Err(sentence);
      }
    };
    let installed = match self.tool_library_path.clone() {
      Some(library_path) => ToolLibrary::open(&library_path)
        .map_err(|error| error.to_string())
        .and_then(|library| {
          library
            .replace_tools(&tools)
            .map_err(|error| error.to_string())
        })
        .map(|()| self.set_tools_from(library_path, tools)),
      None => {
        self.set_tools(tools);
        Ok(())
      }
    };
    match installed {
      Ok(()) => {
        let ids: Vec<String> = self
          .tools
          .iter()
          .map(|tool| tool.geometry.id.clone())
          .collect();
        self.tool_error = None;
        self.tool_saved_pending = ids.clone();
        Ok(ActionReport {
          label,
          note: Some(format!(
            "{} tool(s) read from {}",
            ids.len(),
            file_label(path)
          )),
          ..ActionReport::default()
        })
      }
      Err(sentence) => {
        self.tool_error = Some(sentence.clone());
        Err(sentence)
      }
    }
  }

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
    self.tool_error = None;
    // A write reported by a previous visit is not this visit's news: the report is a fact about this frame.
    self.tool_saved_pending.clear();
    self.show_tool_library = true;
  }

  /// Leave the library without writing anything.
  ///
  /// The drafts go with it: a search left behind would filter the next opening for a reason the operator has
  /// already forgotten, and a selection left behind would be a choice nobody made in this visit.
  /// Leave the library without writing anything.
  ///
  /// The drafts go with it: a search left behind would filter the next opening for a reason the operator
  /// has already forgotten, a selection left behind would be a choice nobody made in this visit, and an
  /// uncommitted tool draft left behind would be a form describing a tool nobody is editing with numbers
  /// nobody confirmed — the next opening seeds a fresh draft from whatever the tree selects.
  pub fn close_tool_library(&mut self) {
    self.show_tool_library = false;
    self.tool_library_target = None;
    self.tool_library_selected = None;
    self.tool_library_for_draft = false;
    self.tool_search.clear();
    self.tool_draft = None;
    self.tool_groups_closed.clear();
    self.tool_error = None;
    self.tool_saved_pending.clear();
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
    self.tool_error = None;
    self.tool_saved_pending.clear();
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

/// The id a new tool takes: `tool-<n>`, with the lowest `n` no tool in `tools` already uses.
///
/// An id and not a name, and derived from the ids on hand rather than from a counter, because two windows
/// editing the same library must not mint the same one — the store lists a tool by this id, so a collision is
/// one tool written over another.
fn fresh_tool_id(tools: &[StoredTool]) -> String {
  let mut candidate = 1;
  loop {
    let id = format!("tool-{candidate}");
    if !tools.iter().any(|tool| tool.geometry.id == id) {
      return id;
    }
    candidate += 1;
  }
}
