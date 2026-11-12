//! The command language (M10 T1): a line-oriented script that reads and mutates one document.
//!
//! The language is deliberately **not** a programming language (S1: "DSL de comandos, sin motor"): one
//! command per line, a command word and its arguments, `#` comments, blank lines ignored — and no
//! variables, no conditionals and no loops, because the document is the state (S2) and every command is
//! one of the subcommands `cam-cli` already offers.
//!
//! Two laws hold this file together:
//!
//! 1. **Every command is a caller, never a second implementation.** `open` calls this crate's `open`,
//!    commands with flags hand their tokens to the subcommand's own argument parser and then to
//!    the subcommand's own execution function, and `save` calls
//!    `cam_store::document::save_document`. There is no geometry, nesting-engine, import or
//!    emission step in this file at all (M10 invariant 1).
//! 2. **Every refusal is located**, in the `cam-post` parser's shape — `line {line}:{column}: {message}`
//!    — and every mutation passes through `Document::validate` with the script line attached (M10
//!    invariants 2 and 4).
//! 3. **The report has one exit, and it is a sink.** [`run_script_to`] takes the two `Write`s the
//!    caller chose: `cam-cli script` passes stdout for reports and stderr for notices, which is the
//!    split the binary's own contract has, and a caller that wants to display the whole run — the
//!    shell panel (T4) — passes two handles onto its own output area. Nothing in this file prints a
//!    report or a notice, so a captured run and a printed run cannot disagree about a single byte.
//! 4. **The document can come from the caller.** [`run_script_to`] reads it from the `open` line and
//!    writes it with `save`; [`run_script_on`] mutates a document its caller already owns — the shell's
//!    — as **one transaction**, and refuses `open` and `save`: a file it did not read is not its to
//!    write, and the shell's own Save writes the document.

use std::io::Write;
use std::path::{Path, PathBuf};

use cam_model::{Document, Nesting, NestingPart, NestingSheet};
use cam_units::Length;

use crate::pieces::{author_parts, cut_list_document, cutlist_args, parts_args, save_report};
use crate::{fresh_part_id, fresh_sheet_id, only_path, validate_report, write_report, Failure};

/// Why a command that needs the document says so: a script reads one, it never guesses at one.
const NO_DOCUMENT: &str = "no document is open: a script reads one with `open <document>`";

/// Why a scripted command takes no document path of its own.
const NO_POSITIONAL: &str = "takes no document path: a script mutates the document `open` read";

/// Why a scripted command takes no `--out`.
const NO_OUT: &str = "takes no --out: a script writes its document with `save <document>`";

/// Why a command that resolves a file beside the document refuses an untitled one.
const NO_PATH: &str = "this document has no path yet: a command that reads a file beside it needs one, so save the document first";

/// Why a script that mutates its caller's document has nothing to open.
const ALREADY_OPEN: &str = "a script run from the shell already has a document open: it mutates the shell's own, so `open` has nothing to read";

/// Why a script that mutates its caller's document writes nothing.
const NO_WRITE: &str = "a script run from the shell writes no file: the document belongs to the shell, and its Save writes it";

/// One command the language knows: its word, what its line reads like, and the function that runs it.
///
/// The table **is** the dispatch: [`Session::execute`] looks the word up here and calls the pointer, so
/// a command cannot be dispatched and undocumented, and the unknown-command refusal prints this list.
/// Every entry below names the `cam-cli` subcommand and the function it calls, which is the whole of
/// M10 invariant 1 in one place.
struct Command {
  /// The word that starts a line.
  word: &'static str,
  /// What follows the word, for the refusal that says how many arguments a line had.
  usage: &'static str,
  /// The one function this command runs.
  run: fn(&mut Session, &ScriptLine) -> Result<(), Failure>,
}

