//! Import of a fabrication folder as a PCB board.
//!
//! Scans a directory for fabrication artefacts (Gerber copper/mask/silk/outline layers and Excellon
//! drill files), detects each file's board role by name and by content, parses it with the
//! corresponding reader, and assembles a [`cam_model::Document`] whose layers carry the correct
//! [`cam_model::LayerRole`] and [`cam_model::Side`].

use std::path::{Path, PathBuf};

use cam_geom::{primitive, Bounds, Contour, Point2, Vec2};
use cam_model::{Board, Document, Entity, EntityKind, Layer, LayerRole, Provenance, Side};
use cam_units::Length;

use crate::{ImportError, PLACEMENT_MARGIN};

/// The role and side assigned to one fabrication file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardRole {
  /// The layer role.
  pub role: LayerRole,
  /// The board side.
  pub side: Side,
}

impl BoardRole {
  const fn new(role: LayerRole, side: Side) -> Self {
    Self { role, side }
  }
}

/// One file grouped into the fabrication set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupedFile {
  /// Absolute or original path to the file.
  pub path: PathBuf,
  /// File name as seen in the directory.
  pub file_name: String,
  /// Detected board role, if any.
  pub role: Option<BoardRole>,
  /// How the role was decided.
  pub detection: Detection,
}

/// How a file's role was decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detection {
  /// Decided from the file name and/or extension.
  Name,
  /// Decided from the file's content (attributes/signature).
  Content,
  /// Name and content agreed or content confirmed the name.
  NameAndContent,
  /// No fabrication role could be assigned.
  Unrecognized,
}

/// The result of grouping and importing a fabrication folder.
#[derive(Debug, Clone, PartialEq)]
pub struct FolderImport {
  /// The assembled PCB document.
  pub document: Document,
  /// Files that were recognized and imported, in directory order.
  pub imported: Vec<GroupedFile>,
  /// Files present but not assigned to a board role.
  pub unrecognized: Vec<PathBuf>,
  /// Non-fatal issues encountered while importing recognized files.
  pub warnings: Vec<String>,
}

/// Scan `dir` and import it as a PCB board.
///
/// Recognized fabrication files are grouped by role (`top`/`bottom`/`mask`/`silk`/`outline`/`drill`)
/// using file name and content, parsed (Gerber or Excellon), and assembled into a [`Document`] with
/// correct [`LayerRole`] and [`Side`]. Board dimensions are derived from the outline when present,
/// otherwise from the bounds of all imported geometry.
///
/// # Errors
///
/// Returns [`ImportError::Io`] when the directory cannot be read, and [`ImportError::NothingImported`]
/// when no fabrication layer could be produced.
pub fn import_folder(
  dir: &Path,
  options: &crate::ImportOptions,
) -> Result<FolderImport, ImportError> {
  let entries = read_dir_sorted(dir)?;
  let mut imported: Vec<GroupedFile> = Vec::new();
  let mut unrecognized: Vec<PathBuf> = Vec::new();
  let mut warnings: Vec<String> = Vec::new();
  let mut pending: Vec<(GroupedFile, ParsedLayer)> = Vec::new();

  for path in entries {
    if !path.is_file() {
      continue;
    }
    let file_name = path
      .file_name()
      .and_then(|n| n.to_str())
      .unwrap_or("")
      .to_string();
    let bytes = match std::fs::read(&path) {
      Ok(bytes) => bytes,
      Err(error) => {
        warnings.push(format!("{}: could not be read: {error}", path.display()));
        unrecognized.push(path);
        continue;
      }
    };
    let (role, detection) = detect_role(&file_name, &bytes);
    let grouped = GroupedFile {
      path: path.clone(),
      file_name: file_name.clone(),
      role,
      detection,
    };
    let Some(role) = role else {
      unrecognized.push(path);
      continue;
    };

    match parse_layer(&path, &bytes, role, options.tolerance) {
      Ok(Some(parsed)) => {
        pending.push((grouped.clone(), parsed));
        imported.push(grouped);
      }
      Ok(None) => {
        warnings.push(format!(
          "{}: recognized as {:?}/{:?} but produced no geometry",
          path.display(),
          role.role,
          role.side
        ));
        imported.push(grouped);
      }
      Err(message) => {
        warnings.push(format!("{}: {message}", path.display()));
      }
    }
  }

  if pending.is_empty() {
    return Err(ImportError::NothingImported {
      format: "folder",
      reason: "no fabrication layer could be imported from this folder".to_string(),
    });
  }

  let document = assemble_document(&pending, options.tolerance)?;

  Ok(FolderImport {
    document,
    imported,
    unrecognized,
    warnings,
  })
}

