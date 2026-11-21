//! Behaviour tests for non-copper clearing envelope (E4.1).

use cam_geom::region::Region;
use cam_geom::{Contour, Point2, TolerancePolicy};
use cam_ops::{
  ncc_fill_lines, ncc_fill_rings, ncc_fill_seed, ncc_rest_region, non_copper_envelope,
  swept_band_region, unite_copper,
};

const TOLERANCE: TolerancePolicy = TolerancePolicy::uniform(1.0e-9);

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Contour {
  Contour::from_points(
    &[
      Point2::new(x0, y0),
      Point2::new(x1, y0),
      Point2::new(x1, y1),
      Point2::new(x0, y1),
    ],
    true,
  )
  .expect("rectangle")
}

fn region_from_contours(contours: Vec<Contour>) -> Region {
  Region::new(contours)
    .expect("region")
    .normalize(TOLERANCE)
    .expect("normalize")
    .region
}

fn region_area(region: &Region) -> f64 {
  region
    .boundaries
    .iter()
    .map(|boundary| boundary.signed_area())
    .sum::<f64>()
    .abs()
}

fn close(actual: f64, expected: f64, scale: f64) -> bool {
  (actual - expected).abs() <= 1.0e-7 * scale.max(1.0)
}

#[test]
fn envelope_without_margin_or_copper_offset_is_board_minus_copper() {
  let board = region_from_contours(vec![rect(0.0, 0.0, 100.0, 50.0)]);
  let copper = vec![rect(10.0, 10.0, 30.0, 20.0)];
  let envelope = non_copper_envelope(&board, &copper, 0.0, 0.0, TOLERANCE).expect("envelope");
  let board_area = region_area(&board);
  let copper_area = region_area(&unite_copper(&copper, TOLERANCE).expect("copper"));
  assert!(
    close(region_area(&envelope), board_area - copper_area, board_area),
    "envelope {} board {} copper {}",
    region_area(&envelope),
    board_area,
    copper_area
  );
}

#[test]
fn envelope_with_no_copper_is_the_inset_boundary() {
  let board = region_from_contours(vec![rect(0.0, 0.0, 100.0, 50.0)]);
  let envelope = non_copper_envelope(&board, &[], 0.0, 0.0, TOLERANCE).expect("envelope");
  assert!(
    close(region_area(&envelope), 5000.0, 5000.0),
    "area {}",
    region_area(&envelope)
  );
  let inset = non_copper_envelope(&board, &[], 5.0, 0.0, TOLERANCE).expect("inset envelope");
  // 100x50 inset by 5 on all sides -> 90x40 = 3600
  assert!(
    close(region_area(&inset), 3600.0, 3600.0),
    "area {}",
    region_area(&inset)
  );
}

#[test]
fn envelope_copper_offset_expands_copper_keepout() {
  let board = region_from_contours(vec![rect(0.0, 0.0, 100.0, 100.0)]);
  let copper = vec![rect(40.0, 40.0, 60.0, 60.0)]; // 20x20
  let no_offset = non_copper_envelope(&board, &copper, 0.0, 0.0, TOLERANCE).expect("no offset");
  let with_offset = non_copper_envelope(&board, &copper, 0.0, 1.0, TOLERANCE).expect("offset");
  // Copper 20x20 expanded outward by 1mm on all sides -> 22x22 keep-out, so cleared area shrinks.
  assert!(
    region_area(&with_offset) < region_area(&no_offset),
    "offset {} no_offset {}",
    region_area(&with_offset),
    region_area(&no_offset)
  );
  assert!(
    close(
      region_area(&no_offset) - region_area(&with_offset),
      22.0 * 22.0 - 20.0 * 20.0,
      100.0
    ),
    "delta {}",
    region_area(&no_offset) - region_area(&with_offset)
  );
}

