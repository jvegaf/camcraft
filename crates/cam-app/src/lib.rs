//! Application state: the document being edited, the selection, and the undo history.
//!
//! Everything the user can do is a [`Command`], applied through [`AppState::execute`]. That is what
//! makes the editor testable without a window: the UI is a thin adapter that turns input into
//! commands, and every rule lives here.
//!
//! # Undo strategy
//!
//! Snapshots, not inverse operations (decision D-M1-1). A document is kilobytes of JSON, so copying
//! it per command is cheap, and "undo restores exactly the previous document" is then true by
//! construction instead of by careful bookkeeping across every geometry edit. History is bounded by
//! [`MAX_HISTORY`] and reports how many old steps were dropped.
//!
//! Must not depend on `cam-ui`, `cam-view`, `egui` or any rendering crate.

#![warn(missing_docs)]

pub mod camera;
pub mod command;

use std::collections::{BTreeSet, VecDeque};

pub use camera::{Camera, ScreenPoint, Viewport, MAX_PIXELS_PER_MM, MIN_PIXELS_PER_MM};
pub use command::{AlignMode, Command, CommandError, CommandOutcome, Transform, VertexEdit};

use cam_model::Document;
use serde_json::Value;

/// How many undo steps are kept before the oldest is dropped.
pub const MAX_HISTORY: usize = 128;

/// The set of selected entities, ordered so iteration is deterministic.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
  ids: BTreeSet<String>,
}

impl Selection {
  /// True when nothing is selected.
  pub fn is_empty(&self) -> bool {
    self.ids.is_empty()
  }

  /// How many entities are selected.
  pub fn len(&self) -> usize {
    self.ids.len()
  }

  /// True when `id` is selected.
  pub fn contains(&self, id: &str) -> bool {
    self.ids.contains(id)
  }

  /// Selected ids, in ascending order.
  pub fn ids(&self) -> impl Iterator<Item = &str> {
    self.ids.iter().map(String::as_str)
  }
}

/// Undo and redo stacks of whole-document snapshots.
#[derive(Clone, Debug, Default)]
struct History {
  past: VecDeque<Document>,
  future: Vec<Document>,
  dropped: usize,
}

impl History {
  /// Record a snapshot before a change and discard the redo branch.
  fn record(&mut self, previous: Document) {
    self.future.clear();
    self.past.push_back(previous);
    while self.past.len() > MAX_HISTORY {
      self.past.pop_front();
      self.dropped += 1;
    }
  }

  fn undo(&mut self, current: &mut Document) -> bool {
    match self.past.pop_back() {
      Some(previous) => {
        self.future.push(current.clone());
        *current = previous;
        true
      }
      None => false,
    }
  }

  fn redo(&mut self, current: &mut Document) -> bool {
    match self.future.pop() {
      Some(next) => {
        self.past.push_back(current.clone());
        *current = next;
        true
      }
      None => false,
    }
  }
}

/// The editor's state.
#[derive(Clone, Debug)]
pub struct AppState {
  document: Document,
  camera: Camera,
  selection: Selection,
  history: History,
  dirty: bool,
  next_entity: u64,
}

impl AppState {
  /// Start editing `document` with an empty history.
  pub fn new(document: Document) -> Self {
    let next_entity = next_free_entity_number(&document);
    Self {
      document,
      camera: Camera::default(),
      selection: Selection::default(),
      history: History::default(),
      dirty: false,
      next_entity,
    }
  }

  /// The viewport camera.
  pub fn camera(&self) -> &Camera {
    &self.camera
  }

  /// The viewport camera, for pan, zoom and fit.
  pub fn camera_mut(&mut self) -> &mut Camera {
    &mut self.camera
  }

  /// Frame the given world bounds in the viewport.
  pub fn fit_camera(&mut self, bounds: cam_geom::Bounds, margin_px: f64) {
    self.camera.zoom_to_fit(bounds, margin_px);
  }

  /// The document being edited.
  pub fn document(&self) -> &Document {
    &self.document
  }

  /// Replace the document wholesale, clearing history and selection. Used when opening a file.
  pub fn replace_document(&mut self, document: Document) {
    self.next_entity = next_free_entity_number(&document);
    self.document = document;
    self.selection = Selection::default();
    self.history = History::default();
    self.dirty = false;
  }

