//! The shell's visual tokens, in one place, checked by tests instead of by eye.
//!
//! `design/camui-shell/04-tokens.html` is the authority for what these numbers are and why; this module is
//! its Rust projection, and the only place a colour, a radius, a control height or a type size may be
//! declared. A widget that wants a shade of grey asks here, so the picture stays consistent when a token
//! moves.
//!
//! Two decisions are worth stating because they are easy to undo by accident.
//!
//! **The canvas outranks the panels.** In the dark set the drawing surface is the darkest thing on screen
//! and every panel step is a step *up* in luminance. That is the whole hierarchy: no shadow, no border
//! weight, no drop-down chrome — a machinist's eye has to land on the geometry first, and the only budget
//! for that is value contrast. The test at the bottom pins the ladder for both sets.
//!
//! **There are exactly two signals.** [`Role::Sel`] (cyan) owns anything the pointer owns — selection,
//! hover, focus, the active tool. [`Role::Cut`] (amber) owns what the tool does to material — toolpaths,
//! depth, and the single primary action. Every other colour is a neutral or a verdict. A third accent hue
//! is not a matter of taste here: it would make "the tool will touch this" ambiguous with "you clicked
//! this", and that ambiguity costs a sheet of MDF.

use egui::{Color32, Context, Stroke, TextStyle, Visuals};

/// The smallest readable type size in the shell: units, status bar, help lines.
pub const CAPTION: f32 = 11.0;
/// Secondary rows, chips, tooltips.
pub const SMALL: f32 = 12.0;
/// Everything the operator reads as a sentence.
pub const BODY: f32 = 13.0;
/// Dock section and group titles.
pub const TITLE: f32 = 15.0;
/// Dialog titles.
pub const DIALOG: f32 = 20.0;
/// Empty states and the one line that greets an empty drawing. Used once per screen at most.
pub const HERO: f32 = 28.0;

/// WCAG AA for body text: 4.5:1.
pub const AA_BODY: f32 = 4.5;
/// WCAG AA for large text, and the floor we accept for a signal drawn as a stroke rather than as a word.
pub const AA_LARGE: f32 = 3.0;
/// The floor for a solid against the surface it stands on: 1.45:1.
///
/// The 3D stock is a filled block, not a word and not a hairline, so neither [`AA_BODY`] nor
/// [`AA_LARGE`] is the right yardstick: 3:1 against the canvas would make the material darker than the
/// cut signal it carries, and 1:1 is what the light set shipped before this was measured. 1.45:1 is the
/// largest step the light set can afford — the fill is capped by the darkest stop the artboard's
/// `#a86200` toolpath still reads on — and it is a step a shadow-lifting compositor cannot eat: measured
/// on the running app, an 8-code difference between `#ffffff` and `#f7f9fb` arrived as 2–5 codes, so a
/// 37-code difference survives as 10–20. Compare ratios with
/// [`contrast_ratio`]: `(lighter + 0.05) / (darker + 0.05)` on WCAG relative luminance.
pub const SOLID_ON_CANVAS: f32 = 1.45;

/// The box every glyph in [`crate::icons`] is authored in.
pub const ICON_SIZE: f32 = 24.0;
/// The one stroke weight the icon set is drawn at, in that box.
pub const ICON_STROKE: f32 = 1.6;

/// Which of the two sets is showing.
///
/// Not a boolean: the reference app offers a third answer (follow the system), and a `bool` would have to
/// grow a separate "auto" flag later. The set is named by what it *is*, and the preference is derived.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
  /// A dark shop, a bright screen: the default.
  Dark,
  /// Office, quoting, printing to paper.
  Light,
}