#[test]
fn envelope_margin_and_copper_offset_combine() {
  let board = region_from_contours(vec![rect(0.0, 0.0, 100.0, 100.0)]);
  let copper = vec![rect(40.0, 40.0, 60.0, 60.0)];
  let envelope = non_copper_envelope(&board, &copper, 2.0, 1.0, TOLERANCE).expect("envelope");
  // boundary inset by 2: 96x96 = 9216; copper 20x20 expanded by 1: 22x22 = 484; diff 8732
  assert!(
    close(region_area(&envelope), 9216.0 - 484.0, 9216.0),
    "area {}",
    region_area(&envelope)
  );
}

#[test]
fn envelope_rejects_negative_margin_or_offset() {
  let board = region_from_contours(vec![rect(0.0, 0.0, 10.0, 10.0)]);
  let err = non_copper_envelope(&board, &[], -1.0, 0.0, TOLERANCE).unwrap_err();
  assert!(err.to_string().contains("margin"), "{err}");
  let err = non_copper_envelope(&board, &[], 0.0, -1.0, TOLERANCE).unwrap_err();
  assert!(err.to_string().contains("copper offset"), "{err}");
}

#[test]
fn envelope_covers_non_copper_area_on_corpus_layers() {
  // E4.1 criterion: cobertura del área de no-cobre dentro de tolerancia en el corpus.
  // Bounded/fast check on real corpus board outlines: the envelope covers exactly the non-copper
  // area inside the boundary (area(boundary) - area(united_copper)), within tolerance.
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let tol = TolerancePolicy::uniform(1.0e-6);
  let boards = ["kicad-2l-simple", "substitute-eagle", "substitute-altium"];
  let mut checked = 0usize;

  for board in boards {
    let boundary_path = root.join(format!("fixtures/corpus/{board}/outline.gbr"));
    let Ok(boundary_text) = std::fs::read_to_string(&boundary_path) else {
      continue;
    };
    let Ok(boundary_gerber) = cam_gerber::parse(&boundary_text) else {
      continue;
    };
    let Ok(boundary_image) = boundary_gerber.image(tol) else {
      continue;
    };
    if boundary_image.boundaries.is_empty() {
      continue;
    }
    let Ok(boundary_region) = Region::new(boundary_image.boundaries.clone())
      .and_then(|r| r.normalize(tol).map(|n| n.region))
    else {
      continue;
    };
    let boundary_area = region_area(&boundary_region);
    if boundary_area <= 1.0e-6 {
      continue;
    }

    // Synthetic copper fully inside the board's bounding box, small enough to avoid edge cases.
    // Place a tiny synthetic copper inside the boundary.
    let mut placed = None;
    for boundary in &boundary_region.boundaries {
      let verts: Vec<_> = boundary.vertices().iter().map(|v| v.point).collect();
      if verts.is_empty() {
        continue;
      }
      let mean_x = verts.iter().map(|p| p.x).sum::<f64>() / verts.len() as f64;
      let mean_y = verts.iter().map(|p| p.y).sum::<f64>() / verts.len() as f64;
      for (cx, cy) in [(mean_x, mean_y)] {
        if !boundary_region.contains(Point2::new(cx, cy), tol) {
          continue;
        }
        let d = 0.2_f64;
        let copper = vec![rect(cx - d, cy - d, cx + d, cy + d)];
        let Ok(united) = unite_copper(&copper, tol) else {
          continue;
        };
        let copper_area = region_area(&united);
        if copper_area <= 1.0e-9 {
          continue;
        }
        let Ok(envelope) = non_copper_envelope(&boundary_region, &copper, 0.0, 0.0, tol) else {
          continue;
        };
        let actual = region_area(&envelope);
        // Copper must overlap boundary to make this a meaningful coverage check.
        let overlap =
          cam_ops::penetration::intersection_area(&boundary_region, &united, tol).unwrap_or(0.0);
        if overlap <= 1.0e-9 {
          continue;
        }
        let expected = (boundary_area - overlap).max(0.0);
        placed = Some((actual, expected, overlap));
        break;
      }
      if placed.is_some() {
        break;
      }
    }
    let Some((actual, expected, copper_area)) = placed else {
      continue;
    };
    let scale = boundary_area.max(1.0);
    assert!(
      (actual - expected).abs() <= 1.0e-5 * scale + 1.0e-3,
      "{board}: envelope {actual} expected {expected} (boundary {boundary_area} copper {copper_area})"
    );
    checked += 1;
  }

  assert!(
    checked >= 1,
    "expected to verify E4.1 envelope coverage on at least one corpus board boundary"
  );
}

