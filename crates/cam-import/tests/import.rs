//! Behaviour tests for the import spine: format detection, the report and document assembly (M3
//! A1–A3, A14).

use std::path::Path;

use cam_geom::{primitive, Contour, Point2};
use cam_import::{
  detect, import_bytes, into_document, Format, ImportError, ImportOptions, ImportReport, Imported,
  ImportedLayer, PLACEMENT_MARGIN,
};

fn report_with_counts() -> ImportReport {
  let mut report = ImportReport::new("dxf", 0.01);
  report.count("LINE", 4);
  report.count("LWPOLYLINE", 2);
  report.flatten(3);
  report.unit_scale = 25.4;
  report.contours = 6;
  report.skip(
    "TEXT",
    "text is not vector geometry",
    Some("line 412".into()),
  );
  report.warn("no $INSUNITS: millimetres assumed");
  report
}

#[test]
fn the_extension_decides_the_format_first() {
  for (name, format) in [
    ("part.dxf", Format::Dxf),
    ("PART.DXF", Format::Dxf),
    ("logo.svg", Format::Svg),
    ("sheet.pdf", Format::Pdf),
    ("plot.eps", Format::Eps),
    ("art.PNG", Format::Png),
  ] {
    assert_eq!(
      detect(Path::new(name), b"ignored").expect("detect"),
      format,
      "{name}"
    );
  }
}

#[test]
fn content_signatures_identify_a_format_with_no_known_extension() {
  let cases: [(&str, &[u8], Format); 5] = [
    ("part", b"%PDF-1.7\n1 0 obj\n", Format::Pdf),
    ("plot", b"%!PS-Adobe-3.0 EPSF-3.0\n", Format::Eps),
    ("art", b"\x89PNG\r\n\x1a\n\x00\x00", Format::Png),
    ("logo", b"<?xml version=\"1.0\"?>\n<svg/>", Format::Svg),
    ("drawing", b"0\nSECTION\n2\nENTITIES\n0\nEOF\n", Format::Dxf),
  ];
  for (name, bytes, format) in cases {
    assert_eq!(
      detect(Path::new(name), bytes).expect("detect"),
      format,
      "{name}"
    );
  }
}

#[test]
fn an_unreadable_format_is_refused_naming_the_supported_set() {
  let error = detect(Path::new("part.dwg"), b"whatever").expect_err("refused");
  assert_eq!(
    error,
    ImportError::Unsupported {
      extension: "dwg".into(),
      supported: cam_import::SUPPORTED_EXTENSIONS,
    }
  );
  assert_eq!(
    error.to_string(),
    "unsupported format dwg; supported formats are dxf, svg, pdf, eps, png"
  );
}

#[test]
fn a_file_with_no_extension_and_no_signature_is_refused() {
  let error = detect(Path::new("noext"), b"just words").expect_err("refused");
  assert!(matches!(
    error,
    ImportError::Unsupported { ref extension, .. } if extension == "<none>"
  ));
}

#[test]
fn the_report_renders_the_same_text_every_time() {
  let rendered = report_with_counts().render();
  let expected = [
    "format: dxf",
    "tolerance: 0.010 mm",
    "unit scale: 25.400000",
    "contours: 6",
    "flattened curves: 3",
    "sources: LINE=4 LWPOLYLINE=2",
    "skipped: 1",
    "  - TEXT at line 412: text is not vector geometry",
    "warnings:",
    "  - no $INSUNITS: millimetres assumed",
    "",
  ]
  .join("\n");
  assert_eq!(rendered, expected);
  assert_eq!(rendered, report_with_counts().render());
  assert!(ImportReport::new("svg", 0.01)
    .render()
    .contains("sources: none\nskipped: 0\n"));
}