const COMMANDS: &[Command] = &[
  // `cam-cli`'s document loader (`crate::open`), the same one every subcommand reads with.
  Command {
    word: "open",
    usage: "<document>",
    run: command_open,
  },
  // Authors a `NestingSheet` — named by the
  // `fresh_sheet_id` scan, so a scripted `sheet` that authors one board per line names them.
  Command {
    word: "sheet",
    usage: "<width> <height> in millimetres",
    run: command_sheet,
  },
  // Authors a `NestingPart`, named by the `fresh_part_id` scan.
  Command {
    word: "part",
    usage: "<instances> <entity ids>",
    run: command_part,
  },
  // `cam-cli validate`: `Document::validate` and `validate_report`, the subcommand's own report.
  Command {
    word: "validate",
    usage: "no arguments",
    run: command_validate,
  },
  // `cam_store::document::save_document`, the one writer, which validates as it writes.
  Command {
    word: "save",
    usage: "<document>",
    run: command_save,
  },
  // `cam-cli parts`: `parts_args` for the flags, `author_parts` for the import.
  Command {
    word: "parts",
    usage: "--csv <file> [--layer <name>]",
    run: command_parts,
  },
  // `cam-cli cutlist`: `cutlist_args` for the flag, `cut_list_document` for the rows.
  Command {
    word: "cutlist",
    usage: "[--out <path>]",
    run: command_cutlist,
  },
];

/// Run a script: `cam-cli script <file>`.
///
/// The script's own path is the one thing this command reads from the command line; every path a
/// command needs comes from the script itself, so nothing is read from the environment, the working
/// directory or another file (M10 invariant 3).
pub(crate) fn script_command(rest: &[String]) -> Result<(), Failure> {
  let path = only_path(rest, "script")?;
  run_script(&path, &mut std::io::stdout())
}

/// Run a script, writing every report its commands produce to `out`.
///
/// This is the shape the binary's contract has always had: the reports go to the caller's sink and the
/// one notice a script prints — `save`'s `wrote …` line — goes to this process's stderr, which is where
/// a command line expects a progress line. A caller with one output area calls [`run_script_to`]
/// instead and points both writers at it.
///
/// The refusals are the same [`Failure`] either way, each carrying the `line {line}:{column}: {message}`
/// diagnostic of the line that caused it (M10 invariant 2).
pub fn run_script(path: &Path, out: &mut dyn Write) -> Result<(), Failure> {
  run_script_to(path, out, &mut std::io::stderr())
}

/// Run a script, writing every report to `reports` and every notice to `notices`.
///
/// **Reports** are the answers the document's commands produce — `valid: …`, the cut list,
/// the part list's own report — and **notices** are the one line `save` writes once the document is on
/// disk. The two are separate streams because the binary separates them: a command's data is stdout and
/// its progress line is stderr, and `crates/cam-cli/tests/script.rs` pins both. The shell's scripting
/// panel (T4b) wants one area, so it passes two handles onto one sink and shows the run in the order
/// the script wrote it.
pub fn run_script_to(
  path: &Path,
  reports: &mut dyn Write,
  notices: &mut dyn Write,
) -> Result<(), Failure> {
  let text = read_script(path)?;
  let mut session = Session {
    document: None,
    path: None,
    in_memory: false,
    out: reports,
    notices,
  };
  run_lines(&text, &mut session)
}