/// A named slot in the palette.
///
/// Widgets ask for a role, never for a colour: that is what lets the light set be a re-resolution of the
/// same design rather than a second design. The tests walk this enum's slots to prove contrast, so adding
/// a surface means adding its readable pairs in one place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
  /// Window frame and app bar.
  App,
  /// The drawing surface: the end of the ladder.
  Canvas,
  /// Rail, dock, status bar.
  Panel,
  /// Inputs and hovered rows.
  Raised,
  /// Menus, tooltips, popovers — the only surfaces allowed a shadow.
  Overlay,
  /// A one-pixel division.
  Hairline,
  /// The border of an input or a control.
  Border,
  /// Titles, values, labels.
  TextHi,
  /// Body and list rows.
  Text,
  /// Help, units, secondary information.
  TextDim,
  /// Disabled only. Never used for anything a person has to read.
  TextOff,
  /// The selection signal.
  Sel,
  /// The selection signal as a 13 % wash for fills behind it.
  SelSoft,
  /// The cut signal.
  Cut,
  /// The cut signal as a 13 % wash.
  CutSoft,
  /// Verdict: a program that verifies.
  Ok,
  /// Verdict: legal but worth a second look.
  Warn,
  /// Verdict: gouge, crash, or a refusal.
  Danger,
  /// The untouched top of the stock: the `0.0` end of the 3D depth ramp.
  StockTop,
  /// The stock's floor: the `1.0` end of the 3D depth ramp.
  StockBottom,
  /// The silhouette of a solid: the 3D stock box's twelve edges.
  ///
  /// Not [`Role::Border`], and the difference is a job, not a taste. A border is a one-pixel division
  /// *inside* one surface — a field's outline, a menu's frame — where 1.8:1 is right and 5:1 would draw
  /// a cage. The edge of a 3D block is the only thing that says "this object ends here" against a field
  /// with no grid, no shadow and no perspective cue, so it has to clear [`AA_LARGE`] against *both* the
  /// canvas behind it and the face it bounds. Only a value in a narrow band does both (see the ramp's
  /// values in `dark()` and `light()`), which is why this is a role and not a reuse.
  StockEdge,
}

impl Role {
  /// Every role, so an audit can walk the set instead of keeping a second list in step with it.
  ///
  /// The order is the palette's own: surfaces, text, the two signals, then the verdicts and the
  /// stock's ramp. A test that
  /// asserts "no widget paints a colour no role owns" needs the whole set, and a hand-written copy
  /// of it would grow a hole the day a role is added.
  pub const ALL: [Role; 21] = [
    Role::App,
    Role::Canvas,
    Role::Panel,
    Role::Raised,
    Role::Overlay,
    Role::Hairline,
    Role::Border,
    Role::TextHi,
    Role::Text,
    Role::TextDim,
    Role::TextOff,
    Role::Sel,
    Role::SelSoft,
    Role::Cut,
    Role::CutSoft,
    Role::Ok,
    Role::Warn,
    Role::Danger,
    Role::StockTop,
    Role::StockBottom,
    Role::StockEdge,
  ];
}

/// Every colour the shell may draw with.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
  app: Color32,
  canvas: Color32,
  panel: Color32,
  raised: Color32,
  overlay: Color32,
  hairline: Color32,
  border: Color32,
  text_hi: Color32,
  text: Color32,
  text_dim: Color32,
  text_off: Color32,
  sel: Color32,
  sel_soft: Color32,
  cut: Color32,
  cut_soft: Color32,
  ok: Color32,
  warn: Color32,
  danger: Color32,
  stock_top: Color32,
  stock_bottom: Color32,
  stock_edge: Color32,
}

impl Palette {
  /// Answers a [`Role`] with the colour it names in this set.
  pub fn get(&self, role: Role) -> Color32 {
    match role {
      Role::App => self.app,
      Role::Canvas => self.canvas,
      Role::Panel => self.panel,
      Role::Raised => self.raised,
      Role::Overlay => self.overlay,
      Role::Hairline => self.hairline,
      Role::Border => self.border,
      Role::TextHi => self.text_hi,
      Role::Text => self.text,
      Role::TextDim => self.text_dim,
      Role::TextOff => self.text_off,
      Role::Sel => self.sel,
      Role::SelSoft => self.sel_soft,
      Role::Cut => self.cut,
      Role::CutSoft => self.cut_soft,
      Role::Ok => self.ok,
      Role::Warn => self.warn,
      Role::Danger => self.danger,
      Role::StockTop => self.stock_top,
      Role::StockBottom => self.stock_bottom,
      Role::StockEdge => self.stock_edge,
    }
  }

  /// The colour a point `depth` into the stock is painted with: the 3D surface's shading.
  ///
  /// `depth` is the mesh's own ramp coordinate — `0.0` at the untouched top, `1.0` at the stock's
  /// floor — so the same palette says the same thing about an 18 mm sheet and a 6 mm one. Both ends
  /// are roles, and this is the only place in the shell a colour is *mixed* rather than chosen: a
  /// gradient whose ends live outside the palette is a colour the palette does not own, which is
  /// what [`Role::StockTop`] and [`Role::StockBottom`] exist to prevent. A value that is not a
  /// number reads as the top rather than as a colour a shader would have to defend against.
  pub fn stock_shade(&self, depth: f32) -> Color32 {
    let t = if depth.is_finite() {
      depth.clamp(0.0, 1.0)
    } else {
      0.0
    };
    mix(self.get(Role::StockTop), self.get(Role::StockBottom), t)
  }