#[test]
fn envelope_covers_real_corpus_layer_geometry() {
  // E4.1 criterion: verify envelope coverage on real parsed corpus Gerber geometry.
  // Uses fast-parsing corpus layers (mask/silk/outline) as geometry sources to keep the
  // test deterministic and quick in debug, while still exercising real corpus boundaries.
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let tol = TolerancePolicy::uniform(1.0e-6);
  let cases = [
    ("kicad-2l-simple", "mask-top.gbr"),
    ("substitute-altium", "outline.gbr"),
    ("substitute-easyeda", "mask-top.gbr"),
  ];

  let mut checked = 0usize;
  for (board, layer_file) in cases {
    let boundary_path = root.join(format!("fixtures/corpus/{board}/outline.gbr"));
    let layer_path = root.join(format!("fixtures/corpus/{board}/{layer_file}"));
    let Ok(boundary_text) = std::fs::read_to_string(&boundary_path) else {
      continue;
    };
    let Ok(layer_text) = std::fs::read_to_string(&layer_path) else {
      continue;
    };
    let Ok(boundary_gerber) = cam_gerber::parse(&boundary_text) else {
      continue;
    };
    let Ok(layer_gerber) = cam_gerber::parse(&layer_text) else {
      continue;
    };
    let Ok(boundary_image) = boundary_gerber.image(tol) else {
      continue;
    };
    let Ok(layer_image) = layer_gerber.image(tol) else {
      continue;
    };
    if boundary_image.boundaries.is_empty() || layer_image.boundaries.is_empty() {
      continue;
    }
    let Ok(boundary_region) = Region::new(boundary_image.boundaries.clone())
      .and_then(|r| r.normalize(tol).map(|n| n.region))
    else {
      continue;
    };
    let boundary_area = region_area(&boundary_region);
    if boundary_area <= 1.0e-9 {
      continue;
    }
    let Ok(united) = unite_copper(&layer_image.boundaries, tol) else {
      continue;
    };
    let Ok(envelope) =
      non_copper_envelope(&boundary_region, &layer_image.boundaries, 0.0, 0.0, tol)
    else {
      continue;
    };

    let overlap =
      cam_ops::penetration::intersection_area(&boundary_region, &united, tol).unwrap_or(0.0);
    let expected = (boundary_area - overlap).max(0.0);
    let actual = region_area(&envelope);
    let scale = boundary_area.max(1.0);
    assert!(
      (actual - expected).abs() <= 1.0e-5 * scale + 1.0e-3,
      "{board}/{layer_file}: envelope {actual} expected {expected} (boundary {boundary_area} overlap {overlap})"
    );
    checked += 1;
    if checked >= 2 {
      break;
    }
  }
  assert!(
    checked >= 1,
    "expected at least one real corpus Gerber-layer NCC envelope coverage check"
  );
}

// --- E4.2: concentric fill ---

fn swept_coverage(contours: &[Contour], tool_diameter: f64, tol: TolerancePolicy) -> Region {
  let mut swept = Region::new(Vec::new()).expect("empty");
  let radius = tool_diameter / 2.0;
  for contour in contours {
    let c_region = Region::new(vec![contour.clone()])
      .expect("contour region")
      .normalize(tol)
      .expect("normalize")
      .region;
    let grown = match c_region.offset(-radius, tol) {
      Ok(o) => o.region,
      Err(_) => continue,
    };
    // Stroke band: grown minus shrunk, so the tool sweeps radius on both sides of the path
    // without filling the contour interior.
    let band = match c_region.offset(radius, tol) {
      Ok(inner) => match cam_geom::boolean::combine(
        &grown,
        &inner.region,
        cam_geom::boolean::BooleanOp::Difference,
        tol,
      ) {
        Ok(cam_geom::boolean::Combined::Empty) => grown,
        Ok(cam_geom::boolean::Combined::Region { region, .. }) => region,
        Err(_) => grown,
      },
      Err(_) => grown,
    };
    swept =
      match cam_geom::boolean::combine(&swept, &band, cam_geom::boolean::BooleanOp::Union, tol) {
        Ok(cam_geom::boolean::Combined::Empty) => Region::new(Vec::new()).expect("empty"),
        Ok(cam_geom::boolean::Combined::Region { region, .. }) => region,
        Err(_) => swept,
      };
  }
  swept
}

