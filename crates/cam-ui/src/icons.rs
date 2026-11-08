//! The shell's icon set: seventy-one glyphs, drawn with egui primitives instead of rasterised.
//!
//! `design/camui-shell/03-iconography.html` is the authority for the drawings; this file is their
//! transliteration, one [`Cmd`] per SVG command, so a glyph can be read against the artboard line by
//! line and a test can walk the geometry without a renderer.
//!
//! **One hand, one weight.** Every glyph is authored in the same 24 × 24 box at the same 1.6 stroke,
//! and the language has exactly two words beyond the plain line: a dashed subpath is *virtual* (a
//! mirror axis, a pitch circle) and a 22 % wash is *kept* (the region an operation leaves behind).
//! There is no second weight and no filled variant beside the line set: twenty icons in a rail have
//! to read as one hand, and a "bold" glyph would read as a different one.
//!
//! **Why geometry and not a texture.** The artboard's own note suggests rasterising through
//! `resvg`/`usvg`; campilot will not grow that dependency (S6), so the drawing is flattened here and
//! stroked by egui. Flattening in box units at paint time also makes the set resolution-free: the
//! rail asks for 18 px, a dialog for 32, and neither is a scaled-down copy of the other.
//!
//! **Colour is an input, never a constant.** Everything is derived from the `tint` a caller passes —
//! the artboard's `currentColor` — so the theme owns the hue and this file never learns whether it
//! is painting the dark or the light set.

use egui::{Color32, Pos2, Rect, Shape, Stroke};

/// The box every glyph is authored in, taken from the tokens so the set and the layout cannot
/// disagree about what "24" means.
const BOX_UNITS: f32 = crate::theme::ICON_SIZE;

/// How far a flattened curve may stray from the curve it approximates, in box units.
///
/// 0.05 is 0.2 % of the box: a tenth of a pixel even in a 32 px dialog, and fine enough that the
/// largest arcs in the set cost about eleven segments a quarter.
const TOLERANCE: f32 = 0.05;

/// The artboard's `stroke-dasharray="2 2"`, in box units. Scaled with everything else.
const DASH: f32 = 2.0;

/// The "kept area" wash, as the artboard writes `fill-opacity=".22"`.
const WASH: f32 = 0.22;

/// Recursion floor for cubic flattening: 2^10 segments is far past what [`TOLERANCE`] asks for, so
/// hitting it means the control points are degenerate, not that the curve needs more room.
const MAX_CURVE_DEPTH: u32 = 10;

/// One command of one glyph element, in box units, with every coordinate already absolute.
///
/// The artboard writes SVG — `M`, `L`, `C`, `A`, `Z` and a handful of relative commands — and this is
/// that vocabulary with the relative offsets resolved. Nothing is parsed at run time, so no glyph can
/// fail to load and no test needs to cover a parser error.
#[derive(Clone, Copy, Debug)]
enum Cmd {
  /// Start a subpath.
  Move(f32, f32),
  /// Straight segment from the current point.
  Line(f32, f32),
  /// Cubic Bézier: two control points, then the endpoint.
  Curve {
    /// Control point leaving the current point.
    c1: (f32, f32),
    /// Control point arriving at the endpoint.
    c2: (f32, f32),
    /// Endpoint.
    to: (f32, f32),
  },
  /// Elliptical arc in endpoint parameterisation.
  ///
  /// The artboard's arcs are all unrotated, so the x-axis rotation is not carried: a rotation here
  /// would be a fifth flag nobody drew. `large` and `sweep` are the SVG flags as drawn.
  Arc {
    /// Radius along X, before the endpoint-to-centre correction.
    rx: f32,
    /// Radius along Y.
    ry: f32,
    /// `large-arc-flag`: take the long way round.
    large: bool,
    /// `sweep-flag`: run clockwise in the artboard's y-down frame.
    sweep: bool,
    /// Endpoint.
    to: (f32, f32),
  },
  /// A whole `<circle>` or `<ellipse>`: opens and closes its own subpath.
  Ellipse {
    /// Centre.
    c: (f32, f32),
    /// Radius along X.
    rx: f32,
    /// Radius along Y.
    ry: f32,
  },
  /// A whole `<rect>` with a corner radius, as `<rect x y width height rx>`.
  Rect {
    /// Top-left corner.
    min: (f32, f32),
    /// Width and height.
    size: (f32, f32),
    /// Corner radius; `0.0` means sharp.
    r: f32,
  },
  /// Close the current subpath back to its first point.
  Close,
}

/// The artboard's `M`, with a name short enough to keep the table readable.
const fn at(x: f32, y: f32) -> Cmd {
  Cmd::Move(x, y)
}

/// The artboard's `L`.
const fn to(x: f32, y: f32) -> Cmd {
  Cmd::Line(x, y)
}

/// The artboard's `C`, offsets resolved.
const fn curve(c1: (f32, f32), c2: (f32, f32), to: (f32, f32)) -> Cmd {
  Cmd::Curve { c1, c2, to }
}

/// The artboard's `A`, offsets resolved.
const fn arc(rx: f32, ry: f32, large: bool, sweep: bool, to: (f32, f32)) -> Cmd {
  Cmd::Arc {
    rx,
    ry,
    large,
    sweep,
    to,
  }
}

/// A whole `<ellipse>`; a `<circle>` passes the same radius twice.
const fn ellipse(c: (f32, f32), rx: f32, ry: f32) -> Cmd {
  Cmd::Ellipse { c, rx, ry }
}

/// A whole `<rect>`.
const fn rect(min: (f32, f32), size: (f32, f32), r: f32) -> Cmd {
  Cmd::Rect { min, size, r }
}

/// The artboard's `Z`.
const fn close() -> Cmd {
  Cmd::Close
}

/// One SVG element of a glyph, with the style its attributes gave it.
///
/// An element is kept whole rather than split into shapes because that is how the artboard is
/// written: one `opacity`, one dash array, one element. Splitting would let two elements with
/// different fades merge, which is exactly the distinction the set uses to say "this one is virtual".
struct Part {
  /// The element's geometry, in box units.
  cmds: &'static [Cmd],
  /// The element's `opacity`, applied to the tint. `1.0` means the tint as given.
  fade: f32,
  /// Whether the element carries `stroke-dasharray="2 2"`.
  dashed: bool,
  /// Whether the element is the 22 % wash rather than a stroke. The artboard never draws both on one
  /// element: a fill is a region, a stroke is a line, and mixing them would blur "kept" with "cut".
  fill: bool,
}

/// A plain stroke at the tint.
const fn stroke(cmds: &'static [Cmd]) -> Part {
  Part {
    cmds,
    fade: 1.0,
    dashed: false,
    fill: false,
  }
}

/// A stroke at part of the tint — the artboard's `opacity`, used for the ghost of a second state.
const fn faded(cmds: &'static [Cmd], fade: f32) -> Part {
  Part {
    cmds,
    fade,
    dashed: false,
    fill: false,
  }
}

/// A dashed stroke at the tint: a mirror axis, a pitch circle, a construction line.
const fn dashed(cmds: &'static [Cmd]) -> Part {
  Part {
    cmds,
    fade: 1.0,
    dashed: true,
    fill: false,
  }
}

/// A dashed stroke at part of the tint, which the artboard uses once, for the ellipse's axis.
const fn dashed_faded(cmds: &'static [Cmd], fade: f32) -> Part {
  Part {
    cmds,
    fade,
    dashed: true,
    fill: false,
  }
}

/// The "this region is kept" wash, always at [`WASH`].
const fn wash(cmds: &'static [Cmd]) -> Part {
  Part {
    cmds,
    fade: WASH,
    dashed: false,
    fill: true,
  }
}

/// One glyph: the name the shell shows and the elements that draw it.
struct Glyph {
  /// The artboard's kebab-case name, which is also the name a tooltip or a test looks up.
  name: &'static str,
  /// The elements, in the artboard's order.
  parts: &'static [Part],
}

const NEW: &[Part] = &[
  stroke(&[
    at(6.0, 3.0),
    to(14.0, 3.0),
    to(18.0, 7.0),
    to(18.0, 21.0),
    to(6.0, 21.0),
    close(),
  ]),
  stroke(&[at(14.0, 3.0), to(14.0, 7.0), to(18.0, 7.0)]),
  stroke(&[
    at(12.0, 11.0),
    to(12.0, 17.0),
    at(9.0, 14.0),
    to(15.0, 14.0),
  ]),
];

const OPEN: &[Part] = &[
  stroke(&[
    at(3.0, 8.0),
    to(3.0, 5.5),
    to(8.5, 5.5),
    to(10.5, 8.0),
    to(17.0, 8.0),
    to(17.0, 11.0),
  ]),
  stroke(&[
    at(3.0, 8.0),
    to(20.5, 8.0),
    to(18.5, 19.0),
    to(5.0, 19.0),
    close(),
  ]),
];

const SAVE: &[Part] = &[
  stroke(&[
    at(4.5, 3.0),
    to(15.5, 3.0),
    to(19.5, 7.0),
    to(19.5, 21.0),
    to(4.5, 21.0),
    close(),
  ]),
  stroke(&[at(8.0, 3.0), to(8.0, 8.5), to(15.5, 8.5), to(15.5, 3.0)]),
  stroke(&[at(8.0, 21.0), to(8.0, 15.0), to(16.0, 15.0), to(16.0, 21.0)]),
];