  /// The default set.
  const fn dark() -> Self {
    Self {
      app: Color32::from_rgb(0x12, 0x15, 0x1a),
      canvas: Color32::from_rgb(0x0d, 0x10, 0x14),
      panel: Color32::from_rgb(0x17, 0x1b, 0x21),
      raised: Color32::from_rgb(0x1e, 0x24, 0x2b),
      overlay: Color32::from_rgb(0x23, 0x2a, 0x32),
      hairline: Color32::from_rgb(0x26, 0x2d, 0x36),
      border: Color32::from_rgb(0x35, 0x40, 0x4c),
      text_hi: Color32::from_rgb(0xe7, 0xec, 0xf3),
      text: Color32::from_rgb(0xc3, 0xcc, 0xd8),
      text_dim: Color32::from_rgb(0x8b, 0x95, 0xa3),
      // Deliberately below AA: disabled text is exempt, and a legible disabled control is a lie.
      text_off: Color32::from_rgb(0x5c, 0x66, 0x73),
      sel: Color32::from_rgb(0x6f, 0xd3, 0xe8),
      sel_soft: Color32::from_rgba_premultiplied(0x0e, 0x1b, 0x1e, 0x22),
      cut: Color32::from_rgb(0xf5, 0xa6, 0x23),
      cut_soft: Color32::from_rgba_premultiplied(0x20, 0x16, 0x04, 0x22),
      ok: Color32::from_rgb(0x5e, 0xc2, 0x7f),
      warn: Color32::from_rgb(0xf0, 0xb4, 0x29),
      danger: Color32::from_rgb(0xf2, 0x63, 0x5f),
      // The material, and how deep the tool has gone into it. The top is the set's brightest
      // neutral — the picture has to land on the stock before it lands on the chrome — and the
      // floor is a graphite that is still clearly above the canvas, so the deepest cut reads as
      // material rather than as a hole in the screen.
      stock_top: Color32::from_rgb(0xd6, 0xdc, 0xe6),
      stock_bottom: Color32::from_rgb(0x42, 0x4c, 0x58),
      // The band that clears both the canvas (4.42:1) and the stock's own top (3.13:1) at AA large
      // text: `#8b95a3`, the set's secondary text, is 6.29:1 off the canvas but only 2.20:1 against
      // the face it has to bound, and `#35404c`, the control border, is 1.81:1 off the canvas, which
      // is how T9 lost the block's lower edges in the first place.
      stock_edge: Color32::from_rgb(0x6e, 0x7b, 0x8a),
    }
  }

  /// The same design on paper. Signals darken until they pass AA on white instead of glowing.
  const fn light() -> Self {
    Self {
      app: Color32::from_rgb(0xee, 0xf1, 0xf5),
      canvas: Color32::from_rgb(0xf7, 0xf9, 0xfb),
      panel: Color32::from_rgb(0xff, 0xff, 0xff),
      raised: Color32::from_rgb(0xf2, 0xf5, 0xf8),
      overlay: Color32::from_rgb(0xff, 0xff, 0xff),
      hairline: Color32::from_rgb(0xdd, 0xe3, 0xea),
      border: Color32::from_rgb(0xc3, 0xcc, 0xd6),
      text_hi: Color32::from_rgb(0x12, 0x18, 0x1f),
      text: Color32::from_rgb(0x2b, 0x34, 0x40),
      text_dim: Color32::from_rgb(0x5f, 0x6b, 0x7a),
      text_off: Color32::from_rgb(0x8b, 0x95, 0xa3),
      sel: Color32::from_rgb(0x0d, 0x7c, 0x92),
      sel_soft: Color32::from_rgba_premultiplied(0x02, 0x0d, 0x0f, 0x18),
      cut: Color32::from_rgb(0xa8, 0x62, 0x00),
      cut_soft: Color32::from_rgba_premultiplied(0x11, 0x0a, 0x00, 0x14),
      ok: Color32::from_rgb(0x1f, 0x7a, 0x43),
      warn: Color32::from_rgb(0x96, 0x61, 0x0a),
      danger: Color32::from_rgb(0xb3, 0x34, 0x2f),
      // The same design on paper, and the one place the two sets are not a mirror. A dark set can put
      // a near-white solid on a near-black canvas; paper cannot, so the material has to be *darker*
      // than the page to read at all. `#c6d1dd` is as dark as the fill can go without losing the
      // artboard's `#a86200` toolpath on it (3.08:1, the stroke floor this file already holds every
      // signal to): it stands 1.47:1 off the canvas, a 37-code step that a compositor which turned an
      // 8-code step into 2–5 codes cannot flatten. The floor is a mid grey 1.40:1 under the top, so
      // depth still reads as value while the amber on the deepest stop measures 2.23:1 instead of the
      // 1.57:1 the old `#8b95a3` floor gave it.
      stock_top: Color32::from_rgb(0xc6, 0xd1, 0xdd),
      stock_bottom: Color32::from_rgb(0xa6, 0xb3, 0xc3),
      // 7.18:1 off the canvas and 4.90:1 against the stock's top — darker than the page and darker than
      // the face it bounds, the way a drawn silhouette is. Deliberately not `text_dim`'s `#5f6b7a` in
      // this set: that is the ink a rapid travels in, and two 1 px lines in the same picture must not be
      // the same colour, so this sits 1.40:1 below it.
      stock_edge: Color32::from_rgb(0x4a, 0x55, 0x63),
    }
  }
}

