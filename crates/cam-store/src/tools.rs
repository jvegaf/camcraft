//! The SQLite tool library.
//!
//! # Why this shape
//!
//! The analysed product's library stores tool **geometry**, tool **cutting data** and the
//! (material, machine) context as separate records joined by `tool_entity`. That normalisation is
//! the reason a tool library is worth having at all: the same 6 mm end mill feeds differently in MDF
//! and in hardwood, and differently again on a light hobby machine. Copying the shape is a deliberate
//! decision recorded in the roadmap (analysis report §2.1).
//!
//! `tool_type` is preserved as the raw integer found in the analysed schema, whose numeric mapping the
//! artifact did not establish. This build now **writes** that integer itself, so it has to say what its
//! own numbers mean: [`ToolKind`] is that vocabulary, and [`ToolKind::of`] reads a row by its stored kind
//! when the row carries one and falls back to the shape the geometry proves when it does not. Zero is
//! reserved for "the writer did not say" — `cam-cli tools add` has always written it — so a library
//! written before this vocabulary existed still reads as the tool it geometrically is.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};

use cam_units::{Angle, Format, Length, UnitSystem};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};

use crate::StoreError;

/// One forward-only schema migration.
#[derive(Clone, Copy, Debug)]
pub struct Migration {
  /// Target schema version.
  pub version: u32,
  /// Short description, recorded in the `migration` table.
  pub name: &'static str,
  /// SQL applied when moving to this version.
  pub sql: &'static str,
}

/// The migration ladder. Append only; never edit an applied entry.
pub const MIGRATIONS: &[Migration] = &[
  Migration {
    version: 1,
    name: "initial tool library",
    sql: "
      CREATE TABLE tool_geometry (
        id TEXT PRIMARY KEY,
        name_format TEXT NOT NULL,
        notes TEXT,
        tool_type INTEGER NOT NULL,
        units INTEGER NOT NULL,
        diameter REAL,
        included_angle REAL,
        flat_diameter REAL,
        num_flutes INTEGER,
        flute_length REAL,
        tip_radius REAL
      );
      CREATE TABLE tool_cutting_data (
        id TEXT PRIMARY KEY,
        rate_units INTEGER NOT NULL,
        feed_rate REAL,
        plunge_rate REAL,
        spindle_speed INTEGER,
        spindle_dir INTEGER,
        stepdown REAL,
        stepover REAL,
        clear_stepover REAL,
        tool_number INTEGER,
        notes TEXT
      );
      CREATE TABLE material (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL
      );
      CREATE TABLE machine (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        make TEXT,
        model TEXT,
        controller_type TEXT,
        max_width REAL,
        max_height REAL,
        support_rotary INTEGER,
        support_tool_change INTEGER
      );
      CREATE TABLE tool_entity (
        id TEXT PRIMARY KEY,
        material_id TEXT,
        machine_id TEXT,
        tool_geometry_id TEXT NOT NULL,
        tool_cutting_data_id TEXT NOT NULL
      );
      CREATE TABLE tool_tree_entry (
        id TEXT PRIMARY KEY,
        parent_group_id TEXT,
        sibling_order INTEGER NOT NULL,
        tool_geometry_id TEXT,
        name TEXT,
        notes TEXT,
        expanded INTEGER
      );
    ",
  },
  Migration {
    version: 2,
    name: "custom tool outline and per-tool name formats",
    sql: "
      ALTER TABLE tool_geometry ADD COLUMN custom_attributes TEXT;
      CREATE TABLE tool_name_format (
        id TEXT PRIMARY KEY,
        tool_type INTEGER NOT NULL,
        format TEXT NOT NULL
      );
      CREATE INDEX tool_entity_by_geometry
        ON tool_entity (tool_geometry_id, material_id, machine_id);
    ",
  },
  Migration {
    version: 3,
    name: "shank diameter and overall length",
    sql: "
      ALTER TABLE tool_geometry ADD COLUMN shank_diameter REAL;
      ALTER TABLE tool_geometry ADD COLUMN overall_length REAL;
    ",
  },
];

/// The schema version this build writes.
pub fn head_version() -> u32 {
  MIGRATIONS
    .iter()
    .map(|migration| migration.version)
    .max()
    .unwrap_or(0)
}

/// The kind of cutting tool a library row describes.
///
/// The reference screen offers six kinds and this is that list, in its order. It has to be *stored*
/// rather than derived, for one reason worth writing down: a drill and a V-bit are both described by an
/// included angle, so no rule over the geometry can tell a 118° drill point from a 90° V. The shape
/// families (the drawing still derives those) say how a tool is *drawn*; this says what the operator
/// *called* it.
///
/// The stored code is [`ToolKind::raw`], and [`ToolKind::of`] is the only reader: a row whose code is
/// [`ToolKind::LEGACY_RAW`] — every row `cam-cli tools add` has ever written — is read as the shape its
/// geometry proves, so no library has to be migrated to gain a name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolKind {
  /// A straight cutter with a square end.
  FlatEndMill,
  /// A hemisphere for a tip.
  BallNose,
  /// A square corner with a radius on it.
  CornerRadius,
  /// A cone ending in a flat or a point.
  VBit,
  /// A flat-ended cutter for engraving.
  Engraving,
  /// A pointed drill.
  Drill,
}