const SAVE_AS: &[Part] = &[
  stroke(&[at(4.5, 3.0), to(13.5, 3.0), to(17.0, 6.5), to(17.0, 11.0)]),
  stroke(&[at(4.5, 3.0), to(4.5, 21.0), to(10.5, 21.0)]),
  stroke(&[at(8.0, 3.0), to(8.0, 8.0), to(14.0, 8.0)]),
  stroke(&[
    at(13.5, 21.0),
    to(20.0, 14.5),
    to(18.0, 12.5),
    to(11.5, 19.0),
    to(11.5, 21.0),
    close(),
  ]),
];

const IMPORT_VECTORS: &[Part] = &[
  stroke(&[at(12.0, 3.0), to(12.0, 13.0)]),
  stroke(&[at(8.0, 9.5), to(12.0, 13.5), to(16.0, 9.5)]),
  stroke(&[at(4.0, 16.0), to(4.0, 21.0), to(20.0, 21.0), to(20.0, 16.0)]),
];

const IMPORT_IMAGE: &[Part] = &[
  stroke(&[rect((3.0, 4.0), (14.0, 10.0), 1.0)]),
  stroke(&[
    at(3.0, 11.0),
    to(6.5, 7.5),
    to(10.0, 11.0),
    to(12.0, 9.0),
    to(15.0, 12.0),
  ]),
  // Artboard: point (22.8, 18.2) moved to (22.0, 18.2), outside the 2 unit safe area the same artboard states.
  stroke(&[
    at(20.0, 15.0),
    to(20.0, 21.0),
    at(17.2, 18.2),
    to(20.0, 21.0),
    to(22.0, 18.2),
  ]),
];

const EXPORT_DXF: &[Part] = &[
  stroke(&[at(12.0, 14.0), to(12.0, 4.0)]),
  stroke(&[at(8.5, 7.5), to(12.0, 4.0), to(15.5, 7.5)]),
  stroke(&[rect((3.5, 16.0), (17.0, 5.0), 1.0)]),
  stroke(&[
    at(7.0, 18.5),
    to(11.0, 18.5),
    to(13.0, 16.5),
    to(15.0, 18.5),
    to(17.0, 18.5),
  ]),
];

const EXPORT_SVG: &[Part] = &[
  stroke(&[at(12.0, 14.0), to(12.0, 4.0)]),
  stroke(&[at(8.5, 7.5), to(12.0, 4.0), to(15.5, 7.5)]),
  stroke(&[rect((3.5, 16.0), (17.0, 5.0), 1.0)]),
  stroke(&[
    at(7.0, 18.5),
    curve((8.5, 16.5), (9.5, 16.5), (11.0, 18.5)),
    curve((12.5, 20.5), (13.5, 20.5), (15.0, 18.5)),
  ]),
];

const STOCK_SHEET: &[Part] = &[
  stroke(&[
    at(3.0, 9.5),
    to(12.0, 5.0),
    to(21.0, 9.5),
    to(12.0, 14.0),
    close(),
  ]),
  stroke(&[
    at(3.0, 9.5),
    to(3.0, 14.5),
    to(12.0, 19.0),
    to(21.0, 14.5),
    to(21.0, 9.5),
  ]),
];

const STOCK_ROTARY: &[Part] = &[
  stroke(&[
    at(4.0, 7.5),
    to(4.0, 16.5),
    curve((4.0, 17.9), (7.6, 19.0), (12.0, 19.0)),
    curve((16.4, 19.0), (20.0, 17.9), (20.0, 16.5)),
    to(20.0, 7.5),
  ]),
  stroke(&[ellipse((12.0, 7.5), 8.0, 2.5)]),
];

const TOOL_LIBRARY: &[Part] = &[
  stroke(&[
    at(10.0, 3.0),
    to(14.0, 3.0),
    to(14.0, 8.5),
    to(13.0, 10.5),
    to(13.0, 16.5),
    to(12.0, 18.5),
    to(11.0, 16.5),
    to(11.0, 10.5),
    to(10.0, 8.5),
    close(),
  ]),
  stroke(&[at(11.0, 10.5), to(11.0, 13.5)]),
];

const SELECT: &[Part] = &[stroke(&[
  at(5.0, 3.0),
  to(11.4, 19.6),
  to(13.7, 12.8),
  to(20.5, 10.4),
  close(),
])];

const NODE_EDIT: &[Part] = &[
  stroke(&[
    at(3.5, 18.0),
    curve((6.5, 9.0), (9.5, 20.0), (12.5, 15.0)),
    curve((15.5, 10.0), (17.5, 9.0), (20.5, 7.0)),
  ]),
  stroke(&[rect((2.0, 16.5), (3.0, 3.0), 0.0)]),
  stroke(&[rect((10.5, 12.0), (3.0, 3.0), 0.0)]),
  stroke(&[rect((19.0, 4.5), (3.0, 3.0), 0.0)]),
];

const LINE: &[Part] = &[
  stroke(&[at(4.0, 19.0), to(20.0, 6.0)]),
  stroke(&[ellipse((4.0, 19.0), 1.3, 1.3)]),
  stroke(&[ellipse((20.0, 6.0), 1.3, 1.3)]),
];

const POLYLINE: &[Part] = &[
  stroke(&[at(3.0, 18.0), to(7.5, 10.0), to(12.0, 15.5), to(20.0, 6.0)]),
  // Artboard: rectangle corner (1.8, 16.8) moved to (2.0, 16.8), outside the 2 unit safe area the same artboard states.
  stroke(&[rect((2.0, 16.8), (2.4, 2.4), 0.0)]),
  stroke(&[rect((6.3, 8.8), (2.4, 2.4), 0.0)]),
  stroke(&[rect((10.8, 14.3), (2.4, 2.4), 0.0)]),
  stroke(&[rect((18.8, 4.8), (2.4, 2.4), 0.0)]),
];

const ARC: &[Part] = &[
  stroke(&[at(4.0, 18.0), arc(11.0, 11.0, false, true, (19.5, 11.5))]),
  stroke(&[ellipse((4.0, 18.0), 1.3, 1.3)]),
  stroke(&[ellipse((19.5, 11.5), 1.3, 1.3)]),
];

const CIRCLE: &[Part] = &[
  stroke(&[ellipse((12.0, 12.0), 8.0, 8.0)]),
  dashed(&[at(12.0, 12.0), to(20.0, 12.0)]),
  stroke(&[ellipse((12.0, 12.0), 1.3, 1.3)]),
];

const ELLIPSE: &[Part] = &[
  stroke(&[ellipse((12.0, 12.0), 9.0, 5.5)]),
  dashed_faded(&[at(3.0, 12.0), to(21.0, 12.0)], 0.5),
];

const RECTANGLE: &[Part] = &[stroke(&[rect((4.0, 6.5), (16.0, 11.0), 1.0)])];

const POLYGON: &[Part] = &[stroke(&[
  at(12.0, 3.5),
  to(19.4, 7.8),
  to(19.4, 16.2),
  to(12.0, 20.5),
  to(4.6, 16.2),
  to(4.6, 7.8),
  close(),
])];

const STAR: &[Part] = &[stroke(&[
  at(12.0, 3.5),
  to(14.6, 9.1),
  to(20.7, 9.9),
  to(16.3, 14.1),
  to(17.4, 20.1),
  to(12.0, 17.1),
  to(6.6, 20.1),
  to(7.7, 14.1),
  to(3.3, 9.9),
  to(9.4, 9.1),
  close(),
])];

const TEXT: &[Part] = &[
  stroke(&[at(5.0, 6.5), to(5.0, 4.5), to(19.0, 4.5), to(19.0, 6.5)]),
  stroke(&[at(12.0, 4.5), to(12.0, 19.5)]),
  stroke(&[at(9.0, 19.5), to(15.0, 19.5)]),
];

const DIMENSION: &[Part] = &[
  stroke(&[at(4.0, 6.0), to(4.0, 15.0), at(20.0, 6.0), to(20.0, 15.0)]),
  stroke(&[at(4.0, 10.0), to(20.0, 10.0)]),
  stroke(&[
    at(6.5, 8.2),
    to(4.0, 10.0),
    to(6.5, 11.8),
    at(17.5, 8.2),
    to(20.0, 10.0),
    to(17.5, 11.8),
  ]),
];

const TRIM: &[Part] = &[
  stroke(&[at(3.0, 12.0), to(9.5, 12.0), at(14.5, 12.0), to(21.0, 12.0)]),
  stroke(&[at(11.2, 8.5), to(11.2, 15.5), at(12.8, 8.5), to(12.8, 15.5)]),
];

const EXTEND: &[Part] = &[
  stroke(&[at(3.0, 12.0), to(11.0, 12.0)]),
  dashed(&[at(11.0, 12.0), to(17.0, 12.0)]),
  stroke(&[at(20.0, 4.0), to(20.0, 20.0)]),
  stroke(&[at(14.0, 9.0), to(17.0, 12.0), to(14.0, 15.0)]),
];

