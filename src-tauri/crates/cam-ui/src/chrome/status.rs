use crate::Shell;

impl Shell {
  /// Toggle one kind of snap target.
  ///
  /// The chips in the status bar call this and nothing else, so the rule lives in one place: at least one
  /// kind stays armed, because a snapping strip with everything off is a magnet with no field.
  pub fn toggle_snap(&mut self, kind: cam_view::SnapKind) {
    match self.snap_kinds.iter().position(|armed| *armed == kind) {
      Some(index) if self.snap_kinds.len() > 1 => {
        self.snap_kinds.remove(index);
      }
      Some(_) => {}
      None => self.snap_kinds.push(kind),
    }
  }

  /// Whether a snap kind is armed.
  pub fn snap_armed(&self, kind: cam_view::SnapKind) -> bool {
    self.snap_kinds.contains(&kind)
  }

  /// The line the status bar shows at its right end: what the current gesture will do.
  ///
  /// The reference app keeps a sentence like this next to its snap chips ("Clique para selecionar · arraste
  /// da esquerda p/ direita"), and it is the cheapest teaching in the whole interface: it answers "what
  /// happens if I drag now" without a manual, and it changes with the tool instead of lying about it.
  pub fn hint(&self) -> String {
    if self.show_3d {
      // Both halves are on screen at once (S8), so the sentence names both gestures: the drawing half
      // still picks and draws, and the 3D half orbits.
      return "3D half — drag to orbit, wheel to zoom; the drawing half still picks and draws"
        .to_owned();
    }
    if self.active_tool.draws() {
      return format!(
        "{} — drag on the sheet to place it; Esc cancels",
        self.active_tool.label()
      );
    }
    "click to pick · drag left→right keeps what is fully inside · right→left takes what it crosses"
      .to_owned()
  }
}