/// Run the script at `script` against `document`, which the caller owns.
///
/// This is the entry point the shell's scripting panel calls: the document is the one the shell already
/// has open, so a run mutates the caller's document and writes no file. `document_path` is the path the
/// document was loaded from — `None` when it is untitled — and it is used for one thing: the commands
/// that resolve a file beside the document need it, exactly as they do from a command line.
///
/// **The whole run is one transaction.** The lines run against a copy, and the copy replaces `document`
/// only when every line was accepted, so a refusal leaves the caller's document as it was — which is
/// what a run whose result the user has not seen must do. Each accepted mutation is still validated on
/// the way, in [`Session::mutate`]: a run cannot buy a document the model would refuse.
///
/// Two commands belong to the file rather than to the document, and a script in this shape refuses both:
/// `open` has nothing to read, because the shell's document is already open, and `save` writes no file,
/// because the shell's own Save does. A script written for `cam-cli script` therefore says so on the
/// line that tried, instead of quietly editing a second document.
pub fn run_script_on(
  script: &Path,
  document: &mut Document,
  document_path: Option<&Path>,
  reports: &mut dyn Write,
  notices: &mut dyn Write,
) -> Result<(), Failure> {
  let text = read_script(script)?;
  let mut session = Session {
    document: Some(document.clone()),
    path: document_path.map(Path::to_path_buf),
    in_memory: true,
    out: reports,
    notices,
  };
  run_lines(&text, &mut session)?;
  *document = session
    .document
    .take()
    .expect("a run against the caller's document always holds it");
  Ok(())
}

/// Read the script at `path`, refusing with the path in the message.
fn read_script(path: &Path) -> Result<String, Failure> {
  std::fs::read_to_string(path)
    .map_err(|error| Failure::input(format!("{}: {error}", path.display())))
}

/// Run every line of `text`, in order, and stop at the first refusal.
fn run_lines(text: &str, session: &mut Session<'_>) -> Result<(), Failure> {
  for (index, raw) in text.lines().enumerate() {
    let line = lex(index + 1, raw)?;
    if line.tokens.is_empty() {
      continue;
    }
    session.execute(&line)?;
  }
  Ok(())
}

/// The script's state: the one document it mutates, the path it was read from, and the sinks its
/// reports and notices go to.
///
/// The path is kept because `tools_ref` is resolved relative to the document, exactly as the
/// subcommands resolve it. Nothing else of the process is read, and nothing is printed: `out` and
/// `notices` are the only exits either stream has.
struct Session<'sink> {
  document: Option<Document>,
  path: Option<PathBuf>,
  /// Whether the document belongs to the caller — a shell, not a file — so no command may read or
  /// write a file for it. See [`run_script_on`].
  in_memory: bool,
  /// The `Write` the caller of [`run_script_to`] chose for reports.
  out: &'sink mut dyn Write,
  /// The `Write` the caller of [`run_script_to`] chose for notices.
  notices: &'sink mut dyn Write,
}

