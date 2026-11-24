//! `cam-cli`'s binary: it collects the arguments, calls the library and answers with the exit code.
//!
//! Every command lives in the crate's library (`cam_cli::run`), which is where the shell and a script
//! call the same functions (M10 T3b, D-M10-1), so the binary cannot drift from its callers.

use std::process::ExitCode;

fn main() -> ExitCode {
  match cam_cli::run(&std::env::args().skip(1).collect::<Vec<_>>()) {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("error: {error}");
      ExitCode::from(error.code() as u8)
    }
  }
}