#[derive(Debug, Clone)]
struct ParsedLayer {
  role: BoardRole,
  contours: Vec<Contour>,
  provenance: Provenance,
}

fn read_dir_sorted(dir: &Path) -> Result<Vec<PathBuf>, ImportError> {
  let read = std::fs::read_dir(dir).map_err(|error| ImportError::Io {
    path: dir.display().to_string(),
    message: error.to_string(),
  })?;
  let mut paths: Vec<PathBuf> = Vec::new();
  for entry in read {
    let entry = entry.map_err(|error| ImportError::Io {
      path: dir.display().to_string(),
      message: error.to_string(),
    })?;
    paths.push(entry.path());
  }
  paths.sort_by(|a, b| {
    a.file_name()
      .unwrap_or_default()
      .cmp(b.file_name().unwrap_or_default())
  });
  Ok(paths)
}

/// Detect a file's board role from its name and content.
pub fn detect_role(file_name: &str, bytes: &[u8]) -> (Option<BoardRole>, Detection) {
  let name_role = detect_role_by_name(file_name);
  let content_role = detect_role_by_content(bytes);

  match (name_role, content_role) {
    (Some(n), Some(c)) if n == c => (Some(n), Detection::NameAndContent),
    (Some(n), Some(_)) => (Some(n), Detection::Name),
    (Some(n), None) => (Some(n), Detection::Name),
    (None, Some(c)) => (Some(c), Detection::Content),
    (None, None) => (None, Detection::Unrecognized),
  }
}

fn detect_role_by_name(file_name: &str) -> Option<BoardRole> {
  let lower = file_name.to_ascii_lowercase();
  let stem = Path::new(&lower)
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or(&lower);
  let ext = Path::new(&lower)
    .extension()
    .and_then(|s| s.to_str())
    .unwrap_or("");

  // More specific masks/silks before generic top/bottom copper.
  if is_mask_top(stem, ext) {
    return Some(BoardRole::new(LayerRole::Mask, Side::Top));
  }
  if is_mask_bottom(stem, ext) {
    return Some(BoardRole::new(LayerRole::Mask, Side::Bottom));
  }
  if is_silk_top(stem, ext) {
    return Some(BoardRole::new(LayerRole::Silk, Side::Top));
  }
  if is_silk_bottom(stem, ext) {
    return Some(BoardRole::new(LayerRole::Silk, Side::Bottom));
  }
  if is_outline(stem, ext) {
    return Some(BoardRole::new(LayerRole::Outline, Side::Both));
  }
  if is_drill(stem, ext) {
    return Some(BoardRole::new(LayerRole::Drill, Side::Both));
  }
  if is_copper_top(stem, ext) {
    return Some(BoardRole::new(LayerRole::CopperTop, Side::Top));
  }
  if is_copper_bottom(stem, ext) {
    return Some(BoardRole::new(LayerRole::CopperBottom, Side::Bottom));
  }

  None
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
  needles.iter().any(|n| haystack.contains(n))
}

fn is_mask_top(stem: &str, ext: &str) -> bool {
  ext == "gts"
    || ext == "stc"
    || contains_any(
      stem,
      &[
        "f_mask",
        "f-mask",
        "f.mask",
        "mask_top",
        "mask-top",
        "top_mask",
        "top-mask",
        "soldermask_top",
        "soldermask-top",
        "soldermask.top",
        "masktop",
        "topmask",
        "tmask",
        "sm_top",
      ],
    )
    || stem == "mask-top"
    || stem == "mask_top"
}

fn is_mask_bottom(stem: &str, ext: &str) -> bool {
  ext == "gbs"
    || ext == "sts"
    || contains_any(
      stem,
      &[
        "b_mask",
        "b-mask",
        "b.mask",
        "mask_bottom",
        "mask-bottom",
        "bottom_mask",
        "bottom-mask",
        "soldermask_bottom",
        "soldermask-bottom",
        "soldermask.bottom",
        "maskbottom",
        "bottommask",
        "bmask",
        "sm_bottom",
      ],
    )
    || stem == "mask-bottom"
    || stem == "mask_bottom"
}

fn is_silk_top(stem: &str, ext: &str) -> bool {
  ext == "gto"
    || ext == "plc"
    || contains_any(
      stem,
      &[
        "f_silks",
        "f-silks",
        "f.silks",
        "f_silk",
        "f-silk",
        "f.silk",
        "silk_top",
        "silk-top",
        "top_silk",
        "top-silk",
        "legend_top",
        "legend-top",
        "silkscreen_top",
        "silkscreen-top",
        "f_silkscreen",
        "f-silkscreen",
        "silktop",
        "topsilk",
      ],
    )
    || stem == "silk-top"
    || stem == "silk_top"
}

