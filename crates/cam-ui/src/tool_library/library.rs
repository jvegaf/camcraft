//! Where the application's own tool library lives, and how the window opens it.
//!
//! The library belongs to the **application** and not to any drawing (tool-library-app S1): one `tools.sqlite`
//! in the application's data directory, the same tools for every document the window opens and still there
//! after the document is closed. A document may still name a `tools_ref` and `cam-cli` still honours it; this
//! window does not read it, because a library a drawing owns is exactly what S1 removed.
//!
//! Three decisions worth writing down:
//!
//! 1. **The directory is one function, and the override is one variable.** [`app_data_dir`] answers where the
//!    library lives, and `CAM_PILOT_DATA_DIR` is its only override — which is how a person keeps two libraries
//!    and how a test keeps its own. Nothing else in this crate reads the environment.
//! 2. **The snapshot is still handed in, not held.** [`Shell::open_library_at`] opens the file, reads it and
//!    calls [`Shell::set_tools_from`], so every rule about writing a library that
//!    [`crate::tool_library::shell`] already keeps is untouched by where the file came from.
//! 3. **A library is seeded only when the file did not exist.** A fresh install starts with the factory
//!    catalogue instead of an empty screen, and a library a person deliberately emptied stays empty, because
//!    re-seeding it would undo the emptying they asked for (S2, S8).

use std::path::{Path, PathBuf};

use cam_store::tool_edit::factory_tools;
use cam_store::tools::ToolLibrary;

use crate::Shell;

/// The file the application's library is kept in, inside its data directory.
pub const LIBRARY_FILE: &str = "tools.sqlite";

/// The sentence the modal shows when the application could not open its library.
pub(crate) const NO_LIBRARY: &str =
  "The application could not open its tool library, so these actions are unavailable.";

/// The directory the application keeps its data in.
///
/// `CAM_PILOT_DATA_DIR` when it is set and not empty, and otherwise the platform's per-user data directory.
pub fn app_data_dir() -> Option<PathBuf> {
  data_dir_from(
    std::env::var_os("CAM_PILOT_DATA_DIR"),
    std::env::var_os("XDG_DATA_HOME"),
    std::env::var_os("HOME"),
    std::env::var_os("APPDATA"),
  )
}

/// [`app_data_dir`] with every input passed in, so the rule can be read and tested without an environment.
fn data_dir_from(
  override_dir: Option<std::ffi::OsString>,
  xdg_data_home: Option<std::ffi::OsString>,
  home: Option<std::ffi::OsString>,
  appdata: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
  /// The directory name this application's data lives under, on every platform.
  const DIRECTORY: &str = "campilot";

  fn non_empty(value: Option<std::ffi::OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
  }

  if let Some(chosen) = non_empty(override_dir) {
    return Some(chosen);
  }
  if cfg!(windows) {
    // Windows has no XDG convention at all, so `%APPDATA%` is the whole rule there.
    return non_empty(appdata).map(|base| base.join(DIRECTORY));
  }
  non_empty(xdg_data_home)
    .or_else(|| non_empty(home).map(|home| home.join(".local").join("share")))
    .map(|base| base.join(DIRECTORY))
}

/// The library file inside `directory`.
pub fn library_path_in(directory: &Path) -> PathBuf {
  directory.join(LIBRARY_FILE)
}

impl Shell {
  /// Open the application's own library and hand the window its tools.
  ///
  /// A failure is recorded in [`Shell::tool_library_error`] as well as returned, so the modal's own sentence
  /// can say what went wrong rather than going quiet; the caller (the desktop host) decides whether to repeat
  /// it in the status bar.
  pub fn open_app_library(&mut self) -> Result<PathBuf, String> {
    let opened = match app_data_dir() {
      Some(directory) => self.open_library_at(&directory),
      None => Err(NO_LIBRARY.to_owned()),
    };
    if let Err(sentence) = &opened {
      self.tool_library_error = Some(sentence.clone());
    }
    opened
  }

  /// Open — and, when the file is not there yet, create — the library in `directory`.
  ///
  /// Public because a modal whose library cannot be opened from outside a frame is a modal only a mouse could
  /// test: this is the seam the suite opens its libraries through, and the desktop host's own path.
  pub fn open_library_at(&mut self, directory: &Path) -> Result<PathBuf, String> {
    let path = library_path_in(directory);
    let fresh = !path.exists();
    let library = ToolLibrary::open(&path).map_err(|error| error.to_string())?;
    if fresh {
      library
        .replace_tools(&factory_tools())
        .map_err(|error| error.to_string())?;
    }
    let tools = library.tools().map_err(|error| error.to_string())?;
    self.set_tools_from(path.clone(), tools);
    self.tool_library_error = None;
    Ok(path)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn os(value: &str) -> Option<std::ffi::OsString> {
    Some(std::ffi::OsString::from(value))
  }

  #[test]
  fn the_override_wins_over_every_platform_convention() {
    assert_eq!(
      data_dir_from(os("/tmp/mine"), os("/xdg"), os("/home/me"), os("C:\\app")),
      Some(PathBuf::from("/tmp/mine"))
    );
  }

  #[test]
  fn an_empty_override_is_not_an_override() {
    let chosen = data_dir_from(os(""), os("/xdg"), os("/home/me"), os("C:\\app"));
    assert_eq!(chosen, Some(PathBuf::from("/xdg").join("campilot")));
  }

  #[test]
  fn without_xdg_the_unix_answer_is_the_home_data_directory() {
    assert_eq!(
      data_dir_from(None, None, os("/home/me"), None),
      Some(PathBuf::from("/home/me/.local/share/campilot"))
    );
  }

  #[test]
  fn with_nothing_to_go_on_there_is_no_data_directory() {
    assert_eq!(data_dir_from(None, None, None, None), None);
  }
}