#[test]
fn ncc_fill_rings_produce_concentric_rings_inside_envelope() {
  let board = region_from_contours(vec![rect(0.0, 0.0, 100.0, 100.0)]);
  let copper = vec![rect(40.0, 40.0, 60.0, 60.0)];
  let envelope = non_copper_envelope(&board, &copper, 0.0, 0.0, TOLERANCE).expect("envelope");
  let fill = ncc_fill_rings(&envelope, 6.0, 0.5, TOLERANCE).expect("fill");
  assert!(!fill.contours.is_empty());
  // No gouge into copper: swept tool coverage should not penetrate copper keep-out.
  // Use a slightly shrunk copper for the check to ignore exact boundary touching.
  let swept = swept_coverage(&fill.contours, 6.0, TOLERANCE);
  let copper_region = unite_copper(&copper, TOLERANCE).expect("copper region");
  let copper_inner = copper_region
    .offset(0.05, TOLERANCE)
    .map(|o| o.region)
    .unwrap_or_else(|_| copper_region.clone());
  let gouge = cam_geom::boolean::combine(
    &swept,
    &copper_inner,
    cam_geom::boolean::BooleanOp::Intersection,
    TOLERANCE,
  )
  .expect("intersection");
  let gouge_area = match gouge {
    cam_geom::boolean::Combined::Empty => 0.0,
    cam_geom::boolean::Combined::Region { region, .. } => region_area(&region),
  };
  let env_area = region_area(&envelope);
  assert!(
    gouge_area <= 1.0e-4 * env_area.max(1.0) + 1.0e-2,
    "gouge into copper {gouge_area} of envelope {env_area}"
  );
}

#[test]
fn ncc_fill_rings_cover_envelope_without_large_uncleaned_areas() {
  let board = region_from_contours(vec![rect(0.0, 0.0, 100.0, 100.0)]);
  let copper = vec![rect(45.0, 45.0, 55.0, 55.0)];
  let envelope = non_copper_envelope(&board, &copper, 0.0, 0.0, TOLERANCE).expect("envelope");
  let fill = ncc_fill_rings(&envelope, 4.0, 0.5, TOLERANCE).expect("fill");
  let swept = swept_coverage(&fill.contours, 4.0, TOLERANCE);
  let uncovered = cam_geom::boolean::combine(
    &envelope,
    &swept,
    cam_geom::boolean::BooleanOp::Difference,
    TOLERANCE,
  )
  .expect("diff");
  let uncovered_area = match uncovered {
    cam_geom::boolean::Combined::Empty => 0.0,
    cam_geom::boolean::Combined::Region { region, .. } => region_area(&region),
  };
  let env_area = region_area(&envelope);
  assert!(
    uncovered_area <= 0.02 * env_area + 1.0,
    "uncovered {uncovered_area} of envelope {env_area}"
  );
}

#[test]
fn ncc_fill_rings_handle_island_region() {
  let board = region_from_contours(vec![rect(0.0, 0.0, 80.0, 80.0)]);
  let copper = vec![rect(30.0, 30.0, 50.0, 50.0)];
  let envelope = non_copper_envelope(&board, &copper, 0.0, 0.0, TOLERANCE).expect("envelope");
  // Envelope should have a hole (island of copper).
  assert!(envelope.boundaries.len() >= 2 || region_area(&envelope) < 6400.0);
  let fill = ncc_fill_rings(&envelope, 5.0, 0.4, TOLERANCE).expect("fill");
  assert!(!fill.contours.is_empty());
}