impl Session<'_> {
  /// Write a command's report to the caller's sink.
  fn say(&mut self, report: &str) -> Result<(), Failure> {
    write_report(&mut *self.out, report)
  }

  /// Write a notice — a line about what a command did to the disk — to the caller's notice sink.
  fn notice(&mut self, notice: &str) -> Result<(), Failure> {
    write_report(&mut *self.notices, notice)
  }

  /// Run one line's command, through the table.
  fn execute(&mut self, line: &ScriptLine) -> Result<(), Failure> {
    let word = line.word();
    let Some(command) = COMMANDS.iter().find(|command| command.word == word.text) else {
      return Err(line.fail_at(
        word,
        format!(
          "unknown script command {:?}; expected one of {}",
          word.text,
          COMMANDS
            .iter()
            .map(|command| command.word)
            .collect::<Vec<_>>()
            .join(", ")
        ),
      ));
    };
    (command.run)(self, line)
  }

  /// The open document, or the refusal that says a script has read none.
  ///
  /// A command that reads nothing beside the document asks for this and no more: `validate` and
  /// `cutlist` answer from the document alone, and an untitled document — what a shell holds before its
  /// first save — is a perfectly good document to answer about.
  fn the_document(&self, line: &ScriptLine) -> Result<&Document, Failure> {
    self
      .document
      .as_ref()
      .ok_or_else(|| line.fail(line.word().column, NO_DOCUMENT))
  }

  /// The open document and the path it was read from, or the refusal that says which is missing.
  fn open_document(&self, line: &ScriptLine) -> Result<(&Document, &std::path::Path), Failure> {
    let document = self.the_document(line)?;
    let path = self.document_path(line)?;
    Ok((document, path))
  }

  /// The path of the open document, or the refusal that says a command needs one.
  ///
  /// A document can be open **and** untitled — that is what a shell holds before its first save — and a
  /// command that resolves a file beside it says so, instead of claiming no document is open.
  fn document_path(&self, line: &ScriptLine) -> Result<&std::path::Path, Failure> {
    match (&self.document, &self.path) {
      (_, Some(path)) => Ok(path),
      (Some(_), None) => Err(line.fail(line.word().column, NO_PATH)),
      (None, None) => Err(line.fail(line.word().column, NO_DOCUMENT)),
    }
  }

  /// Apply one command's change to the open document, and keep it only when the model accepts it.
  ///
  /// Every mutation a script can make passes through here: the command runs against a copy, the copy is
  /// validated, and only an accepted document replaces the open one. A refusal therefore leaves the
  /// script's document — and, because nothing is written before `save`, the file on disk — exactly as
  /// it was, and the model's own message is reported with the line of the command that caused it (M10
  /// invariant 4).
  fn mutate<T>(
    &mut self,
    line: &ScriptLine,
    command: impl FnOnce(&mut Document) -> Result<T, Failure>,
  ) -> Result<T, Failure> {
    let Some(document) = self.document.as_mut() else {
      return Err(line.fail(line.word().column, NO_DOCUMENT));
    };
    let mut working = document.clone();
    let value = command(&mut working)?;
    working
      .validate()
      .map_err(|error| line.at_word(Failure::input(error.to_string())))?;
    *document = working;
    Ok(value)
  }
}

/// One token of a line: its text and the 1-based byte column it starts at.
struct Token {
  text: String,
  column: usize,
}

/// One line of the script: its 1-based number and its tokens.
///
/// Every refusal is built through the line, so no command can forget where it was: the diagnostic is
/// `line {line}:{column}: {message}`, which is what `cam-post`'s parser prints for its own dialect (M10
/// invariant 2).
struct ScriptLine {
  number: usize,
  tokens: Vec<Token>,
  /// Where an argument the line does not have would have gone: one past the last token, counting a
  /// quoted token's two quotes as part of it.
  end_column: usize,
}

impl ScriptLine {
  /// The command word.
  fn word(&self) -> &Token {
    &self.tokens[0]
  }

  /// Everything after the command word.
  fn arguments(&self) -> &[Token] {
    &self.tokens[1..]
  }

  /// The line's arguments as the strings a subcommand's own parser takes.
  fn tokens_as_strings(&self) -> Vec<String> {
    self
      .arguments()
      .iter()
      .map(|token| token.text.clone())
      .collect()
  }

  /// The failure said at `column`, in the diagnostic shape (M10 invariant 2).
  fn fail(&self, column: usize, message: impl Into<String>) -> Failure {
    Failure::input(message).at(self.number, column)
  }

  /// The same failure, said at the token it is about.
  fn fail_at(&self, token: &Token, message: impl Into<String>) -> Failure {
    self.fail(token.column, message)
  }

  /// A failure the command is about as a whole, said at its word.
  fn at_word(&self, failure: Failure) -> Failure {
    failure.at(self.number, self.word().column)
  }

