//! Behaviour tests for the SQLite tool library (A4).

use cam_store::tools::{
  head_version, render_name, CuttingData, Machine, Material, StoredTool, ToolGeometry, ToolLibrary,
  MIGRATIONS,
};
use cam_units::{Angle, Length, UnitSystem};

fn end_mill() -> ToolGeometry {
  ToolGeometry {
    id: "G1".to_owned(),
    name_format: "{Tool Type} ({Diameter|F}{Units Short})".to_owned(),
    tool_type: 0,
    units: UnitSystem::Mm,
    diameter: Some(Length::from_mm(6.35)),
    included_angle: None,
    flat_diameter: None,
    num_flutes: Some(2),
    flute_length: Some(Length::from_mm(20.0)),
    tip_radius: None,
    custom_attributes: None,
  }
}

fn cutting(id: &str, feed: f64, stepdown: f64) -> CuttingData {
  CuttingData {
    id: id.to_owned(),
    rate_units: UnitSystem::Mm,
    feed_rate: Some(feed),
    plunge_rate: Some(feed / 3.0),
    spindle_speed: Some(18_000),
    stepdown: Some(Length::from_mm(stepdown)),
    stepover: Some(Length::from_mm(3.0)),
    clear_stepover: Some(Length::from_mm(4.5)),
    tool_number: Some(1),
    notes: None,
  }
}