fn is_silk_bottom(stem: &str, ext: &str) -> bool {
  ext == "gbo"
    || ext == "pls"
    || contains_any(
      stem,
      &[
        "b_silks",
        "b-silks",
        "b.silks",
        "b_silk",
        "b-silk",
        "b.silk",
        "silk_bottom",
        "silk-bottom",
        "bottom_silk",
        "bottom-silk",
        "legend_bottom",
        "legend-bottom",
        "silkscreen_bottom",
        "silkscreen-bottom",
        "b_silkscreen",
        "b-silkscreen",
        "silkbottom",
        "bottomsilk",
      ],
    )
    || stem == "silk-bottom"
    || stem == "silk_bottom"
}

fn is_outline(stem: &str, ext: &str) -> bool {
  ext == "gm1"
    || ext == "gko"
    || ext == "gml"
    || ext == "outline"
    || contains_any(
      stem,
      &[
        "edge_cuts",
        "edge-cuts",
        "edgecuts",
        "edge.cut",
        "outline",
        "boardoutline",
        "board_outline",
        "board-outline",
        "profile",
        "board_edge",
        "board-edge",
        "boardedge",
      ],
    )
}

fn is_drill(stem: &str, ext: &str) -> bool {
  ext == "drl"
    || ext == "xln"
    || ext == "exc"
    || contains_any(stem, &["drill", "pth", "npth", "hole", "holes"])
}

fn is_copper_top(stem: &str, ext: &str) -> bool {
  ext == "gtl"
    || ext == "cmp"
    || contains_any(
      stem,
      &[
        "f_cu",
        "f-cu",
        "f.cu",
        "top_copper",
        "top-copper",
        "top.copper",
        "copper_top",
        "copper-top",
        "front_copper",
        "front-copper",
      ],
    )
    || stem == "top"
    || stem == "cu_top"
}

fn is_copper_bottom(stem: &str, ext: &str) -> bool {
  ext == "gbl"
    || ext == "sol"
    || contains_any(
      stem,
      &[
        "b_cu",
        "b-cu",
        "b.cu",
        "bottom_copper",
        "bottom-copper",
        "bottom.copper",
        "copper_bottom",
        "copper-bottom",
        "back_copper",
        "back-copper",
      ],
    )
    || stem == "bottom"
    || stem == "cu_bottom"
}

fn detect_role_by_content(bytes: &[u8]) -> Option<BoardRole> {
  let head = &bytes[..bytes.len().min(65_536)];
  let text = String::from_utf8_lossy(head);

  // Excellon signature
  let trimmed = text.trim_start();
  if trimmed.starts_with("M48") || text.contains("\nM48") || text.contains("\rM48") {
    // could be drill; confirm with common markers
    if text.contains("METRIC") || text.contains("INCH") || text.contains('T') && text.contains('C')
    {
      return Some(BoardRole::new(LayerRole::Drill, Side::Both));
    }
  }

  // Gerber X2 / TF attributes
  for line in text.lines() {
    let line = line.trim();
    if let Some(inner) = line
      .strip_prefix("%TF.FileFunction,")
      .or_else(|| line.strip_prefix("%TF.FileFunction,"))
      .and_then(|s| s.strip_suffix("*%").or_else(|| s.strip_suffix("%")))
    {
      return file_function_role(inner);
    }
    // Also try to find FileFunction inside attribute blocks
    if let Some(pos) = line.find("FileFunction,") {
      let rest = &line[pos + "FileFunction,".len()..];
      let end = rest
        .find("*%")
        .or_else(|| rest.find('%'))
        .unwrap_or(rest.len());
      if let Some(role) = file_function_role(&rest[..end]) {
        return Some(role);
      }
    }
  }

  None
}