const FILLET: &[Part] = &[
  stroke(&[
    at(4.5, 20.0),
    to(4.5, 11.0),
    arc(6.0, 6.0, false, true, (10.5, 5.0)),
    to(20.0, 5.0),
  ]),
  faded(
    &[at(4.5, 5.5), to(7.5, 5.5), at(4.5, 5.5), to(4.5, 8.5)],
    0.45,
  ),
];

const CHAMFER: &[Part] = &[
  stroke(&[at(4.5, 20.0), to(4.5, 11.5), to(11.0, 5.0), to(20.0, 5.0)]),
  faded(
    &[at(4.5, 5.0), to(7.5, 5.0), at(4.5, 5.0), to(4.5, 8.0)],
    0.45,
  ),
];

const BREAK: &[Part] = &[
  stroke(&[at(3.0, 12.0), to(9.0, 12.0), at(15.0, 12.0), to(21.0, 12.0)]),
  stroke(&[ellipse((9.6, 12.0), 1.2, 1.2)]),
  stroke(&[ellipse((14.4, 12.0), 1.2, 1.2)]),
];

const OFFSET: &[Part] = &[
  stroke(&[rect((3.0, 3.0), (18.0, 18.0), 3.0)]),
  stroke(&[rect((7.5, 7.5), (9.0, 9.0), 1.0)]),
];

const UNION: &[Part] = &[
  stroke(&[ellipse((9.0, 12.0), 5.5, 5.5)]),
  stroke(&[ellipse((15.0, 12.0), 5.5, 5.5)]),
  wash(&[
    at(12.0, 7.3),
    arc(5.0, 5.0, false, true, (12.0, 16.7)),
    arc(5.0, 5.0, false, true, (12.0, 7.3)),
    close(),
  ]),
];

const SUBTRACT: &[Part] = &[
  stroke(&[ellipse((11.0, 12.0), 7.0, 7.0)]),
  dashed(&[ellipse((16.0, 8.5), 4.5, 4.5)]),
];

const INTERSECT: &[Part] = &[
  dashed(&[ellipse((9.0, 12.0), 6.0, 6.0)]),
  dashed(&[ellipse((15.0, 12.0), 6.0, 6.0)]),
  wash(&[
    at(12.0, 7.2),
    arc(6.0, 6.0, false, true, (12.0, 16.8)),
    arc(6.0, 6.0, false, true, (12.0, 7.2)),
    close(),
  ]),
];

const JOIN: &[Part] = &[
  stroke(&[at(3.0, 17.0), to(9.5, 10.0)]),
  dashed(&[at(14.5, 10.0), to(21.0, 4.0)]),
  stroke(&[ellipse((9.5, 10.0), 1.6, 1.6)]),
  stroke(&[ellipse((14.5, 10.0), 1.6, 1.6)]),
];

const CLOSE: &[Part] = &[
  stroke(&[at(4.0, 17.0), to(12.0, 3.0), to(20.0, 17.0)]),
  dashed(&[at(20.0, 17.0), to(4.0, 17.0)]),
];

const SMOOTH: &[Part] = &[
  faded(
    &[
      at(3.0, 15.0),
      to(5.5, 10.0),
      to(7.5, 13.0),
      to(10.0, 7.0),
      to(12.5, 11.5),
      to(15.5, 5.5),
      to(18.5, 9.0),
    ],
    0.45,
  ),
  stroke(&[
    at(3.0, 16.0),
    curve((7.0, 8.0), (9.0, 18.0), (12.0, 13.5)),
    curve((15.0, 9.0), (17.0, 13.5), (19.0, 10.0)),
  ]),
];

const REVERSE: &[Part] = &[
  stroke(&[
    at(4.0, 20.0),
    to(4.0, 9.0),
    arc(6.0, 6.0, false, true, (10.0, 3.0)),
    to(19.5, 3.0),
  ]),
  stroke(&[at(16.5, 4.5), to(20.0, 7.5), to(16.5, 10.5)]),
];

const MOVE: &[Part] = &[
  stroke(&[at(12.0, 3.0), to(12.0, 21.0), at(3.0, 12.0), to(21.0, 12.0)]),
  stroke(&[
    at(12.0, 3.0),
    to(9.6, 5.6),
    at(12.0, 3.0),
    to(14.4, 5.6),
    at(12.0, 21.0),
    to(9.6, 18.4),
    at(12.0, 21.0),
    to(14.4, 18.4),
    at(3.0, 12.0),
    to(5.6, 9.6),
    at(3.0, 12.0),
    to(5.6, 14.4),
    at(21.0, 12.0),
    to(18.4, 9.6),
    at(21.0, 12.0),
    to(18.4, 14.4),
  ]),
];

const ROTATE: &[Part] = &[
  stroke(&[at(20.0, 12.0), arc(8.0, 8.0, true, true, (17.5, 6.2))]),
  stroke(&[at(20.0, 4.0), to(20.0, 8.5), to(15.5, 8.5)]),
  stroke(&[ellipse((12.0, 12.0), 1.2, 1.2)]),
];

const SCALE: &[Part] = &[
  stroke(&[
    at(3.0, 13.0),
    to(11.0, 5.0),
    to(17.0, 11.0),
    to(9.0, 19.0),
    close(),
  ]),
  dashed(&[at(13.5, 3.5), to(20.5, 10.5)]),
  stroke(&[at(9.0, 7.0), to(12.0, 4.0)]),
];

const MIRROR_H: &[Part] = &[
  dashed(&[at(12.0, 3.0), to(12.0, 21.0)]),
  stroke(&[
    at(9.0, 7.0),
    to(3.5, 12.0),
    to(9.0, 17.0),
    close(),
    at(15.0, 7.0),
    to(20.5, 12.0),
    to(15.0, 17.0),
    close(),
  ]),
];

const MIRROR_V: &[Part] = &[
  dashed(&[at(3.0, 12.0), to(21.0, 12.0)]),
  stroke(&[
    at(7.0, 9.0),
    to(12.0, 3.5),
    to(17.0, 9.0),
    close(),
    at(7.0, 15.0),
    to(12.0, 20.5),
    to(17.0, 15.0),
    close(),
  ]),
];

const MIRROR_LINE: &[Part] = &[
  dashed(&[at(3.0, 21.0), to(21.0, 3.0)]),
  faded(&[at(4.0, 13.0), to(9.0, 8.0), to(13.0, 12.0)], 0.85),
  faded(&[at(11.0, 20.0), to(16.0, 15.0), to(20.0, 19.0)], 0.85),
];

const ARRAY_GRID: &[Part] = &[
  stroke(&[rect((3.0, 3.0), (7.0, 7.0), 1.0)]),
  faded(&[rect((14.0, 3.0), (7.0, 7.0), 1.0)], 0.55),
  faded(&[rect((3.0, 14.0), (7.0, 7.0), 1.0)], 0.55),
  faded(&[rect((14.0, 14.0), (7.0, 7.0), 1.0)], 0.55),
];

const ARRAY_CIRCULAR: &[Part] = &[
  dashed(&[ellipse((12.0, 12.0), 9.0, 9.0)]),
  stroke(&[ellipse((12.0, 12.0), 2.0, 2.0)]),
  stroke(&[ellipse((12.0, 4.0), 1.5, 1.5)]),
  stroke(&[ellipse((19.0, 12.0), 1.5, 1.5)]),
  stroke(&[ellipse((12.0, 20.0), 1.5, 1.5)]),
  stroke(&[ellipse((5.0, 12.0), 1.5, 1.5)]),
];

const ALIGN: &[Part] = &[
  stroke(&[at(4.0, 3.0), to(4.0, 21.0)]),
  stroke(&[rect((4.0, 6.0), (13.0, 4.5), 1.0)]),
  faded(&[rect((4.0, 13.5), (8.0, 4.5), 1.0)], 0.55),
];

const NEST: &[Part] = &[
  dashed(&[rect((3.0, 4.0), (18.0, 16.0), 1.0)]),
  stroke(&[rect((6.0, 7.0), (8.0, 5.0), 1.0)]),
  stroke(&[rect((6.0, 14.0), (4.0, 4.0), 1.0)]),
  stroke(&[rect((12.0, 14.0), (5.0, 4.0), 1.0)]),
];

const ZOOM_FIT: &[Part] = &[
  stroke(&[
    at(3.0, 8.0),
    to(3.0, 3.0),
    to(8.0, 3.0),
    at(16.0, 3.0),
    to(21.0, 3.0),
    to(21.0, 8.0),
    at(21.0, 16.0),
    to(21.0, 21.0),
    to(16.0, 21.0),
    at(8.0, 21.0),
    to(3.0, 21.0),
    to(3.0, 16.0),
  ]),
  faded(&[rect((8.0, 8.0), (8.0, 8.0), 1.0)], 0.5),
];

const ZOOM_SELECTION: &[Part] = &[
  stroke(&[ellipse((10.5, 10.5), 6.0, 6.0)]),
  stroke(&[at(15.0, 15.0), to(20.5, 20.5)]),
  faded(&[rect((8.0, 8.0), (5.0, 5.0), 0.0)], 0.55),
];

const GRID: &[Part] = &[
  stroke(&[rect((3.0, 3.0), (18.0, 18.0), 1.0)]),
  stroke(&[
    at(9.0, 3.0),
    to(9.0, 21.0),
    at(15.0, 3.0),
    to(15.0, 21.0),
    at(3.0, 9.0),
    to(21.0, 9.0),
    at(3.0, 15.0),
    to(21.0, 15.0),
  ]),
];

