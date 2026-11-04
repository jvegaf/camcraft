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
//!    each command with flags hands its tokens to the subcommand's own argument parser (`nest_args`,
//!    `toolpath_args`, `emit_args`, `simulate_args`) and then to the subcommand's own execution
//!    function (`nest_document`, `append_toolpath`, `emit_document`, `simulate_document`), and `save`
//!    calls `cam_store::document::save_document`. There is no geometry, nesting or emission step in
//!    this file at all (M10 invariant 1).
//! 2. **Every refusal is located**, in the `cam-post` parser's shape — `line {line}:{column}: {message}`
//!    — and every mutation passes through `Document::validate` with the script line attached (M10
//!    invariants 2 and 4).

use std::path::PathBuf;

use cam_model::{Document, Nesting, NestingPart, NestingSheet};
use cam_units::Length;

use crate::{
  append_toolpath, emit_args, emit_document, fresh_part_id, fresh_sheet_id, nest_args,
  nest_document, only_path, simulate_args, simulate_document, toolpath_args, validate_report,
  Failure, NestRequest, Outcome,
};

/// Why a command that needs the document says so: a script reads one, it never guesses at one.
const NO_DOCUMENT: &str = "no document is open: a script reads one with `open <document>`";

/// Why a scripted command takes no document path of its own.
const NO_POSITIONAL: &str = "takes no document path: a script mutates the document `open` read";

/// Why a scripted command takes no `--out`.
const NO_OUT: &str = "takes no --out: a script writes its document with `save <document>`";

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
  // Authors a `NestingSheet` — the record `cam-cli nest` writes from `--sheets` — named by the
  // `fresh_sheet_id` scan that `nest` already uses for a board it has to open.
  Command {
    word: "sheet",
    usage: "<width> <height> in millimetres",
    run: command_sheet,
  },
  // Authors a `NestingPart`, the record `cam-cli nest` nests, named by the `fresh_part_id` scan.
  Command {
    word: "part",
    usage: "<instances> <entity ids>",
    run: command_part,
  },
  // `cam-cli nest`: `nest_args` for the flags, `nest_document` for the run.
  Command {
    word: "nest",
    usage: "[--sheets <WxH>] [--gap <mm>] [--rotation on|off] [--algorithm <mode>] [--step <mm>]",
    run: command_nest,
  },
  // `cam-cli toolpath`: `toolpath_args` for the flags, `append_toolpath` for the whole command.
  Command {
    word: "toolpath",
    usage: "--kind <kind> --entities <ids> --tool <number> and toolpath's other flags",
    run: command_toolpath,
  },
  // `cam-cli validate`: `Document::validate` and `validate_report`, the subcommand's own report.
  Command {
    word: "validate",
    usage: "no arguments",
    run: command_validate,
  },
  // `cam-cli emit`: `emit_args` for the flags, `emit_document` for the program.
  Command {
    word: "emit",
    usage: "--post <file> [--out <path>] [--library <path>]",
    run: command_emit,
  },
  // `cam-cli simulate`: `simulate_args` for the flags, `simulate_document` for the report.
  Command {
    word: "simulate",
    usage: "[--library <path>] [--resolution <mm>]",
    run: command_simulate,
  },
  // `cam_store::document::save_document`, the one writer, which validates as it writes.
  Command {
    word: "save",
    usage: "<document>",
    run: command_save,
  },
];

/// Run a script: `cam-cli script <file>`.
///
/// The script's own path is the one thing this command reads from the command line; every path a
/// command needs comes from the script itself, so nothing is read from the environment, the working
/// directory or another file (M10 invariant 3).
pub(crate) fn script_command(rest: &[String]) -> Result<(), Failure> {
  let path = only_path(rest, "script")?;
  let text = std::fs::read_to_string(&path)
    .map_err(|error| Failure::input(format!("{}: {error}", path.display())))?;

  let mut session = Session::default();
  for (index, raw) in text.lines().enumerate() {
    let line = lex(index + 1, raw)?;
    if line.tokens.is_empty() {
      continue;
    }
    session.execute(&line)?;
  }
  Ok(())
}

/// The script's state: the one document it mutates and the path it was read from.
///
/// The path is kept because `tools_ref` is resolved relative to the document, exactly as the
/// subcommands resolve it. Nothing else of the process is read.
#[derive(Default)]
struct Session {
  document: Option<Document>,
  path: Option<PathBuf>,
}

impl Session {
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

