use cam_app::{AlignMode, AppState, VertexEdit};
use cam_geom::Point2;
use egui::RichText;

use crate::chrome;
use crate::params;
use crate::theme;
use crate::{Shell, ShellOutput, Tool, UiAction};

impl Shell {
  pub(crate) fn layer_dock(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    {
      egui::ScrollArea::vertical().show(ui, |ui| {
        for layer in &app.document().layers {
          let mut visible = layer.visible;
          ui.horizontal(|ui| {
            if ui.checkbox(&mut visible, "").changed() {
              output.actions.push(UiAction::ToggleLayerVisible {
                layer_id: layer.id.clone(),
              });
            }
            ui.label(format!("{} — {:?}/{:?}", layer.id, layer.role, layer.side));
          });
          ui.label(format!("  {} entit(ies)", layer.entities.len()));
          output
            .drawn
            .layer_rows
            .push((layer.id.clone(), layer.entities.len(), layer.visible));
          ui.separator();
        }
      });
    }
  }

  pub(crate) fn properties(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    {
      ui.heading("Properties");
      ui.separator();
      if ui
        .add_enabled(
          !app.selection().is_empty(),
          egui::Button::new("Delete selection"),
        )
        .clicked()
      {
        output.actions.push(UiAction::DeleteSelection);
      }
      let job = &app.document().board;
      let mut lines = vec![
        format!(
          "Job: {:.1} x {:.1} x {:.1} mm",
          job.width.mm(),
          job.height.mm(),
          job.thickness.mm()
        ),
        format!("Origin {:?}, side up {:?}", job.origin, job.side_up),
        format!("Default safe Z: {:.1} mm", cam_model::DEFAULT_SAFE_Z_MM),
      ];
      ui.label(&lines[0]);
      ui.label(&lines[1]);
      ui.label(&lines[2]);
      ui.separator();

      let selected: Vec<String> = app.selection().ids().map(str::to_owned).collect();
      if selected.is_empty() {
        lines.push("Selection: none".to_owned());
        ui.label("Selection: none");
      } else {
        for id in &selected {
          let summary = match app
            .document()
            .entity(id)
            .and_then(|entity| entity.kind.as_contour())
          {
            Some(contour) => format!(
              "{id}: contour, {:.1} mm perimeter, area {:.1} mm²",
              contour.perimeter(),
              contour.signed_area().abs()
            ),
            None => format!("{id}: (missing)"),
          };
          ui.label(&summary);
          lines.push(summary);
        }
      }
      ui.separator();
      lines.push(format!("History: {} step(s)", app.history_depth()));
      ui.label(format!("History: {} step(s)", app.history_depth()));
      output.drawn.property_lines = lines;
    }
  }