impl ToolKind {
  /// The code that means "the writer did not say", which is what a pre-vocabulary row carries.
  ///
  /// Zero and not a kind, deliberately: `cam-cli tools add` has always written `0`, so giving `0` to a
  /// real kind would relabel every library that already exists.
  pub const LEGACY_RAW: u8 = 0;

  /// Every kind, in the order the form's combo offers them.
  pub const ALL: [ToolKind; 6] = [
    ToolKind::FlatEndMill,
    ToolKind::BallNose,
    ToolKind::CornerRadius,
    ToolKind::VBit,
    ToolKind::Engraving,
    ToolKind::Drill,
  ];

  /// The stored code: `1` to `6`, in [`ToolKind::ALL`]'s order. Never [`ToolKind::LEGACY_RAW`].
  pub fn raw(self) -> u8 {
    match self {
      ToolKind::FlatEndMill => 1,
      ToolKind::BallNose => 2,
      ToolKind::CornerRadius => 3,
      ToolKind::VBit => 4,
      ToolKind::Engraving => 5,
      ToolKind::Drill => 6,
    }
  }

  /// The kind a stored code names, or `None` for a code this build does not know.
  pub fn from_raw(raw: u8) -> Option<ToolKind> {
    ToolKind::ALL.into_iter().find(|kind| kind.raw() == raw)
  }

  /// The kind a tool is, read from its own record.
  ///
  /// The stored code decides when it names a kind; otherwise the shape decides, which is the whole of
  /// the backwards compatibility argument in this type's header.
  pub fn of(geometry: &ToolGeometry) -> ToolKind {
    ToolKind::from_raw(geometry.tool_type).unwrap_or_else(|| ToolKind::from_shape(geometry))
  }

  /// The kind a tool's geometry proves, for a row that carries no kind of its own.
  ///
  /// A shape cannot prove `Drill` or `Engraving` — a drill's included angle reads exactly like a V's,
  /// and an engraving cutter's flat end reads exactly like an end mill's — so those two are reachable
  /// only by a stored code. The four that *are* provable are read the way the drawing reads them: an
  /// included angle makes a V, a tip radius of exactly half the diameter makes a ball, any other tip
  /// radius makes a corner radius, and no radius at all makes a flat end mill.
  pub fn from_shape(geometry: &ToolGeometry) -> ToolKind {
    if geometry.included_angle.is_some() {
      return ToolKind::VBit;
    }
    match (geometry.diameter, geometry.tip_radius) {
      (Some(diameter), Some(radius)) if (radius.mm() * 2.0 - diameter.mm()).abs() <= 1.0e-6 => {
        ToolKind::BallNose
      }
      (_, Some(_)) => ToolKind::CornerRadius,
      _ => ToolKind::FlatEndMill,
    }
  }

  /// The word the combo and the tree's heading carry.
  pub fn label(self) -> &'static str {
    match self {
      ToolKind::FlatEndMill => "Flat end mill",
      ToolKind::BallNose => "Ball nose",
      ToolKind::CornerRadius => "Corner radius",
      ToolKind::VBit => "V-bit",
      ToolKind::Engraving => "Engraving",
      ToolKind::Drill => "Drill",
    }
  }

  /// Whether this kind's tip is described by an included angle.
  pub fn has_included_angle(self) -> bool {
    matches!(self, ToolKind::VBit | ToolKind::Drill)
  }
}

/// A tool's geometric definition.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolGeometry {
  /// Stable identifier.
  pub id: String,
  /// Display-name template, e.g. `{Tool Type} ({Diameter|F}{Units Short})`.
  pub name_format: String,
  /// Raw `tool_type` value: [`ToolKind::raw`], or [`ToolKind::LEGACY_RAW`] for a row written before
  /// this build named its kinds. Read it through [`ToolKind::of`] and never directly.
  pub tool_type: u8,
  /// Units the geometry was authored in. Stored lengths are always millimetres.
  pub units: UnitSystem,
  /// Cutting diameter.
  pub diameter: Option<Length>,
  /// Included angle of a V-bit.
  pub included_angle: Option<Angle>,
  /// Flat width of a V-bit's tip.
  pub flat_diameter: Option<Length>,
  /// Number of flutes.
  pub num_flutes: Option<u32>,
  /// Usable flute length.
  pub flute_length: Option<Length>,
  /// Tip radius of a ball-nose or bull-nose tool.
  pub tip_radius: Option<Length>,
  /// Diameter of the shank the cutter is held by, when the record carries one.
  ///
  /// A separate number from [`ToolGeometry::diameter`] on purpose: a tool can cut wider than its
  /// shank, and which end is which is the one thing a drawing of a shank has to get right.
  pub shank_diameter: Option<Length>,
  /// The whole tool's length, shank included, when the record carries one.
  pub overall_length: Option<Length>,
  /// Free-form extra attributes, reserved for callers.
  pub custom_attributes: Option<String>,
}