  /// The open document and the path it was read from, or the refusal that says none is open.
  fn open_document(&self, line: &ScriptLine) -> Result<(&Document, &std::path::Path), Failure> {
    match (&self.document, &self.path) {
      (Some(document), Some(path)) => Ok((document, path)),
      _ => Err(line.fail(line.word().column, NO_DOCUMENT)),
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
  /// Where an argument the line does not have would have gone: one past the last token's text.
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
  /// command line deserves: `unknown nest option "--x"`, `--gap expects a number of millimetres, got
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
      tokens.push(Token {
        text,
        column: start + 1,
      });
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
    tokens.push(Token {
      text,
      column: start + 1,
    });
  }

  let end_column = tokens
    .last()
    .map_or(1, |token| token.column + token.text.len());
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

/// `nest [options]`: nest the authored parts onto the authored sheets.
///
/// The tokens go to `nest`'s own parser and then to the run every `cam-cli nest` performs, so a script
/// cannot nest with a different engine, a different default or a second cost arithmetic.
fn command_nest(session: &mut Session, line: &ScriptLine) -> Result<(), Failure> {
  let tokens = line.tokens_as_strings();
  let args = nest_args(&tokens).map_err(|failure| line.locate(failure))?;
  refuse_document_destination(line, None, &args.out)?;
  let request = NestRequest {
    sheets: args.sheets,
    gap: args.gap,
    rotation: args.rotation,
    algorithm: args.algorithm,
    step: args.step,
  };
  let outcome = session.mutate(line, |document| {
    nest_document(document, &request).map_err(|failure| line.locate(failure))
  })?;
  match outcome {
    Outcome::Done(report) => {
      print!("{report}");
      Ok(())
    }
    Outcome::Finding(report, failure) => {
      print!("{report}");
      Err(line.at_word(failure))
    }
  }
}

/// `toolpath [options]`: append a toolpath to the open document.
///
/// The tokens go to `toolpath`'s own parser and then to the function the subcommand calls, so every
/// kind, every default and every refusal a toolpath has is the same one a command line gets.
fn command_toolpath(session: &mut Session, line: &ScriptLine) -> Result<(), Failure> {
  let tokens = line.tokens_as_strings();
  let args = toolpath_args(&tokens).map_err(|failure| line.locate(failure))?;
  refuse_document_destination(line, args.path.as_ref(), &args.out)?;
  let document_path = session
    .path
    .clone()
    .ok_or_else(|| line.fail(line.word().column, NO_DOCUMENT))?;
  let report = session.mutate(line, |document| {
    append_toolpath(document, &document_path, &args).map_err(|failure| line.locate(failure))
  })?;
  print!("{report}");
  Ok(())
}

/// `validate`: check the open document and print the counts the subcommand prints.
fn command_validate(session: &mut Session, line: &ScriptLine) -> Result<(), Failure> {
  line.exactly(0)?;
  let (document, _) = session.open_document(line)?;
  document
    .validate()
    .map_err(|error| line.at_word(Failure::input(error.to_string())))?;
  print!("{}", validate_report(document));
  Ok(())
}

/// `emit --post <file>`: write the program the open document makes.
fn command_emit(session: &mut Session, line: &ScriptLine) -> Result<(), Failure> {
  let tokens = line.tokens_as_strings();
  let args = emit_args(&tokens).map_err(|failure| line.locate(failure))?;
  if let Some(path) = &args.document {
    return Err(line.fail_at(
      line.token_for(&path.to_string_lossy()),
      format!("emit {NO_POSITIONAL}"),
    ));
  }
  let (document, document_path) = session.open_document(line)?;
  emit_document(document, document_path, &args).map_err(|failure| line.locate(failure))
}

/// `simulate`: simulate the program the open document makes and report it.
fn command_simulate(session: &mut Session, line: &ScriptLine) -> Result<(), Failure> {
  let tokens = line.tokens_as_strings();
  let args = simulate_args(&tokens).map_err(|failure| line.locate(failure))?;
  if let Some(path) = &args.document {
    return Err(line.fail_at(
      line.token_for(&path.to_string_lossy()),
      format!("simulate {NO_POSITIONAL}"),
    ));
  }
  let (document, document_path) = session.open_document(line)?;
  let outcome =
    simulate_document(document, document_path, &args).map_err(|failure| line.locate(failure))?;
  // The report is the finding's body, so it is printed before the exit code says which one it was.
  outcome
    .print_and_result()
    .map_err(|failure| line.at_word(failure))
}

/// `save <document>`: validate the open document and write it here.
///
/// The one writer, and it validates again as it writes, so a script cannot leave a document on disk
/// that would not load (M10 invariant 4).
fn command_save(session: &mut Session, line: &ScriptLine) -> Result<(), Failure> {
  let arguments = line.exactly(1)?;
  let token = &arguments[0];
  let path = PathBuf::from(&token.text);
  let (document, _) = session.open_document(line)?;
  document
    .validate()
    .map_err(|error| line.at_word(Failure::input(error.to_string())))?;
  cam_store::document::save_document(&path, document)
    .map_err(|error| line.fail_at(token, error.to_string()))?;
  eprintln!(
    "wrote {} layer(s), {} entity(ies) to {}",
    document.layers.len(),
    document.entities().count(),
    path.display()
  );
  Ok(())
}

/// Refuse a scripted `nest` or `toolpath` a document destination of its own.
///
/// Both subcommands write their document where `--out` says, and a script writes its document with
/// `save`: accepting the flag and ignoring it would leave a `--out` that was never applied looking
/// applied, so it is refused by name instead.
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
