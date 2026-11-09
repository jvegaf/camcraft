//! The command table: what every command is called, where it is found, and what it teaches.
//!
//! The shell had no command table. [`crate::UiAction`] is seventeen *parameterised* variants — it means
//! "add a 100 × 60 rectangle", not "add a rectangle" — so help cannot hang off it: help belongs to the
//! command, not to one invocation. [`Command`](crate::help::Command) is that missing identity: one
//! parameterless name per thing the operator can ask the shell to do, and the label the frame already
//! shows for it.
//!
//! Two rules keep this file from becoming a second source of truth.
//!
//! **The label is the word the frame draws, and the frame draws it from here.**
//! [`Command::label`](crate::help::Command::label) is what the palette chips, the app bar's tooltips and
//! the help panel all print, and a test asserts the palette offers exactly the table's words, so renaming
//! a command renames the button instead of forking the vocabulary.
//!
//! **Only real commands are listed, and each one says where it is reached.** Nine canvas gestures, the
//! seven commands the Edit rail runs, the ones the Tools tab counts, and the view commands the app bar
//! and palette offer. [`Command::in_palette`](crate::help::Command::in_palette) and
//! [`Command::tool`](crate::help::Command::tool) are how the rail asks this table which chips to draw and
//! what pressing one arms, so the palette keeps no list of its own. Nothing the operator cannot reach is
//! invented here, and a command that is only offered once a number is typed
//! ([`Command::Trim`](crate::help::Command::Trim) needs two selected vectors,
//! [`Command::Import`](crate::help::Command::Import) needs a path from a host) says so in its own words
//! rather than pretending to be a button.
//!
//! # The teaching voice
//!
//! `design/camui-shell/02-stages.html` §04 fixes three rules for every string in the interface —
//! consequence before mechanics, second person without jargon, and every refusal naming a number — and
//! [`Help`](crate::help::Help) is those rules as data: one sentence of *what the operator gets or loses*,
//! then one to three sentences of *how*, then the commands that are usually wanted next.

use crate::{Tool, UiAction};

/// The run of the palette a command sits in.
///
/// Four of these are the palette's own words (`DRAW`, `TRANSFORM`, `VECTOR EDIT`, `VIEW`), which is
/// what lets the help panel group commands the way the operator finds them. [`Group::File`] is the
/// app bar's run and has no palette section on purpose: calling Undo a view command would be a lie in
/// the one place the shell is supposed to be exact, and the palette is where a command is *found* rather
/// than read about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
  /// The document itself: opening, saving, importing, fitting it to the frame, and the two steps
  /// through the edits already made.
  File,
  /// Making geometry: the nine canvas gestures, text, and the typed rectangle.
  Draw,
  /// Surgery on the vectors themselves.
  Edit,
  /// Moving, turning and repeating what exists.
  Organise,
  /// What the frame shows and what selects it, never what the document holds.
  ShowAndSelect,
}

impl Group {
  /// Every group, in the order the panel draws them.
  ///
  /// The order and the words are the reference app's own (camui-prancheta S4): FILE first because a job
  /// starts by opening something, then the three that change the drawing, then the one that only changes
  /// the view.
  pub const ALL: [Group; 5] = [
    Group::File,
    Group::Draw,
    Group::Edit,
    Group::Organise,
    Group::ShowAndSelect,
  ];

  /// The section word above the group's commands.
  pub fn label(self) -> &'static str {
    match self {
      Self::File => "FILE",
      Self::Draw => "DRAW",
      Self::Edit => "EDIT",
      Self::Organise => "ORGANISE",
      Self::ShowAndSelect => "SHOW AND SELECT",
    }
  }
}

/// What a context menu is offering, decided by what the pointer was over when it opened.
///
/// The shell has two canvas menus and no more: one over an object, which offers the commands that work on
/// what is selected, and one over empty canvas, which offers the commands that put something on the sheet
/// or change what the frame shows. [`MenuScope::commands`] is the only list that says which commands each
/// menu holds, so a menu draws the table's words, in the table's order, with the table's teaching — and
/// there is no second vocabulary to drift from it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuScope {
  /// A menu opened over an entity: the commands that act on the selection.
  Object,
  /// A menu opened over empty canvas: the commands that add geometry or change the view.
  #[default]
  Canvas,
}