fn file_function_role(value: &str) -> Option<BoardRole> {
  let v = value.to_ascii_lowercase();
  if v.starts_with("profile") {
    return Some(BoardRole::new(LayerRole::Outline, Side::Both));
  }
  if v.starts_with("copper") {
    if v.contains("top") || v.contains("l1") {
      return Some(BoardRole::new(LayerRole::CopperTop, Side::Top));
    }
    if v.contains("inr") || v.contains("inner") {
      return None;
    }
    if v.contains("bot") || v.contains("l2") {
      return Some(BoardRole::new(LayerRole::CopperBottom, Side::Bottom));
    }
  }
  if v.starts_with("soldermask") {
    if v.contains("top") {
      return Some(BoardRole::new(LayerRole::Mask, Side::Top));
    }
    if v.contains("bot") {
      return Some(BoardRole::new(LayerRole::Mask, Side::Bottom));
    }
  }
  if v.starts_with("legend") || v.starts_with("silkscreen") {
    if v.contains("top") {
      return Some(BoardRole::new(LayerRole::Silk, Side::Top));
    }
    if v.contains("bot") {
      return Some(BoardRole::new(LayerRole::Silk, Side::Bottom));
    }
  }
  if v.starts_with("plated") || v.starts_with("nonplated") || v.contains("drill") {
    return Some(BoardRole::new(LayerRole::Drill, Side::Both));
  }
  None
}

fn parse_layer(
  path: &Path,
  bytes: &[u8],
  role: BoardRole,
  tolerance: f64,
) -> Result<Option<ParsedLayer>, String> {
  let text = String::from_utf8_lossy(bytes);
  let ext = path
    .extension()
    .and_then(|s| s.to_str())
    .map(|s| s.to_ascii_lowercase())
    .unwrap_or_default();

  let is_drill =
    matches!(role.role, LayerRole::Drill) || matches!(ext.as_str(), "drl" | "xln" | "exc");

  let contours = if is_drill {
    parse_drill(&text)?
  } else {
    parse_gerber(&text, tolerance)?
  };

  if contours.is_empty() {
    return Ok(None);
  }

  let provenance = Provenance {
    file: Some(path.display().to_string()),
    format: Some(if is_drill { "excellon" } else { "gerber" }.to_string()),
    ..Provenance::default()
  };

  Ok(Some(ParsedLayer {
    role,
    contours,
    provenance,
  }))
}

fn parse_gerber(text: &str, tolerance: f64) -> Result<Vec<Contour>, String> {
  let gerber = cam_gerber::parse(text).map_err(|e| e.to_string())?;
  let region = gerber.image(tolerance).map_err(|e| e.to_string())?;
  Ok(region.boundaries)
}

fn parse_drill(text: &str) -> Result<Vec<Contour>, String> {
  let excellon = cam_excellon::parse(text).map_err(|e| e.to_string())?;
  let mut contours = Vec::new();
  let tools = excellon.tools;
  for feature in &excellon.features {
    match *feature {
      cam_excellon::Feature::Hole { tool, at } => {
        let diameter = tool_diameter(&tools, tool);
        let radius = (diameter / 2.0).max(0.001);
        let contour = primitive::circle(at, radius).map_err(|e| e.to_string())?;
        contours.push(contour);
      }
      cam_excellon::Feature::Slot { tool, from, to } => {
        let diameter = tool_diameter(&tools, tool);
        let radius = (diameter / 2.0).max(0.001);
        if let Some(contour) = slot_contour(from, to, radius) {
          contours.push(contour);
        } else {
          // Degenerate slot: fall back to a hole at midpoint
          let mid = Point2::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
          let contour = primitive::circle(mid, radius).map_err(|e| e.to_string())?;
          contours.push(contour);
        }
      }
    }
  }
  Ok(contours)
}

fn tool_diameter(tools: &[cam_excellon::Tool], number: u32) -> f64 {
  tools
    .iter()
    .find(|t| t.number == number)
    .map(|t| t.diameter.mm())
    .unwrap_or(1.0)
}

fn slot_contour(from: Point2, to: Point2, radius: f64) -> Option<Contour> {
  let dx = to.x - from.x;
  let dy = to.y - from.y;
  let len = (dx * dx + dy * dy).sqrt();
  if len < 1e-9 {
    return None;
  }
  let ux = dx / len;
  let uy = dy / len;
  // perpendicular to (ux,uy): (-uy, ux)
  let px = -uy * radius;
  let py = ux * radius;
  // CCW stadium: from+perp, to+perp (line), semicircle around to to to-perp, to-perp, from-perp (line), semicircle around from
  let a = Point2::new(from.x + px, from.y + py);
  let b = Point2::new(to.x + px, to.y + py);
  let c = Point2::new(to.x - px, to.y - py);
  let d = Point2::new(from.x - px, from.y - py);
  // semicircle bulge = 1.0
  Contour::new(
    vec![
      cam_geom::Vertex::corner(a),
      cam_geom::Vertex::new(b, 1.0),
      cam_geom::Vertex::corner(c),
      cam_geom::Vertex::new(d, 1.0),
    ],
    true,
  )
  .ok()
}