const LAYERS: &[Part] = &[
  stroke(&[
    at(12.0, 3.0),
    to(21.0, 7.5),
    to(12.0, 12.0),
    to(3.0, 7.5),
    close(),
  ]),
  stroke(&[at(3.0, 12.0), to(12.0, 16.5), to(21.0, 12.0)]),
  stroke(&[at(3.0, 16.5), to(12.0, 21.0), to(21.0, 16.5)]),
];

const VISIBLE: &[Part] = &[
  stroke(&[
    at(2.0, 12.0),
    curve((2.0, 12.0), (6.0, 5.0), (12.0, 5.0)),
    curve((18.0, 5.0), (22.0, 12.0), (22.0, 12.0)),
    curve((22.0, 12.0), (18.0, 19.0), (12.0, 19.0)),
    curve((6.0, 19.0), (2.0, 12.0), (2.0, 12.0)),
    close(),
  ]),
  stroke(&[ellipse((12.0, 12.0), 3.0, 3.0)]),
];

const HIDDEN: &[Part] = &[
  faded(
    &[
      at(2.0, 12.0),
      curve((2.0, 12.0), (6.0, 5.0), (12.0, 5.0)),
      curve((18.0, 5.0), (22.0, 12.0), (22.0, 12.0)),
      curve((22.0, 12.0), (18.0, 19.0), (12.0, 19.0)),
      curve((6.0, 19.0), (2.0, 12.0), (2.0, 12.0)),
      close(),
    ],
    0.35,
  ),
  stroke(&[at(4.0, 4.0), to(20.0, 20.0)]),
];

const LOCKED: &[Part] = &[
  stroke(&[rect((5.0, 11.0), (14.0, 10.0), 1.5)]),
  stroke(&[
    at(8.0, 11.0),
    to(8.0, 8.0),
    arc(4.0, 4.0, false, true, (16.0, 8.0)),
    to(16.0, 11.0),
  ]),
];

const UNLOCKED: &[Part] = &[
  faded(&[rect((5.0, 11.0), (14.0, 10.0), 1.5)], 0.5),
  faded(
    &[
      at(8.0, 11.0),
      to(8.0, 8.0),
      arc(4.0, 4.0, false, true, (15.6, 6.3)),
    ],
    0.5,
  ),
];

const HELP: &[Part] = &[
  stroke(&[ellipse((12.0, 12.0), 9.0, 9.0)]),
  stroke(&[
    at(9.5, 9.5),
    arc(2.5, 2.5, true, true, (13.0, 11.8)),
    curve((12.3, 12.1), (12.0, 12.7), (12.0, 13.5)),
    to(12.0, 13.8),
  ]),
  stroke(&[at(12.0, 17.4), to(12.0, 17.6)]),
];

const VIEW_2D: &[Part] = &[
  stroke(&[rect((3.0, 5.0), (18.0, 14.0), 1.0)]),
  stroke(&[
    at(7.0, 15.0),
    to(10.0, 9.0),
    to(13.0, 13.0),
    to(15.0, 11.0),
    to(17.0, 15.0),
  ]),
];

const VIEW_3D: &[Part] = &[
  stroke(&[
    at(12.0, 3.0),
    to(20.0, 7.5),
    to(20.0, 16.5),
    to(12.0, 21.0),
    to(4.0, 16.5),
    to(4.0, 7.5),
    close(),
  ]),
  stroke(&[
    at(4.0, 7.5),
    to(12.0, 12.0),
    to(20.0, 7.5),
    at(12.0, 12.0),
    to(12.0, 21.0),
  ]),
];

const THEME_AUTO: &[Part] = &[
  stroke(&[ellipse((12.0, 12.0), 8.0, 8.0)]),
  wash(&[
    at(12.0, 4.0),
    arc(8.0, 8.0, false, true, (12.0, 20.0)),
    close(),
  ]),
];

const SIMULATE: &[Part] = &[
  stroke(&[
    at(3.0, 15.0),
    to(12.0, 19.0),
    to(21.0, 15.0),
    to(21.0, 7.5),
    to(12.0, 3.5),
    to(3.0, 7.5),
    close(),
  ]),
  stroke(&[
    at(3.0, 7.5),
    to(12.0, 11.5),
    to(21.0, 7.5),
    at(12.0, 11.5),
    to(12.0, 19.0),
  ]),
  stroke(&[ellipse((12.0, 5.0), 1.4, 1.4)]),
];

const GCODE: &[Part] = &[
  stroke(&[
    at(6.0, 3.0),
    to(15.0, 3.0),
    to(19.0, 7.0),
    to(19.0, 21.0),
    to(6.0, 21.0),
    close(),
  ]),
  stroke(&[at(15.0, 3.0), to(15.0, 7.0), to(19.0, 7.0)]),
  stroke(&[at(9.0, 12.0), to(14.0, 12.0), at(9.0, 15.5), to(13.0, 15.5)]),
  faded(
    &[
      at(12.0, 18.0),
      to(12.0, 20.5),
      at(10.0, 18.8),
      to(12.0, 20.8),
      to(14.0, 18.8),
    ],
    0.6,
  ),
];

const TOOLPATH_PROFILE: &[Part] = &[
  stroke(&[rect((7.5, 7.5), (9.0, 9.0), 1.0)]),
  dashed(&[rect((3.0, 3.0), (18.0, 18.0), 5.0)]),
];

const TOOLPATH_POCKET: &[Part] = &[
  stroke(&[rect((3.0, 4.0), (18.0, 16.0), 1.5)]),
  stroke(&[
    at(6.5, 8.0),
    to(17.5, 8.0),
    to(17.5, 16.0),
    to(10.0, 16.0),
    to(10.0, 10.5),
    to(14.5, 10.5),
  ]),
];

const TOOLPATH_DRILL: &[Part] = &[
  stroke(&[
    at(10.0, 3.0),
    to(14.0, 3.0),
    to(14.0, 7.5),
    to(13.0, 9.0),
    to(13.0, 15.0),
    to(12.0, 16.5),
    to(11.0, 15.0),
    to(11.0, 9.0),
    to(10.0, 7.5),
    close(),
  ]),
  stroke(&[ellipse((5.5, 18.5), 1.8, 1.8)]),
  stroke(&[ellipse((19.0, 18.5), 1.8, 1.8)]),
];

const TOOLPATH_VCARVE: &[Part] = &[
  stroke(&[
    at(3.0, 6.0),
    to(7.5, 6.0),
    to(12.0, 13.0),
    to(16.5, 6.0),
    to(21.0, 6.0),
  ]),
  stroke(&[at(3.0, 6.0), to(3.0, 19.0), to(21.0, 19.0), to(21.0, 6.0)]),
];

const POST_PROCESSOR: &[Part] = &[
  stroke(&[
    at(6.0, 3.0),
    to(15.0, 3.0),
    to(19.0, 7.0),
    to(19.0, 21.0),
    to(6.0, 21.0),
    close(),
  ]),
  stroke(&[
    at(8.5, 12.0),
    to(11.0, 14.5),
    to(8.5, 17.0),
    at(15.0, 12.0),
    to(12.5, 14.5),
    to(15.0, 17.0),
  ]),
];

const RELIEF: &[Part] = &[
  stroke(&[at(4.0, 19.0), to(20.0, 19.0)]),
  wash(&[
    at(6.0, 19.0),
    curve((6.0, 12.5), (8.7, 9.0), (12.0, 9.0)),
    curve((15.3, 9.0), (18.0, 12.5), (18.0, 19.0)),
  ]),
  stroke(&[
    at(6.0, 19.0),
    curve((6.0, 12.5), (8.7, 9.0), (12.0, 9.0)),
    curve((15.3, 9.0), (18.0, 12.5), (18.0, 19.0)),
  ]),
];

/// A keyboard: two keycaps over a space bar. Added for the `Shortcuts` palette chip, which had been
/// drawn with the letterform `Add text` already used — two chips, one picture (T8.2).
///
/// Three rounded outlines rather than one body with key marks: at 16 px a body would have to hold its
/// keys inside a 1.6 stroke, and a keycap two units tall is a smear. Three outlines at this spacing are
/// still three separate shapes at the rail's smallest size, and nothing in the set is shaped like them.
const SHORTCUTS: &[Part] = &[
  stroke(&[rect((3.0, 5.0), (7.0, 6.0), 1.5)]),
  stroke(&[rect((14.0, 5.0), (7.0, 6.0), 1.5)]),
  stroke(&[rect((6.0, 14.0), (12.0, 5.0), 1.5)]),
];

/// Every glyph, in the artboard's order. The order is load-bearing: [`Icon`] indexes
/// this table by its discriminant, so appending here means appending a variant there.
/// An arrow curving back on itself: undo.
///
/// Added for the app bar, which the artboard draws with a glyph the first sheet did not contain: the
/// vocabulary has to cover the frame that uses it, or the shell invents an icon at the call site.
const UNDO: &[Part] = &[
  stroke(&[at(9.0, 14.0), to(4.0, 9.0), to(9.0, 4.0)]),
  stroke(&[
    at(4.0, 9.0),
    to(14.0, 9.0),
    arc(6.0, 6.0, false, true, (14.0, 21.0)),
    to(11.0, 21.0),
  ]),
];