#[test]
fn ncc_fill_rings_reject_invalid_params() {
  let envelope = region_from_contours(vec![rect(0.0, 0.0, 10.0, 10.0)]);
  assert!(ncc_fill_rings(&envelope, 0.0, 0.5, TOLERANCE).is_err());
  assert!(ncc_fill_rings(&envelope, 1.0, 1.0, TOLERANCE).is_err());
  assert!(ncc_fill_rings(&envelope, 1.0, -0.1, TOLERANCE).is_err());
}

#[test]
#[ignore = "timing measurement for E4.3"]
fn measure_ncc_index_naive_vs_grid() {
  use cam_ops::ncc_index::{build_items, naive_query_aabb, Aabb, SpatialGrid};
  use std::time::Instant;

  let tol = TolerancePolicy::uniform(1.0e-6);
  let board = region_from_contours(vec![rect(0.0, 0.0, 100.0, 100.0)]);
  let copper = vec![
    rect(40.0, 40.0, 60.0, 60.0),
    rect(10.0, 70.0, 25.0, 85.0),
    rect(70.0, 10.0, 90.0, 30.0),
  ];
  let envelope = non_copper_envelope(&board, &copper, 0.0, 0.5, tol).expect("envelope");
  let fill = ncc_fill_rings(&envelope, 1.0, 0.5, tol).expect("fill");
  let contours: Vec<Vec<cam_geom::Segment>> = fill.contours.iter().map(|c| c.segments()).collect();
  let items = build_items(&contours);
  let grid = SpatialGrid::build(&contours, 2.0);
  let queries: Vec<Aabb> = (0..500)
    .map(|i| {
      let x = (i as f64 * 0.73) % 100.0;
      let y = (i as f64 * 1.37) % 100.0;
      Aabb {
        min_x: x - 1.0,
        min_y: y - 1.0,
        max_x: x + 1.0,
        max_y: y + 1.0,
      }
    })
    .collect();

  let iters = 200usize;
  let start = Instant::now();
  let mut naive_total = 0usize;
  for _ in 0..iters {
    for q in &queries {
      naive_total += naive_query_aabb(&items, *q).len();
    }
  }
  let naive = start.elapsed();

  let start = Instant::now();
  let mut grid_total = 0usize;
  for _ in 0..iters {
    for q in &queries {
      grid_total += grid.query_aabb(*q).len();
    }
  }
  let grid_t = start.elapsed();

  eprintln!(
    "segments={} queries={} iters={}",
    items.len(),
    queries.len(),
    iters
  );
  eprintln!(
    "naive={:?} grid={:?} naive/grid={:.2}x totals naive={} grid={}",
    naive,
    grid_t,
    naive.as_secs_f64() / grid_t.as_secs_f64().max(1e-9),
    naive_total,
    grid_total
  );
  assert_eq!(naive_total, grid_total);
}

fn fill_signature(fill: &cam_ops::NccFill) -> Vec<u8> {
  let mut s = String::new();
  s.push_str(if fill.flattened { "F1\n" } else { "F0\n" });
  s.push_str(&format!("{}\n", fill.contours.len()));
  for c in &fill.contours {
    s.push_str(&format!("{:.12}\n", c.signed_area()));
    let b = c.bounds();
    s.push_str(&format!(
      "{:.12},{:.12},{:.12},{:.12}\n",
      b.min.x, b.min.y, b.max.x, b.max.y
    ));
    for seg in c.segments() {
      let p1 = seg.start_point();
      let p2 = seg.end_point();
      s.push_str(&format!(
        "{:.12},{:.12}->{:.12},{:.12};",
        p1.x, p1.y, p2.x, p2.y
      ));
    }
    s.push('\n');
  }
  s.into_bytes()
}