  /// What follows this line's command word, from the table that dispatches it.
  ///
  /// The usage text lives in the table, so a command's line and its dispatch cannot describe two
  /// different things.
  fn usage(&self) -> &'static str {
    COMMANDS
      .iter()
      .find(|command| command.word == self.word().text)
      .map_or("", |command| command.usage)
  }

  /// The arguments of a command that takes positional ones, or the refusal that says how many it got.
  fn exactly(&self, count: usize) -> Result<&[Token], Failure> {
    let usage = self.usage();
    let arguments = self.arguments();
    if let Some(extra) = arguments.get(count) {
      return Err(self.fail_at(
        extra,
        format!(
          "{} expects {usage}, got {} argument(s)",
          self.word().text,
          arguments.len()
        ),
      ));
    }
    if arguments.len() < count {
      return Err(self.fail(
        self.end_column,
        format!(
          "{} expects {usage}, got {} argument(s)",
          self.word().text,
          arguments.len()
        ),
      ));
    }
    Ok(arguments)
  }

  /// The argument at `index`, or the refusal that says the command needed one more.
  fn argument(&self, index: usize, what: &str) -> Result<&Token, Failure> {
    self.arguments().get(index).ok_or_else(|| {
      self.fail(
        self.end_column,
        format!("{} expects {what}, got nothing", self.word().text),
      )
    })
  }

  /// The argument at `index` as a number, or the refusal that names the token it could not read.
  fn number(&self, index: usize, what: &str) -> Result<f64, Failure> {
    let token = self.argument(index, what)?;
    token.text.parse::<f64>().map_err(|_| {
      self.fail_at(
        token,
        format!("{} expects {what}, got {:?}", self.word().text, token.text),
      )
    })
  }

  /// The argument at `index` as a whole number, or the refusal that names the token it could not read.
  fn count(&self, index: usize, what: &str) -> Result<usize, Failure> {
    let token = self.argument(index, what)?;
    token.text.parse::<usize>().map_err(|_| {
      self.fail_at(
        token,
        format!("{} expects {what}, got {:?}", self.word().text, token.text),
      )
    })
  }

  /// The token whose text is `text`, or the command word when no token carries it.
  fn token_for(&self, text: &str) -> &Token {
    self
      .tokens
      .iter()
      .find(|token| token.text == text)
      .unwrap_or_else(|| self.word())
  }

  /// A refusal written by a shared parser, said where it is about.
  ///
  /// A command with flags hands its tokens to the subcommand's own parser, which answers the way a
  /// command line deserves: `unknown option "--x"`, `--gap expects a number of millimetres, got
  /// "abc"`, `--gap must be a non-negative distance in mm, got -1`. Every such refusal quotes the value
  /// it could not use or names the flag it is about, and both are tokens of this line, so the position
  /// is read back from the message. When it names neither, the refusal is about the command as a whole
  /// and points at its word.
  fn locate(&self, failure: Failure) -> Failure {
    let message = failure.message.clone();
    let column = self
      .quoted(&message)
      .or_else(|| self.flag(&message))
      .unwrap_or_else(|| self.word().column);
    failure.at(self.number, column)
  }

  /// The column of the first token whose text `message` quotes as `"…"`.
  fn quoted(&self, message: &str) -> Option<usize> {
    self
      .tokens
      .iter()
      .find(|token| message.contains(&format!("{:?}", token.text)))
      .map(|token| token.column)
  }

  /// The column of the first token that is a flag `message` names.
  fn flag(&self, message: &str) -> Option<usize> {
    self
      .tokens
      .iter()
      .find(|token| token.text.starts_with("--") && message.contains(&token.text))
      .map(|token| token.column)
  }
}