/// [`UNDO`] mirrored: redo.
const REDO: &[Part] = &[
  stroke(&[at(15.0, 14.0), to(20.0, 9.0), to(15.0, 4.0)]),
  stroke(&[
    at(20.0, 9.0),
    to(10.0, 9.0),
    arc(6.0, 6.0, false, false, (10.0, 21.0)),
    to(13.0, 21.0),
  ]),
];

/// A sun: the light set is showing, and pressing gives the dark one.
const THEME_LIGHT: &[Part] = &[
  stroke(&[ellipse((12.0, 12.0), 4.0, 4.0)]),
  stroke(&[at(12.0, 2.5), to(12.0, 5.0)]),
  stroke(&[at(12.0, 19.0), to(12.0, 21.5)]),
  stroke(&[at(2.5, 12.0), to(5.0, 12.0)]),
  stroke(&[at(19.0, 12.0), to(21.5, 12.0)]),
  stroke(&[at(5.3, 5.3), to(7.0, 7.0)]),
  stroke(&[at(17.0, 17.0), to(18.7, 18.7)]),
  stroke(&[at(18.7, 5.3), to(17.0, 7.0)]),
  stroke(&[at(7.0, 17.0), to(5.3, 18.7)]),
];

/// A crescent: the dark set is showing.
const THEME_DARK: &[Part] = &[stroke(&[
  at(20.0, 14.5),
  arc(8.0, 8.0, true, true, (9.5, 4.0)),
  arc(8.0, 8.0, true, false, (20.0, 14.5)),
])];

const GLYPHS: &[Glyph] = &[
  Glyph {
    name: "new",
    parts: NEW,
  },
  Glyph {
    name: "open",
    parts: OPEN,
  },
  Glyph {
    name: "save",
    parts: SAVE,
  },
  Glyph {
    name: "save-as",
    parts: SAVE_AS,
  },
  Glyph {
    name: "import-vectors",
    parts: IMPORT_VECTORS,
  },
  Glyph {
    name: "import-image",
    parts: IMPORT_IMAGE,
  },
  Glyph {
    name: "export-dxf",
    parts: EXPORT_DXF,
  },
  Glyph {
    name: "export-svg",
    parts: EXPORT_SVG,
  },
  Glyph {
    name: "stock-sheet",
    parts: STOCK_SHEET,
  },
  Glyph {
    name: "stock-rotary",
    parts: STOCK_ROTARY,
  },
  Glyph {
    name: "tool-library",
    parts: TOOL_LIBRARY,
  },
  Glyph {
    name: "select",
    parts: SELECT,
  },
  Glyph {
    name: "node-edit",
    parts: NODE_EDIT,
  },
  Glyph {
    name: "line",
    parts: LINE,
  },
  Glyph {
    name: "polyline",
    parts: POLYLINE,
  },
  Glyph {
    name: "arc",
    parts: ARC,
  },
  Glyph {
    name: "circle",
    parts: CIRCLE,
  },
  Glyph {
    name: "ellipse",
    parts: ELLIPSE,
  },
  Glyph {
    name: "rectangle",
    parts: RECTANGLE,
  },
  Glyph {
    name: "polygon",
    parts: POLYGON,
  },
  Glyph {
    name: "star",
    parts: STAR,
  },
  Glyph {
    name: "text",
    parts: TEXT,
  },
  Glyph {
    name: "dimension",
    parts: DIMENSION,
  },
  Glyph {
    name: "trim",
    parts: TRIM,
  },
  Glyph {
    name: "extend",
    parts: EXTEND,
  },
  Glyph {
    name: "fillet",
    parts: FILLET,
  },
  Glyph {
    name: "chamfer",
    parts: CHAMFER,
  },
  Glyph {
    name: "break",
    parts: BREAK,
  },
  Glyph {
    name: "offset",
    parts: OFFSET,
  },
  Glyph {
    name: "union",
    parts: UNION,
  },
  Glyph {
    name: "subtract",
    parts: SUBTRACT,
  },
  Glyph {
    name: "intersect",
    parts: INTERSECT,
  },
  Glyph {
    name: "join",
    parts: JOIN,
  },
  Glyph {
    name: "close",
    parts: CLOSE,
  },
  Glyph {
    name: "smooth",
    parts: SMOOTH,
  },
  Glyph {
    name: "reverse",
    parts: REVERSE,
  },
  Glyph {
    name: "move",
    parts: MOVE,
  },
  Glyph {
    name: "rotate",
    parts: ROTATE,
  },
  Glyph {
    name: "scale",
    parts: SCALE,
  },
  Glyph {
    name: "mirror-h",
    parts: MIRROR_H,
  },
  Glyph {
    name: "mirror-v",
    parts: MIRROR_V,
  },
  Glyph {
    name: "mirror-line",
    parts: MIRROR_LINE,
  },
  Glyph {
    name: "array-grid",
    parts: ARRAY_GRID,
  },
  Glyph {
    name: "array-circular",
    parts: ARRAY_CIRCULAR,
  },
  Glyph {
    name: "align",
    parts: ALIGN,
  },
  Glyph {
    name: "nest",
    parts: NEST,
  },
  Glyph {
    name: "zoom-fit",
    parts: ZOOM_FIT,
  },
  Glyph {
    name: "zoom-selection",
    parts: ZOOM_SELECTION,
  },
  Glyph {
    name: "grid",
    parts: GRID,
  },
  Glyph {
    name: "layers",
    parts: LAYERS,
  },
  Glyph {
    name: "visible",
    parts: VISIBLE,
  },
  Glyph {
    name: "hidden",
    parts: HIDDEN,
  },
  Glyph {
    name: "locked",
    parts: LOCKED,
  },
  Glyph {
    name: "unlocked",
    parts: UNLOCKED,
  },
  Glyph {
    name: "help",
    parts: HELP,
  },
  Glyph {
    name: "shortcuts",
    parts: SHORTCUTS,
  },
  Glyph {
    name: "view-2d",
    parts: VIEW_2D,
  },
  Glyph {
    name: "view-3d",
    parts: VIEW_3D,
  },
  Glyph {
    name: "theme-auto",
    parts: THEME_AUTO,
  },
  Glyph {
    name: "simulate",
    parts: SIMULATE,
  },
  Glyph {
    name: "gcode",
    parts: GCODE,
  },
  Glyph {
    name: "toolpath-profile",
    parts: TOOLPATH_PROFILE,
  },
  Glyph {
    name: "toolpath-pocket",
    parts: TOOLPATH_POCKET,
  },
  Glyph {
    name: "toolpath-drill",
    parts: TOOLPATH_DRILL,
  },
  Glyph {
    name: "toolpath-vcarve",
    parts: TOOLPATH_VCARVE,
  },
  Glyph {
    name: "post-processor",
    parts: POST_PROCESSOR,
  },
  Glyph {
    name: "relief",
    parts: RELIEF,
  },
  Glyph {
    name: "undo",
    parts: UNDO,
  },
  Glyph {
    name: "redo",
    parts: REDO,
  },
  Glyph {
    name: "theme-light",
    parts: THEME_LIGHT,
  },
  Glyph {
    name: "theme-dark",
    parts: THEME_DARK,
  },
];

/// A flattened subpath in box units, its closing point left implicit.
struct Subpath {
  /// The points, in drawing order.
  points: Vec<(f32, f32)>,
  /// Whether the subpath was closed, which decides the round caps at its ends.
  closed: bool,
}

/// Flatten one element into the subpaths it draws.
///
/// One element can draw several subpaths — `M12 3v18M3 12h18` is two — and a closed one must not get
/// end caps, so they are kept apart here instead of being welded into one polyline.
fn subpaths(cmds: &[Cmd]) -> Vec<Subpath> {
  fn flush(out: &mut Vec<Subpath>, points: &mut Vec<(f32, f32)>, closed: bool) {
    if points.len() >= 2 {
      out.push(Subpath {
        points: std::mem::take(points),
        closed,
      });
    } else {
      points.clear();
    }
  }

  let mut out = Vec::new();
  let mut points: Vec<(f32, f32)> = Vec::new();
  let mut from = (0.0, 0.0);
  for cmd in cmds {
    match *cmd {
      Cmd::Move(x, y) => {
        flush(&mut out, &mut points, false);
        points.push((x, y));
        from = (x, y);
      }
      Cmd::Line(x, y) => {
        points.push((x, y));
        from = (x, y);
      }
      Cmd::Curve { c1, c2, to } => {
        flatten_curve(from, c1, c2, to, 0, &mut points);
        from = to;
      }
      Cmd::Arc {
        rx,
        ry,
        large,
        sweep,
        to,
      } => {
        flatten_arc(from, rx, ry, large, sweep, to, &mut points);
        from = to;
      }
      Cmd::Ellipse { c, rx, ry } => {
        flush(&mut out, &mut points, false);
        let mut ring = Vec::new();
        flatten_ellipse(c, rx, ry, &mut ring);
        out.push(Subpath {
          points: ring,
          closed: true,
        });
      }
      Cmd::Rect { min, size, r } => {
        flush(&mut out, &mut points, false);
        let mut ring = Vec::new();
        flatten_rect(min, size, r, &mut ring);
        out.push(Subpath {
          points: ring,
          closed: true,
        });
      }
      Cmd::Close => flush(&mut out, &mut points, true),
    }
  }
  flush(&mut out, &mut points, false);
  out
}