/// Cutting parameters for one tool in one material on one machine.
#[derive(Clone, Debug, PartialEq)]
pub struct CuttingData {
  /// Stable identifier.
  pub id: String,
  /// Units the rates were authored in. Stored rates are millimetres per second.
  pub rate_units: UnitSystem,
  /// Feed rate along the cut.
  pub feed_rate: Option<f64>,
  /// Plunge rate.
  pub plunge_rate: Option<f64>,
  /// Spindle speed in rpm.
  pub spindle_speed: Option<u32>,
  /// Maximum depth of cut per pass.
  pub stepdown: Option<Length>,
  /// Stepover between passes.
  pub stepover: Option<Length>,
  /// Stepover used while clearing.
  pub clear_stepover: Option<Length>,
  /// Tool number used in the program.
  pub tool_number: Option<u32>,
  /// Free-form notes.
  pub notes: Option<String>,
}

/// A workpiece material.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
  /// Stable identifier.
  pub id: String,
  /// Display name.
  pub name: String,
}

/// A machine the library holds cutting data for.
#[derive(Clone, Debug, PartialEq)]
pub struct Machine {
  /// Stable identifier.
  pub id: String,
  /// Display name.
  pub name: String,
  /// Manufacturer.
  pub make: Option<String>,
  /// Model name.
  pub model: Option<String>,
  /// Controller family.
  pub controller_type: Option<String>,
  /// Maximum travel along X.
  pub max_width: Option<Length>,
  /// Maximum travel along Y.
  pub max_height: Option<Length>,
}

/// A tool as the library stores it: geometry plus the cutting data it is linked to.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredTool {
  /// The tool's shape.
  pub geometry: ToolGeometry,
  /// The feeds and speeds it is linked to.
  pub cutting: CuttingData,
}

/// An open tool library.
pub struct ToolLibrary {
  connection: Connection,
  path: String,
}

/// The magic every SQLite database file starts with.
const SQLITE_MAGIC: &[u8; 16] = b"SQLite format 3\0";

/// The byte offset of the file format's read and write versions in a SQLite header.
const FORMAT_VERSION_OFFSET: usize = 18;

/// The journal mode value that means "WAL".
const WAL_JOURNAL_MODE: u8 = 2;

/// The suffix SQLite appends to a database's own name for its write-ahead log.
const WAL_LOG_SUFFIX: &str = "-wal";

/// Whether a library file declares WAL journal mode, read out of its own header.
///
/// The header is read rather than the mode being asked of SQLite, because asking means opening the
/// database, and opening a WAL database is exactly what creates its `-shm` and `-wal` companions. A
/// constructor that cannot promise "nothing is created" is not read-only, so the question has to be
/// answered before there is a connection to answer it with.
///
/// A file that cannot be read, or that is not a SQLite database at all, answers `false`: both already
/// have their own refusal further down, and inventing a second one here would make a single failure
/// say two things.
fn declares_wal(path: &Path) -> bool {
  let mut header = [0u8; FORMAT_VERSION_OFFSET + 2];
  let mut file = match std::fs::File::open(path) {
    Ok(file) => file,
    Err(_) => return false,
  };
  if file.read_exact(&mut header).is_err() || !header.starts_with(SQLITE_MAGIC) {
    return false;
  }
  header[FORMAT_VERSION_OFFSET] == WAL_JOURNAL_MODE
    || header[FORMAT_VERSION_OFFSET + 1] == WAL_JOURNAL_MODE
}

/// The path of the write-ahead log SQLite keeps beside `path`, or `None` when `path` has no file name
/// to hang one on.
///
/// The log is built from `path`'s **canonical** target, not from the path as written. The two name the
/// same file only when no symlink is involved: a `link.sqlite` beside a document may point at
/// `outside/real.sqlite`, and SQLite opens what the path resolves to, so it reads and writes
/// `outside/real.sqlite-wal`. Asking about `link.sqlite-wal` — a name nobody reads — would answer "no
/// log" while SQLite built `outside/real.sqlite-shm` from the target's log. The guard and SQLite have
/// to be looking at the same file, otherwise the guard is answering about a name.
///
/// Canonicalising is not a refusal of symlinked libraries: following one for reading stays allowed, and
/// only a non-empty log at the target refuses. A path that cannot be canonicalised — absent, or under a
/// parent that cannot be read — falls back to the path as written, since there is no target to ask
/// about and its own refusal is further down.
fn wal_log_path(path: &Path) -> Option<PathBuf> {
  let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
  let mut log = OsString::from(resolved.file_name()?);
  log.push(WAL_LOG_SUFFIX);
  Some(resolved.with_file_name(log))
}

/// Whether a non-empty write-ahead log sits beside `path`.
///
/// The header is not the whole answer: a crash between the header's rewrite and the log's deletion, or
/// a hand-crafted file, leaves a header that says rollback journal mode beside a live log, and SQLite
/// builds the shared-memory index for that log anyway. The log itself therefore has to be asked
/// about, before there is a connection that would answer by creating what is asked about.
///
/// An absent log, one that cannot be asked about, and an empty one all answer `false`: an empty log is
/// a post-crash leftover that creates no companion, and a log that cannot be read already has its own
/// refusal further down, so inventing a second one here would make a single failure say two things.
fn has_wal_log(path: &Path) -> bool {
  match wal_log_path(path).and_then(|log| std::fs::metadata(log).ok()) {
    Some(metadata) => metadata.len() > 0,
    None => false,
  }
}