/// Interpolates between two ends on the sRGB curve, `t` in `0.0..=1.0`.
///
/// One shared function so the two ends and every value between them are painted by one rule: the
/// ramp is a line through the palette, not a second gradient somebody re-derived.
fn mix(top: Color32, bottom: Color32, t: f32) -> Color32 {
  let channel = |top: u8, bottom: u8| -> u8 {
    let top = f32::from(top);
    let bottom = f32::from(bottom);
    (top + (bottom - top) * t).round().clamp(0.0, 255.0) as u8
  };
  Color32::from_rgb(
    channel(top.r(), bottom.r()),
    channel(top.g(), bottom.g()),
    channel(top.b(), bottom.b()),
  )
}

/// The measurements, which are the same in both sets because a token that changes with the theme is not a
/// measurement.
#[derive(Clone, Copy, Debug)]
pub struct Metrics {
  /// The spacing scale. Nothing in the shell may use a gap that is not one of these.
  pub spacing: [f32; 8],
  /// Small controls: buttons, fields, chips.
  pub radius_control: f32,
  /// Docked surfaces and cards.
  pub radius_panel: f32,
  /// Floating windows only.
  pub radius_dialog: f32,
  /// Height of a field, a button, a chip.
  pub control_height: f32,
  /// Height of one row in the layer list or the inspector.
  pub row_height: f32,
  /// The app bar.
  pub app_bar_height: f32,
  /// The status bar.
  pub status_height: f32,
  /// The left palette: grouped, labelled columns of commands.
  pub palette_width: f32,
  /// The two ruler bands: the one along the canvas' top edge and the one down its left edge.
  ///
  /// The same number for both, because a ruler is one instrument drawn twice: unequal bands would read as
  /// two different scales. It is a measurement and not a spacing step because the numbers inside it are
  /// what sets its height, and it is not a control height because nothing in it is pressable.
  pub ruler_thickness: f32,
  /// The right inspector.
  pub dock_width: f32,
  /// One pixel of hairline.
  pub hairline: f32,
  /// How far an icon is scaled when it is painted.
  pub icon_size: f32,
  /// The stroke the icon set is drawn at, in the 24 × 24 authoring box.
  pub icon_stroke: f32,
}

impl Metrics {
  /// The one set of measurements the shell has.
  pub const fn shell() -> Self {
    Self {
      spacing: [2.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0],
      radius_control: 4.0,
      radius_panel: 6.0,
      radius_dialog: 10.0,
      control_height: 24.0,
      row_height: 26.0,
      app_bar_height: 40.0,
      status_height: 26.0,
      palette_width: 200.0,
      ruler_thickness: 22.0,
      dock_width: 288.0,
      hairline: 1.0,
      icon_size: ICON_SIZE,
      icon_stroke: ICON_STROKE,
    }
  }

  /// The `index`-th step of the spacing scale, clamped at both ends.
  pub fn space(&self, index: usize) -> f32 {
    self.spacing[index.min(self.spacing.len() - 1)]
  }
}

impl Theme {
  /// The colours of this set.
  pub fn palette(self) -> Palette {
    match self {
      Theme::Dark => Palette::dark(),
      Theme::Light => Palette::light(),
    }
  }