  /// The authoring dock: which tool is armed, and the numbers the commands use.
  ///
  /// Every button here reads a field of [`Shell`] into one [`UiAction`]; none of them talks to
  /// [`AppState`]. It is its own dock rather than a corner of the properties panel because a reviewer
  /// has to be able to see every command at once: egui culls widgets that fall outside their panel, so
  /// a panel that cannot fit them all would hide half of them.
  ///
  /// The seven parameterised commands are drawn and built from [`crate::params`] — the same module the
  /// palette's chips go through — so this tab owns neither their numbers nor their payloads any more. What
  /// stays here is the tab's own furniture: the tool list, the typed rectangle, the polygon and star counts,
  /// Align, Join, Trim and the three node edits, none of which a palette chip reaches.
  pub(crate) fn authoring(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    let centre = app.camera().center();
    // The trim this selection asks for, built by the same rule the canvas menu's row uses: None while fewer
    // than two vectors are selected, which is what leaves the button dark.
    let trim = chrome::trim_action(app);

    {
      ui.heading("Authoring");
      ui.separator();
      ui.horizontal_wrapped(|ui| {
        for tool in Tool::ALL {
          if ui
            .selectable_label(self.active_tool == tool, tool.label())
            .clicked()
          {
            self.active_tool = tool;
          }
        }
        ui.label(if self.active_tool.draws() {
          "drag on the canvas to draw"
        } else {
          "click to select, drag to marquee"
        });
      });

      // A rectangle by its numbers, next to the tool that draws one by hand: the same command, one
      // gesture or two typed values.
      ui.horizontal_wrapped(|ui| {
        ui.add(
          egui::DragValue::new(&mut self.rectangle_size.0)
            .speed(1.0)
            .suffix(" mm"),
        );
        ui.label("x");
        ui.add(
          egui::DragValue::new(&mut self.rectangle_size.1)
            .speed(1.0)
            .suffix(" mm"),
        );
        if ui.button("Add rectangle").clicked() {
          output.actions.push(UiAction::AddRectangle {
            width: self.rectangle_size.0,
            height: self.rectangle_size.1,
          });
        }
      });

      ui.horizontal_wrapped(|ui| {
        ui.label("Polygon sides");
        ui.add(egui::DragValue::new(&mut self.polygon_sides).range(3..=64));
        ui.label("Star tips");
        ui.add(egui::DragValue::new(&mut self.star_points).range(3..=64));
        ui.label("inner ratio");
        ui.add(
          egui::DragValue::new(&mut self.star_inner_ratio)
            .speed(0.05)
            .range(0.05..=0.95),
        );
      });

      ui.separator();
      ui.label(RichText::new("Transform").size(theme::SMALL));
      ui.horizontal_wrapped(|ui| {
        params::edit(ui, self, crate::help::Command::Move);
        if authoring_button(ui, output, crate::help::Command::Move) {
          output.actions.push(params::translate(self, centre));
        }
        ui.separator();
        params::edit(ui, self, crate::help::Command::Rotate);
        if authoring_button(ui, output, crate::help::Command::Rotate) {
          output.actions.push(params::rotate(self, centre));
        }
        ui.separator();
        params::edit(ui, self, crate::help::Command::Scale);
        if authoring_button(ui, output, crate::help::Command::Scale) {
          output.actions.push(params::scale(self, centre));
        }
        if authoring_button(ui, output, crate::help::Command::Mirror) {
          output.actions.push(params::mirror(self, centre));
        }
      });

      ui.separator();
      ui.label(RichText::new("Repeat and align").size(theme::SMALL));
      ui.horizontal_wrapped(|ui| {
        params::edit(ui, self, crate::help::Command::Array);
        if authoring_button(ui, output, crate::help::Command::Array) {
          output.actions.push(params::array(self, centre));
        }
        ui.separator();
        ui.label("Align");
        for (label, mode) in [
          ("Left", AlignMode::Left),
          ("Right", AlignMode::Right),
          ("Top", AlignMode::Top),
          ("Bottom", AlignMode::Bottom),
          ("Centre H", AlignMode::HorizontalCentre),
          ("Centre V", AlignMode::VerticalCentre),
        ] {
          if ui.button(label).clicked() {
            output.actions.push(UiAction::Align(mode));
          }
        }
      });

      ui.separator();
      ui.label(RichText::new("Vectors").size(theme::SMALL));
      ui.horizontal_wrapped(|ui| {
        params::edit(ui, self, crate::help::Command::Offset);
        if authoring_button(ui, output, crate::help::Command::Offset) {
          output.actions.push(params::offset(self, centre));
        }
        if ui.button("Join").clicked() {
          output.actions.push(UiAction::Join);
        }
        // Trim needs to know which of the two selected entities is the target and which the cutter; the
        // pairing lives in `chrome::trim_action`, so the button and the context menu's row cannot disagree.
        if ui
          .add_enabled(trim.is_some(), egui::Button::new("Trim"))
          .clicked()
        {
          output.actions.extend(trim.clone());
        }
      });
      ui.label(RichText::new("Nodes").size(theme::SMALL));
      ui.horizontal_wrapped(|ui| {
        ui.label("Node edit");
        ui.add(egui::DragValue::new(&mut self.vertex_index));
        ui.add(egui::DragValue::new(&mut self.segment_index));
        ui.add(
          egui::DragValue::new(&mut self.vertex_position.0)
            .speed(1.0)
            .suffix(" mm"),
        );
        ui.add(
          egui::DragValue::new(&mut self.vertex_position.1)
            .speed(1.0)
            .suffix(" mm"),
        );
        let vertex_at = Point2::new(self.vertex_position.0, self.vertex_position.1);
        if ui.button("Move node").clicked() {
          output.actions.push(UiAction::EditVertex(VertexEdit::Move {
            index: self.vertex_index,
            to: vertex_at,
          }));
        }
        if ui.button("Insert node").clicked() {
          output
            .actions
            .push(UiAction::EditVertex(VertexEdit::Insert {
              segment_index: self.segment_index,
              at: vertex_at,
            }));
        }
        if ui.button("Delete node").clicked() {
          output
            .actions
            .push(UiAction::EditVertex(VertexEdit::Delete {
              index: self.vertex_index,
            }));
        }
      });
    }
  }
}

/// Draw one Tools-tab command button, report where it landed, and say whether it was pressed.
///
/// The palette's chips report their points for the same reason
/// ([`DrawnState::palette_hits`](crate::DrawnState::palette_hits)): egui offers no
/// widget-by-text lookup from outside a frame, so a test that wants to press *this* button — the second of
/// the two routes a parameterised command is reachable by — is handed the point instead of guessing at the
/// panel's margins.
fn authoring_button(
  ui: &mut egui::Ui,
  output: &mut ShellOutput,
  command: crate::help::Command,
) -> bool {
  let response = ui.button(command.label());
  let centre = response.rect.center();
  output
    .drawn
    .authoring_hits
    .push((command.label().to_owned(), (centre.x, centre.y)));
  response.clicked()
}