/// A scratch directory under the system temp directory, named so two tests cannot collide.
fn scratch(name: &str) -> std::path::PathBuf {
  let directory =
    std::env::temp_dir().join(format!("campilot-tools-{name}-{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&directory);
  std::fs::create_dir_all(&directory).expect("scratch directory");
  directory
}

#[test]
fn a_fresh_library_is_at_the_head_version() {
  let library = ToolLibrary::open_in_memory().expect("opens");
  assert_eq!(library.schema_version().expect("version"), head_version());
  assert!(head_version() >= 2, "the ladder must actually have rungs");
}

#[test]
fn the_same_tool_carries_different_cutting_data_per_material() {
  let library = ToolLibrary::open_in_memory().expect("opens");
  library.put_geometry(&end_mill()).expect("geometry");
  library
    .put_material(&Material {
      id: "M1".to_owned(),
      name: "MDF".to_owned(),
    })
    .expect("material");
  library
    .put_material(&Material {
      id: "M2".to_owned(),
      name: "Hardwood".to_owned(),
    })
    .expect("material");
  library
    .put_cutting_data(&cutting("C1", 120.0, 6.0))
    .expect("mdf data");
  library
    .put_cutting_data(&cutting("C2", 60.0, 3.0))
    .expect("wood data");
  library
    .link("E1", "G1", "C1", Some("M1"), None)
    .expect("link mdf");
  library
    .link("E2", "G1", "C2", Some("M2"), None)
    .expect("link wood");

  let mdf = library
    .cutting_data_for("G1", Some("M1"), None)
    .expect("query")
    .expect("mdf cutting data");
  let hardwood = library
    .cutting_data_for("G1", Some("M2"), None)
    .expect("query")
    .expect("hardwood cutting data");
  assert_eq!(mdf.feed_rate, Some(120.0));
  assert_eq!(hardwood.feed_rate, Some(60.0));
  assert_eq!(hardwood.stepdown, Some(Length::from_mm(3.0)));
  assert!(library
    .cutting_data_for("G1", Some("M-unknown"), None)
    .expect("query")
    .is_none());
}

#[test]
fn geometry_round_trips_through_a_reopened_file() {
  let directory = std::env::temp_dir().join(format!("campilot-tools-{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&directory);
  std::fs::create_dir_all(&directory).expect("dir");
  let path = directory.join("tools.sqlite");

  {
    let library = ToolLibrary::open(&path).expect("opens");
    library.put_geometry(&end_mill()).expect("geometry");
  }
  {
    let library = ToolLibrary::open(&path).expect("reopens");
    let geometry = library.geometry("G1").expect("query").expect("present");
    assert_eq!(geometry, end_mill());
    assert_eq!(library.schema_version().expect("version"), head_version());
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_older_library_is_migrated_forward_without_losing_rows() {
  let directory = std::env::temp_dir().join(format!("campilot-migrate-{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&directory);
  std::fs::create_dir_all(&directory).expect("dir");
  let path = directory.join("legacy.sqlite");

  // Build a version-1 library by hand, exactly as an older build would have left it.
  {
    let connection = rusqlite::Connection::open(&path).expect("open legacy");
    connection
      .execute_batch(
        "CREATE TABLE migration (version INTEGER NOT NULL, subversion INTEGER NOT NULL,
                                 name TEXT NOT NULL, checksum TEXT NOT NULL,
                                 PRIMARY KEY (version, subversion));",
      )
      .expect("migration table");
    connection
      .execute_batch(MIGRATIONS[0].sql)
      .expect("v1 schema");
    connection
      .execute(
        "INSERT INTO migration (version, subversion, name, checksum) VALUES (1, 0, 'initial', 'x')",
        [],
      )
      .expect("record v1");
    connection
      .execute(
        "INSERT INTO tool_geometry (id, name_format, tool_type, units, diameter)
         VALUES ('LEGACY', '{Tool Type}', 0, 0, 3.0)",
        [],
      )
      .expect("legacy row");
  }

  {
    let library = ToolLibrary::open(&path).expect("migrates");
    assert_eq!(library.schema_version().expect("version"), head_version());
    let geometry = library
      .geometry("LEGACY")
      .expect("query")
      .expect("legacy row survived");
    assert_eq!(geometry.diameter, Some(Length::from_mm(3.0)));
    // The v2 column exists and is readable.
    assert_eq!(geometry.custom_attributes, None);
    library
      .put_material(&Material {
        id: "M1".to_owned(),
        name: "MDF".to_owned(),
      })
      .expect("v2 tables are usable");
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_read_only_open_reads_a_library_and_writes_nothing() {
  let directory = scratch("read-only");
  let path = directory.join("tools.sqlite");
  {
    let library = ToolLibrary::open(&path).expect("the owner writes the library");
    let tool = StoredTool {
      geometry: end_mill(),
      cutting: cutting("C1", 100.0, 4.0),
    };
    library.put_geometry(&tool.geometry).expect("geometry");
    library.put_cutting_data(&tool.cutting).expect("cutting");
    library
      .link("E1", &tool.geometry.id, &tool.cutting.id, None, None)
      .expect("link");
  }
  let bytes = std::fs::read(&path).expect("the library is there");

  {
    let library = ToolLibrary::open_read_only(&path).expect("an existing library opens");
    assert_eq!(
      library.schema_version().expect("version"),
      head_version(),
      "a read-only open reads the version the file records"
    );
    let tools = library.tools().expect("the tools read");
    assert_eq!(tools.len(), 1, "the tool the file holds is read: {tools:?}");
    assert_eq!(tools[0].geometry, end_mill());
    assert_eq!(tools[0].cutting.feed_rate, Some(100.0));
    let resolved = library
      .tool_for_number(1)
      .expect("the query runs")
      .expect("T1 is in the library");
    assert_eq!(resolved.0.id, "G1");
    assert_eq!(resolved.1.id, "C1");
  }

  assert_eq!(
    std::fs::read(&path).expect("the library is still there"),
    bytes,
    "reading a library through the read-only constructor leaves every byte of it alone"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_read_only_open_of_an_empty_file_fails_and_leaves_it_zero_bytes() {
  let directory = scratch("read-only-empty");
  let path = directory.join("empty.sqlite");
  std::fs::write(&path, b"").expect("an empty file");

  let error = match ToolLibrary::open_read_only(&path) {
    Ok(_) => panic!("an empty file holds no library, so the open must fail"),
    Err(error) => error,
  };
  let message = error.to_string();
  assert!(
    message.contains("empty.sqlite") && message.contains("cannot be used"),
    "the error must name the file and say it cannot be used, got {message:?}"
  );
  assert!(
    message.contains("never been initialised"),
    "the error must say the file was never initialised, got {message:?}"
  );
  assert_eq!(
    std::fs::metadata(&path)
      .expect("the file is still there")
      .len(),
    0,
    "a read-only open must not write a schema into the file it was pointed at"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_read_only_open_of_a_missing_path_fails_and_creates_nothing() {
  let directory = scratch("read-only-missing");
  let parent = directory.join("nested");
  let path = parent.join("tools.sqlite");

  let error = match ToolLibrary::open_read_only(&path) {
    Ok(_) => panic!("there is no library to read, so the open must fail"),
    Err(error) => error,
  };
  assert!(
    error.to_string().contains("cannot be used"),
    "the error must say the library cannot be used, got {error}"
  );
  assert!(
    !path.exists(),
    "a read-only open must not create the library it was pointed at"
  );
  assert!(
    !parent.exists(),
    "a read-only open must not create the directory the library would live in"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_tool_name_renders_from_its_template() {
  let tool = end_mill();
  let name = render_name(&tool.name_format, "End Mill", &tool).expect("renders");
  assert_eq!(name, "End Mill (6.35mm)");

  let mut v_bit = tool.clone();
  v_bit.diameter = Some(Length::from_mm(12.0));
  v_bit.included_angle = Some(Angle::from_degrees(118.0));
  v_bit.name_format = "{Tool Type} ({Included Angle|.0}deg - {Diameter|F}{Units Short})".to_owned();
  assert_eq!(
    render_name(&v_bit.name_format, "V-Bit", &v_bit).expect("renders"),
    "V-Bit (118deg - 12mm)"
  );

  let mut inches = tool.clone();
  inches.units = UnitSystem::Inch;
  inches.diameter = Some(Length::from_mm(6.35));
  assert_eq!(
    render_name(&inches.name_format, "End Mill", &inches).expect("renders"),
    "End Mill (0.25in)"
  );
}

#[test]
fn a_bad_template_names_the_offending_field() {
  let tool = end_mill();
  let error = render_name("{Nonsense}", "End Mill", &tool).expect_err("must reject");
  assert!(error.contains("Nonsense"), "{error}");

  let error = render_name("{Diameter", "End Mill", &tool).expect_err("must reject");
  assert!(error.contains("unclosed"), "{error}");

  let mut v_bit = tool.clone();
  v_bit.name_format = "{Included Angle}".to_owned();
  let error = render_name(&v_bit.name_format, "End Mill", &v_bit).expect_err("must reject");
  assert!(error.contains("Included Angle"), "{error}");
}

#[test]
fn machines_keep_their_travel_limits() {
  let library = ToolLibrary::open_in_memory().expect("opens");
  library
    .put_machine(&Machine {
      id: "MC1".to_owned(),
      name: "Shop router".to_owned(),
      make: Some("Generic".to_owned()),
      model: Some("3040".to_owned()),
      controller_type: Some("GRBL".to_owned()),
      max_width: Some(Length::from_mm(400.0)),
      max_height: Some(Length::from_mm(300.0)),
    })
    .expect("machine");
  library.put_geometry(&end_mill()).expect("geometry");
  library
    .put_cutting_data(&cutting("C1", 100.0, 5.0))
    .expect("data");
  library
    .link("E1", "G1", "C1", None, Some("MC1"))
    .expect("link");
  let data = library
    .cutting_data_for("G1", None, Some("MC1"))
    .expect("query")
    .expect("present");
  assert_eq!(data.feed_rate, Some(100.0));
}

/// The names in a directory, sorted, so a test can prove that reading created nothing.
fn listing(directory: &std::path::Path) -> Vec<String> {
  let mut names: Vec<String> = std::fs::read_dir(directory)
    .expect("the directory lists")
    .filter_map(|entry| entry.ok())
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .collect();
  names.sort();
  names
}

// Covers shell-tools-ref S8: a read-only open refuses a library in WAL mode, because SQLite would
// create its -shm and -wal companions beside it and this constructor never writes anything.
#[test]
fn a_read_only_open_refuses_a_wal_library_and_creates_no_sidecar_files() {
  let directory = scratch("read-only-wal");
  let path = directory.join("waltools.sqlite");
  drop(ToolLibrary::open(&path).expect("head library"));
  {
    let connection = rusqlite::Connection::open(&path).expect("open the library directly");
    let mode: String = connection
      .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
      .expect("switched to WAL");
    assert_eq!(
      mode.to_lowercase(),
      "wal",
      "the fixture must really be in WAL mode"
    );
  }
  let before = listing(&directory);

  let error = match ToolLibrary::open_read_only(&path) {
    Ok(_) => panic!("a WAL library cannot be read without creating its companions"),
    Err(error) => error.to_string(),
  };
  assert!(
    error.contains("WAL"),
    "the refusal must name the journal mode: {error}"
  );
  assert_eq!(
    listing(&directory),
    before,
    "a read-only open creates nothing beside the library"
  );
  std::fs::remove_dir_all(&directory).ok();
}

// Covers shell-tools-ref S3/S8: reading a library creates nothing anywhere, whatever its header
// declares. A non-empty -wal beside a header that says rollback journal mode (1/1 — a crash between
// the header rewrite and the log's deletion, or a hand-crafted file) still makes SQLite build its
// shared-memory index.
#[test]
fn a_read_only_open_creates_no_companion_for_a_wal_file_beside_a_non_wal_header() {
  let directory = scratch("read-only-wal-sidecar");
  let path = directory.join("sidecar.sqlite");
  drop(ToolLibrary::open(&path).expect("head library"));
  {
    let mut bytes = std::fs::read(&path).expect("read");
    bytes[18] = 1;
    bytes[19] = 1;
    std::fs::write(&path, &bytes).expect("header rewritten");
  }
  std::fs::write(directory.join("sidecar.sqlite-wal"), [0xABu8; 4096]).expect("log");
  let before = listing(&directory);

  let _ = ToolLibrary::open_read_only(&path); // accepted or refused: either way it must not create

  assert_eq!(
    listing(&directory),
    before,
    "a read-only open creates nothing beside the library, whatever its header declares"
  );
  std::fs::remove_dir_all(&directory).ok();
}

// Covers shell-tools-ref S3/S8 through L7's untested gap: an *empty* -wal beside a non-WAL header is
// a post-crash leftover that creates no companion, so a read-only open accepts it and writes nothing.
#[test]
fn a_read_only_open_accepts_an_empty_wal_beside_a_non_wal_header_and_creates_nothing() {
  let directory = scratch("read-only-empty-wal");
  let path = directory.join("empty.sqlite");
  drop(ToolLibrary::open(&path).expect("head library"));
  {
    let mut bytes = std::fs::read(&path).expect("read");
    bytes[18] = 1;
    bytes[19] = 1;
    std::fs::write(&path, &bytes).expect("header rewritten");
  }
  std::fs::write(directory.join("empty.sqlite-wal"), []).expect("empty log");
  let before = listing(&directory);

  let library = ToolLibrary::open_read_only(&path).expect("an empty log creates no companion");

  assert_eq!(library.tools().expect("tools").len(), 0);
  assert_eq!(listing(&directory), before);
  std::fs::remove_dir_all(&directory).ok();
}