/// Split one line into its tokens.
///
/// The whole lexer: whitespace separates tokens, a `#` at the start of a token begins a comment to the
/// end of the line, and a token that begins with `"` runs to the next `"`, so a path with a space in it
/// can be named. There is no escaping, no nesting and no other punctuation: this is a command per line,
/// not a grammar with expressions.
fn lex(number: usize, line: &str) -> Result<ScriptLine, Failure> {
  let mut tokens: Vec<Token> = Vec::new();
  // Where an argument this line does not have would have gone; `1` for a line with no token at all.
  let mut end_column = 1;
  let mut characters = line.char_indices().peekable();
  while let Some(&(start, character)) = characters.peek() {
    if character.is_whitespace() {
      characters.next();
      continue;
    }
    if character == '#' {
      break;
    }
    if character == '"' {
      characters.next();
      let mut text = String::new();
      let mut closed = false;
      for (_, inner) in characters.by_ref() {
        if inner == '"' {
          closed = true;
          break;
        }
        text.push(inner);
      }
      if !closed {
        return Err(
          Failure::input(format!(
            "unterminated quoted argument {text:?}: a quoted token ends at its closing \""
          ))
          .at(number, start + 1),
        );
      }
      let length = text.len();
      tokens.push(Token {
        text,
        column: start + 1,
      });
      // One past the closing quote, not one past the text inside it: `column` is the opening quote's.
      end_column = start + 3 + length;
      continue;
    }
    let mut text = String::new();
    while let Some(&(_, inner)) = characters.peek() {
      if inner.is_whitespace() {
        break;
      }
      text.push(inner);
      characters.next();
    }
    let length = text.len();
    tokens.push(Token {
      text,
      column: start + 1,
    });
    end_column = start + 1 + length;
  }
  Ok(ScriptLine {
    number,
    tokens,
    end_column,
  })
}

/// `open <document>`: read the document this script mutates.
///
/// The same loader every subcommand reads with, so a script cannot open a document a command-line run
/// would refuse.
fn command_open(session: &mut Session, line: &ScriptLine) -> Result<(), Failure> {
  let arguments = line.exactly(1)?;
  let token = &arguments[0];
  if session.in_memory {
    // The rule is about the command, not about the path the caller typed, so it is located at the word.
    return Err(line.fail(line.word().column, ALREADY_OPEN));
  }
  if let Some(path) = &session.path {
    return Err(line.fail_at(
      token,
      format!(
        "the script already opened {}: a script mutates one document",
        path.display()
      ),
    ));
  }
  let path = PathBuf::from(&token.text);
  let document = crate::open(&path).map_err(|failure| failure.at(line.number, token.column))?;
  session.document = Some(document);
  session.path = Some(path);
  Ok(())
}

/// `sheet <width> <height>`: author a nesting sheet in millimetres.
fn command_sheet(session: &mut Session, line: &ScriptLine) -> Result<(), Failure> {
  line.exactly(2)?;
  let width = line.number(0, "a width in millimetres")?;
  let height = line.number(1, "a height in millimetres")?;
  session.mutate(line, move |document| {
    let nesting = document.nesting.get_or_insert_with(Nesting::default);
    let id = fresh_sheet_id(&nesting.sheets);
    nesting.sheets.push(NestingSheet {
      id,
      width: Length::from_mm(width),
      height: Length::from_mm(height),
    });
    Ok(())
  })
}

/// `part <instances> <entity>[,<entity>...]`: author a nesting part from this document's entities.
fn command_part(session: &mut Session, line: &ScriptLine) -> Result<(), Failure> {
  line.exactly(2)?;
  let instances = line.count(0, "an instance count")?;
  let ids = &line.arguments()[1];
  let entity_ids: Vec<String> = ids
    .text
    .split(',')
    .map(str::trim)
    .filter(|id| !id.is_empty())
    .map(str::to_owned)
    .collect();
  if entity_ids.is_empty() {
    return Err(line.fail_at(
      ids,
      format!(
        "part names no entity, got {:?}: a part is cut from at least one of this document's entity ids",
        ids.text
      ),
    ));
  }
  session.mutate(line, move |document| {
    let nesting = document.nesting.get_or_insert_with(Nesting::default);
    let id = fresh_part_id(&nesting.parts);
    nesting.parts.push(NestingPart {
      id,
      entity_ids,
      instances,
    });
    Ok(())
  })
}

/// `validate`: check the open document and write the counts the subcommand prints.
fn command_validate(session: &mut Session, line: &ScriptLine) -> Result<(), Failure> {
  line.exactly(0)?;
  let report = {
    let document = session.the_document(line)?;
    document
      .validate()
      .map_err(|error| line.at_word(Failure::input(error.to_string())))?;
    validate_report(document)
  };
  session.say(&report)
}