#[test]
fn assembly_places_the_geometry_with_its_margin_and_mints_ordered_ids() {
  let imported = Imported {
    format: Format::Dxf,
    layers: vec![
      ImportedLayer {
        name: "OUTSIDE".into(),
        contours: vec![
          primitive::rectangle(Point2::new(50.0, 40.0), 100.0, 50.0).expect("rect"),
          primitive::circle(Point2::new(80.0, 60.0), 5.0).expect("circle"),
        ],
      },
      ImportedLayer {
        name: "HOLES".into(),
        contours: vec![primitive::circle(Point2::new(120.0, 70.0), 2.0).expect("circle")],
      },
    ],
    report: ImportReport::new("dxf", 0.01),
  };

  let document = into_document(&imported, None).expect("document");
  assert_eq!(document.layers.len(), 2);
  assert_eq!(document.layers[0].id, "L1");
  assert_eq!(document.layers[0].name, "OUTSIDE");
  assert_eq!(document.layers[1].id, "L2");
  assert_eq!(document.layers[1].name, "HOLES");
  let ids: Vec<&str> = document
    .entities()
    .map(|entity| entity.id.as_str())
    .collect();
  assert_eq!(ids, vec!["E1", "E2", "E3"]);

  // The bounds start at the margin, and the job is the bounds plus a margin on each side.
  let bounds = document
    .layers
    .iter()
    .flat_map(|layer| layer.entities.iter())
    .filter_map(|entity| entity.kind.as_contour())
    .fold(cam_geom::Bounds::empty(), |bounds, contour| {
      bounds.union(contour.bounds())
    });
  assert!((bounds.min.x - PLACEMENT_MARGIN).abs() < 1e-9);
  assert!((bounds.min.y - PLACEMENT_MARGIN).abs() < 1e-9);
  assert!((document.job.width.mm() - (bounds.width() + 2.0 * PLACEMENT_MARGIN)).abs() < 1e-9);
  assert!((document.job.height.mm() - (bounds.height() + 2.0 * PLACEMENT_MARGIN)).abs() < 1e-9);
}

#[test]
fn a_single_layer_import_keeps_every_contour_in_one_named_layer() {
  let imported = Imported {
    format: Format::Svg,
    layers: vec![
      ImportedLayer {
        name: "a".into(),
        contours: vec![primitive::circle(Point2::ORIGIN, 1.0).expect("circle")],
      },
      ImportedLayer {
        name: "b".into(),
        contours: vec![primitive::circle(Point2::new(10.0, 0.0), 1.0).expect("circle")],
      },
    ],
    report: ImportReport::new("svg", 0.01),
  };

  let document = into_document(&imported, Some("Imported")).expect("document");
  assert_eq!(document.layers.len(), 1);
  assert_eq!(document.layers[0].name, "Imported");
  assert_eq!(document.layers[0].entities.len(), 2);
}

#[test]
fn an_importer_that_produces_nothing_is_an_error_not_an_empty_document() {
  // The png tracer is not implemented yet, so every format refuses for now; the shape of the
  // contract is what this test pins: no geometry is an error, never a silent empty document.
  let error = import_bytes("plot.eps", b"%!PS-Adobe-3.0\n", &ImportOptions::default())
    .expect_err("nothing imported");
  match error {
    ImportError::Refused { format, .. } | ImportError::Parse { format, .. } => {
      assert_eq!(format, "eps")
    }
    other => panic!("unexpected error {other:?}"),
  }
}

#[test]
fn the_default_tolerance_is_one_hundredth_of_a_millimetre() {
  assert_eq!(ImportOptions::default().tolerance, 0.01);
  assert_eq!(ImportOptions::default().trace.dpi, 96.0);
  assert_eq!(ImportOptions::default().trace.effective_threshold(), 128);
}

/// A contour is comparable when we round-trip it through the format that stores it.
#[test]
fn an_imported_contour_survives_its_document_round_trip() {
  let contour: Contour = primitive::circle(Point2::new(5.0, 5.0), 2.5).expect("circle");
  let text = serde_json::to_string(&contour).expect("serialise");
  let back: Contour = serde_json::from_str(&text).expect("deserialise");
  assert_eq!(contour, back);
}
