use cam_app::{AlignMode, AppState, VertexEdit};
use cam_geom::Point2;
use cam_model::{
  CutoutParams, DrillingParams, FillMethod, IsolationKind, IsolationParams, MillingDirection,
  NonCopperClearParams, Operation, PaintParams, PanelizeParams, TabKind,
};
use cam_units::Length;
use egui::RichText;

use crate::chrome;
use crate::chrome::{section_toggle, Category};
use crate::params;
use crate::theme::{self, Role};
use crate::{Shell, ShellOutput, Tool, UiAction};

impl Shell {
  pub(crate) fn layer_dock(&mut self, app: &AppState, ui: &mut egui::Ui, output: &mut ShellOutput) {
    {
      let palette = self.palette();
      ui.horizontal(|ui| {
        let mut copper = self.copper_mode;
        if ui.checkbox(&mut copper, "Copper mode").changed() {
          self.copper_mode = copper;
        }
      });
      ui.add_space(4.0);
      egui::ScrollArea::vertical()
        .scroll_source(egui::scroll_area::ScrollSource {
          drag: egui::scroll_area::DragScroll::Never,
          ..Default::default()
        })
        .show(ui, |ui| {
          for layer in &app.document().layers {
            let mut visible = layer.visible;
            let [r, g, b] = layer.role.color_rgb();
            let role_color = egui::Color32::from_rgb(r, g, b);
            ui.horizontal(|ui| {
              if ui.checkbox(&mut visible, "").changed() {
                output.actions.push(UiAction::ToggleLayerVisible {
                  layer_id: layer.id.clone(),
                });
              }
              let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
              ui.painter().rect_filled(rect, 2.0, role_color);
              ui.colored_label(
                if visible {
                  palette.get(Role::Text)
                } else {
                  palette.get(Role::TextDim)
                },
                format!("{} · {:?}", layer.role.label(), layer.side),
              );
            });
            ui.small(format!(
              "  {}  ·  {} entit(ies)",
              layer.id,
              layer.entities.len()
            ));
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

  /// Floating Studio panels (task 070): browser and inspector as floating cards over the canvas.
  pub(crate) fn floating_panels(
    &mut self,
    app: &AppState,
    ctx: &egui::Context,
    output: &mut ShellOutput,
  ) {
    let palette = self.palette();
    let canvas = egui::Rect::from_min_max(
      egui::pos2(self.last_canvas.x as f32, self.last_canvas.y as f32),
      egui::pos2(
        (self.last_canvas.x + self.last_canvas.width) as f32,
        (self.last_canvas.y + self.last_canvas.height) as f32,
      ),
    );

    // Palette/commands as a floating panel (preserves DrawnState palette_* for tests/tools).
    self.palette_panel(app, ctx, output);

    if !self.project_open {
      return;
    }

    let inspector_w = 324.0;
    let top = canvas.min.y + 12.0;
    let bottom_margin = 16.0;
    let max_h = (canvas.max.y - top - bottom_margin).max(120.0);

    // Inspector: one interactable hover-sense Area so widgets receive clicks but empty panel
    // background does not block canvas clicks (hit-test fall-through).
    let inspector_pos = egui::pos2(canvas.max.x - inspector_w - 16.0, top);
    let inspector_rect = egui::Rect::from_min_size(inspector_pos, egui::vec2(inspector_w, max_h));
    egui::Area::new(egui::Id::unique("floating_inspector"))
      .order(egui::Order::Middle)
      .fixed_pos(inspector_pos)
      .constrain(false)
      .interactable(true)
      .movable(false)
      .sense(egui::Sense::hover())
      .default_size(inspector_rect.size())
      .show(ctx, |ui| {
        ui.set_min_size(inspector_rect.size());
        ui.set_max_size(inspector_rect.size());
        let rect = inspector_rect;
        ui.painter().rect_filled(
          rect,
          egui::CornerRadius::same(8),
          palette.get(Role::Overlay),
        );
        ui.painter().rect_stroke(
          rect,
          egui::CornerRadius::same(8),
          egui::Stroke::new(1.0, palette.get(Role::Border)),
          egui::StrokeKind::Inside,
        );
        ui.add_space(8.0);
        let available = ui.available_size();
        ui.allocate_ui_with_layout(
          available,
          egui::Layout::left_to_right(egui::Align::Min),
          |ui| {
            ui.add_space(12.0);
            ui.vertical(|ui| {
              ui.set_max_width(rect.width() - 24.0);
              egui::ScrollArea::vertical()
                .id_salt(("inspector_scroll", self.task))
                .max_height(max_h - 16.0)
                .scroll_source(egui::scroll_area::ScrollSource {
                  drag: egui::scroll_area::DragScroll::Never,
                  ..Default::default()
                })
                .show(ui, |ui| {
                  ui.horizontal(|ui| {
                    ui.label(
                      RichText::new("Inspector")
                        .size(theme::CAPTION)
                        .color(palette.get(Role::TextDim))
                        .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                      if ui
                        .small_button(if self.inspector_open {
                          "Ocultar"
                        } else {
                          "Mostrar"
                        })
                        .clicked()
                      {
                        self.inspector_open = !self.inspector_open;
                      }
                    });
                  });
                  ui.add_space(6.0);
                  ui.separator();
                  if !self.inspector_open {
                    ui.add_space(8.0);
                    return;
                  }
                  ui.horizontal(|ui| {
                    for category in Category::ALL {
                      let selected = self.task == Some(category);
                      let label = match category {
                        Category::Toolpaths => "Toolpaths",
                        Category::Relief3d => "Relieve/3D",
                        Category::MachineOutput => "Máquina/Salida",
                      };
                      let response =
                        ui.selectable_label(selected, RichText::new(label).size(theme::SMALL));
                      let centre = response.rect.center();
                      output.drawn.categories.push(category.label().to_owned());
                      output
                        .drawn
                        .category_hits
                        .push((category.label().to_owned(), (centre.x, centre.y)));
                      if response.clicked() {
                        if self.task == Some(category) {
                          self.close_task(false);
                        } else {
                          self.open_task(category);
                        }
                      }
                    }
                  });
                  ui.add_space(4.0);
                  ui.separator();
                  ui.add_space(6.0);
                  if let Some(category) = self.task {
                    self.task_header(app, ui, &palette, category, output);
                    ui.add_space(4.0);
                  }
                  match self.task {
                    Some(category) => match category {
                      Category::Toolpaths => {
                        self.toolpath_list(app, ui, &palette, output);
                        if !app.document().toolpaths.is_empty() {
                          self.tool_card(app, ui, &palette, output);
                          self.toolpath_operation_editor(app, ui, &palette, output);
                        }
                        self.authoring(app, ui, output);
                      }
                      Category::Relief3d => self.relief_task(app, ui, &palette, output),
                      Category::MachineOutput => self.job_section(app, ui, &palette, output),
                    },
                    None => {
                      output.drawn.task_editor.clear();
                      if !app.document().toolpaths.is_empty() {
                        self.tool_card(app, ui, &palette, output);
                      }
                      self.authoring(app, ui, output);
                      self.properties(app, ui, output);
                    }
                  }
                  ui.separator();
                  let heading = section_toggle(ui, &palette, "LAYERS", self.layers_open);
                  let centre = heading.rect.center();
                  output.drawn.layers_heading = Some((centre.x, centre.y));
                  if heading.clicked() {
                    self.toggle_layers();
                  }
                  output.drawn.layers_open = self.layers_open;
                  if self.layers_open {
                    self.layer_dock(app, ui, output);
                  }
                  ui.add_space(8.0);
                });
            });
          },
        );
      });
  }

  fn toolpath_operation_editor(
    &mut self,
    app: &AppState,
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    output: &mut ShellOutput,
  ) {
    let Some(toolpath) = self.described_toolpath(app).cloned() else {
      return;
    };
    let doc_changed = self.toolpath_draft_doc_name != toolpath.name
      || self.toolpath_draft_doc_operation.as_ref() != Some(&toolpath.operation);
    if self.toolpath_draft_id.as_deref() != Some(toolpath.id.as_str()) || doc_changed {
      self.toolpath_draft_id = Some(toolpath.id.clone());
      self.toolpath_name_draft = toolpath.name.clone();
      self.toolpath_operation_draft = Some(toolpath.operation.clone());
      self.toolpath_draft_doc_name = toolpath.name.clone();
      self.toolpath_draft_doc_operation = Some(toolpath.operation.clone());
    }

    ui.add_space(6.0);
    ui.separator();
    ui.label(RichText::new("Operation").size(theme::SMALL).strong());
    ui.horizontal(|ui| {
      ui.label("Name");
      ui.text_edit_singleline(&mut self.toolpath_name_draft);
      if ui.button("Rename").clicked() {
        output.actions.push(UiAction::SetToolpathName {
          toolpath_id: toolpath.id.clone(),
          name: self.toolpath_name_draft.clone(),
        });
      }
    });

    let current_kind = self
      .toolpath_operation_draft
      .as_ref()
      .or(Some(&toolpath.operation))
      .map(operation_kind)
      .unwrap_or(OperationKind::Isolation);
    let mut selected_kind = current_kind;
    egui::ComboBox::from_label("Type")
      .selected_text(selected_kind.label())
      .show_ui(ui, |ui| {
        for kind in OperationKind::ALL {
          ui.selectable_value(&mut selected_kind, kind, kind.label());
        }
      });
    if selected_kind != current_kind {
      self.toolpath_operation_draft = Some(default_operation(selected_kind));
    }

    let Some(op) = self.toolpath_operation_draft.as_mut() else {
      return;
    };

    match op {
      Operation::Isolation(params) => {
        ui.add(
          egui::DragValue::new(&mut params.passes)
            .prefix("passes ")
            .range(1..=100),
        );
        ui.add(
          egui::DragValue::new(&mut params.overlap)
            .prefix("overlap ")
            .speed(0.01)
            .range(0.0..=1.0),
        );
        direction_combo(ui, &mut params.direction);
        isolation_kind_combo(ui, &mut params.kind);
        length_mm(ui, "cut depth", &mut params.cut_depth);
        ui.checkbox(&mut params.keep_pours, "keep pours");
      }
      Operation::NonCopperClear(params) => {
        ui.add(
          egui::DragValue::new(&mut params.overlap)
            .prefix("overlap ")
            .speed(0.01)
            .range(0.0..=1.0),
        );
        length_mm(ui, "margin", &mut params.margin);
        fill_method_combo(ui, &mut params.method);
        ui.checkbox(&mut params.connect, "connect");
        ui.checkbox(&mut params.contour, "contour");
        let mut has_copper_offset = params.copper_offset.is_some();
        if ui
          .checkbox(&mut has_copper_offset, "copper offset")
          .changed()
        {
          params.copper_offset = if has_copper_offset {
            Some(Length::from_mm(0.0))
          } else {
            None
          };
        }
        if let Some(offset) = params.copper_offset.as_mut() {
          length_mm(ui, "copper offset mm", offset);
        }
        direction_combo(ui, &mut params.direction);
        length_mm(ui, "cut depth", &mut params.cut_depth);
      }
      Operation::Cutout(params) => {
        length_mm(ui, "margin", &mut params.margin);
        tab_kind_combo(ui, &mut params.tab_kind);
        length_mm(ui, "pass depth", &mut params.pass_depth);
        length_mm(ui, "cut depth", &mut params.cut_depth);
        ui.checkbox(&mut params.cut_interiors, "cut interiors");
        ui.label(
          RichText::new(format!("Tabs: {}", params.tabs.len()))
            .size(theme::SMALL)
            .color(palette.get(Role::TextDim)),
        );
      }
      Operation::Paint(params) => {
        ui.add(
          egui::DragValue::new(&mut params.overlap)
            .prefix("overlap ")
            .speed(0.01)
            .range(0.0..=1.0),
        );
        length_mm(ui, "offset", &mut params.offset);
        fill_method_combo(ui, &mut params.method);
        ui.checkbox(&mut params.connect, "connect");
        ui.checkbox(&mut params.contour, "contour");
        length_mm(ui, "cut depth", &mut params.cut_depth);
      }
      Operation::Drilling(params) => {
        length_mm(ui, "cut depth", &mut params.cut_depth);
        length_mm(ui, "peck depth", &mut params.peck_depth);
        ui.add(
          egui::DragValue::new(&mut params.dwell)
            .prefix("dwell s ")
            .speed(0.01)
            .range(0.0..=60.0),
        );
      }
      Operation::Panelize(params) => {
        ui.add(
          egui::DragValue::new(&mut params.rows)
            .prefix("rows ")
            .range(1..=100),
        );
        ui.add(
          egui::DragValue::new(&mut params.columns)
            .prefix("cols ")
            .range(1..=100),
        );
        length_mm(ui, "spacing x", &mut params.spacing_x);
        length_mm(ui, "spacing y", &mut params.spacing_y);
      }
    }

    if ui.button("Apply operation").clicked() {
      output.actions.push(UiAction::SetToolpathOperation {
        toolpath_id: toolpath.id.clone(),
        operation: op.clone(),
      });
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OperationKind {
  Isolation,
  NonCopperClear,
  Cutout,
  Paint,
  Drilling,
  Panelize,
}

impl OperationKind {
  const ALL: [Self; 6] = [
    Self::Isolation,
    Self::NonCopperClear,
    Self::Cutout,
    Self::Paint,
    Self::Drilling,
    Self::Panelize,
  ];

  fn label(self) -> &'static str {
    match self {
      Self::Isolation => "Isolation",
      Self::NonCopperClear => "NCC",
      Self::Cutout => "Cutout",
      Self::Paint => "Paint",
      Self::Drilling => "Drilling",
      Self::Panelize => "Panelize",
    }
  }
}

fn operation_kind(op: &cam_model::Operation) -> OperationKind {
  match op {
    cam_model::Operation::Isolation(_) => OperationKind::Isolation,
    cam_model::Operation::NonCopperClear(_) => OperationKind::NonCopperClear,
    cam_model::Operation::Cutout(_) => OperationKind::Cutout,
    cam_model::Operation::Paint(_) => OperationKind::Paint,
    cam_model::Operation::Drilling(_) => OperationKind::Drilling,
    cam_model::Operation::Panelize(_) => OperationKind::Panelize,
  }
}

fn default_operation(kind: OperationKind) -> cam_model::Operation {
  match kind {
    OperationKind::Isolation => Operation::Isolation(IsolationParams {
      passes: 1,
      overlap: 0.5,
      direction: MillingDirection::Climb,
      kind: IsolationKind::Full,
      cut_depth: Length::from_mm(0.1),
      keep_pours: false,
    }),
    OperationKind::NonCopperClear => Operation::NonCopperClear(NonCopperClearParams {
      overlap: 0.5,
      margin: Length::from_mm(0.0),
      method: FillMethod::Standard,
      connect: false,
      contour: true,
      copper_offset: None,
      direction: MillingDirection::Climb,
      cut_depth: Length::from_mm(0.1),
    }),
    OperationKind::Cutout => Operation::Cutout(CutoutParams {
      margin: Length::from_mm(0.0),
      tabs: Vec::new(),
      tab_kind: TabKind::Straight,
      pass_depth: Length::from_mm(0.0),
      cut_depth: Length::from_mm(1.6),
      cut_interiors: false,
    }),
    OperationKind::Paint => Operation::Paint(PaintParams {
      overlap: 0.5,
      offset: Length::from_mm(0.0),
      method: FillMethod::Standard,
      connect: false,
      contour: true,
      cut_depth: Length::from_mm(0.1),
    }),
    OperationKind::Drilling => Operation::Drilling(DrillingParams {
      cut_depth: Length::from_mm(1.6),
      peck_depth: Length::from_mm(0.0),
      dwell: 0.0,
    }),
    OperationKind::Panelize => Operation::Panelize(PanelizeParams {
      rows: 1,
      columns: 1,
      spacing_x: Length::from_mm(2.0),
      spacing_y: Length::from_mm(2.0),
      constrain_x: None,
      constrain_y: None,
    }),
  }
}

fn length_mm(ui: &mut egui::Ui, label: &str, value: &mut cam_units::Length) {
  let mut mm = value.mm();
  if ui
    .add(
      egui::DragValue::new(&mut mm)
        .prefix(format!("{label} "))
        .suffix(" mm")
        .speed(0.1),
    )
    .changed()
  {
    *value = cam_units::Length::from_mm(mm);
  }
}

fn direction_combo(ui: &mut egui::Ui, value: &mut cam_model::MillingDirection) {
  egui::ComboBox::from_label("direction")
    .selected_text(format!("{:?}", value))
    .show_ui(ui, |ui| {
      ui.selectable_value(value, cam_model::MillingDirection::Climb, "Climb");
      ui.selectable_value(
        value,
        cam_model::MillingDirection::Conventional,
        "Conventional",
      );
    });
}

fn isolation_kind_combo(ui: &mut egui::Ui, value: &mut cam_model::IsolationKind) {
  egui::ComboBox::from_label("kind")
    .selected_text(format!("{:?}", value))
    .show_ui(ui, |ui| {
      ui.selectable_value(value, cam_model::IsolationKind::Exterior, "Exterior");
      ui.selectable_value(value, cam_model::IsolationKind::Interior, "Interior");
      ui.selectable_value(value, cam_model::IsolationKind::Full, "Full");
    });
}

fn fill_method_combo(ui: &mut egui::Ui, value: &mut cam_model::FillMethod) {
  egui::ComboBox::from_label("method")
    .selected_text(format!("{:?}", value))
    .show_ui(ui, |ui| {
      ui.selectable_value(value, cam_model::FillMethod::Standard, "Standard");
      ui.selectable_value(value, cam_model::FillMethod::Seed, "Seed");
      ui.selectable_value(value, cam_model::FillMethod::Lines, "Lines");
    });
}

fn tab_kind_combo(ui: &mut egui::Ui, value: &mut cam_model::TabKind) {
  egui::ComboBox::from_label("tab kind")
    .selected_text(format!("{:?}", value))
    .show_ui(ui, |ui| {
      ui.selectable_value(value, cam_model::TabKind::Straight, "Straight");
      ui.selectable_value(value, cam_model::TabKind::Thin, "Thin");
      ui.selectable_value(value, cam_model::TabKind::MouseBites, "MouseBites");
    });
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