impl ToolLibrary {
  /// Open (and migrate) a library file, creating it when absent.
  pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
    let path = path.as_ref();
    let display = path.display().to_string();
    if let Some(parent) = path.parent() {
      if !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent).map_err(|source| StoreError::Io {
          path: parent.display().to_string(),
          source,
        })?;
      }
    }
    let connection = Connection::open(path).map_err(|source| StoreError::ToolLibrary {
      path: display.clone(),
      reason: source.to_string(),
    })?;
    let mut library = Self {
      connection,
      path: display,
    };
    library.migrate()?;
    Ok(library)
  }

  /// Open an existing library **read-only**: no directory is created, no file is created, nothing is
  /// ever written, and no migration runs.
  ///
  /// This is the constructor for a caller that reads a library it does not own — the window, resolving
  /// the `tools_ref` a document names (camui-shell S3). It is a connection flag rather than a check on
  /// the path, because the flag is enforced by SQLite for every statement the connection ever runs: the
  /// window reads a library and cannot write one, whatever it asks the connection to do. A check on the
  /// path, or a canonicalised comparison of it, would only decide *which* file the window is willing to
  /// write to, so a symlink beside a document could still make the writer create a library in a file
  /// outside the document's directory — which is exactly the hole this constructor closes.
  ///
  /// The price is that a library this build cannot read without writing is **refused** rather than
  /// repaired: a file that has never been initialised has no schema to read, a library left at an older
  /// [`MIGRATIONS`] rung needs a migration this constructor will not perform, and a library is refused
  /// whenever reading it would make SQLite create a companion beside it — one whose header declares
  /// **WAL** journal mode, and one that carries a non-empty `-wal` log beside a header that does not,
  /// since SQLite builds its `-shm` index for either. All are [`StoreError::ToolLibrary`] errors naming
  /// the file and the reason. An owner that means to create or migrate its own library still calls
  /// [`Self::open`], which keeps doing exactly that.
  pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, StoreError> {
    let path = path.as_ref();
    let display = path.display().to_string();
    // Both companion questions are asked before there is a connection to ask with: see `declares_wal`
    // and `has_wal_log` for why the file and its siblings are the only places they can be answered
    // without producing the very files they are about.
    if declares_wal(path) {
      return Err(StoreError::ToolLibrary {
        path: display,
        reason:
          "it is in WAL journal mode, and reading one makes SQLite create -shm and -wal files \
                 beside it; a library is never written by reading it, so this one is refused"
            .to_owned(),
      });
    }
    if has_wal_log(path) {
      return Err(StoreError::ToolLibrary {
        path: display,
        reason:
          "it has a -wal log beside it, and reading one makes SQLite create its -shm index beside \
                 the library; a library is never written by reading it, so this one is refused"
            .to_owned(),
      });
    }
    let connection = Connection::open_with_flags(
      path,
      OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|source| StoreError::ToolLibrary {
      path: display.clone(),
      reason: source.to_string(),
    })?;
    let library = Self {
      connection,
      path: display,
    };
    let version = library
      .stored_version()
      .map_err(|source| StoreError::ToolLibrary {
        path: library.path.clone(),
        reason: format!(
          "it holds no tool library schema, so it has never been initialised ({source})"
        ),
      })?;
    match version {
      Some(version) if version == head_version() => Ok(library),
      Some(version) => Err(StoreError::ToolLibrary {
        path: library.path.clone(),
        reason: format!(
          "it is at schema version {version} and this build reads version {}; a library is never \
           moved forward by reading it",
          head_version()
        ),
      }),
      None => Err(StoreError::ToolLibrary {
        path: library.path.clone(),
        reason: "it holds no schema version, so it has never been initialised".to_owned(),
      }),
    }
  }

  /// Open an in-memory library. Used by tests and by tools that need a scratch library.
  pub fn open_in_memory() -> Result<Self, StoreError> {
    let connection = Connection::open_in_memory().map_err(|source| StoreError::ToolLibrary {
      path: "<memory>".to_owned(),
      reason: source.to_string(),
    })?;
    let mut library = Self {
      connection,
      path: "<memory>".to_owned(),
    };
    library.migrate()?;
    Ok(library)
  }

  /// The path this library was opened from.
  pub fn path(&self) -> &str {
    &self.path
  }

  /// The schema version currently stored.
  pub fn schema_version(&self) -> Result<u32, StoreError> {
    let version = self.stored_version().map_err(|source| self.error(source))?;
    Ok(version.unwrap_or(0))
  }

  /// The highest version the `migration` table records, or `None` when it records none.
  fn stored_version(&self) -> Result<Option<u32>, rusqlite::Error> {
    self
      .connection
      .query_row("SELECT MAX(version) FROM migration", [], |row| {
        row.get::<_, Option<u32>>(0)
      })
      .optional()
      .map(|version| version.flatten())
  }

  /// Apply every migration the file has not seen yet, in one transaction.
  pub fn migrate(&mut self) -> Result<(), StoreError> {
    let path = self.path.clone();
    self
      .connection
      .execute_batch(
        "CREATE TABLE IF NOT EXISTS migration (
           version INTEGER NOT NULL,
           subversion INTEGER NOT NULL,
           name TEXT NOT NULL,
           checksum TEXT NOT NULL,
           PRIMARY KEY (version, subversion)
         );",
      )
      .map_err(|source| StoreError::ToolLibrary {
        path: path.clone(),
        reason: source.to_string(),
      })?;

    let current: Option<u32> = self
      .connection
      .query_row("SELECT MAX(version) FROM migration", [], |row| row.get(0))
      .optional()
      .map_err(|source| StoreError::ToolLibrary {
        path: path.clone(),
        reason: source.to_string(),
      })?
      .flatten();
    let current = current.unwrap_or(0);

    for migration in MIGRATIONS.iter().filter(|m| m.version > current) {
      let transaction =
        self
          .connection
          .transaction()
          .map_err(|source| StoreError::ToolLibrary {
            path: path.clone(),
            reason: source.to_string(),
          })?;
      transaction
        .execute_batch(migration.sql)
        .map_err(|source| StoreError::ToolLibrary {
          path: path.clone(),
          reason: format!(
            "migration {} ({}) failed: {source}",
            migration.version, migration.name
          ),
        })?;
      transaction
        .execute(
          "INSERT INTO migration (version, subversion, name, checksum) VALUES (?1, 0, ?2, ?3)",
          params![migration.version, migration.name, checksum(migration.sql)],
        )
        .map_err(|source| StoreError::ToolLibrary {
          path: path.clone(),
          reason: source.to_string(),
        })?;
      transaction
        .commit()
        .map_err(|source| StoreError::ToolLibrary {
          path: path.clone(),
          reason: source.to_string(),
        })?;
    }
    Ok(())
  }

  /// Insert or replace a tool geometry.
  pub fn put_geometry(&self, geometry: &ToolGeometry) -> Result<(), StoreError> {
    self
      .connection
      .execute(
        "INSERT INTO tool_geometry
           (id, name_format, notes, tool_type, units, diameter, included_angle, flat_diameter,
            num_flutes, flute_length, tip_radius, shank_diameter, overall_length, custom_attributes)
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(id) DO UPDATE SET
           name_format = excluded.name_format, tool_type = excluded.tool_type,
           units = excluded.units, diameter = excluded.diameter,
           included_angle = excluded.included_angle, flat_diameter = excluded.flat_diameter,
           num_flutes = excluded.num_flutes, flute_length = excluded.flute_length,
           tip_radius = excluded.tip_radius, shank_diameter = excluded.shank_diameter,
           overall_length = excluded.overall_length,
           custom_attributes = excluded.custom_attributes",
        params![
          geometry.id,
          geometry.name_format,
          geometry.tool_type,
          encode_units(geometry.units),
          geometry.diameter.map(Length::mm),
          geometry.included_angle.map(Angle::degrees),
          geometry.flat_diameter.map(Length::mm),
          geometry.num_flutes,
          geometry.flute_length.map(Length::mm),
          geometry.tip_radius.map(Length::mm),
          geometry.shank_diameter.map(Length::mm),
          geometry.overall_length.map(Length::mm),
          geometry.custom_attributes,
        ],
      )
      .map_err(|source| self.error(source))?;
    Ok(())
  }

  /// Read a tool geometry by id.
  pub fn geometry(&self, id: &str) -> Result<Option<ToolGeometry>, StoreError> {
    self
      .connection
      .query_row(
        "SELECT id, name_format, tool_type, units, diameter, included_angle, flat_diameter,
                num_flutes, flute_length, tip_radius, shank_diameter, overall_length,
                custom_attributes
         FROM tool_geometry WHERE id = ?1",
        params![id],
        |row| {
          Ok(ToolGeometry {
            id: row.get(0)?,
            name_format: row.get(1)?,
            tool_type: row.get(2)?,
            units: decode_units(row.get(3)?),
            diameter: row.get::<_, Option<f64>>(4)?.map(Length::from_mm),
            included_angle: row.get::<_, Option<f64>>(5)?.map(Angle::from_degrees),
            flat_diameter: row.get::<_, Option<f64>>(6)?.map(Length::from_mm),
            num_flutes: row.get(7)?,
            flute_length: row.get::<_, Option<f64>>(8)?.map(Length::from_mm),
            tip_radius: row.get::<_, Option<f64>>(9)?.map(Length::from_mm),
            shank_diameter: row.get::<_, Option<f64>>(10)?.map(Length::from_mm),
            overall_length: row.get::<_, Option<f64>>(11)?.map(Length::from_mm),
            custom_attributes: row.get(12)?,
          })
        },
      )
      .optional()
      .map_err(|source| self.error(source))
  }

  /// Insert or replace cutting data.
  pub fn put_cutting_data(&self, data: &CuttingData) -> Result<(), StoreError> {
    self
      .connection
      .execute(
        "INSERT INTO tool_cutting_data
           (id, rate_units, feed_rate, plunge_rate, spindle_speed, spindle_dir, stepdown,
            stepover, clear_stepover, tool_number, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
           rate_units = excluded.rate_units, feed_rate = excluded.feed_rate,
           plunge_rate = excluded.plunge_rate, spindle_speed = excluded.spindle_speed,
           stepdown = excluded.stepdown, stepover = excluded.stepover,
           clear_stepover = excluded.clear_stepover, tool_number = excluded.tool_number,
           notes = excluded.notes",
        params![
          data.id,
          encode_units(data.rate_units),
          data.feed_rate,
          data.plunge_rate,
          data.spindle_speed,
          data.stepdown.map(Length::mm),
          data.stepover.map(Length::mm),
          data.clear_stepover.map(Length::mm),
          data.tool_number,
          data.notes,
        ],
      )
      .map_err(|source| self.error(source))?;
    Ok(())
  }

  /// Read cutting data by id.
  pub fn cutting_data(&self, id: &str) -> Result<Option<CuttingData>, StoreError> {
    self
      .connection
      .query_row(
        "SELECT id, rate_units, feed_rate, plunge_rate, spindle_speed, stepdown, stepover,
                clear_stepover, tool_number, notes
         FROM tool_cutting_data WHERE id = ?1",
        params![id],
        |row| {
          Ok(CuttingData {
            id: row.get(0)?,
            rate_units: decode_units(row.get(1)?),
            feed_rate: row.get(2)?,
            plunge_rate: row.get(3)?,
            spindle_speed: row.get(4)?,
            stepdown: row.get::<_, Option<f64>>(5)?.map(Length::from_mm),
            stepover: row.get::<_, Option<f64>>(6)?.map(Length::from_mm),
            clear_stepover: row.get::<_, Option<f64>>(7)?.map(Length::from_mm),
            tool_number: row.get(8)?,
            notes: row.get(9)?,
          })
        },
      )
      .optional()
      .map_err(|source| self.error(source))
  }

  /// Record that `geometry` is used with `cutting` in a given material and machine.
  pub fn link(
    &self,
    id: &str,
    geometry_id: &str,
    cutting_id: &str,
    material_id: Option<&str>,
    machine_id: Option<&str>,
  ) -> Result<(), StoreError> {
    self
      .connection
      .execute(
        "INSERT INTO tool_entity (id, material_id, machine_id, tool_geometry_id, tool_cutting_data_id)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET
           material_id = excluded.material_id, machine_id = excluded.machine_id,
           tool_geometry_id = excluded.tool_geometry_id, tool_cutting_data_id = excluded.tool_cutting_data_id",
        params![id, material_id, machine_id, geometry_id, cutting_id],
      )
      .map_err(|source| self.error(source))?;
    Ok(())
  }

  /// The cutting data for a tool in a material on a machine, if the library has it.
  pub fn cutting_data_for(
    &self,
    geometry_id: &str,
    material_id: Option<&str>,
    machine_id: Option<&str>,
  ) -> Result<Option<CuttingData>, StoreError> {
    let cutting_id: Option<String> = self
      .connection
      .query_row(
        "SELECT tool_cutting_data_id FROM tool_entity
         WHERE tool_geometry_id = ?1
           AND material_id IS ?2
           AND machine_id IS ?3",
        params![geometry_id, material_id, machine_id],
        |row| row.get(0),
      )
      .optional()
      .map_err(|source| self.error(source))?;
    match cutting_id {
      Some(id) => self.cutting_data(&id),
      None => Ok(None),
    }
  }

  /// Every tool in the library, geometry paired with its cutting data.
  pub fn tools(&self) -> Result<Vec<StoredTool>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT tool_geometry_id, tool_cutting_data_id FROM tool_entity ORDER BY id")
      .map_err(|source| StoreError::ToolLibrary {
        path: self.path.clone(),
        reason: source.to_string(),
      })?;
    let pairs = statement
      .query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
      })
      .map_err(|source| StoreError::ToolLibrary {
        path: self.path.clone(),
        reason: source.to_string(),
      })?;
    let mut tools = Vec::new();
    for pair in pairs {
      let (geometry_id, cutting_id) = pair.map_err(|source| StoreError::ToolLibrary {
        path: self.path.clone(),
        reason: source.to_string(),
      })?;
      if let (Some(geometry), Some(cutting)) = (
        self.geometry(&geometry_id)?,
        self.cutting_data(&cutting_id)?,
      ) {
        tools.push(StoredTool { geometry, cutting });
      }
    }
    Ok(tools)
  }

  /// The tool whose cutting data carries this program number.
  ///
  /// Reads the library as a snapshot and hands it to [`stored_tool_for_number`], which is what the
  /// shell's tool card resolves the same number with: one rule, so a tool the emitter can find is a
  /// tool the card can describe and a number the emitter refuses is a number the card says is missing.
  /// The cost is one scan of the library per call instead of one indexed query — the price of the two
  /// agreeing by construction rather than by a copy of the query (camui-toolpath S3).
  pub fn tool_for_number(
    &self,
    number: u32,
  ) -> Result<Option<(ToolGeometry, CuttingData)>, StoreError> {
    let tools = self.tools()?;
    Ok(
      stored_tool_for_number(&tools, number)
        .map(|tool| (tool.geometry.clone(), tool.cutting.clone())),
    )
  }

  /// Record a material.
  pub fn put_material(&self, material: &Material) -> Result<(), StoreError> {
    self
      .connection
      .execute(
        "INSERT INTO material (id, name) VALUES (?1, ?2)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name",
        params![material.id, material.name],
      )
      .map_err(|source| self.error(source))?;
    Ok(())
  }

  /// Record a machine.
  pub fn put_machine(&self, machine: &Machine) -> Result<(), StoreError> {
    self
      .connection
      .execute(
        "INSERT INTO machine
           (id, name, make, model, controller_type, max_width, max_height, support_rotary,
            support_tool_change)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0)
         ON CONFLICT(id) DO UPDATE SET
           name = excluded.name, make = excluded.make, model = excluded.model,
           controller_type = excluded.controller_type, max_width = excluded.max_width,
           max_height = excluded.max_height",
        params![
          machine.id,
          machine.name,
          machine.make,
          machine.model,
          machine.controller_type,
          machine.max_width.map(Length::mm),
          machine.max_height.map(Length::mm),
        ],
      )
      .map_err(|source| self.error(source))?;
    Ok(())
  }

  /// The store's error for one of this library's own paths.
  fn error(&self, source: rusqlite::Error) -> StoreError {
    StoreError::ToolLibrary {
      path: self.path.clone(),
      reason: source.to_string(),
    }
  }
  /// The store's error for one of this library's own paths, for [`crate::tool_edit`] to raise.
  ///
  /// The writer lives in its own module so the validation rules are reviewable apart from the SQL, and it
  /// has to be able to say which library refused a write. Taking `&self` rather than a path string is what
  /// keeps one sentence per failure: two spellings of "this library refused" would be two.
  pub(crate) fn failure(&self, source: rusqlite::Error) -> StoreError {
    self.error(source)
  }

  /// Open a transaction, naming what it is for.
  ///
  /// `unchecked_transaction` and not `transaction`, because the writer takes `&self`: every write in this
  /// module is a single statement SQLite has already made atomic, and the three-row writes in
  /// [`crate::tool_edit`] are the first that need more than that.
  pub(crate) fn begin(&self, what: &str) -> Result<rusqlite::Transaction<'_>, StoreError> {
    self
      .connection
      .unchecked_transaction()
      .map_err(|source| StoreError::ToolLibrary {
        path: self.path.clone(),
        reason: format!("could not {what}: {source}"),
      })
  }
}