  /// The measurements of this set.
  pub fn metrics(self) -> Metrics {
    Metrics::shell()
  }

  /// The set colours from this theme as egui names it.
  pub fn egui(self) -> egui::Theme {
    match self {
      Theme::Dark => egui::Theme::Dark,
      Theme::Light => egui::Theme::Light,
    }
  }

  /// The set this theme asks egui for.
  pub fn preference(self) -> egui::ThemePreference {
    match self {
      Theme::Dark => egui::ThemePreference::Dark,
      Theme::Light => egui::ThemePreference::Light,
    }
  }

  /// egui's own widget styling, re-derived from the tokens.
  ///
  /// The shell draws its own frame — bars, rail, dock — but every stock widget (a `DragValue`, a `ComboBox`,
  /// a scroll bar, a menu) still paints from `Visuals`. Left alone those would be egui's defaults beside our
  /// tokens, which is exactly the "assembled from two apps" look the design is replacing. So the mapping
  /// lives here, once, and no widget sets a colour by hand.
  pub fn visuals(self) -> Visuals {
    let p = self.palette();
    let mut v = match self {
      Theme::Dark => Visuals::dark(),
      Theme::Light => Visuals::light(),
    };
    v.panel_fill = p.panel;
    v.window_fill = p.overlay;
    v.faint_bg_color = p.raised;
    v.extreme_bg_color = p.canvas;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.selection.bg_fill = p.sel_soft;
    v.selection.stroke = Stroke::new(1.0, p.sel);
    v.hyperlink_color = p.sel;
    v.warn_fg_color = p.warn;
    v.error_fg_color = p.danger;
    // The canvas must read as the deepest surface: no stock widget inside it may paint lighter.
    v.widgets.noninteractive.bg_fill = p.panel;
    v.widgets.noninteractive.weak_bg_fill = p.panel;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.hairline);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.inactive.bg_fill = p.raised;
    v.widgets.inactive.weak_bg_fill = p.raised;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text_hi);
    v.widgets.hovered.bg_fill = p.raised;
    v.widgets.hovered.weak_bg_fill = p.raised;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.sel);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text_hi);
    v.widgets.active.bg_fill = p.raised;
    v.widgets.active.weak_bg_fill = p.raised;
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.sel);
    v.widgets.active.fg_stroke = Stroke::new(1.0, p.text_hi);
    v.widgets.open.bg_fill = p.overlay;
    v.widgets.open.weak_bg_fill = p.overlay;
    v.widgets.open.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.open.fg_stroke = Stroke::new(1.0, p.text_hi);
    v
  }
}

/// The name of one of the sizes egui has no variant for.
///
/// Three of the six sizes are egui's own variants — [`TextStyle::Small`], [`TextStyle::Body`] and
/// [`TextStyle::Button`] — and those are *not* registered by name: a `TextStyle::Name("Body")` would sit
/// beside the real `TextStyle::Body` in the same map, both claiming to be the body size, while every stock
/// widget kept reading the other one. So the native variants are re-sized where egui looks for them, and only
/// the extras (`Caption`, `Title`, `Dialog`, `Hero`) are named.
pub fn style_name(name: &str) -> TextStyle {
  TextStyle::Name(name.into())
}

/// The caption style: units, status bar, help lines.
pub fn caption() -> TextStyle {
  style_name("Caption")
}

/// The title style: dock sections and group headers.
pub fn title() -> TextStyle {
  style_name("Title")
}

/// The dialog title style.
pub fn dialog() -> TextStyle {
  style_name("Dialog")
}

/// The empty-state style.
pub fn hero() -> TextStyle {
  style_name("Hero")
}

/// Publishes the tokens into an egui context: the two sets' `Visuals`, and the six type sizes.
///
/// Called once at startup, and again if the operator flips the theme. It deliberately does not touch
/// spacing or control sizes: those are read from [`Metrics`] by the code that lays out, because forcing them
/// through egui's `Spacing` would make every stock widget a little different from the design and impossible
/// to audit.
pub fn apply(ctx: &Context, theme: Theme) {
  ctx.set_theme(theme.preference());
  ctx.set_visuals_of(egui::Theme::Dark, Theme::Dark.visuals());
  ctx.set_visuals_of(egui::Theme::Light, Theme::Light.visuals());
  ctx.all_styles_mut(|style| {
    // egui's own variants, in the place every stock widget reads them.
    style
      .text_styles
      .insert(TextStyle::Small, egui::FontId::proportional(SMALL));
    style
      .text_styles
      .insert(TextStyle::Body, egui::FontId::proportional(BODY));
    style
      .text_styles
      .insert(TextStyle::Button, egui::FontId::proportional(BODY));
    style
      .text_styles
      .insert(TextStyle::Monospace, egui::FontId::monospace(SMALL));
    // The sizes egui has no variant for.
    for (name, size) in [
      ("Caption", CAPTION),
      ("Title", TITLE),
      ("Dialog", DIALOG),
      ("Hero", HERO),
    ] {
      style
        .text_styles
        .insert(style_name(name), egui::FontId::proportional(size));
    }
  });
}

