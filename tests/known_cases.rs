use serde_yaml::{value::Mapping, Value};
use yars_yaml_formatter::format_yaml_string;

#[path = "support/mod.rs"]
mod support;

use support::{approx_equal, parse_yaml};

struct KnownCase {
    name: &'static str,
    input: &'static str,
}

const GREEDY_REGEX_CASE: &str = r#"---
column:
  canonical_name: SOURCE
  data_type: StringType
  description: 'Vendor name who is performing IHA outreach. Ex: HCMG'
  length: 20
  name: source
  nullable:
    MD: false
    ME: false
    MP: false
  reporting_requirement: R
  source: data
validations:
  - kwargs:
      column: source
    meta:
      description: Column source must exist in table schema
"#;

const ESCAPED_BACKSLASH_CASE: &str = r#"
explanation: "Requirement: This is a very long requirement text that contains newlines.\nIf it is not available, leave it blank.\n\nProvenance policy: enterprise_only. The target field represents an operational event date that must be tracked carefully."
"#;

const NO_AT_LINE_END_CASE: &str = r#"
description: 'Detects duplicate rows within a delivered file. Constraint: no
        two rows from the same file may share the same meta_checksum.'
"#;

const MULTIPLE_NO_CASE: &str = r#"
description: 'There should be no duplicates. If there are no matches, no action is taken. This ensures no data loss.'
"#;

const YES_ON_OFF_CASE: &str = r#"
description: 'Turn on the feature. If yes is selected, validation runs. Turn off when done. Answer no to skip.'
"#;

const KNOWN_CASES: &[KnownCase] = &[
    KnownCase {
        name: "greedy_regex_case",
        input: GREEDY_REGEX_CASE,
    },
    KnownCase {
        name: "escaped_backslash_case",
        input: ESCAPED_BACKSLASH_CASE,
    },
];

fn mapping_get<'a>(map: &'a Mapping, key: &str) -> Option<&'a Value> {
    let key_value = Value::String(key.to_string());
    map.get(&key_value)
}

#[test]
fn known_cases_round_trip() {
    for case in KNOWN_CASES {
        let formatted = format_yaml_string(case.input.trim()).unwrap();
        let second = format_yaml_string(&formatted).unwrap();
        assert_eq!(formatted, second, "case {} not idempotent", case.name);

        let original = parse_yaml(case.input.trim());
        let reparsed = parse_yaml(&formatted);
        assert!(
            approx_equal(&original, &reparsed),
            "case {} altered data",
            case.name
        );
    }
}

#[test]
fn greedy_regex_case_preserves_structure() {
    let formatted = format_yaml_string(GREEDY_REGEX_CASE.trim()).unwrap();
    let parsed = parse_yaml(&formatted);
    let root = parsed.as_mapping().expect("root mapping expected");
    let column = mapping_get(root, "column")
        .and_then(Value::as_mapping)
        .expect("column map should exist");

    let description = mapping_get(column, "description")
        .and_then(Value::as_str)
        .unwrap();
    assert_eq!(
        description,
        "Vendor name who is performing IHA outreach. Ex: HCMG"
    );
    let lower = description.to_lowercase();
    assert!(
        !lower.contains("length: 20")
            && !lower.contains("nullable:")
            && !lower.contains("validations:")
            && !lower.contains("kwargs:"),
        "description should not absorb surrounding YAML structure"
    );
}

#[test]
fn boolean_words_preserved_no_false_conversions() {
    let formatted = format_yaml_string(NO_AT_LINE_END_CASE.trim()).unwrap();
    let parsed = parse_yaml(&formatted);
    let root = parsed.as_mapping().unwrap();
    let description = mapping_get(root, "description")
        .and_then(Value::as_str)
        .unwrap();

    assert!(
        description.contains("no two"),
        "expected 'no two' in description, got {description}"
    );
    assert!(
        !description.contains("false two"),
        "should not substitute 'no' → 'false'"
    );
}

#[test]
fn multiple_no_instances_preserved() {
    let formatted = format_yaml_string(MULTIPLE_NO_CASE.trim()).unwrap();
    let parsed = parse_yaml(&formatted);
    let root = parsed.as_mapping().unwrap();
    let description = mapping_get(root, "description")
        .and_then(Value::as_str)
        .unwrap();

    let no_count = description.to_lowercase().matches(" no ").count();
    assert!(
        no_count >= 3,
        "expected at least 3 instances of 'no', got {no_count}"
    );
    assert!(!description.contains("false "), "should not inject 'false'");
}

#[test]
fn yes_on_off_preserved() {
    let formatted = format_yaml_string(YES_ON_OFF_CASE.trim()).unwrap();
    let parsed = parse_yaml(&formatted);
    let root = parsed.as_mapping().unwrap();
    let description = mapping_get(root, "description")
        .and_then(Value::as_str)
        .unwrap();

    assert!(
        description.contains("on the feature") || description.contains("on  the feature"),
        "missing 'on the feature' phrase"
    );
    assert!(
        description.contains("yes is selected") || description.contains("yes  is selected"),
        "missing 'yes is selected' phrase"
    );
    assert!(
        description.contains("off when") || description.contains("off  when"),
        "missing 'off when' phrase"
    );
    assert!(
        description.contains("no to skip") || description.contains("no  to skip"),
        "missing 'no to skip' phrase"
    );

    assert!(
        !description.contains("true the feature")
            && !description.contains("true is selected")
            && !description.contains("false when")
            && !description.contains("false to skip"),
        "boolean words should not be converted"
    );
}

#[test]
fn escaped_backslash_case_is_idempotent() {
    let first = format_yaml_string(ESCAPED_BACKSLASH_CASE.trim()).unwrap();
    let second = format_yaml_string(&first).unwrap();
    let third = format_yaml_string(&second).unwrap();

    assert_eq!(first, second, "escaped backslash case failed after first pass");
    assert_eq!(second, third, "escaped backslash case failed after second pass");
}