/// The tool a program number names, read from a snapshot of the library.
///
/// `tools` is the library in its own order — the order [`ToolLibrary::tools`] returns, which is
/// `tool_entity.id` ascending, the order the library itself resolves a number in. A number more than
/// one tool carries resolves to the first of them in that order, and a number no tool carries resolves
/// to `None` rather than to a neighbour.
///
/// This is the one place a number becomes a tool. [`ToolLibrary::tool_for_number`] calls it, and so
/// does the shell's tool card, which is what keeps the two from disagreeing: a card that said "T7 is
/// not in the library" beside a program that cuts with T7 would be a second opinion about a library
/// that has only one answer (camui-toolpath S3, T1).
pub fn stored_tool_for_number(tools: &[StoredTool], number: u32) -> Option<&StoredTool> {
  tools
    .iter()
    .find(|tool| tool.cutting.tool_number == Some(number))
}

/// Render a tool display name from its template.
///
/// Supported fields: `Tool Type`, `Diameter`, `Flat Diameter`, `Included Angle`, `Tip Radius`,
/// `Flutes`, `Units Short`. A field may carry a format suffix, as in `{Diameter|F}` where `F` means
/// "fewest decimals that keep the value exact up to three places".
pub fn render_name(
  template: &str,
  tool_type_label: &str,
  geometry: &ToolGeometry,
) -> Result<String, String> {
  let units = geometry.units;
  let mut output = String::with_capacity(template.len() + 16);
  let mut rest = template;
  while let Some(start) = rest.find('{') {
    output.push_str(&rest[..start]);
    let Some(end) = rest[start..].find('}') else {
      return Err(format!("unclosed field in {template:?}"));
    };
    let field = &rest[start + 1..start + end];
    rest = &rest[start + end + 1..];
    let (name, format) = match field.split_once('|') {
      Some((name, format)) => (name, Some(format)),
      None => (field, None),
    };
    output.push_str(&resolve_field(
      name,
      format,
      tool_type_label,
      geometry,
      units,
    )?);
  }
  output.push_str(rest);
  Ok(output)
}

