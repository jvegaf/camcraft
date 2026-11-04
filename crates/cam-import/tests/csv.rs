//! Behaviour tests for the CSV part list (M10 T2a).
//!
//! The dialect is pinned here, one test per rule: the header row is required and names the columns
//! in a fixed order, `#` starts a whole-line comment, blank lines are skipped, quoting is refused by
//! name, and a duplicate name is an error. Every refusal is asserted by the exact text a user reads,
//! which is the only way to keep the line and the offending value in it (invariant 2).
//!
//! The fixtures are string literals in this file, so the tests are hermetic.

use cam_import::csv::{import, PartList, PartRow};
use cam_import::ImportError;

fn parse(text: &str) -> PartList {
  import(text.as_bytes()).expect("import")
}

/// The exact message a refusal prints.
fn refusal(text: &str) -> String {
  import(text.as_bytes())
    .map(|list| format!("{list:?}"))
    .expect_err("refused")
    .to_string()
}

#[test]
fn a_header_row_and_its_parts_come_out_as_typed_rows() {
  let list = parse("name,width,height,quantity\nbracket,120.5,80.25,4\npanel,600,400,1\n");

  assert_eq!(
    list.parts,
    vec![
      PartRow {
        line: 2,
        name: "bracket".to_string(),
        width: 120.5,
        height: 80.25,
        quantity: 4,
      },
      PartRow {
        line: 3,
        name: "panel".to_string(),
        width: 600.0,
        height: 400.0,
        quantity: 1,
      },
    ]
  );
  assert_eq!(list.report.parts, 2);
  assert_eq!(list.report.header_line, 1);
  assert_eq!(list.report.comments, 0);
  assert_eq!(list.report.blank_lines, 0);
  assert_eq!(
    list.report.render(),
    "format: csv\nheader: line 1\nparts: 2\ncomments: 0\nblank lines: 0\n"
  );
}

#[test]
fn the_header_names_the_four_columns_in_a_fixed_order() {
  // Fixing the order is what makes a swapped width and height a refusal instead of a rotated part.
  assert_eq!(
    refusal("name,height,width,quantity\nbracket,80,120,4\n"),
    "csv at line 1, field 2 (\"width\"): expected the header row \"name,width,height,quantity\", found \"height\""
  );
  assert_eq!(
    refusal("\"name\",width,height,quantity\nbracket,120,80,4\n"),
    "csv at line 1, field 1 (\"name\"): quoted fields are not supported; found '\"' in \"\\\"name\\\"\""
  );
  // Case and surrounding space are the same header.
  let list = parse(" Name , WIDTH ,Height, quantity \nbracket,120,80,4\n");
  assert_eq!(list.parts.len(), 1);
  assert_eq!(list.parts[0].name, "bracket");
}

#[test]
fn the_header_row_is_required() {
  let error = import(b"bracket,120,80,4\n").expect_err("refused");
  assert_eq!(
    error,
    ImportError::Parse {
      format: "csv",
      location: Some("line 1, field 1 (\"name\")".to_string()),
      message: "expected the header row \"name,width,height,quantity\", found \"bracket\""
        .to_string(),
    }
  );
  assert_eq!(
    error.to_string(),
    "csv at line 1, field 1 (\"name\"): expected the header row \"name,width,height,quantity\", found \"bracket\""
  );
}

#[test]
fn blank_lines_and_comment_lines_are_skipped_everywhere() {
  let list = parse(concat!(
    "\n",
    "  \n",
    "# the parts for job 12\n",
    "name,width,height,quantity\n",
    "\n",
    "   # a comment indented by spaces\n",
    "bracket,120,80,4\n",
    "\n",
    "panel,600,400,1\n",
    "\n",
  ));

  assert_eq!(
    list
      .parts
      .iter()
      .map(|part| (part.line, part.name.as_str(), part.quantity))
      .collect::<Vec<_>>(),
    vec![(7, "bracket", 4), (9, "panel", 1)]
  );
  assert_eq!(list.report.header_line, 4);
  assert_eq!(list.report.comments, 2);
  assert_eq!(list.report.blank_lines, 5);
}

#[test]
fn a_row_with_the_wrong_number_of_fields_is_refused_naming_the_line() {
  assert_eq!(
    refusal("name,width,height,quantity\nbracket,120,80,4\npanel,600,400\n"),
    "csv at line 3: expected 4 fields (name,width,height,quantity), found 3"
  );
  assert_eq!(
    refusal("name,width,height,quantity\nbracket,120,80,4,spare\n"),
    "csv at line 2: expected 4 fields (name,width,height,quantity), found 5"
  );
  // A header that does not carry four columns is the same refusal, on the header's own line.
  assert_eq!(
    refusal("name,width,height\n"),
    "csv at line 1: expected 4 fields (name,width,height,quantity), found 3"
  );
}