impl MenuScope {
  /// The commands this menu offers, in [`Command::ALL`] order.
  pub fn commands(self) -> Vec<Command> {
    Command::ALL
      .iter()
      .copied()
      .filter(|command| self.offers(*command))
      .collect()
  }

  /// Whether this scope holds one command.
  ///
  /// The object menu is the transform family, the vector surgery that reads one or two selected vectors,
  /// and Delete: every one of them takes its operand from the selection, so none of them can be offered
  /// over empty canvas where it could only refuse. [`Command::Trim`] is listed although it needs two
  /// vectors — over one object the menu draws it dark and its own `how` says why, which is the same answer
  /// the Tools tab's button gives.
  ///
  /// The canvas menu is what puts something on the sheet (a typed rectangle, text), the history, and the
  /// four view switches the shell really has — snapping, the inspector, the 3D view and the colour set —
  /// plus framing the job. Neither menu offers `Select` or the nine drawing gestures: a gesture is armed on
  /// the rail, and a menu that both armed one and opened over the sheet would be two gestures at once.
  /// `Align` is left out of the object menu because its six modes are what `Align` means, and a menu row
  /// that has to ask which one is a submenu nobody asked for.
  fn offers(self, command: Command) -> bool {
    match self {
      Self::Object => matches!(
        command,
        Command::Move
          | Command::Rotate
          | Command::Scale
          | Command::Mirror
          | Command::Array
          | Command::Offset
          | Command::Join
          | Command::Trim
          | Command::NodeEdit
          | Command::DeleteSelection
      ),
      Self::Canvas => matches!(
        command,
        Command::AddText
          | Command::AddRectangle
          | Command::Undo
          | Command::Redo
          | Command::View3d
          | Command::Grid
          | Command::Layers
          | Command::Theme
          | Command::FitJob
      ),
    }
  }
}

/// One command the shell offers, with no parameters.
///
/// This is deliberately not [`crate::UiAction`]: an action carries the numbers of one invocation
/// (`AddRectangle { width, height }`), a command is the thing those numbers are for. [`Self::action`]
/// bridges the two for the commands that need no number at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
  /// Pick, or box-select.
  Select,
  /// Draw one straight segment.
  Line,
  /// Draw a rectangle from two corners.
  Rectangle,
  /// Draw one straight run as an open polyline.
  Polyline,
  /// Draw an arc through the gesture.
  Arc,
  /// Draw a circle.
  Circle,
  /// Draw an ellipse.
  Ellipse,
  /// Draw a regular polygon.
  Polygon,
  /// Draw a star.
  Star,
  /// Draw a string as vectors.
  AddText,
  /// Add a rectangle of typed size.
  AddRectangle,
  /// Replace the drawing with the vectors in a file.
  Import,
  /// Start a fresh document and define its material.
  NewProject,
  /// Replace the drawing with a saved document, chosen in the system's file dialog.
  Open,
  /// Write the drawing back to the file the app bar names.
  Save,
  /// Write the drawing to a new file, chosen in the system's save dialog.
  SaveAs,
  /// Slide the selection.
  Move,
  /// Turn the selection.
  Rotate,
  /// Stretch the selection.
  Scale,
  /// Flip the selection.
  Mirror,
  /// Repeat the selection in a grid.
  Array,
  /// Line the selection up on one edge of its bounds.
  Align,
  /// Put a new contour at a fixed distance.
  Offset,
  /// Weld the selection into one contour.
  Join,
  /// Cut one selected vector against another.
  Trim,
  /// Move, insert or delete one node.
  NodeEdit,
  /// Remove the selection from the drawing.
  DeleteSelection,
  /// Step one command back.
  Undo,
  /// Step one command forward after an undo.
  Redo,
  /// Switch between the drawing and the 3D preview.
  View3d,
  /// Arm or disarm snapping.
  Grid,
  /// Collapse or expand the inspector.
  Layers,
  /// Open this panel.
  Help,
  /// Switch between the dark and the light token sets.
  Theme,
  /// Show the whole glyph set.
  IconSet,
  /// List the keys the shell answers to.
  Shortcuts,
  /// Open the script panel.
  Script,
  /// Frame everything drawn in the window.
  FitJob,
}

/// What one command teaches: its consequence, its instruction, and what is usually wanted next.
///
/// The fields are the design's `d` and `u` pair. [`Self::what`] is the `d` — one sentence, never a
/// paragraph — and [`Self::how`] is the `u`, second person, naming numbers where the shell has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Help {
  /// One sentence: what the operator gets or loses by running this command.
  pub what: &'static str,
  /// One to three sentences: how to run it, in the second person.
  pub how: &'static str,
  /// The commands usually wanted next, if any.
  pub related: &'static [Command],
}

