use std::fs;
use std::io::Write;
use std::path::PathBuf;

use serde_yaml::{self, Value};
use tempfile::NamedTempFile;
use yars_yaml_formatter::{
    format_yaml_dict, format_yaml_file, format_yaml_files, format_yaml_string, YamlFormatError,
};

#[path = "support/mod.rs"]
mod support;
use support::parse_yaml;

#[test]
fn multiline_strings_become_literal_blocks() {
    let input = r#"
description: "This is a long description\nthat spans\nmultiple lines"
"#;
    let formatted = format_yaml_string(input.trim()).expect("formatting should succeed");

    assert!(formatted.contains("description: |-\n  This is a long description"));
    let parsed = parse_yaml(&formatted);
    assert_eq!(
        parsed["description"],
        Value::String("This is a long description\nthat spans\nmultiple lines".into())
    );
}

#[test]
fn leading_or_trailing_whitespace_stays_quoted() {
    let input = r#"
description: " leading space\nvalue "
"#;
    let formatted = format_yaml_string(input.trim()).expect("formatting should succeed");

    assert!(!formatted.contains("description: |"));
    assert!(formatted.contains(r#" " leading space\nvalue ""#));
}

#[test]
fn dictionary_keys_sorted_recursively() {
    let data = serde_yaml::from_str::<Value>(
        r#"
zebra: 1
apple:
  zed: 2
  beta: 1
middle: 3
"#,
    )
    .unwrap();

    let formatted = format_yaml_string(&serde_yaml::to_string(&data).unwrap()).unwrap();
    let lines: Vec<&str> = formatted.lines().collect();
    assert!(lines[0].starts_with("apple:"));
    assert!(lines[1].starts_with("  beta:"));
    assert!(lines[2].starts_with("  zed:"));
    assert!(lines[3].starts_with("middle:"));
    assert!(lines[4].starts_with("zebra:"));
}

#[test]
fn list_order_preserved() {
    let yaml = r#"
items:
  - zebra
  - apple
  - middle
"#;
    let formatted = format_yaml_string(yaml.trim()).unwrap();
    let parsed = parse_yaml(&formatted);
    let sequence = parsed["items"]
        .as_sequence()
        .expect("items should be a list");
    let names: Vec<String> = sequence
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, ["zebra", "apple", "middle"]);
}

#[test]
fn formatter_is_idempotent() {
    let yaml = r#"
meta:
  description: "Something\nwith\nnewlines"
  order: ["z", "a", "m"]
"#;
    let first = format_yaml_string(yaml.trim()).unwrap();
    let second = format_yaml_string(&first).unwrap();
    assert_eq!(first, second);
}

#[test]
fn escape_sequences_round_trip() {
    let mut data = serde_yaml::Mapping::new();
    data.insert(Value::String("escape".into()), Value::String("line1\nline2\x1f𐀀".into()));
    data.insert(Value::String("plain".into()), Value::String("simple".into()));

    let formatted = format_yaml_dict(&data).unwrap();
    let reparsed = parse_yaml(&formatted);
    assert_eq!(reparsed["escape"], Value::String("line1\nline2\x1f𐀀".into()));
    assert_eq!(reparsed["plain"], Value::String("simple".into()));
}

#[test]
fn top_level_list_rejected() {
    let yaml = "- a\n- b\n";
    let err = format_yaml_string(yaml).unwrap_err();
    matches!(
        err,
        YamlFormatError::TopLevelList
    );
}

#[test]
fn invalid_yaml_raises_error() {
    let err = format_yaml_string("foo: [bar").unwrap_err();
    assert!(matches!(err, YamlFormatError::Format(_)));
}

#[test]
fn null_document_returns_original() {
    let original = "null\n";
    let formatted = format_yaml_string(original).unwrap();
    assert_eq!(formatted, original);
}

#[test]
fn format_yaml_dict_requires_mapping() {
    let err = format_yaml_dict(&vec!["a", "b"]).unwrap_err();
    match err {
        YamlFormatError::Format(message) => assert!(message.contains("Expected dict")),
        YamlFormatError::TopLevelList => {}
        other => panic!("unexpected error: {other:?}"),
    };
}

#[test]
fn file_formatting_writes_only_on_change() {
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, "z: 1\na: 2").unwrap();
    let path = temp.into_temp_path();

    // First run should change order.
    let changed = format_yaml_file(path.as_ref(), false).unwrap();
    assert!(changed);

    // Second run should be idempotent.
    let changed_again = format_yaml_file(path.as_ref(), false).unwrap();
    assert!(!changed_again);

    let path_buf = path.to_path_buf();
    let contents = fs::read_to_string(&path_buf).unwrap();
    assert!(contents.starts_with("a: 2"));
}

#[test]
fn batch_formatting_aggregates_errors() {
    let temp = NamedTempFile::new().unwrap();
    let good_path = temp.path().to_path_buf();
    fs::write(&good_path, "b: 2\na: 1\n").unwrap();

    let missing_path = PathBuf::from("surely-does-not-exist.yaml");

    let (changed, errors, messages) =
        format_yaml_files([good_path.as_path(), missing_path.as_path()], false);

    assert_eq!(changed, 1);
    assert_eq!(errors, 1);
    assert_eq!(messages.len(), 1);
    assert!(messages[0].contains("File not found"));
}