/// Sample a cubic Bézier, appending every point after the start.
///
/// Subdivision rather than a fixed sample count: a short curve stays two segments and a long S costs
/// what it needs, with the same chord error everywhere instead of the same number of vertices.
fn flatten_curve(
  p0: (f32, f32),
  p1: (f32, f32),
  p2: (f32, f32),
  p3: (f32, f32),
  depth: u32,
  out: &mut Vec<(f32, f32)>,
) {
  if depth >= MAX_CURVE_DEPTH || flat_enough(p0, p1, p2, p3) {
    out.push(p3);
    return;
  }
  let mid = |a: (f32, f32), b: (f32, f32)| ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
  let (p01, p12, p23) = (mid(p0, p1), mid(p1, p2), mid(p2, p3));
  let (p012, p123) = (mid(p01, p12), mid(p12, p23));
  let centre = mid(p012, p123);
  flatten_curve(p0, p01, p012, centre, depth + 1, out);
  flatten_curve(centre, p123, p23, p3, depth + 1, out);
}

/// Whether a cubic's control points hug its chord closely enough to be drawn as the chord.
fn flat_enough(p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)) -> bool {
  distance_to_chord(p1, p0, p3) <= TOLERANCE && distance_to_chord(p2, p0, p3) <= TOLERANCE
}

/// Perpendicular distance from `p` to the line through `a` and `b`, falling back to the distance to
/// `a` when the chord has no length.
fn distance_to_chord(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
  let (dx, dy) = (b.0 - a.0, b.1 - a.1);
  let length = (dx * dx + dy * dy).sqrt();
  if length < 1e-6 {
    return ((p.0 - a.0).powi(2) + (p.1 - a.1).powi(2)).sqrt();
  }
  ((p.0 - a.0) * dy - (p.1 - a.1) * dx).abs() / length
}

/// Flatten an elliptical arc, appending every point after the start.
///
/// This is SVG's endpoint-to-centre conversion (§B.2.4) with the axis rotation dropped, as
/// [`Cmd::Arc`] explains. Doing it properly matters here rather than approximately: `rotate` and
/// `help` rely on `large-arc-flag`, and guessing the centre from the endpoints alone would draw the
/// peel instead of the wrapping stroke.
fn flatten_arc(
  from: (f32, f32),
  rx: f32,
  ry: f32,
  large: bool,
  sweep: bool,
  to: (f32, f32),
  out: &mut Vec<(f32, f32)>,
) {
  let (mut rx, mut ry) = (rx.abs(), ry.abs());
  let (dx, dy) = ((from.0 - to.0) * 0.5, (from.1 - to.1) * 0.5);
  if dx == 0.0 && dy == 0.0 {
    return;
  }
  if rx < 1e-6 || ry < 1e-6 {
    // A zero radius is a straight line to the endpoint, per the spec.
    out.push(to);
    return;
  }
  // Endpoints further apart than the ellipse can reach mean the radii were too small: the spec scales
  // them up until the arc exists at all. No glyph here needs the correction, but leaving it out would
  // silently move the centre of the next arc that did.
  let reach = dx * dx / (rx * rx) + dy * dy / (ry * ry);
  if reach > 1.0 {
    let scale = reach.sqrt();
    rx *= scale;
    ry *= scale;
  }
  let numerator = (rx * rx * ry * ry - rx * rx * dy * dy - ry * ry * dx * dx).max(0.0);
  let denominator = rx * rx * dy * dy + ry * ry * dx * dx;
  let factor = if denominator > 0.0 {
    (numerator / denominator).sqrt()
  } else {
    0.0
  };
  // The two candidate centres are mirror images; the flags pick one. Same flag, other side.
  let factor = if large == sweep { -factor } else { factor };
  let cxp = factor * rx * dy / ry;
  let cyp = -factor * ry * dx / rx;
  let centre = (cxp + (from.0 + to.0) * 0.5, cyp + (from.1 + to.1) * 0.5);
  let start = ((dx - cxp) / rx, (dy - cyp) / ry);
  let end = ((-dx - cxp) / rx, (-dy - cyp) / ry);
  let start_angle = start.1.atan2(start.0);
  let mut span = (start.0 * end.1 - start.1 * end.0).atan2(start.0 * end.0 + start.1 * end.1);
  if sweep && span < 0.0 {
    span += std::f32::consts::TAU;
  }
  if !sweep && span > 0.0 {
    span -= std::f32::consts::TAU;
  }
  let before = out.len();
  push_arc_points(centre, rx, ry, start_angle, start_angle + span, out);
  if out.len() > before {
    // Land exactly on the endpoint the table names, so a chained subpath starts where it says.
    *out.last_mut().expect("a sample was just pushed") = to;
  }
}

/// Sample an elliptical arc, excluding its start angle and including its end angle.
///
/// The step comes from the sagitta of a chord — `r (1 - cos(δ/2))` — solved for [`TOLERANCE`], so a
/// big arc gets more segments than a small one and neither is wasted.
fn push_arc_points(
  centre: (f32, f32),
  rx: f32,
  ry: f32,
  from_angle: f32,
  to_angle: f32,
  out: &mut Vec<(f32, f32)>,
) {
  let span = to_angle - from_angle;
  let radius = rx.max(ry).max(1e-3);
  // The sagitta of a chord spanning `step` radians is `r (1 - cos(step / 2))`, so the widest step
  // that keeps it inside the tolerance is `2 acos(1 - tolerance / r)`.
  let step = 2.0 * (1.0 - TOLERANCE / radius).clamp(-1.0, 1.0).acos();
  let steps = ((span.abs() / step).ceil() as usize).clamp(1, 512);
  for index in 1..=steps {
    let angle = from_angle + span * (index as f32 / steps as f32);
    out.push((centre.0 + rx * angle.cos(), centre.1 + ry * angle.sin()));
  }
}

/// Flatten a whole ellipse into a closed ring, leaving the duplicated closing point out.
fn flatten_ellipse(centre: (f32, f32), rx: f32, ry: f32, out: &mut Vec<(f32, f32)>) {
  push_arc_points(centre, rx, ry, 0.0, std::f32::consts::TAU, out);
  out.pop();
}

/// Flatten a rounded rectangle into a closed ring, leaving the duplicated closing point out.
fn flatten_rect(min: (f32, f32), size: (f32, f32), radius: f32, out: &mut Vec<(f32, f32)>) {
  let (x0, y0) = min;
  let (x1, y1) = (x0 + size.0, y0 + size.1);
  let r = radius.clamp(0.0, size.0.abs().min(size.1.abs()) * 0.5);
  if r <= 0.0 {
    out.extend([(x0, y0), (x1, y0), (x1, y1), (x0, y1)]);
    return;
  }
  let quarter = std::f32::consts::FRAC_PI_2;
  out.push((x0 + r, y0));
  out.push((x1 - r, y0));
  push_arc_points((x1 - r, y0 + r), r, r, -quarter, 0.0, out);
  out.push((x1, y1 - r));
  push_arc_points((x1 - r, y1 - r), r, r, 0.0, quarter, out);
  out.push((x0 + r, y1));
  push_arc_points((x0 + r, y1 - r), r, r, quarter, 2.0 * quarter, out);
  out.push((x0, y0 + r));
  push_arc_points((x0 + r, y0 + r), r, r, 2.0 * quarter, 3.0 * quarter, out);
  out.pop();
}