#[test]
fn a_dimension_that_is_not_a_positive_finite_number_is_refused_naming_the_field_and_value() {
  for (value, column, field) in [
    ("0", 2, "width"),
    ("-5", 2, "width"),
    ("ten", 2, "width"),
    ("inf", 2, "width"),
    ("NaN", 2, "width"),
    ("", 2, "width"),
    ("0", 3, "height"),
    ("-0.5", 3, "height"),
  ] {
    let mut fields: Vec<String> = vec!["bracket".into(), "120".into(), "80".into(), "4".into()];
    fields[column - 1] = value.to_string();

    assert_eq!(
      refusal(&format!(
        "name,width,height,quantity\n{}\n",
        fields.join(",")
      )),
      format!(
        "csv at line 2, field {column} (\"{field}\"): \
         expected a positive finite number of millimetres, found {value:?}"
      ),
      "{field} = {value:?}"
    );
  }
}

#[test]
fn a_quantity_that_is_not_a_positive_integer_is_refused_naming_the_field_and_value() {
  for value in ["0", "1.5", "-2", "two", ""] {
    assert_eq!(
      refusal(&format!(
        "name,width,height,quantity\nbracket,120,80,{value}\n"
      )),
      format!(
        "csv at line 2, field 4 (\"quantity\"): expected a positive integer, found {value:?}"
      ),
      "quantity = {value:?}"
    );
  }
}

#[test]
fn an_empty_part_name_is_refused_naming_the_field() {
  assert_eq!(
    refusal("name,width,height,quantity\n  ,120,80,4\n"),
    "csv at line 2, field 1 (\"name\"): expected a non-empty part name, found \"\""
  );
}

#[test]
fn a_duplicate_part_name_is_refused_naming_both_lines() {
  // Refused, not summed: quantity is already a column, so summing would silently change the
  // quantity the list asked for and hide the copy-paste that made the row.
  assert_eq!(
    refusal("name,width,height,quantity\nbracket,120,80,4\npanel,600,400,1\nbracket,120,80,2\n"),
    "csv at line 4, field 1 (\"name\"): duplicate part name \"bracket\": already named on line 2"
  );
  // Names are compared exactly: case is part of the name.
  let list = parse("name,width,height,quantity\nbracket,120,80,4\nBracket,600,400,1\n");
  assert_eq!(list.parts.len(), 2);
}

#[test]
fn an_empty_part_list_is_refused_naming_why() {
  assert_eq!(
    refusal("name,width,height,quantity\n"),
    "csv: nothing to import: the header row is not followed by any part row"
  );
  assert_eq!(
    refusal("# a list of nothing\n\n   \n"),
    "csv: nothing to import: no header row: every line was blank or a comment"
  );
  assert_eq!(
    refusal(""),
    "csv: nothing to import: no header row: every line was blank or a comment"
  );
}

#[test]
fn quoted_fields_are_refused_by_name_rather_than_mis_parsed() {
  assert_eq!(
    refusal("name,width,height,quantity\n\"bracket\",120,80,4\n"),
    "csv at line 2, field 1 (\"name\"): quoted fields are not supported; found '\"' in \"\\\"bracket\\\"\""
  );
  // A quote inside an otherwise bare field is refused the same way, not read as part of the name.
  assert_eq!(
    refusal("name,width,height,quantity\nbrack\"et,120,80,4\n"),
    "csv at line 2, field 1 (\"name\"): quoted fields are not supported; found '\"' in \"brack\\\"et\""
  );
  // A `#` inside a field is not a comment: only a line that starts with one is.
  let list = parse("name,width,height,quantity\npart#1,120,80,4\n");
  assert_eq!(list.parts[0].name, "part#1");
  assert_eq!(list.report.comments, 0);
}

#[test]
fn a_byte_order_mark_and_crlf_endings_are_tolerated() {
  let list = import("\u{feff}name,width,height,quantity\r\nbracket,120,80,4\r\n".as_bytes())
    .expect("import");

  assert_eq!(list.parts.len(), 1);
  assert_eq!(list.parts[0].name, "bracket");
  assert_eq!(list.parts[0].width, 120.0);
  assert_eq!(list.report.header_line, 1);
  assert_eq!(
    list.report.render(),
    "format: csv\nheader: line 1\nparts: 1\ncomments: 0\nblank lines: 0\n"
  );
}

#[test]
fn a_file_that_is_not_utf8_is_refused_naming_the_line() {
  assert_eq!(
    import(b"name,width,height,quantity\nbracket\xff,120,80,4\n")
      .expect_err("refused")
      .to_string(),
    "csv at line 2: the file is not valid UTF-8 (byte 34 is not part of a character)"
  );
}