fn resolve_field(
  name: &str,
  format: Option<&str>,
  tool_type_label: &str,
  geometry: &ToolGeometry,
  units: UnitSystem,
) -> Result<String, String> {
  let number = |value: f64| format_number(value, format, name);

  match name {
    "Tool Type" => Ok(tool_type_label.to_owned()),
    "Diameter" => Ok(number(
      geometry
        .diameter
        .ok_or_else(|| format!("field {name:?} is not set on this tool"))?
        .value_in(units),
    )?),
    "Flat Diameter" => Ok(number(
      geometry
        .flat_diameter
        .ok_or_else(|| format!("field {name:?} is not set on this tool"))?
        .value_in(units),
    )?),
    "Tip Radius" => Ok(number(
      geometry
        .tip_radius
        .ok_or_else(|| format!("field {name:?} is not set on this tool"))?
        .value_in(units),
    )?),
    "Included Angle" => Ok(number(
      geometry
        .included_angle
        .ok_or_else(|| format!("field {name:?} is not set on this tool"))?
        .degrees(),
    )?),
    "Flutes" => geometry
      .num_flutes
      .map(|flutes| flutes.to_string())
      .ok_or_else(|| format!("field {name:?} is not set on this tool")),
    "Units Short" => Ok(units.short_name().to_owned()),
    other => Err(format!("unknown field {other:?}")),
  }
}

