use crate::Shell;

impl Shell {
  /// Whether the layers block is expanded.
  pub fn layers_open(&self) -> bool {
    self.layers_open
  }

  /// Expand or collapse the layers block.
  pub fn set_layers_open(&mut self, open: bool) {
    self.layers_open = open;
  }

  /// Collapse or expand the layers block, from its own heading.
  pub fn toggle_layers(&mut self) {
    self.layers_open = !self.layers_open;
  }
}
