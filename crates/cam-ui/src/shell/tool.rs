use cam_app::Command;
use cam_geom::Point2;

/// The tool armed on the canvas.
///
/// [`Tool::Select`] keeps M1's behaviour: a click picks, a drag marquees. Every other tool turns a
/// primary press-drag-release into exactly one drawing command, and is therefore also the shell's
/// "active tool" state: there is no modal command loop, only which gesture the canvas interprets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
  /// Click to pick, drag to marquee. Draws nothing.
  Select,
  /// Drag a line from the press point to the release point.
  Line,
  /// Drag two opposite corners of a rectangle.
  Rectangle,
  /// Drag one straight segment, stored as an open polyline.
  Polyline,
  /// Drag a chord; the arc passes through the gesture box corner `(start.x, end.y)`.
  Arc,
  /// Drag the bounding box of a circle.
  Circle,
  /// Drag the bounding box of an ellipse.
  Ellipse,
  /// Drag the bounding box of a regular polygon.
  Polygon,
  /// Drag the bounding box of a star.
  Star,
}

impl Tool {
  /// Every tool, in the order the tool list shows them.
  pub const ALL: [Tool; 9] = [
    Tool::Select,
    Tool::Line,
    Tool::Rectangle,
    Tool::Polyline,
    Tool::Arc,
    Tool::Circle,
    Tool::Ellipse,
    Tool::Polygon,
    Tool::Star,
  ];

  /// The label on the tool button.
  pub fn label(self) -> &'static str {
    match self {
      Self::Select => "Select",
      Self::Line => "Line",
      Self::Rectangle => "Rectangle",
      Self::Polyline => "Polyline",
      Self::Arc => "Arc",
      Self::Circle => "Circle",
      Self::Ellipse => "Ellipse",
      Self::Polygon => "Polygon",
      Self::Star => "Star",
    }
  }

  /// Whether this tool draws on a canvas drag instead of selecting.
  pub fn draws(self) -> bool {
    self != Tool::Select
  }
}

/// One drawing command, with every number the kernel needs, in world millimetres.
///
/// The shell builds these from a canvas drag, and [`handle_action`] turns one into exactly one
/// [`Command`]. Carrying the geometry in the action is what lets a test drive a drag and then assert
/// on the command it produced without a display, and it is why a shape is committed once, on release,
/// rather than once per mouse move.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawShape {
  /// A rectangle with its lower-left corner at `at`.
  Rectangle {
    /// Lower-left corner, in millimetres.
    at: Point2,
    /// Width in millimetres; it must be positive.
    width: f64,
    /// Height in millimetres; it must be positive.
    height: f64,
  },
  /// A straight line, stored as an open contour.
  Line {
    /// Start point, in millimetres.
    from: Point2,
    /// End point, in millimetres.
    to: Point2,
  },
  /// A chain of straight segments.
  Polyline {
    /// Points to pass through, in millimetres.
    points: Vec<Point2>,
    /// Whether the last point joins the first.
    closed: bool,
  },
  /// A single arc through three ordered points.
  Arc {
    /// Start point, in millimetres.
    start: Point2,
    /// A point the arc passes through, in millimetres.
    through: Point2,
    /// End point, in millimetres.
    end: Point2,
  },
  /// An ellipse approximated by arcs within `tolerance`.
  Ellipse {
    /// Centre, in millimetres.
    at: Point2,
    /// Radius along X, in millimetres.
    rx: f64,
    /// Radius along Y, in millimetres.
    ry: f64,
    /// Largest radial error the caller accepts, in millimetres.
    tolerance: f64,
  },
  /// A circle, stored as two half-turn arcs.
  Circle {
    /// Centre, in millimetres.
    at: Point2,
    /// Radius in millimetres.
    radius: f64,
  },
  /// A regular polygon.
  Polygon {
    /// Centre, in millimetres.
    at: Point2,
    /// Circumscribed radius, in millimetres.
    radius: f64,
    /// Number of sides, at least three.
    sides: usize,
    /// Angle of the first vertex, in radians from the positive X axis.
    rotation: f64,
  },
  /// A star alternating `points` tips and valleys.
  Star {
    /// Centre, in millimetres.
    at: Point2,
    /// Radius of the outer tips, in millimetres.
    outer_radius: f64,
    /// Radius of the inner valleys, in millimetres.
    inner_radius: f64,
    /// Number of tips, at least three.
    points: usize,
    /// Angle of the first tip, in radians from the positive X axis.
    rotation: f64,
  },
}

impl DrawShape {
  /// The status-bar label for this shape.
  pub fn label(&self) -> &'static str {
    match self {
      Self::Rectangle { .. } => "add rectangle",
      Self::Line { .. } => "add line",
      Self::Polyline { .. } => "add polyline",
      Self::Arc { .. } => "add arc",
      Self::Ellipse { .. } => "add ellipse",
      Self::Circle { .. } => "add circle",
      Self::Polygon { .. } => "add polygon",
      Self::Star { .. } => "add star",
    }
  }

  /// The one command this shape becomes on `layer_id` as `entity_id`.
  pub fn to_command(&self, layer_id: &str, entity_id: &str) -> Command {
    let layer_id = layer_id.to_owned();
    let entity_id = entity_id.to_owned();
    match self {
      Self::Rectangle { at, width, height } => Command::AddRectangle {
        layer_id,
        entity_id,
        at: *at,
        width: *width,
        height: *height,
      },
      Self::Line { from, to } => Command::AddLine {
        layer_id,
        entity_id,
        from: *from,
        to: *to,
      },
      Self::Polyline { points, closed } => Command::AddPolyline {
        layer_id,
        entity_id,
        points: points.clone(),
        closed: *closed,
      },
      Self::Arc {
        start,
        through,
        end,
      } => Command::AddArc {
        layer_id,
        entity_id,
        start: *start,
        through: *through,
        end: *end,
      },
      Self::Ellipse {
        at,
        rx,
        ry,
        tolerance,
      } => Command::AddEllipse {
        layer_id,
        entity_id,
        at: *at,
        rx: *rx,
        ry: *ry,
        tolerance: *tolerance,
      },
      Self::Circle { at, radius } => Command::AddCircle {
        layer_id,
        entity_id,
        at: *at,
        radius: *radius,
      },
      Self::Polygon {
        at,
        radius,
        sides,
        rotation,
      } => Command::AddPolygon {
        layer_id,
        entity_id,
        at: *at,
        radius: *radius,
        sides: *sides,
        rotation: *rotation,
      },
      Self::Star {
        at,
        outer_radius,
        inner_radius,
        points,
        rotation,
      } => Command::AddStar {
        layer_id,
        entity_id,
        at: *at,
        outer_radius: *outer_radius,
        inner_radius: *inner_radius,
        points: *points,
        rotation: *rotation,
      },
    }
  }
}