/// Render a number under the template's format suffix.
///
/// The analysed library uses two spellings for the same idea: `F` (fewest decimals that keep the
/// value readable) and a printf-style precision such as `.0` or `.2`.
fn format_number(value: f64, format: Option<&str>, field: &str) -> Result<String, String> {
  match format {
    None => Ok(Format::fixed(0).apply(value)),
    Some("F") => {
      let text = Format::fixed(3).apply(value);
      let trimmed = text.trim_end_matches('0').trim_end_matches('.');
      Ok(if trimmed.is_empty() || trimmed == "-" {
        "0".to_owned()
      } else {
        trimmed.to_owned()
      })
    }
    Some(spec) => {
      let digits = spec.strip_prefix('.').unwrap_or(spec);
      let decimals = digits
        .parse::<u8>()
        .map_err(|_| format!("unknown number format {spec:?} in field {field:?}"))?;
      Ok(Format::fixed(decimals).apply(value))
    }
  }
}

fn checksum(sql: &str) -> String {
  // A stable, dependency-free digest: FNV-1a over the migration text.
  let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
  for byte in sql.as_bytes() {
    hash ^= u64::from(*byte);
    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
  }
  format!("{hash:016x}")
}

pub(crate) fn encode_units(units: UnitSystem) -> i64 {
  match units {
    UnitSystem::Mm => 0,
    UnitSystem::Inch => 1,
  }
}