fn assemble_document(
  layers: &[(GroupedFile, ParsedLayer)],
  _tolerance: f64,
) -> Result<Document, ImportError> {
  // Compute overall bounds and outline bounds
  let mut all_bounds = Bounds::empty();
  let mut outline_bounds = Bounds::empty();
  for (_, parsed) in layers {
    for contour in &parsed.contours {
      let b = contour.bounds();
      all_bounds = all_bounds.union(b);
      if matches!(parsed.role.role, LayerRole::Outline) {
        outline_bounds = outline_bounds.union(b);
      }
    }
  }
  let board_bounds = if outline_bounds.is_empty() {
    all_bounds
  } else {
    outline_bounds
  };
  if board_bounds.is_empty() {
    return Err(ImportError::NothingImported {
      format: "folder",
      reason: "imported layers have no bounds".to_string(),
    });
  }

  let offset = Vec2::new(
    PLACEMENT_MARGIN - board_bounds.min.x,
    PLACEMENT_MARGIN - board_bounds.min.y,
  );

  let mut board = Board::default_board();
  board.width = Length::from_mm(board_bounds.width() + 2.0 * PLACEMENT_MARGIN);
  board.height = Length::from_mm(board_bounds.height() + 2.0 * PLACEMENT_MARGIN);

  let mut document = Document::new(board);
  let mut entity_counter: usize = 1;

  for (grouped, parsed) in layers {
    let key = (parsed.role.role, parsed.role.side);
    let li = if let Some((idx, _)) = document
      .layers
      .iter()
      .enumerate()
      .find(|(_, l)| l.role == key.0 && l.side == key.1)
    {
      idx
    } else {
      let id = default_layer_id(parsed.role.role, parsed.role.side);
      let layer = Layer {
        id,
        role: parsed.role.role,
        side: parsed.role.side,
        color: default_color(parsed.role.role, parsed.role.side),
        visible: true,
        entities: Vec::new(),
        source: parsed.provenance.clone(),
      };
      document.layers.push(layer);
      document.layers.len() - 1
    };

    let _ = grouped;
    for contour in &parsed.contours {
      let contour = cam_geom::transform::translate(contour, offset)
        .map_err(|e| ImportError::parse_plain("folder", e.to_string()))?;
      document.layers[li].entities.push(Entity {
        id: format!("E{entity_counter}"),
        kind: EntityKind::Contour { contour },
      });
      entity_counter += 1;
    }
  }

  document.layers.retain(|l| !l.entities.is_empty());

  document
    .validate()
    .map_err(|e| ImportError::parse_plain("folder", e.to_string()))?;

  Ok(document)
}

fn default_layer_id(role: LayerRole, side: Side) -> String {
  match (role, side) {
    (LayerRole::CopperTop, _) => "copper_top".to_string(),
    (LayerRole::CopperBottom, _) => "copper_bottom".to_string(),
    (LayerRole::Mask, Side::Top) => "mask_top".to_string(),
    (LayerRole::Mask, Side::Bottom) => "mask_bottom".to_string(),
    (LayerRole::Mask, _) => "mask".to_string(),
    (LayerRole::Silk, Side::Top) => "silk_top".to_string(),
    (LayerRole::Silk, Side::Bottom) => "silk_bottom".to_string(),
    (LayerRole::Silk, _) => "silk".to_string(),
    (LayerRole::Outline, _) => "outline".to_string(),
    (LayerRole::Drill, _) => "drill".to_string(),
    (LayerRole::Keepout, _) => "keepout".to_string(),
  }
}

fn default_color(role: LayerRole, side: Side) -> String {
  match (role, side) {
    (LayerRole::CopperTop, _) => "#c87533".to_string(),
    (LayerRole::CopperBottom, _) => "#3375c8".to_string(),
    (LayerRole::Mask, Side::Top) => "#2e8b57".to_string(),
    (LayerRole::Mask, Side::Bottom) => "#1e6b47".to_string(),
    (LayerRole::Mask, _) => "#2e8b57".to_string(),
    (LayerRole::Silk, Side::Top) => "#f0f0f0".to_string(),
    (LayerRole::Silk, Side::Bottom) => "#d0d0d0".to_string(),
    (LayerRole::Silk, _) => "#f0f0f0".to_string(),
    (LayerRole::Outline, _) => "#ffff00".to_string(),
    (LayerRole::Drill, _) => "#808080".to_string(),
    (LayerRole::Keepout, _) => "#ff00ff".to_string(),
  }
}