/// WCAG relative luminance of a colour, on the sRGB curve.
pub fn luminance(c: Color32) -> f32 {
  fn channel(value: u8) -> f32 {
    let v = f32::from(value) / 255.0;
    if v <= 0.039_28 {
      v / 12.92
    } else {
      ((v + 0.055) / 1.055).powf(2.4)
    }
  }
  0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}

/// WCAG contrast ratio between a foreground and a background, from 1:1 to 21:1.
///
/// Public because the design system sync and any future audit should agree with the tests by construction,
/// rather than re-deriving the formula (and the sRGB curve) a second time.
pub fn contrast_ratio(foreground: Color32, background: Color32) -> f32 {
  let (a, b) = (luminance(foreground), luminance(background));
  let (lighter, darker) = if a > b { (a, b) } else { (b, a) };
  (lighter + 0.05) / (darker + 0.05)
}

/// How far apart two colours sit on the colour wheel, in degrees, on the shorter arc (0…180).
pub fn hue_degrees(a: Color32, b: Color32) -> f32 {
  let difference = (hue(a) - hue(b)).abs();
  difference.min(360.0 - difference)
}

/// The hue of a colour in degrees, 0…360.
fn hue(c: Color32) -> f32 {
  let (r, g, b) = (
    f32::from(c.r()) / 255.0,
    f32::from(c.g()) / 255.0,
    f32::from(c.b()) / 255.0,
  );
  let max = r.max(g).max(b);
  let min = r.min(g).min(b);
  let span = max - min;
  if span <= f32::EPSILON {
    return 0.0;
  }
  let h = if (max - r).abs() < f32::EPSILON {
    60.0 * (((g - b) / span) % 6.0)
  } else if (max - g).abs() < f32::EPSILON {
    60.0 * ((b - r) / span + 2.0)
  } else {
    60.0 * ((r - g) / span + 4.0)
  };
  if h < 0.0 {
    h + 360.0
  } else {
    h
  }
}

#[cfg(test)]
mod tests {
  use super::{
    contrast_ratio, hue_degrees, luminance, Role, Theme, AA_BODY, AA_LARGE, SOLID_ON_CANVAS,
  };