/// `save <document>`: validate the open document and write it here.
///
/// The one writer, and it validates again as it writes, so a script cannot leave a document on disk
/// that would not load (M10 invariant 4).
fn command_save(session: &mut Session, line: &ScriptLine) -> Result<(), Failure> {
  if session.in_memory {
    return Err(line.fail(line.word().column, NO_WRITE));
  }
  let arguments = line.exactly(1)?;
  let token = &arguments[0];
  let path = PathBuf::from(&token.text);
  // The document borrow ends with this block: the notice goes to the session's own writer, which is a
  // second borrow of the session and cannot overlap the one the document came from.
  let notice = {
    let (document, _) = session.open_document(line)?;
    document
      .validate()
      .map_err(|error| line.at_word(Failure::input(error.to_string())))?;
    cam_store::document::save_document(&path, document)
      .map_err(|error| line.fail_at(token, error.to_string()))?;
    save_report(document, &path)
  };
  session.notice(&notice)
}

/// `parts --csv <file>`: author a nesting part per row of a part list into the open document.
///
/// The tokens go to `parts`' own parser and then to `author_parts`, the function the subcommand
/// calls, so a scripted import can neither read a different dialect nor author different records
/// (M10 invariant 1). `--out` is refused because a script writes its document with `save`.
fn command_parts(session: &mut Session, line: &ScriptLine) -> Result<(), Failure> {
  let tokens = line.tokens_as_strings();
  let args = parts_args(&tokens).map_err(|failure| line.locate(failure))?;
  if let Some(path) = &args.document {
    return Err(line.fail_at(
      line.token_for(&path.to_string_lossy()),
      format!("parts {NO_POSITIONAL}"),
    ));
  }
  refuse_document_destination(line, None, &args.out)?;
  let report = session.mutate(line, |document| {
    author_parts(document, &args).map_err(|failure| line.locate(failure))
  })?;
  session.say(&report)
}

/// `cutlist [--out <path>]`: write the cut list of the open document.
///
/// The tokens go to `cutlist`' own parser and then to `cut_list_document`, the function the
/// subcommand calls, so a scripted cut list is the bytes the subcommand writes (M10 invariant 1).
fn command_cutlist(session: &mut Session, line: &ScriptLine) -> Result<(), Failure> {
  let tokens = line.tokens_as_strings();
  let args = cutlist_args(&tokens).map_err(|failure| line.locate(failure))?;
  if let Some(path) = &args.document {
    return Err(line.fail_at(
      line.token_for(&path.to_string_lossy()),
      format!("cutlist {NO_POSITIONAL}"),
    ));
  }
  // The document and the sink are two fields of the same session, so they are taken apart here: the
  // cut list needs the document while it writes, which one borrow of the whole session cannot express.
  let Session { document, out, .. } = session;
  let document = document
    .as_ref()
    .ok_or_else(|| line.fail(line.word().column, NO_DOCUMENT))?;
  cut_list_document(document, &args, &mut **out).map_err(|failure| line.locate(failure))
}

/// Refuse a scripted command a document destination of its own.
///
/// The corresponding subcommand writes its document where `--out` says, and a script writes its
/// document with `save`: accepting the flag and ignoring it would leave a `--out` that was never
/// applied looking applied, so it is refused by name instead.
fn refuse_document_destination(
  line: &ScriptLine,
  path: Option<&PathBuf>,
  out: &Option<PathBuf>,
) -> Result<(), Failure> {
  if let Some(path) = path {
    return Err(line.fail_at(
      line.token_for(&path.to_string_lossy()),
      format!("{} {NO_POSITIONAL}", line.word().text),
    ));
  }
  if out.is_some() {
    return Err(line.fail_at(
      line.token_for("--out"),
      format!("{} {NO_OUT}", line.word().text),
    ));
  }
  Ok(())
}