fn decode_units(value: i64) -> UnitSystem {
  if value == 0 {
    UnitSystem::Mm
  } else {
    UnitSystem::Inch
  }
}

#[cfg(test)]
mod tests {
  use super::{stored_tool_for_number, CuttingData, StoredTool, ToolGeometry, ToolLibrary};
  use cam_units::{Length, UnitSystem};

  /// One stored tool, geometry and cutting data, with `number` as its program number.
  fn stored(id: &str, number: Option<u32>) -> StoredTool {
    StoredTool {
      geometry: ToolGeometry {
        id: format!("{id}-geometry"),
        name_format: "{Tool Type} ({Diameter|F}{Units Short})".to_owned(),
        tool_type: 0,
        units: UnitSystem::Mm,
        diameter: Some(Length::from_mm(6.0)),
        included_angle: None,
        flat_diameter: None,
        num_flutes: Some(2),
        flute_length: None,
        tip_radius: None,
        shank_diameter: None,
        overall_length: None,
        custom_attributes: None,
      },
      cutting: CuttingData {
        id: format!("{id}-cutting"),
        rate_units: UnitSystem::Mm,
        feed_rate: Some(100.0),
        plunge_rate: Some(30.0),
        spindle_speed: Some(18_000),
        stepdown: None,
        stepover: None,
        clear_stepover: None,
        tool_number: number,
        notes: None,
      },
    }
  }

  #[test]
  fn the_first_tool_in_library_order_carrying_the_number_is_the_one_that_resolves() {
    let tools = vec![
      stored("E1", Some(1)),
      stored("E2", Some(7)),
      stored("E3", Some(7)),
    ];
    assert_eq!(
      stored_tool_for_number(&tools, 7).map(|tool| tool.geometry.id.as_str()),
      Some("E2-geometry"),
      "two entities carrying one number resolve to the first in library order"
    );
    assert_eq!(
      stored_tool_for_number(&tools, 1).map(|tool| tool.geometry.id.as_str()),
      Some("E1-geometry")
    );
    assert!(
      stored_tool_for_number(&tools, 9).is_none(),
      "a number no tool carries resolves to nothing rather than to a neighbour"
    );
    assert!(
      stored_tool_for_number(&[stored("E1", None)], 1).is_none(),
      "a tool with no program number carries no number"
    );
  }

  #[test]
  fn the_library_and_a_snapshot_of_it_resolve_a_number_the_same_way() {
    let library = ToolLibrary::open_in_memory().expect("opens");
    for (id, number) in [("E2", 7_u32), ("E1", 7_u32), ("E3", 4_u32)] {
      let tool = stored(id, Some(number));
      library.put_geometry(&tool.geometry).expect("geometry");
      library.put_cutting_data(&tool.cutting).expect("cutting");
      library
        .link(id, &tool.geometry.id, &tool.cutting.id, None, None)
        .expect("link");
    }
    let snapshot = library.tools().expect("the library reads");

    for number in [1_u32, 4, 7, 9] {
      let from_library = library.tool_for_number(number).expect("the query runs");
      assert_eq!(
        from_library
          .as_ref()
          .map(|(geometry, _cutting)| geometry.id.as_str()),
        stored_tool_for_number(&snapshot, number).map(|tool| tool.geometry.id.as_str()),
        "T{number} must resolve to the same tool whether it is asked of the library or of a snapshot of it"
      );
    }
  }
}