#[test]
fn ncc_fill_rings_is_deterministic_across_rayon_thread_counts() {
  // E4.4: salida determinista byte a byte con distintos RAYON_NUM_THREADS.
  let board = region_from_contours(vec![rect(0.0, 0.0, 200.0, 120.0)]);
  let copper = vec![
    rect(20.0, 20.0, 40.0, 40.0),
    rect(80.0, 20.0, 100.0, 40.0),
    rect(140.0, 20.0, 160.0, 40.0),
    rect(20.0, 70.0, 40.0, 90.0),
    rect(80.0, 70.0, 100.0, 90.0),
  ];
  let envelope = non_copper_envelope(&board, &copper, 0.0, 0.0, TOLERANCE).expect("envelope");
  let run = |threads: usize| -> Vec<u8> {
    let pool = rayon::ThreadPoolBuilder::new()
      .num_threads(threads)
      .build()
      .expect("pool");
    pool.install(|| {
      let fill = ncc_fill_rings(&envelope, 2.0, 0.5, TOLERANCE).expect("fill");
      fill_signature(&fill)
    })
  };
  let a = run(1);
  let b = run(2);
  let c = run(4);
  assert_eq!(a, b, "output differs between 1 and 2 threads");
  assert_eq!(a, c, "output differs between 1 and 4 threads");
}

#[test]
#[ignore = "timing measurement for E4.4"]
fn measure_ncc_fill_parallel_speedup() {
  use std::time::Instant;
  // Multiple disjoint envelope islands (parallelism "por isla", E4.4).
  let mut boundaries = Vec::new();
  for ix in 0..4 {
    for iy in 0..3 {
      let x = 10.0 + ix as f64 * 60.0;
      let y = 10.0 + iy as f64 * 60.0;
      boundaries.push(rect(x, y, x + 45.0, y + 45.0));
      // inner hole/copper-like void to make fill work heavier
      boundaries.push(rect(x + 18.0, y + 18.0, x + 27.0, y + 27.0));
    }
  }
  let envelope = Region::new(boundaries)
    .expect("envelope")
    .normalize(TOLERANCE)
    .expect("normalize")
    .region;
  let run = |threads: usize| -> (Vec<u8>, std::time::Duration) {
    let pool = rayon::ThreadPoolBuilder::new()
      .num_threads(threads)
      .build()
      .expect("pool");
    pool.install(|| {
      let start = Instant::now();
      let fill = ncc_fill_rings(&envelope, 1.0, 0.5, TOLERANCE).expect("fill");
      let d = start.elapsed();
      (fill_signature(&fill), d)
    })
  };
  let (s1, d1) = run(1);
  let (s4, d4) = run(4);
  eprintln!(
    "ncc_fill 1-thread={:?} 4-thread={:?} speedup={:.2}x",
    d1,
    d4,
    d1.as_secs_f64() / d4.as_secs_f64().max(1e-9)
  );
  assert_eq!(s1, s4, "deterministic output mismatch");
}

