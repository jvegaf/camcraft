//! Inlay: the male/female pair one command means (M6 A6, D-M6-7).
//!
//! An inlay is not a new cutting law. Both halves are **pockets** — the same ring machinery, the same
//! depth chunking, the same conservative linking between rings (D-M4-6) — and what separates them is
//! *what* they clear:
//!
//! - the **female** clears the selected region **grown by the allowance**: the cavity, `allowance`
//!   millimetres bigger than the region on every side;
//! - the **male** clears the job rectangle with the region **standing as an island**: the plug left
//!   behind is the region itself, so it drops into a cavity that is `allowance` bigger with exactly that
//!   clearance.
//!
//! Both halves therefore reuse [`crate::pocket`] instead of re-deriving a clearing pattern (D-M6-7), and
//! the job rectangle comes from [`cam_model::Job::outline`] — the one place it is built — so the male's
//! boundary in the program is the same rectangle `cam-sim` authorises. A second copy of the rectangle
//! here would be a second answer to "where does the stock end".
//!
//! The female's growth is expressed as the pocket's own `allowance` parameter *negated*: that parameter
//! is how far the tool centre keeps back from the boundary, so a negative one puts the tool the cavity's
//! oversize *past* the region and clears it — the same rings, one sign apart, instead of a grown contour
//! the kernel would have to normalize a second time. At `allowance = 0` the female is the plain pocket
//! of the region, cell for cell.

use cam_geom::region::Region;
use cam_geom::{Contour, DEFAULT_TOLERANCE};
use cam_model::{InlayParams, Job, PocketParams, PocketStrategy};
use cam_move::Move;
use cam_units::Length;

use crate::{pocket, PocketRequest, ToolSpec, ToolpathError};

/// Everything an inlay pair needs.
#[derive(Clone, Copy, Debug)]
pub struct InlayRequest<'a> {
  /// The closed boundaries of the region the inlay is cut from.
  ///
  /// One region, the way a texture's selection is one region (`TextureParams::grooves`): the kernel's
  /// parity rule makes an appended boundary a hole, so a region with an island in it needs no island
  /// concept here either (D-RB-2).
  pub contours: &'a [&'a Contour],
  /// The allowance, the stepover and the depths both halves share.
  pub params: &'a InlayParams,
  /// The tool doing the cutting, the same one for both halves.
  pub tool: &'a ToolSpec,
  /// Job setup: the male's boundary ([`Job::outline`]) and the machine coordinate mapping.
  pub job: &'a Job,
}

/// The two programs one inlay means, in the order a command appends them.
///
/// The order is fixed because it is the strategy's own statement: the cavity first, the plug that fits
/// it second (`cam-cli toolpath --kind inlay` appends them in exactly this order, ids `TPn` then
/// `TPn+1`).
#[derive(Clone, Debug, PartialEq)]
pub struct Inlaypath {
  /// The cavity: the region grown by the allowance.
  pub female: Vec<Move>,
  /// The plug: the job rectangle cleared around the region.
  pub male: Vec<Move>,
}

/// Compute both halves of an inlay, female first.
///
/// Refuses — all of them by name, before any move is produced — an open contour
/// ([`ToolpathError::OpenContour`]), an allowance whose shrink collapses the region
/// ([`ToolpathError::InlayAllowanceCollapsed`]) and a region that does not lie strictly inside the stock
/// ([`ToolpathError::InlayRegionOutsideJob`]), which is the male with nothing left to clear around it.
/// The pocket's own refusals (a stepover with no distance behind it, a depth range with no pass in it, a
/// tool that does not fit) come through unchanged from [`crate::pocket`], which either half calls.
pub fn inlay(request: &InlayRequest<'_>) -> Result<Inlaypath, ToolpathError> {
  Ok(Inlaypath {
    female: female(request)?,
    male: male(request)?,
  })
}

/// Compute the female half alone: the region grown by the allowance, cleared as a pocket.
///
/// `cam-cli` builds one program segment per document toolpath, so it asks for one half at a time; a
/// caller that wants the pair asks [`inlay`], which is these two calls in the fixed order and nothing
/// else.
pub fn female(request: &InlayRequest<'_>) -> Result<Vec<Move>, ToolpathError> {
  preflight(request)?;
  // The cavity reaches `allowance` millimetres past the region, which is a pocket whose tool centre
  // keeps `-allowance` back from it. A zero allowance is exactly the plain pocket (M5's rule for the
  // pocket's own allowance: zero must not perturb a single cell).
  let params = pocket_params(
    request.params,
    Length::from_mm(-request.params.allowance.mm()),
    // Rings follow the wall a plug has to fit, which is what a cavity's wall is for.
    PocketStrategy::Offset,
  );
  let Some((boundary, islands)) = request.contours.split_first() else {
    return Err(ToolpathError::OpenContour);
  };
  pocket(&PocketRequest {
    contour: boundary,
    islands,
    params: &params,
    tool: request.tool,
    job: request.job,
  })
  .map(|path| path.moves)
}