  /// Every pair of (foreground, background) roles the shell actually paints words on.
  ///
  /// The list is deliberately explicit rather than derived: a new surface has to be added here by hand,
  /// which is the point — the moment someone introduces a second panel fill, the missing pair is a failing
  /// test instead of a screen nobody can read. [`Role::TextOff`] is absent on purpose: it is disabled-only,
  /// and WCAG exempts inactive controls.
  fn readable_pairs() -> [(&'static str, Role, Role, f32); 12] {
    [
      ("text_hi on app", Role::TextHi, Role::App, AA_BODY),
      ("text_hi on panel", Role::TextHi, Role::Panel, AA_BODY),
      ("text on panel", Role::Text, Role::Panel, AA_BODY),
      ("text on canvas", Role::Text, Role::Canvas, AA_BODY),
      ("text_dim on app", Role::TextDim, Role::App, AA_BODY),
      ("text_dim on panel", Role::TextDim, Role::Panel, AA_BODY),
      // The signals are drawn as strokes, chips, and 11 px status text, on the panel or on the canvas. AA
      // large-text (3:1) is the honest floor for a 1.6 px stroke, which is not text at all.
      ("selection on panel", Role::Sel, Role::Panel, AA_LARGE),
      ("selection on canvas", Role::Sel, Role::Canvas, AA_LARGE),
      ("cut on panel", Role::Cut, Role::Panel, AA_LARGE),
      ("cut on canvas", Role::Cut, Role::Canvas, AA_LARGE),
      ("ok on panel", Role::Ok, Role::Panel, AA_LARGE),
      ("warn on panel", Role::Warn, Role::Panel, AA_LARGE),
    ]
  }

  #[test]
  fn every_pair_the_shell_writes_on_meets_aa() {
    for (label, foreground, background, minimum) in readable_pairs() {
      for theme in [Theme::Dark, Theme::Light] {
        let palette = theme.palette();
        let ratio = contrast_ratio(palette.get(foreground), palette.get(background));
        assert!(
          ratio >= minimum,
          "{theme:?}: {label} is {ratio:.2}:1, below the {minimum:.1}:1 the design requires"
        );
      }
    }
  }

  #[test]
  fn the_canvas_sits_at_the_end_of_the_luminance_ladder() {
    let dark = Theme::Dark.palette();
    let ladder = [
      ("canvas", dark.get(Role::Canvas)),
      ("app", dark.get(Role::App)),
      ("panel", dark.get(Role::Panel)),
      ("raised", dark.get(Role::Raised)),
      ("overlay", dark.get(Role::Overlay)),
    ];
    for pair in ladder.windows(2) {
      let (lower, upper) = (pair[0], pair[1]);
      assert!(
        luminance(lower.1) < luminance(upper.1),
        "dark: {} must be darker than {} — the canvas leads and every floating surface follows",
        lower.0,
        upper.0
      );
    }

    let light = Theme::Light.palette();
    let ladder = [
      ("app", light.get(Role::App)),
      ("raised", light.get(Role::Raised)),
      ("canvas", light.get(Role::Canvas)),
      ("panel", light.get(Role::Panel)),
    ];
    for pair in ladder.windows(2) {
      let (lower, upper) = (pair[0], pair[1]);
      assert!(
        luminance(lower.1) < luminance(upper.1),
        "light: {} must be darker than {}",
        lower.0,
        upper.0
      );
    }
  }

  #[test]
  fn the_two_signals_never_share_a_hue() {
    for theme in [Theme::Dark, Theme::Light] {
      let palette = theme.palette();
      let apart = hue_degrees(palette.get(Role::Sel), palette.get(Role::Cut));
      assert!(
        apart >= 60.0,
        "{theme:?}: selection and cut are only {apart:.0}° apart; a colour-blind operator reads one signal"
      );
    }
  }

  #[test]
  fn the_stock_ramp_darkens_with_depth_in_both_sets() {
    // The 3D surface says "this is how deep the tool went" with value alone, so the ramp has to be a
    // ramp in both sets: two distinct ends, and the floor darker than the top. If this drifts, the
    // picture still draws and stops meaning anything.
    for theme in [Theme::Dark, Theme::Light] {
      let palette = theme.palette();
      let (top, bottom) = (Role::StockTop, Role::StockBottom);
      assert_ne!(
        palette.get(top),
        palette.get(bottom),
        "{theme:?}: the stock's two ends are the same colour, so depth is invisible"
      );
      assert!(
        luminance(palette.get(bottom)) < luminance(palette.get(top)),
        "{theme:?}: the stock's floor must be darker than its top, or the ramp reads upside down"
      );
      // The material also has to be distinguishable from the surface it sits on: a stock the same
      // colour as the canvas is a stock nobody can see.
      assert_ne!(
        palette.get(bottom),
        palette.get(Role::Canvas),
        "{theme:?}: the deepest cut is the canvas's own colour"
      );
    }
  }

  #[test]
  fn the_stock_reads_as_a_solid_against_the_canvas() {
    // A solid has to be distinguishable from the surface it stands on before any of its shading means
    // anything, and the fill is the only thing that says so over the whole silhouette: an edge can
    // outline a block, but it cannot make a white slab look loaded. This is the assertion the
    // screenshot asked for — the light set shipped `#ffffff` on a `#f7f9fb` canvas, which is 1.06:1 and
    // arrived on screen as a 2-to-5 code difference, so the stock disappeared into the page.
    for theme in [Theme::Dark, Theme::Light] {
      let palette = theme.palette();
      // Every stop on the ramp is material, so every stop is held to the same margin: the top of the
      // sheet and the floor of a pocket both have to stand off the canvas, or the deepest cut reads as a
      // hole in the screen rather than as a place the tool has been.
      for (label, role) in [("top", Role::StockTop), ("floor", Role::StockBottom)] {
        let ratio = contrast_ratio(palette.get(role), palette.get(Role::Canvas));
        assert!(
          ratio >= SOLID_ON_CANVAS,
          "{theme:?}: the stock's {label} is {ratio:.2}:1 against the canvas, under the \
           {SOLID_ON_CANVAS:.2}:1 a solid needs to read as a solid"
        );
      }
      // The silhouette is what carries the block when the fill is a page-light grey, so it is held to
      // the stroke floor against the field behind it *and* against the face it bounds: an edge that
      // merges with either one is a hairline rectangle again.
      for (label, other) in [
        ("canvas", Role::Canvas),
        ("the stock's top", Role::StockTop),
      ] {
        let ratio = contrast_ratio(palette.get(Role::StockEdge), palette.get(other));
        assert!(
          ratio >= AA_LARGE,
          "{theme:?}: the stock's edge is {ratio:.2}:1 against {label}, under the {AA_LARGE:.1}:1 a \
           silhouette needs"
        );
      }
    }
  }

  #[test]
  fn the_stock_shade_returns_its_ends_and_interpolates_between_them() {
    for theme in [Theme::Dark, Theme::Light] {
      let palette = theme.palette();
      let (top, bottom) = (Role::StockTop, Role::StockBottom);
      assert_eq!(
        palette.stock_shade(0.0),
        palette.get(top),
        "{theme:?}: depth 0.0 is the untouched top"
      );
      assert_eq!(
        palette.stock_shade(1.0),
        palette.get(bottom),
        "{theme:?}: depth 1.0 is the stock's floor"
      );
      let middle = palette.stock_shade(0.5);
      assert!(
        luminance(palette.get(bottom)) < luminance(middle)
          && luminance(middle) < luminance(palette.get(top)),
        "{theme:?}: half depth must sit strictly between the two ends, not on one of them"
      );
      // Out-of-range and non-finite depths clamp where the painter clamps them: the ends, never a
      // colour nobody specified.
      assert_eq!(palette.stock_shade(-1.0), palette.get(top), "{theme:?}");
      assert_eq!(palette.stock_shade(2.0), palette.get(bottom), "{theme:?}");
      assert_eq!(
        palette.stock_shade(f32::NAN),
        palette.get(top),
        "{theme:?}: a depth that is not a number reads as the top"
      );
    }
  }

  #[test]
  fn the_metrics_are_ordered_and_land_on_the_grid() {
    let metrics = Theme::Dark.metrics();
    let mut previous = 0.0;
    for (index, value) in metrics.spacing.iter().enumerate() {
      assert!(
        *value > previous,
        "spacing[{index}] = {value} does not grow"
      );
      assert_eq!(
        (*value as i32) % 2,
        0,
        "spacing[{index}] = {value} leaves the 2 px grid"
      );
      previous = *value;
    }
    assert!(
      metrics.control_height <= metrics.row_height,
      "a control may not be taller than a row"
    );
    assert!(
      metrics.radius_control < metrics.radius_panel,
      "panels are rounder than controls"
    );
    assert!(
      metrics.icon_stroke * metrics.icon_size / super::ICON_SIZE >= 1.0,
      "an icon stroke thinner than one physical pixel disappears on a 100 % display"
    );
  }

  #[test]
  fn apply_registers_every_named_text_style() {
    let ctx = egui::Context::default();
    super::apply(&ctx, Theme::Dark);
    let style = ctx.style_of(egui::Theme::Dark);
    // The variants egui itself hands to stock widgets. If one of these is wrong, every label in the shell is
    // the wrong size and nothing else would notice.
    for (name, text_style, size) in [
      ("Small", egui::TextStyle::Small, super::SMALL),
      ("Body", egui::TextStyle::Body, super::BODY),
      ("Button", egui::TextStyle::Button, super::BODY),
      ("Monospace", egui::TextStyle::Monospace, super::SMALL),
    ] {
      let font = style
        .text_styles
        .get(&text_style)
        .unwrap_or_else(|| panic!("TextStyle::{name} is not registered"));
      assert_eq!(font.size, size, "TextStyle::{name} has the wrong size");
    }
    // The sizes egui has no variant for.
    for (name, text_style, size) in [
      ("Caption", super::caption(), super::CAPTION),
      ("Title", super::title(), super::TITLE),
      ("Dialog", super::dialog(), super::DIALOG),
      ("Hero", super::hero(), super::HERO),
    ] {
      let font = style
        .text_styles
        .get(&text_style)
        .unwrap_or_else(|| panic!("{name} is not registered"));
      assert_eq!(font.size, size, "{name} has the wrong size");
    }
    assert_eq!(
      style.visuals.panel_fill,
      Theme::Dark.palette().get(Role::Panel),
      "the panel fill did not come from the tokens"
    );
  }
}