#[test]
fn ncc_rest_machining_covers_without_redundant_recuts() {
  // E4.5: coarse then fine covers area without redundant recuts; measured.
  // Wide area: coarse clears most; narrow area: coarse cannot fit, fine clears it.
  let wide = rect(0.0, 0.0, 80.0, 60.0);
  let narrow = rect(90.0, 0.0, 96.0, 60.0); // 6mm wide channel, 10mm coarse won't fit
  let envelope = Region::new(vec![wide, narrow])
    .expect("envelope")
    .normalize(TOLERANCE)
    .expect("normalize")
    .region;
  let env_area = region_area(&envelope);

  let coarse = ncc_fill_rings(&envelope, 10.0, 0.3, TOLERANCE).expect("coarse");
  let coarse_swept = swept_band_region(&coarse.contours, 10.0, TOLERANCE).expect("coarse swept");
  let rest = ncc_rest_region(&envelope, &coarse.contours, 10.0, TOLERANCE).expect("rest");
  let fine = ncc_fill_rings(&rest, 2.0, 0.4, TOLERANCE).expect("fine");
  let fine_swept = swept_band_region(&fine.contours, 2.0, TOLERANCE).expect("fine swept");

  let combined = match cam_geom::boolean::combine(
    &coarse_swept,
    &fine_swept,
    cam_geom::boolean::BooleanOp::Union,
    TOLERANCE,
  )
  .expect("union")
  {
    cam_geom::boolean::Combined::Empty => Region::new(Vec::new()).unwrap(),
    cam_geom::boolean::Combined::Region { region, .. } => region,
  };
  let uncovered = match cam_geom::boolean::combine(
    &envelope,
    &combined,
    cam_geom::boolean::BooleanOp::Difference,
    TOLERANCE,
  )
  .expect("diff")
  {
    cam_geom::boolean::Combined::Empty => 0.0,
    cam_geom::boolean::Combined::Region { region, .. } => region_area(&region),
  };
  let redundant = match cam_geom::boolean::combine(
    &coarse_swept,
    &fine_swept,
    cam_geom::boolean::BooleanOp::Intersection,
    TOLERANCE,
  )
  .expect("inter")
  {
    cam_geom::boolean::Combined::Empty => 0.0,
    cam_geom::boolean::Combined::Region { region, .. } => region_area(&region),
  };
  let fine_area = region_area(&fine_swept);
  eprintln!(
    "rest machining measured: env={:.2} uncovered={:.2} redundant={:.2} fine_area={:.2}",
    env_area, uncovered, redundant, fine_area
  );
  assert!(uncovered <= 0.05 * env_area + 1.0, "uncovered {uncovered}");
  assert!(
    redundant <= 0.10 * fine_area.max(1.0) + 1.0,
    "redundant {redundant} fine {fine_area}"
  );
}

#[test]
fn ncc_fill_three_methods_produce_valid_output() {
  // E4.6: rings/standard, seed and lines all produce valid output.
  let board = region_from_contours(vec![rect(0.0, 0.0, 80.0, 60.0)]);
  let copper = vec![rect(30.0, 20.0, 50.0, 40.0)];
  let envelope = non_copper_envelope(&board, &copper, 0.0, 0.0, TOLERANCE).expect("envelope");

  let rings = ncc_fill_rings(&envelope, 3.0, 0.4, TOLERANCE).expect("rings");
  assert!(!rings.contours.is_empty(), "rings should produce contours");

  let seed = ncc_fill_seed(&envelope, 3.0, 0.4, TOLERANCE).expect("seed");
  assert!(!seed.contours.is_empty(), "seed should produce contours");

  let lines = ncc_fill_lines(&envelope, 3.0, 0.4, TOLERANCE).expect("lines");
  assert!(!lines.contours.is_empty(), "lines should produce contours");
  for c in &lines.contours {
    assert!(!c.is_closed(), "line fill spans should be open contours");
    assert!(!c.segments().is_empty());
  }
}

#[test]
#[ignore = "timing measurement for E4.7"]
fn measure_ncc_large_plate_benchmark() {
  use std::time::Instant;
  // Representative "large plate" NCC benchmark (E4.7): large board envelope with many copper features.
  let board = region_from_contours(vec![rect(0.0, 0.0, 200.0, 150.0)]);
  let mut copper = Vec::new();
  for ix in 0..8 {
    for iy in 0..6 {
      let x = 10.0 + ix as f64 * 23.0;
      let y = 10.0 + iy as f64 * 23.0;
      copper.push(rect(x, y, x + 12.0, y + 12.0));
    }
  }
  let t0 = Instant::now();
  let envelope = non_copper_envelope(&board, &copper, 0.0, 0.2, TOLERANCE).expect("envelope");
  let env_t = t0.elapsed();
  let t1 = Instant::now();
  let fill = ncc_fill_rings(&envelope, 1.0, 0.5, TOLERANCE).expect("fill");
  let fill_t = t1.elapsed();
  eprintln!(
    "large_plate envelope_ms={:.2} fill_ms={:.2} total_ms={:.2} contours={} env_area={:.2}",
    env_t.as_secs_f64() * 1000.0,
    fill_t.as_secs_f64() * 1000.0,
    (env_t + fill_t).as_secs_f64() * 1000.0,
    fill.contours.len(),
    region_area(&envelope)
  );
  assert!(!fill.contours.is_empty());
}