/// Compute the male half alone: the job rectangle with the region standing as an island.
///
/// The boundary is [`Job::outline`], never a rectangle built here, and the region is the island list, so
/// the material left standing is the region itself — the plug, one allowance smaller than the cavity the
/// female cuts. The male clears with the pocket's **raster** pattern rather than its rings, and the
/// reason is the shape it clears: the material around a region is a *frame*, and the ring lattice
/// converges on the frame's medial axis and stops. Measured on a 40 mm region at (20, 20) in a 100 mm
/// stock, with a 6 mm tool at a 4 mm stepover: the rings reach 10 mm out from the stock's edge and 6 mm
/// in from the region's, and **2808 of the frame's 8400 mm² are never cut** — the four diagonal corners
/// between the last two rings, which is where a plug would still be attached to the stock. The row
/// lattice has no such blind spot: its rows are clipped to the frame by the kernel's own raster spans,
/// so every row reaches both walls and the material beside the plug is cleared.
pub fn male(request: &InlayRequest<'_>) -> Result<Vec<Move>, ToolpathError> {
  preflight(request)?;
  let params = pocket_params(request.params, Length::ZERO, PocketStrategy::Raster);
  let outline = request.job.outline();
  pocket(&PocketRequest {
    contour: &outline,
    islands: request.contours,
    params: &params,
    tool: request.tool,
    job: request.job,
  })
  .map(|path| path.moves)
}

/// The pocket one half cuts, from the pair's one copy of the numbers.
///
/// `allowance` here is the *pocket's* parameter — the material its tool centre keeps back from the
/// boundary — and not the inlay's: the female passes it negated so its tool reaches the cavity's
/// oversize, and the male passes zero because it leaves the region standing instead of keeping a wall.
/// The `strategy` is the half's own (see [`female`] and [`male`]): the pair shares its numbers, never its
/// pattern, because the two halves clear two different shapes.
fn pocket_params(
  params: &InlayParams,
  allowance: Length,
  strategy: PocketStrategy,
) -> PocketParams {
  PocketParams {
    strategy,
    allowance,
    stepover: params.stepover,
    pass_depth: params.pass_depth,
    start_depth: params.start_depth,
    cut_depth: params.cut_depth,
  }
}

/// Everything both halves refuse, asked once per half.
///
/// The order matters for the message a user sees: the geometry first (an open vector is not a region at
/// all), then the allowance, then the stock. Each refusal names its own value, so a spelled-out number is
/// never quietly reinterpreted.
fn preflight(request: &InlayRequest<'_>) -> Result<(), ToolpathError> {
  let contours = request.contours;
  if contours.is_empty() || contours.iter().any(|contour| !contour.is_closed()) {
    return Err(ToolpathError::OpenContour);
  }
  let allowance = request.params.allowance.mm();
  if allowance < 0.0 {
    // A negative allowance is an interference fit: the cavity is *smaller* than the plug. That is a
    // legitimate request, but a large enough one shaves the region until there is nothing to cut, and
    // only the kernel can say whether it survived. Asked here so the refusal names the allowance instead
    // of surfacing as a pocket that "does not fit".
    let region = Region::new(contours.iter().map(|contour| (*contour).clone()).collect()).map_err(
      |error| ToolpathError::InlayAllowanceCollapsed {
        allowance,
        reason: error.to_string(),
      },
    )?;
    region
      .offset(-allowance, DEFAULT_TOLERANCE)
      .map_err(|error| ToolpathError::InlayAllowanceCollapsed {
        allowance,
        reason: error.to_string(),
      })?;
  }
  // The male clears the stock around the region, so the region has to lie strictly inside it: a region
  // that reaches or crosses the stock's edge has no material beside it to clear, and the pair would come
  // out with a plug whose wall is the stock's own saw cut.
  let outline = request.job.outline();
  for contour in contours {
    if !matches!(
      cam_geom::winding::relation(contour, &outline, DEFAULT_TOLERANCE),
      cam_geom::winding::Relation::FirstInside
    ) {
      return Err(ToolpathError::InlayRegionOutsideJob {
        width: request.job.width.mm(),
        height: request.job.height.mm(),
      });
    }
  }
  Ok(())
}