  /// Replace the document but keep the previous one in history, so one undo brings it back.
  ///
  /// This is the import path (M3 A17): the drawing on screen changes, but the user did not ask to
  /// lose it, so the replacement is a step like any other command. [`AppState::replace_document`]
  /// stays the open-a-file path, where a fresh history is the correct behaviour.
  pub fn replace_document_undoable(&mut self, document: Document) {
    let previous = std::mem::replace(&mut self.document, document);
    self.next_entity = next_free_entity_number(&self.document);
    self.selection = Selection::default();
    self.history.record(previous);
    self.dirty = true;
  }

  /// The current selection.
  pub fn selection(&self) -> &Selection {
    &self.selection
  }

  /// True when there are unsaved changes.
  pub fn is_dirty(&self) -> bool {
    self.dirty
  }

  /// Record that the document was saved.
  pub fn mark_saved(&mut self) {
    self.dirty = false;
  }

  /// Apply a command, recording an undo snapshot first.
  ///
  /// The command is applied to a copy and validated; an invalid result is refused and the document,
  /// the history and the dirty flag are left untouched.
  pub fn execute(&mut self, command: Command) -> Result<(), CommandError> {
    self.execute_with_outcome(command).map(|_| ())
  }

  /// Apply a command and report what it produced beyond "it worked".
  ///
  /// Same contract as [`Self::execute`], which is this call with the report discarded: the command
  /// is applied to a copy, validated, and refused atomically. The report is how a drawing command
  /// hands back something the caller needs — today the radial error of an approximated ellipse
  /// (M2 A3) — without every other variant having to invent one.
  pub fn execute_with_outcome(&mut self, command: Command) -> Result<CommandOutcome, CommandError> {
    let mut candidate = self.document.clone();
    let outcome = command.apply_with_outcome(&mut candidate)?;
    candidate.validate()?;
    let previous = std::mem::replace(&mut self.document, candidate);
    self.history.record(previous);
    self.dirty = true;
    Ok(outcome)
  }

  /// Undo the last command. Returns false when there is nothing to undo.
  pub fn undo(&mut self) -> bool {
    let undone = self.history.undo(&mut self.document);
    if undone {
      self.dirty = true;
      self.prune_selection();
    }
    undone
  }

  /// Redo the last undone command. Returns false when there is nothing to redo.
  pub fn redo(&mut self) -> bool {
    let redone = self.history.redo(&mut self.document);
    if redone {
      self.dirty = true;
      self.prune_selection();
    }
    redone
  }

  /// True when [`Self::undo`] would do something.
  pub fn can_undo(&self) -> bool {
    !self.history.past.is_empty()
  }

  /// True when [`Self::redo`] would do something.
  pub fn can_redo(&self) -> bool {
    !self.history.future.is_empty()
  }

  /// Number of undo steps currently held.
  pub fn history_depth(&self) -> usize {
    self.history.past.len()
  }

  /// Number of steps available to redo.
  pub fn redo_depth(&self) -> usize {
    self.history.future.len()
  }

  /// Number of undo steps discarded because the history was full.
  pub fn history_dropped(&self) -> usize {
    self.history.dropped
  }

  /// Replace the selection.
  pub fn select(&mut self, ids: impl IntoIterator<Item = String>) {
    self.selection = Selection {
      ids: ids.into_iter().collect(),
    };
    self.prune_selection();
  }

  /// Clear the selection.
  pub fn clear_selection(&mut self) {
    self.selection = Selection::default();
  }

  /// The next deterministic entity id, `E1`, `E2`, ...
  ///
  /// Deterministic on purpose: random ids would make golden files and bug reports unstable.
  pub fn next_entity_id(&mut self) -> String {
    let id = format!("E{}", self.next_entity);
    self.next_entity += 1;
    id
  }

  /// Drop selected ids that no longer exist after an undo or redo.
  fn prune_selection(&mut self) {
    let existing: BTreeSet<String> = self
      .document
      .entities()
      .map(|entity| entity.id.clone())
      .collect();
    self.selection.ids.retain(|id| existing.contains(id));
  }

  /// The document as canonical JSON, used by tests and by the dirty-check on save.
  pub fn document_json(&self) -> Value {
    serde_json::to_value(&self.document).unwrap_or(Value::Null)
  }
}

/// The first `E<n>` identifier not already used by `document`.
///
/// Starting the counter at 1 regardless of what the file already contains is how a drawing session
/// ends up with two entities called `E1` and a silently broken selection.
fn next_free_entity_number(document: &Document) -> u64 {
  let highest = document
    .entities()
    .filter_map(|entity| {
      entity
        .id
        .strip_prefix('E')
        .and_then(|rest| rest.parse::<u64>().ok())
    })
    .max()
    .unwrap_or(0);
  highest + 1
}