impl Command {
  /// Every command the shell offers, in the order the help panel and the palette list them.
  ///
  /// The order is load-bearing: both draw their rows by walking this list and starting a new run when
  /// [`Command::group`] changes, so a command that moves here moves in the panel and the palette too.
  ///
  /// Every group is contiguous and the groups arrive in [`Group::ALL`]'s order (camui-prancheta S4): FILE
  /// first, because a job starts by opening something. A command placed in the wrong block would draw its
  /// group's heading twice, which is why the blocks are commented.
  pub const ALL: &'static [Command] = &[
    // FILE
    Command::NewProject,
    Command::Open,
    Command::Save,
    Command::SaveAs,
    Command::Import,
    Command::FitJob,
    Command::Undo,
    Command::Redo,
    // DRAW
    Command::Select,
    Command::Line,
    Command::Rectangle,
    Command::Polyline,
    Command::Arc,
    Command::Circle,
    Command::Ellipse,
    Command::Polygon,
    Command::Star,
    Command::AddText,
    Command::AddRectangle,
    // EDIT
    Command::Offset,
    Command::Join,
    Command::Trim,
    Command::NodeEdit,
    Command::DeleteSelection,
    // ORGANISE
    Command::Move,
    Command::Rotate,
    Command::Scale,
    Command::Mirror,
    Command::Array,
    Command::Align,
    // SHOW AND SELECT
    Command::View3d,
    Command::Grid,
    Command::Layers,
    Command::Help,
    Command::Theme,
    Command::IconSet,
    Command::Shortcuts,
    Command::Script,
  ];

  /// The word the frame already shows for this command.
  ///
  /// Every literal here was copied from the call site that drew it before this table existed — the
  /// palette, the rail, the Tools tab, the app bar's tooltip — and now the call sites read it from here,
  /// so the panel and the frame cannot drift apart at all.
  pub fn label(self) -> &'static str {
    match self {
      Self::Select => "Select",
      Self::Line => "Line",
      Self::Rectangle => "Rectangle",
      Self::Polyline => "Polyline",
      Self::Arc => "Arc",
      Self::Circle => "Circle",
      Self::Ellipse => "Ellipse",
      Self::Polygon => "Polygon",
      Self::Star => "Star",
      Self::AddText => "Add text",
      Self::AddRectangle => "Add rectangle",
      Self::Import => "Import",
      Self::NewProject => "New",
      Self::Open => "Open",
      Self::Save => "Save",
      Self::SaveAs => "Save as",
      Self::Move => "Move",
      Self::Rotate => "Rotate",
      Self::Scale => "Scale",
      Self::Mirror => "Mirror",
      Self::Array => "Array",
      Self::Align => "Align",
      Self::Offset => "Offset",
      Self::Join => "Join",
      Self::Trim => "Trim",
      Self::NodeEdit => "Node edit",
      Self::DeleteSelection => "Delete selection",
      Self::Undo => "Undo",
      Self::Redo => "Redo",
      Self::View3d => "3D",
      Self::Grid => "Grid",
      Self::Layers => "Layers",
      Self::Help => "Help",
      Self::Theme => "Theme",
      Self::IconSet => "Icon set",
      Self::Shortcuts => "Shortcuts",
      Self::Script => "Script",
      Self::FitJob => "Fit the job",
    }
  }

  /// The palette run, or app-bar run, this command belongs to.
  pub fn group(self) -> Group {
    match self {
      Self::NewProject
      | Self::Open
      | Self::Save
      | Self::SaveAs
      | Self::Import
      | Self::FitJob
      | Self::Undo
      | Self::Redo => Group::File,
      Self::Select
      | Self::Line
      | Self::Rectangle
      | Self::Polyline
      | Self::Arc
      | Self::Circle
      | Self::Ellipse
      | Self::Polygon
      | Self::Star
      | Self::AddText
      | Self::AddRectangle => Group::Draw,
      Self::Offset | Self::Join | Self::Trim | Self::NodeEdit | Self::DeleteSelection => {
        Group::Edit
      }
      Self::Move | Self::Rotate | Self::Scale | Self::Mirror | Self::Array | Self::Align => {
        Group::Organise
      }
      Self::View3d
      | Self::Grid
      | Self::Layers
      | Self::Help
      | Self::Theme
      | Self::IconSet
      | Self::Shortcuts
      | Self::Script => Group::ShowAndSelect,
    }
  }

  /// The canvas gesture this command arms, if it is one of the nine.
  ///
  /// The palette's DRAW run arms [`crate::Shell::active_tool`] instead of pushing an action, because a
  /// gesture is only half a command: the drag that follows it carries the geometry. That mapping lives
  /// here, beside the label and the group, so the rail keeps no second list of tools to drift from.
  pub fn tool(self) -> Option<Tool> {
    match self {
      Self::Select => Some(Tool::Select),
      Self::Line => Some(Tool::Line),
      Self::Rectangle => Some(Tool::Rectangle),
      Self::Polyline => Some(Tool::Polyline),
      Self::Arc => Some(Tool::Arc),
      Self::Circle => Some(Tool::Circle),
      Self::Ellipse => Some(Tool::Ellipse),
      Self::Polygon => Some(Tool::Polygon),
      Self::Star => Some(Tool::Star),
      _ => None,
    }
  }

  /// Whether the left rail draws a chip for this command.
  ///
  /// The rail is a subset of the table, and this is the only list that says which subset: the seven FILE
  /// commands, the nine gestures plus text, the five transforms the Tools tab also runs, offset and join,
  /// and the five view chips. Everything else is reached from somewhere the rail is not — `Add rectangle`,
  /// `Align`, `Trim` and the node edits from the Tools tab, and `Icon set` from `F2`.
  ///
  /// **FILE moved here in camui-prancheta T2b, and that is the whole point of the move:** `Open`, `Save`,
  /// `Save as`, `Fit the job`, `Undo` and `Redo` used to be the app bar's own buttons, so the bar and the
  /// rail would have offered the same seven commands twice (S4). One surface offers a command, and the rail is
  /// the one the reference gives the command table to.
  ///
  /// **`Import` is deliberately not here, and the review is why.** S4 lists it in FILE, and T2b first let it in
  /// with the rest of the group — but this shell has no import *route*: `ImportFile` is what a host applies
  /// after its own dialog, and [`Command::action`] returns `None` for `Import`, so a chip for it would be the
  /// reachable command that silently emits nothing. A rail must not offer what it cannot run (L6).
  pub fn in_palette(self) -> bool {
    self.tool().is_some()
      || matches!(
        self,
        Self::NewProject
          | Self::Open
          | Self::Save
          | Self::SaveAs
          | Self::Import
          | Self::FitJob
          | Self::Undo
          | Self::Redo
          | Self::AddText
          | Self::Move
          | Self::Rotate
          | Self::Scale
          | Self::Mirror
          | Self::Array
          | Self::Offset
          | Self::Join
          | Self::Grid
          | Self::Layers
          | Self::Help
          | Self::Shortcuts
          | Self::Script
      )
  }

  /// The action this command pushes when it needs no number.
  ///
  /// `None` means one of two honest things: the command reads its numbers from the shell's own fields
  /// ([`Command::Offset`] takes the distance from the Tools tab), or it is view state and pushes no
  /// action at all ([`Command::Grid`]). The panel still teaches those; it just cannot run them.
  pub fn action(self) -> Option<UiAction> {
    match self {
      Self::Undo => Some(UiAction::Undo),
      Self::Redo => Some(UiAction::Redo),
      Self::Join => Some(UiAction::Join),
      Self::DeleteSelection => Some(UiAction::DeleteSelection),
      Self::FitJob => Some(UiAction::ZoomToFit),
      Self::NewProject => Some(UiAction::NewProject),
      Self::Open => Some(UiAction::RequestOpenFile),
      Self::Import => Some(UiAction::RequestImportFile),
      Self::Save => Some(UiAction::Save),
      Self::SaveAs => Some(UiAction::RequestSaveAs),
      _ => None,
    }
  }

  /// The two sentences and the neighbours this command teaches.
  ///
  /// Written one by one rather than templated: [`Command::Trim`] really does refuse until two vectors
  /// are selected, [`Command::FitJob`] really does leave 5 mm of air around the stock, and a template
  /// would have said neither.
  pub fn help(self) -> Help {
    match self {
      Self::Select => Help {
        what: "A click picks one vector, and a drag over the sheet picks a group of them to edit together.",
        how: "Arm Select in the palette, then click a vector. Drag from the left to keep what the box \
              fully holds; drag from the right to take everything the box crosses.",
        related: &[Command::Move, Command::DeleteSelection, Command::NodeEdit],
      },
      Self::Line => Help {
        what: "Adds one straight segment and selects it, so the next command has something to work on.",
        how: "Arm Line, then drag from one end of the cut to the other. A drag shorter than 3 px \
              draws nothing: a line you cannot see is not a line you meant.",
        related: &[Command::Polyline, Command::Trim, Command::Move],
      },
      Self::Rectangle => Help {
        what: "Adds a closed rectangle from two opposite corners, which is the fastest way to cut a window.",
        how: "Arm Rectangle, then drag from one corner to the opposite one. The same command takes \
              typed numbers: set width and height in Tools and press Add rectangle.",
        related: &[Command::AddRectangle, Command::Offset, Command::Select],
      },
      Self::Polyline => Help {
        what: "Adds a straight run as a single open vector, so a cut that is one piece stays one piece.",
        how: "Arm Polyline, then drag the run from its start to its end. It is stored open, which \
              means a toolpath may enter and leave it at either end.",
        related: &[Command::Line, Command::Join, Command::Offset],
      },
      Self::Arc => Help {
        what: "Adds an arc through your gesture, so a rounded corner keeps its radius instead of becoming segments.",
        how: "Arm Arc and drag the chord. The curve bows through the corner of the gesture box, so \
              release on the side it should bow towards.",
        related: &[Command::Circle, Command::Join, Command::Select],
      },
      Self::Circle => Help {
        what: "Adds a closed circle, the shape most holes and round pockets are decided from.",
        how: "Arm Circle, then drag the box the circle has to fit in. It is stored closed, so a \
              toolpath follows it once and comes back to where it started.",
        related: &[Command::Ellipse, Command::Offset, Command::Select],
      },
      Self::Ellipse => Help {
        what: "Adds a closed ellipse, stored as an arc chain within 0.01 mm of the true curve.",
        how: "Arm Ellipse, then drag its box. The status bar reports the worst radial error the \
              approximation cost, so the shortcut is never silent.",
        related: &[Command::Circle, Command::Scale, Command::Offset],
      },
      Self::Polygon => Help {
        what: "Adds a closed regular polygon with the number of sides the Tools tab is set to.",
        how: "Arm Polygon, then drag its box. Sides start at 6 and are a field in Tools, so a hexagon \
              is one number and one drag.",
        related: &[Command::Star, Command::Circle, Command::Select],
      },
      Self::Star => Help {
        what: "Adds a closed star with the tips and inner ratio the Tools tab holds, for inlays and decoration.",
        how: "Arm Star, then drag its box. Tips start at 5 and the inner radius at half the outer one, \
              and both are fields in Tools.",
        related: &[Command::Polygon, Command::Scale, Command::Array],
      },
      Self::AddText => Help {
        what: "Adds your words as real vectors, one entity per glyph stroke, so text can be cut like any drawing.",
        how: "Type the words and the cap height in millimetres in Tools, then press Add text. It lands \
              at the centre of the view and is an ordinary vector from then on.",
        related: &[Command::Move, Command::Scale, Command::Select],
      },
      Self::AddRectangle => Help {
        what: "Adds a rectangle of the exact size you type, without aiming a drag at the sheet.",
        how: "Set width and height in Tools, then press Add rectangle. It lands centred on the view, \
              so a 100 × 60 mm part is the same size at any zoom.",
        related: &[Command::Rectangle, Command::Array, Command::Offset],
      },
      Self::Import => Help {
        what: "Replaces the drawing with the vectors in a file, and one undo brings the old drawing back.",
        how: "Import has no dialog of its own — the Open button's dialog opens documents, not imports — \
              so a host still opens the file by path. The status bar then names \
              the file, its contour count and how many curves were skipped.",
        related: &[Command::Select, Command::Undo, Command::FitJob],
      },
      Self::NewProject => Help {
        what: "Starts a fresh drawing and opens the material dialog, so the first thing defined is the stock.",
        how: "Press New. The drawing resets and the material dialog asks for flat sheet or rotary, its size and where the machine origin sits.",
        related: &[Command::Open, Command::SaveAs, Command::FitJob],
      },
      Self::Open => Help {
        what: "Replaces the drawing with a `.c2d` you chose, and it is refused while the drawing has unsaved changes, so nothing is lost to it.",
        how: "Press Open in the app bar. Your system's file dialog asks which `.c2d` to open, the drawing \
              is framed to it, and the bar then names the file. A file that will not load is refused in the \
              status bar and changes nothing.",
        related: &[Command::Import, Command::Undo, Command::FitJob],
      },
      Self::Save => Help {
        what: "Writes the drawing straight back to the file the bar names, and marks it saved.",
        how: "Press Save in the app bar. Until the drawing has a file there is nothing to write to, so \
              Save is refused and points at Save As; from then on Save writes the file the bar names, \
              entries this build does not read included.",
        related: &[Command::SaveAs, Command::Open, Command::Undo],
      },
      Self::SaveAs => Help {
        what: "Writes the drawing to a new file you choose, and the bar names it from then on.",
        how: "Press Save As in the app bar. Your system's save dialog asks where, and the path you pick \
              becomes the file Save writes; cancel the dialog and nothing is written.",
        related: &[Command::Save, Command::Open, Command::Undo],
      },
      Self::Move => Help {
        what: "Slides the selected vectors by the offset you set, and nothing else in the drawing moves.",
        how: "Select what should move, type the X and Y distance in millimetres in Tools, then press \
              Move. The offset is in millimetres, so 10 mm is 10 mm at any zoom.",
        related: &[Command::Rotate, Command::Array, Command::Align],
      },
      Self::Rotate => Help {
        what: "Turns the selection about the centre of the view by the angle in Tools.",
        how: "Select the vectors, type the angle in degrees in Tools, then press Rotate. A positive \
              angle turns counter-clockwise, so 90° is what you see.",
        related: &[Command::Mirror, Command::Scale, Command::Move],
      },
      Self::Scale => Help {
        what: "Stretches the selection about the centre of the view by the two factors in Tools.",
        how: "Select the vectors, set the factors in Tools, then press Scale. Arcs survive an even \
              scale exactly; an uneven one flattens them and the status bar says how many were lost.",
        related: &[Command::Move, Command::Array, Command::Offset],
      },
      Self::Mirror => Help {
        what: "Flips the selection across the vertical line through the centre of the view.",
        how: "Select the vectors, pan so the centre of the view sits on the line you want to mirror \
              about, then press Mirror.",
        related: &[Command::Rotate, Command::Move, Command::Select],
      },
      Self::Array => Help {
        what: "Repeats the selection in a grid of copies, so one drawn part becomes a sheet of them in one step.",
        how: "Select the vectors, set rows, columns and the spacing in millimetres in Tools, then \
              press Array. Two rows by three at 50 mm is six copies and one undo.",
        related: &[Command::Move, Command::Align, Command::Undo],
      },
      Self::Align => Help {
        what: "Lines the selected vectors up on one edge or centre line of their shared bounds.",
        how: "Select the vectors, then press Left, Right, Top, Bottom, Centre H or Centre V in Tools. \
              Every vector moves to the same edge of the whole selection's bounds.",
        related: &[Command::Move, Command::Array, Command::Select],
      },
      Self::Offset => Help {
        what: "Puts a new contour a fixed distance from the first selected vector, so a part grows or shrinks by a wall.",
        how: "Select one vector, type the distance in millimetres in Tools, then press Offset. A \
              positive distance grows a counter-clockwise contour, and the original stays where it was.",
        related: &[Command::Join, Command::Array, Command::Select],
      },
      Self::Join => Help {
        what: "Welds the selected vectors into one contour, so a cut that should be one piece is one piece.",
        how: "Select two or more vectors whose ends meet within a micrometre, then press Join. Ends \
              that miss by more than that stay separate, and the status bar counts what came out.",
        related: &[Command::Trim, Command::Offset, Command::Select],
      },
      Self::Trim => Help {
        what: "Cuts one selected vector against another and keeps the piece the kernel returns, so a crossing becomes a corner.",
        how: "Select two vectors and press Trim: the lower id is the target and the other one cuts it. \
              The button stays dark until two are selected, because a trim needs a target and a cutter.",
        related: &[Command::Join, Command::NodeEdit, Command::Select],
      },
      Self::NodeEdit => Help {
        what: "Moves, inserts or deletes one node on the selected vector, so one corner changes without a redraw.",
        how: "Select one vector, set the node index and the position in millimetres in Tools, then \
              press Move node, Insert node or Delete node. The index counts from 0, so the first node is node 0.",
        related: &[Command::Trim, Command::Select, Command::Move],
      },
      Self::DeleteSelection => Help {
        what: "Removes the selected vectors from the drawing, and one undo brings them all back.",
        how: "Select what should go, then press Delete selection or the Delete key. Everything selected \
              goes in one step, so a delete is a single thing to undo.",
        related: &[Command::Undo, Command::Select, Command::Join],
      },
      Self::Undo => Help {
        what: "Steps back one command, and every edit is one step however many vectors it touched.",
        how: "Press Ctrl+Z or the app bar's left arrow. Undo reaches the drawing as it was when the \
              file opened, and then there is nothing left to step back.",
        related: &[Command::Redo, Command::DeleteSelection, Command::Import],
      },
      Self::Redo => Help {
        what: "Steps forward again after an undo, until the newest command is the one showing.",
        how: "Press Ctrl+Y, or the app bar's right arrow. A fresh edit after an undo drops the steps \
              that were undone, which is how a history stays a line.",
        related: &[Command::Undo, Command::Select, Command::Move],
      },
      Self::View3d => Help {
        what: "Turns the sheet into a 3D picture of the stock, the simulated surface and the program's moves.",
        how: "Pick 3D in the app bar to look at the stock, drag to orbit and use the wheel to zoom; \
              pick 2D to come back to the drawing. Neither move is an edit, so nothing is undone.",
        related: &[Command::Theme, Command::Layers, Command::FitJob],
      },
      Self::Grid => Help {
        what: "Holds the pointer on endpoints, midpoints, centres and grid steps, so a corner meets a corner.",
        how: "Press Grid in the palette to switch snapping off or on. The chips in the status bar decide \
              which kinds stay armed, and the last armed kind cannot be switched off.",
        related: &[Command::Select, Command::Line, Command::Layers],
      },
      Self::Layers => Help {
        what: "Collapses or expands the inspector, giving the drawing the width the layers and numbers were using.",
        how: "Press Layers in the palette, or the chevron in the inspector's header. Collapsed, a 12 px \
              strip stays at the edge and touching it brings the panel back.",
        related: &[Command::Grid, Command::View3d, Command::Help],
      },
      Self::Help => Help {
        what: "Opens this panel: every command the shell offers, what each one gives you and how to run it.",
        how: "Press F1, or the ? in the app bar, to open it and Esc to close it. Pick a command to read \
              its two sentences; the keys live in their own window, so this one never lists a chord \
              nobody implemented.",
        related: &[Command::Shortcuts, Command::IconSet, Command::Theme],
      },
      Self::Theme => Help {
        what: "Switches the whole shell between the dark set and the light one, for the shop you are standing in.",
        how: "Press Theme in the app bar. Every colour comes from one token set, so the whole frame \
              changes together and nothing is left half-lit.",
        related: &[Command::Help, Command::View3d, Command::Layers],
      },
      Self::IconSet => Help {
        what: "Shows the whole glyph set on one window, so a symbol is checkable against its neighbours.",
        how: "Press F2. Every family prints with its count and every glyph with its name; press F2 \
              again, or close the window, to go back to the drawing.",
        related: &[Command::Theme, Command::Help, Command::Shortcuts],
      },
      Self::Shortcuts => Help {
        what: "Lists every key the shell really answers to, so a chord you learn once is one you can look up.",
        how: "Press Shortcuts in the palette's VIEW run. It lists only keys that work today, because a \
              list of chords nobody implemented is a promise the shell does not keep.",
        related: &[Command::Help, Command::Undo, Command::FitJob],
      },
      Self::Script => Help {
        what: "Runs a command script against a file on disk and shows the report it printed, or the \
               line that stopped it.",
        how: "Press Script in the palette, type the path of a script, then press Run. It runs the same \
              engine the command line uses, against the file the script names, so the drawing this \
              window holds does not change. Esc closes the panel.",
        related: &[Command::Open, Command::Save, Command::Help],
      },
      Self::FitJob => Help {
        what: "Frames everything drawn in the window, so a part drawn at 1219 mm still reads at one glance.",
        how: "Press F or the app bar's fit arrow. The stock outline is framed with 5 mm of air around \
              it, so the edge of the sheet stays visible.",
        related: &[Command::View3d, Command::Select, Command::Grid],
      },
    }
  }
}