/// One glyph of the shell's icon set.
///
/// The variants follow the artboard's order, so [`Icon::ALL`] is the sheet read left to right and the
/// icon sheet (T3) shows the set in the order the designer drew it. Each variant says what it draws,
/// because a name like "Stock" or "Advanced" would not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
  /// A page with a plus.
  New,
  /// An open folder.
  Open,
  /// A page with a shutter and a label: a floppy, on this grid.
  Save,
  /// The same page, half drawn, with a pencil over its corner.
  SaveAs,
  /// An arrow pointing down into a tray.
  ImportVectors,
  /// A picture in a frame, with an arrow arriving at its corner.
  ImportImage,
  /// An arrow pointing up out of a tray, with a stepped line in the tray.
  ExportDxf,
  /// The same arrow, with a wave in the tray.
  ExportSvg,
  /// A flat sheet, drawn as the top of a box.
  StockSheet,
  /// A cylinder: an ellipse on the sides.
  StockRotary,
  /// A router bit: shank, taper and cutter.
  ToolLibrary,
  /// The arrow cursor.
  Select,
  /// A curve with three square node handles.
  NodeEdit,
  /// A straight line with a node at each end.
  Line,
  /// A path with four square nodes.
  Polyline,
  /// A quarter arc with a node at each end.
  Arc,
  /// A circle with a dashed radius and a centre dot.
  Circle,
  /// An ellipse with a dashed axis.
  Ellipse,
  /// A rectangle with rounded corners.
  Rectangle,
  /// A hexagon.
  Polygon,
  /// A five-pointed star.
  Star,
  /// A capital T: bar, stem, foot.
  Text,
  /// A dimension line with extensions and arrowheads.
  Dimension,
  /// A line cut in two across a gap.
  Trim,
  /// A solid line, a dashed extension, and the wall it reaches.
  Extend,
  /// An L with its corner rounded, and the sharp corner ghosted.
  Fillet,
  /// The same corner cut flat.
  Chamfer,
  /// A line broken between two round ends.
  Break,
  /// A rounded square inside a larger one.
  Offset,
  /// Two circles and the lens they add up to, washed.
  Union,
  /// A circle with the shape to be removed dashed.
  Subtract,
  /// Two dashed circles and the lens they keep, washed.
  Intersect,
  /// Two segments meeting at a pair of nodes.
  Join,
  /// An open peak closed by a dashed base.
  Close,
  /// A jagged line above, the smooth curve it becomes below.
  Smooth,
  /// An L-shaped path whose arrowhead shows the direction flipping.
  Reverse,
  /// Four arrows from one centre.
  Move,
  /// A circular arrow around a pivot.
  Rotate,
  /// A box with a ghosted diagonal and its handle.
  Scale,
  /// A dashed vertical axis with a triangle either side.
  MirrorH,
  /// A dashed horizontal axis with a triangle above and below.
  MirrorV,
  /// Two strokes either side of a dashed diagonal axis.
  MirrorLine,
  /// Four squares: the source solid, its copies faded.
  ArrayGrid,
  /// A dashed pitch circle with five instances on it.
  ArrayCircular,
  /// A wall with two bars aligned to it, the second faded.
  Align,
  /// A dashed sheet with three parts nested inside.
  Nest,
  /// Four corner brackets with a faded box between them.
  ZoomFit,
  /// A magnifier over a faded selection box.
  ZoomSelection,
  /// A framed grid.
  Grid,
  /// Three stacked sheets.
  Layers,
  /// An eye.
  Visible,
  /// The same eye, faded and struck through.
  Hidden,
  /// A closed padlock.
  Locked,
  /// A padlock with its shackle open, faded.
  Unlocked,
  /// A question mark in a circle.
  Help,
  /// A keyboard: two keycaps over a space bar.
  Shortcuts,
  /// A frame with a profile line inside it.
  View2d,
  /// The cube, seen from one corner.
  View3d,
  /// A circle half washed: the system decides.
  ThemeAuto,
  /// The cube with a node on its top face.
  Simulate,
  /// A program file with text lines and a download arrow.
  Gcode,
  /// A part outline inside a dashed offset.
  ToolpathProfile,
  /// A pocket with a step-over spiral.
  ToolpathPocket,
  /// A drill bit with two holes.
  ToolpathDrill,
  /// A V-groove between two walls.
  ToolpathVcarve,
  /// A program file marked with a pair of arrows.
  PostProcessor,
  /// A dome washed above a baseline.
  Relief,
  /// An arrow curving back: undo.
  Undo,
  /// An arrow curving forward: redo.
  Redo,
  /// A sun: the light set.
  ThemeLight,
  /// A crescent: the dark set.
  ThemeDark,
}

impl Icon {
  /// Every glyph, in the artboard's order.
  pub const ALL: &'static [Icon] = &[
    Icon::New,
    Icon::Open,
    Icon::Save,
    Icon::SaveAs,
    Icon::ImportVectors,
    Icon::ImportImage,
    Icon::ExportDxf,
    Icon::ExportSvg,
    Icon::StockSheet,
    Icon::StockRotary,
    Icon::ToolLibrary,
    Icon::Select,
    Icon::NodeEdit,
    Icon::Line,
    Icon::Polyline,
    Icon::Arc,
    Icon::Circle,
    Icon::Ellipse,
    Icon::Rectangle,
    Icon::Polygon,
    Icon::Star,
    Icon::Text,
    Icon::Dimension,
    Icon::Trim,
    Icon::Extend,
    Icon::Fillet,
    Icon::Chamfer,
    Icon::Break,
    Icon::Offset,
    Icon::Union,
    Icon::Subtract,
    Icon::Intersect,
    Icon::Join,
    Icon::Close,
    Icon::Smooth,
    Icon::Reverse,
    Icon::Move,
    Icon::Rotate,
    Icon::Scale,
    Icon::MirrorH,
    Icon::MirrorV,
    Icon::MirrorLine,
    Icon::ArrayGrid,
    Icon::ArrayCircular,
    Icon::Align,
    Icon::Nest,
    Icon::ZoomFit,
    Icon::ZoomSelection,
    Icon::Grid,
    Icon::Layers,
    Icon::Visible,
    Icon::Hidden,
    Icon::Locked,
    Icon::Unlocked,
    Icon::Help,
    Icon::Shortcuts,
    Icon::View2d,
    Icon::View3d,
    Icon::ThemeAuto,
    Icon::Simulate,
    Icon::Gcode,
    Icon::ToolpathProfile,
    Icon::ToolpathPocket,
    Icon::ToolpathDrill,
    Icon::ToolpathVcarve,
    Icon::PostProcessor,
    Icon::Relief,
    Icon::Undo,
    Icon::Redo,
    Icon::ThemeLight,
    Icon::ThemeDark,
  ];

  /// The artboard's kebab-case name, which is what a tooltip, a settings file or a test looks up.
  pub fn name(self) -> &'static str {
    self.glyph().name
  }

  /// The glyph with that artboard name, if the set has one.
  ///
  /// A name lookup rather than a second enum: the shell's icon-only controls are driven by strings
  /// from the design, and a set that could not answer to its own names would need a parallel table.
  pub fn from_name(name: &str) -> Option<Icon> {
    Self::ALL.iter().copied().find(|icon| icon.name() == name)
  }

  /// The table entry for this variant.
  ///
  /// Indexed by discriminant: the enum's order *is* the artboard's order, which
  /// `the_set_matches_the_artboard` proves by walking [`Icon::ALL`] and checking each variant's
  /// position, so the two lists cannot drift apart unnoticed.
  fn glyph(self) -> &'static Glyph {
    &GLYPHS[self as usize]
  }
}

/// The six families the sheet groups the set into, in the order the artboard draws them.
///
/// A family is documentation, not behaviour: nothing in the shell branches on it. It exists so seventy-one
/// glyphs can be read as six arguments instead of one wall, which is also why the sheet in the app groups
/// them the same way the artboard does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
  /// Opening, saving, importing, exporting, stock and tools.
  FileStock,
  /// The gestures that put geometry on the sheet.
  Draw,
  /// Vector surgery: trim, fillet, booleans, joins.
  VectorEdit,
  /// Moving, copying, aligning what exists.
  Transform,
  /// Looking at it, naming it, choosing a theme.
  ViewDocument,
  /// What removes material.
  Cutting,
}

impl Family {
  /// Every family, in sheet order.
  pub const ALL: [Family; 6] = [
    Family::FileStock,
    Family::Draw,
    Family::VectorEdit,
    Family::Transform,
    Family::ViewDocument,
    Family::Cutting,
  ];

  /// The word the sheet prints above the family.
  pub fn label(self) -> &'static str {
    match self {
      Self::FileStock => "File and stock",
      Self::Draw => "Draw",
      Self::VectorEdit => "Vector edit",
      Self::Transform => "Transform",
      Self::ViewDocument => "View, document, display",
      Self::Cutting => "Cutting",
    }
  }
}

impl Icon {
  /// The family this glyph belongs to.
  ///
  /// Keyed on the glyph's position in [`Icon::ALL`], which is the same order the sheet draws, because
  /// seventy match arms would be seventy chances to disagree with the table next door. The order itself is
  /// pinned by `tests::the_families_cover_the_set_in_order`, so a reordering that moved a glyph between
  /// families fails a test instead of quietly mislabelling it.
  pub fn family(self) -> Family {
    match self as usize {
      0..=10 => Family::FileStock,
      11..=22 => Family::Draw,
      23..=35 => Family::VectorEdit,
      36..=45 => Family::Transform,
      46..=62 => Family::ViewDocument,
      _ => Family::Cutting,
    }
  }

  /// The glyphs of one family, in sheet order.
  pub fn in_family(family: Family) -> impl Iterator<Item = Icon> {
    Icon::ALL
      .iter()
      .copied()
      .filter(move |icon| icon.family() == family)
  }
}

/// The shapes for one glyph, laid out in `box_rect`.
///
/// Pure — no painter, no texture, no side effects — so the geometry can be tested for the two things
/// a screenshot would never tell you: that every point is inside the safe box, and that one glyph
/// only ever paints with one colour.
///
/// `stroke_px` is in device pixels and is not scaled by the box: the artboard's 1.6 at 24 px is one
/// line weight at every size, which is what keeps a rail of twenty reading as one hand.
pub fn shapes(box_rect: Rect, icon: Icon, tint: Color32, stroke_px: f32) -> Vec<Shape> {
  let unit = box_rect.width() / BOX_UNITS;
  let place = |(x, y): (f32, f32)| Pos2::new(box_rect.min.x + x * unit, box_rect.min.y + y * unit);
  let mut out = Vec::new();
  for part in icon.glyph().parts {
    let colour = tint.gamma_multiply(part.fade);
    let stroke = Stroke::new(stroke_px, colour);
    for path in subpaths(part.cmds) {
      let points: Vec<Pos2> = path.points.iter().copied().map(place).collect();
      if part.fill {
        // The wash is a kept region, and the artboard's regions are convex by construction: a lens,
        // a dome, a half disc. A concave wash would need a mesh, and a mesh would be a redesign.
        out.push(Shape::convex_polygon(points, colour, Stroke::NONE));
        continue;
      }
      let mut ring = points.clone();
      if path.closed {
        // `Shape::line` does not close a path; the repeated first point is what joins the last
        // segment to the first, and it is also why a closed subpath gets no end caps.
        ring.push(points[0]);
      }
      if part.dashed {
        out.extend(Shape::dashed_line(&ring, stroke, DASH * unit, DASH * unit));
      } else {
        out.push(Shape::line(ring, stroke));
      }
      if !path.closed {
        // egui strokes have no cap setting, so the round caps the artboard asks for are a half-width
        // dot at each open end. Without them a dashed axis grows two square corners and the set
        // stops matching at 18 px.
        let cap = stroke_px * 0.5;
        out.push(Shape::circle_filled(points[0], cap, colour));
        out.push(Shape::circle_filled(points[points.len() - 1], cap, colour));
      }
    }
  }
  out
}

/// Paint one glyph into `box_rect`: the shell's way in.
pub fn paint(painter: &egui::Painter, box_rect: Rect, icon: Icon, tint: Color32, stroke_px: f32) {
  painter.extend(shapes(box_rect, icon, tint, stroke_px));
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::theme::{Role, Theme};

  /// The box a glyph is authored in, at 1:1.
  fn box_rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, egui::vec2(BOX_UNITS, BOX_UNITS))
  }

  /// The tint a test paints with, taken from the tokens rather than written here.
  fn tint() -> Color32 {
    Theme::Dark.palette().get(Role::Sel)
  }

  /// Every point a shape paints, back in box units.
  ///
  /// A cap dot contributes its centre only: its radius is the stroke, and the safe-area rule is about
  /// the drawing, not about how wide a pen is. A dash is a `LineSegment`, not a path, so it has to be
  /// walked too — the dashes are exactly the strokes that run to the edge of the box.
  fn points_in_box(painted: &[Shape], rect: Rect) -> Vec<Pos2> {
    let unit = rect.width() / BOX_UNITS;
    let mut points = Vec::new();
    for shape in painted {
      let authored: Vec<Pos2> = match shape {
        Shape::Path(path) => path.points.clone(),
        Shape::LineSegment { points, .. } => points.to_vec(),
        Shape::Circle(circle) => vec![circle.center],
        _ => Vec::new(),
      };
      points.extend(
        authored
          .into_iter()
          .map(|point| Pos2::new((point.x - rect.min.x) / unit, (point.y - rect.min.y) / unit)),
      );
    }
    points
  }

  /// Every stroke width a shape paints with.
  fn stroke_widths(shape: &Shape) -> Vec<f32> {
    match shape {
      Shape::Path(path) if !path.stroke.is_empty() => vec![path.stroke.width],
      Shape::LineSegment { stroke, .. } if !stroke.is_empty() => vec![stroke.width],
      _ => Vec::new(),
    }
  }

  /// Every colour a shape paints with.
  fn painted_colours(shape: &Shape) -> Vec<Color32> {
    fn solid(stroke: &egui::epaint::PathStroke) -> Color32 {
      match stroke.color {
        egui::epaint::ColorMode::Solid(colour) => colour,
        // The set never asks for a UV gradient; anything else would be a colour this module did not
        // choose, which is what the test is looking for.
        egui::epaint::ColorMode::UV(_) => Color32::TRANSPARENT,
      }
    }
    match shape {
      Shape::Path(path) => vec![path.fill, solid(&path.stroke)],
      Shape::LineSegment { stroke, .. } => vec![stroke.color],
      Shape::Circle(circle) => vec![circle.fill, circle.stroke.color],
      _ => Vec::new(),
    }
  }

  #[test]
  fn every_icon_has_geometry() {
    for &icon in Icon::ALL {
      let painted = shapes(box_rect(), icon, tint(), crate::theme::ICON_STROKE);
      assert!(!painted.is_empty(), "{} draws nothing at all", icon.name());
    }
  }

  #[test]
  fn every_glyph_stays_inside_the_safe_box() {
    let rect = box_rect();
    for &icon in Icon::ALL {
      let painted = shapes(rect, icon, tint(), crate::theme::ICON_STROKE);
      for point in points_in_box(&painted, rect) {
        assert!(
          point.x >= 2.0 - 0.01
            && point.x <= 22.0 + 0.01
            && point.y >= 2.0 - 0.01
            && point.y <= 22.0 + 0.01,
          "{}: ({:.2}, {:.2}) is outside the 2..22 safe area; the artboard's rule is that nothing \
           crosses 2 units from the edge",
          icon.name(),
          point.x,
          point.y
        );
      }
    }
  }

  #[test]
  fn the_set_matches_the_artboard() {
    // `design/camui-shell/03-iconography.html` holds 66 glyph cells, although its COUNT heading still
    // says 65 and §03's family hints sum to 61. The cells are the drawing, so the cells are what is
    // transliterated: a glyph that is drawn but not shipped is worse than a stale sentence.
    assert_eq!(
      Icon::ALL.len(),
      71,
      "the artboard draws 66 glyphs, plus the four the app bar needs, plus the keyboard the \
       Shortcuts chip needs"
    );
    let mut names: Vec<&str> = Vec::new();
    for (index, &icon) in Icon::ALL.iter().enumerate() {
      assert_eq!(
        icon as usize,
        index,
        "{} is out of the artboard's order",
        icon.name()
      );
      assert_eq!(
        Icon::from_name(icon.name()),
        Some(icon),
        "{} does not round-trip through from_name",
        icon.name()
      );
      names.push(icon.name());
    }
    names.sort_unstable();
    let unique = names.len();
    names.dedup();
    assert_eq!(names.len(), unique, "two glyphs share a name");
    assert_eq!(Icon::from_name("no-such-glyph"), None);
  }

  #[test]
  fn one_glyph_uses_one_colour() {
    // The washes and `opacity` values the artboard actually uses. A glyph painting anything else has
    // invented a shade, which is a theme decision this module does not get to make.
    let fades = [0.22, 0.35, 0.4, 0.45, 0.5, 0.55, 0.6, 0.85, 1.0];
    let allowed: Vec<Color32> = fades
      .iter()
      .map(|fade| tint().gamma_multiply(*fade))
      .collect();
    for &icon in Icon::ALL {
      for shape in shapes(box_rect(), icon, tint(), crate::theme::ICON_STROKE) {
        for colour in painted_colours(&shape) {
          if colour == Color32::TRANSPARENT {
            continue;
          }
          assert!(
            allowed.contains(&colour),
            "{} paints {colour:?}, which is the tint at none of {:?}",
            icon.name(),
            fades
          );
        }
      }
    }
  }

  #[test]
  fn curves_and_arcs_are_flattened_finely() {
    // The tolerance is the one thing a walk over the shapes cannot otherwise see: a ring sampled at
    // three points still sits inside the safe box and still paints the tint. It is also the bug this
    // test was written for — a sagitta formula that asked `acos` of the wrong term sampled every
    // ring three times and drew every circle as a vertical line.
    let mut ring = Vec::new();
    flatten_ellipse((12.0, 12.0), 8.0, 8.0, &mut ring);
    assert!(
      ring.len() >= 24,
      "an 8 unit circle came out as {} points",
      ring.len()
    );
    for pair in ring.windows(2) {
      let mid = ((pair[0].0 + pair[1].0) * 0.5, (pair[0].1 + pair[1].1) * 0.5);
      let sagitta = 8.0 - ((mid.0 - 12.0).powi(2) + (mid.1 - 12.0).powi(2)).sqrt();
      assert!(
        sagitta <= TOLERANCE + 1e-4,
        "a chord sags {sagitta:.4} box units, past the {TOLERANCE} tolerance"
      );
    }

    let mut s_curve = Vec::new();
    flatten_curve(
      (3.0, 16.0),
      (7.0, 8.0),
      (9.0, 18.0),
      (12.0, 13.5),
      0,
      &mut s_curve,
    );
    assert!(
      s_curve.len() >= 8,
      "an S-curve came out as {} points",
      s_curve.len()
    );
    assert_eq!(*s_curve.last().unwrap_or(&(0.0, 0.0)), (12.0, 13.5));

    let mut straight = Vec::new();
    flatten_curve(
      (0.0, 0.0),
      (1.0, 0.0),
      (2.0, 0.0),
      (3.0, 0.0),
      0,
      &mut straight,
    );
    assert_eq!(
      straight,
      vec![(3.0, 0.0)],
      "a curve already on its chord costs one segment"
    );
  }

  #[test]
  fn the_stroke_follows_the_token() {
    let rect = box_rect();
    for stroke_px in [crate::theme::ICON_STROKE, 2.0 * crate::theme::ICON_STROKE] {
      for &icon in Icon::ALL {
        for shape in shapes(rect, icon, tint(), stroke_px) {
          for width in stroke_widths(&shape) {
            assert!(
              (width - stroke_px).abs() < 1e-5,
              "{} paints a {width:.3} px stroke where {stroke_px:.3} was asked for",
              icon.name()
            );
          }
        }
      }
    }
  }
} // A family appears in one contiguous run, so the sheet prints each heading once.
