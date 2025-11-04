use proptest::prelude::*;
use serde_yaml::value::Mapping;
use serde_yaml::{Number, Value};
use yars_yaml_formatter::{format_yaml_dict, format_yaml_string};

#[path = "support/mod.rs"]
mod support;

use support::{
    approx_equal, bool_value, has_literal_block_bug, int_value, parse_yaml, sequence_value,
    string_value, value_from_pairs,
};

fn chars_to_string(chars: Vec<char>) -> String {
    chars.into_iter().collect()
}

fn mapping_get<'a>(map: &'a Mapping, key: &str) -> Option<&'a Value> {
    let key_value = Value::String(key.to_string());
    map.get(&key_value)
}

fn value_sort_key(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(true) => "true".to_owned(),
        Value::Bool(false) => "false".to_owned(),
        Value::Number(num) => num.to_string(),
        Value::String(text) => text.clone(),
        Value::Tagged(tagged) => format!("{}:{}", tagged.tag, value_sort_key(&tagged.value)),
        Value::Sequence(_) | Value::Mapping(_) => serde_yaml::to_string(value)
            .unwrap_or_else(|_| format!("{value:?}"))
            .replace('\n', " ")
            .trim()
            .to_owned(),
    }
}

fn identifier_string(min: usize, max: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            prop::char::range('a', 'z'),
            prop::char::range('A', 'Z'),
            prop::char::range('0', '9'),
            Just('_'),
        ],
        min..=max,
    )
    .prop_map(chars_to_string)
}

fn uppercase_identifier_string(min: usize, max: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            prop::char::range('A', 'Z'),
            prop::char::range('0', '9'),
            Just('_'),
        ],
        min..=max,
    )
    .prop_map(chars_to_string)
}

fn text_char() -> impl Strategy<Value = char> {
    prop_oneof![
        prop::char::range(' ', '~'),
        Just('\n'),
        Just('\t'),
    ]
}

fn text_string(min: usize, max: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(text_char(), min..=max).prop_map(chars_to_string)
}

fn scalar_string_strategy() -> BoxedStrategy<String> {
    prop_oneof![
        text_string(0, 80),
        text_string(0, 200),
    ]
    .boxed()
}

fn scalar_value_strategy() -> BoxedStrategy<Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        (-10_000i64..=10_000i64).prop_map(|i| Value::Number(Number::from(i))),
        any::<f64>()
            .prop_filter("finite float", |f| f.is_finite())
            .prop_map(|f| Value::Number(Number::from(f))),
        scalar_string_strategy().prop_map(Value::String),
    ]
    .boxed()
}

fn yaml_value_strategy() -> BoxedStrategy<Value> {
    scalar_value_strategy()
        .prop_recursive(4, 128, 4, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..=4).prop_map(Value::Sequence),
                prop::collection::btree_map(identifier_string(1, 24), inner, 0..=4).prop_map(
                    |entries| {
                        let mut mapping = Mapping::new();
                        for (k, v) in entries {
                            mapping.insert(Value::String(k), v);
                        }
                        Value::Mapping(mapping)
                    },
                ),
            ]
        })
        .boxed()
}

fn mapping_strategy() -> BoxedStrategy<Value> {
    prop::collection::btree_map(identifier_string(1, 24), yaml_value_strategy(), 0..=6)
        .prop_map(|entries| {
            let mut mapping = Mapping::new();
            for (k, v) in entries {
                mapping.insert(Value::String(k), v);
            }
            Value::Mapping(mapping)
        })
        .boxed()
}

fn kwargs_value_strategy() -> BoxedStrategy<Value> {
    prop_oneof![
        text_string(1, 60)
            .prop_filter("avoid bare hyphen", |s| s.trim() != "-")
            .prop_map(string_value),
        (-5000i64..=5000i64).prop_map(int_value),
        any::<bool>().prop_map(bool_value),
    ]
    .boxed()
}

fn kwargs_strategy() -> BoxedStrategy<Value> {
    prop::collection::btree_map(identifier_string(1, 20), kwargs_value_strategy(), 1..=5)
        .prop_map(|entries| {
            let pairs = entries
                .into_iter()
                .map(|(k, v)| (k, v))
                .collect::<Vec<_>>();
            value_from_pairs(pairs)
        })
        .boxed()
}

fn meta_strategy() -> BoxedStrategy<Value> {
    (
        text_string(10, 200),
        prop::sample::select(vec!["critical", "warning", "info"]),
        uppercase_identifier_string(5, 40),
    )
        .prop_map(|(description, severity, rule_id)| {
            value_from_pairs(vec![
                ("description".to_string(), string_value(description)),
                ("severity".to_string(), string_value(severity.to_string())),
                ("rule_id".to_string(), string_value(rule_id)),
            ])
        })
        .boxed()
}

fn validation_strategy() -> BoxedStrategy<Value> {
    (
        kwargs_strategy(),
        meta_strategy(),
        identifier_string(5, 30),
    )
        .prop_map(|(kwargs, meta, type_name)| {
            value_from_pairs(vec![
                ("kwargs".to_string(), kwargs),
                ("meta".to_string(), meta),
                ("type".to_string(), string_value(type_name)),
            ])
        })
        .boxed()
}

fn validations_strategy() -> BoxedStrategy<Value> {
    prop::collection::vec(validation_strategy(), 0..=5)
        .prop_map(sequence_value)
        .boxed()
}

fn nullable_strategy() -> BoxedStrategy<Value> {
    prop::sample::subsequence(vec!["MD", "ME", "MP"], 1..=3)
        .prop_flat_map(|keys| {
            let len = keys.len();
            prop::collection::vec(any::<bool>(), len).prop_map(move |values| {
                let pairs = keys
                    .iter()
                    .cloned()
                    .zip(values.into_iter())
                    .map(|(k, v)| (k.to_string(), bool_value(v)))
                    .collect::<Vec<_>>();
                value_from_pairs(pairs)
            })
        })
        .boxed()
}

fn length_strategy() -> BoxedStrategy<Value> {
    prop_oneof![
        Just(Value::Null),
        (1..=1000i64).prop_map(int_value),
    ]
    .boxed()
}

fn umf_like_structure() -> BoxedStrategy<Value> {
    (
        identifier_string(1, 30),
        uppercase_identifier_string(1, 30),
        prop::sample::select(vec!["StringType", "IntegerType", "DateType", "BooleanType"]),
        text_string(10, 200),
        nullable_strategy(),
        length_strategy(),
        validations_strategy(),
    )
        .prop_map(
            |(name, canonical_name, data_type, description, nullable, length, validations)| {
                let column = value_from_pairs(vec![
                    ("name".to_string(), string_value(name)),
                    ("canonical_name".to_string(), string_value(canonical_name)),
                    ("data_type".to_string(), string_value(data_type.to_string())),
                    ("description".to_string(), string_value(description)),
                    ("nullable".to_string(), nullable),
                    ("length".to_string(), length),
                ]);

                value_from_pairs(vec![
                    ("column".to_string(), column),
                    ("validations".to_string(), validations),
                ])
            },
        )
        .boxed()
}

fn colon_string_strategy() -> BoxedStrategy<String> {
    text_string(50, 500)
        .prop_filter("must contain colon", |text| text.contains(':'))
        .boxed()
}

fn list_of_dicts_strategy() -> BoxedStrategy<Vec<Value>> {
    prop::collection::vec(
        prop::collection::btree_map(identifier_string(1, 15), kwargs_value_strategy(), 1..=6)
            .prop_map(|entries| {
                let pairs = entries
                    .into_iter()
                    .map(|(k, v)| (k, v))
                    .collect::<Vec<_>>();
                value_from_pairs(pairs)
            }),
        1..=15,
    )
    .boxed()
}

fn simple_string_dict_strategy() -> BoxedStrategy<Value> {
    prop::collection::btree_map(
        identifier_string(1, 20),
        text_string(0, 300),
        3..=50,
    )
    .prop_filter("avoid yaml boolean keys", |map| {
        !map
            .keys()
            .any(|key| matches!(key.to_ascii_lowercase().as_str(), "yes" | "no" | "on" | "off"))
    })
    .prop_map(|entries| {
        let pairs = entries
            .into_iter()
            .map(|(k, v)| (k, string_value(v)))
            .collect::<Vec<_>>();
        value_from_pairs(pairs)
    })
    .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 150,
        .. ProptestConfig::default()
    })]
    #[test]
    fn prop_formatter_idempotence_arbitrary_yaml(value in mapping_strategy()) {
        prop_assume!(!has_literal_block_bug(&value));

        let yaml_str = serde_yaml::to_string(&value).unwrap();
        let formatted_once = format_yaml_string(&yaml_str).expect("formatter should succeed");
        let formatted_twice = format_yaml_string(&formatted_once).expect("formatter should succeed");

        let parsed_once: Value = serde_yaml::from_str(&formatted_once).unwrap();
        let parsed_twice: Value = serde_yaml::from_str(&formatted_twice).unwrap();
        prop_assert!(approx_equal(&parsed_once, &parsed_twice));

        let original_parsed: Value = serde_yaml::from_str(&yaml_str).unwrap();
        prop_assert!(approx_equal(&parsed_once, &original_parsed));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 100,
        .. ProptestConfig::default()
    })]
    #[test]
    fn prop_formatter_idempotence_umf_like(value in umf_like_structure()) {
        let yaml_str = serde_yaml::to_string(&value).unwrap();
        let formatted_once = format_yaml_string(&yaml_str).unwrap();
        let formatted_twice = format_yaml_string(&formatted_once).unwrap();
        prop_assert!(formatted_once == formatted_twice);

        let parsed: Value = serde_yaml::from_str(&formatted_twice).unwrap();
        let map = parsed.as_mapping().expect("root should be a mapping");
        prop_assert!(mapping_get(map, "column").is_some());
        prop_assert!(mapping_get(map, "validations").is_some());
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 80,
        .. ProptestConfig::default()
    })]
    #[test]
    fn prop_long_strings_with_colons(text in colon_string_strategy()) {
        let data = value_from_pairs(vec![
            ("meta".to_string(), value_from_pairs(vec![
                ("description".to_string(), string_value(text.clone())),
                ("severity".to_string(), string_value("warning")),
            ])),
            ("other_field".to_string(), string_value("value")),
        ]);

        let yaml_str = serde_yaml::to_string(&data).unwrap();
        let formatted = format_yaml_string(&yaml_str).unwrap();

        let parsed: Value = parse_yaml(&formatted);
        let root = parsed.as_mapping().unwrap();
        let meta = mapping_get(root, "meta").and_then(Value::as_mapping).unwrap();
        let description = mapping_get(meta, "description")
            .and_then(Value::as_str)
            .unwrap()
            .to_string();

        let normalized_original = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let normalized_output = description.split_whitespace().collect::<Vec<_>>().join(" ");
        prop_assert_eq!(normalized_original, normalized_output);

        prop_assert_eq!(
            mapping_get(root, "other_field").and_then(Value::as_str).unwrap(),
            "value"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 100,
        .. ProptestConfig::default()
    })]
    #[test]
    fn prop_list_of_dicts_preservation(list in list_of_dicts_strategy()) {
        let data = value_from_pairs(vec![
            ("validations".to_string(), sequence_value(list.clone())),
        ]);

        let yaml_str = serde_yaml::to_string(&data).unwrap();
        let formatted_once = format_yaml_string(&yaml_str).unwrap();
        let formatted_twice = format_yaml_string(&formatted_once).unwrap();
        prop_assert!(formatted_once == formatted_twice);

        let parsed: Value = serde_yaml::from_str(&formatted_twice).unwrap();
        let root = parsed.as_mapping().unwrap();
        let parsed_list = mapping_get(root, "validations")
            .and_then(Value::as_sequence)
            .unwrap();
        prop_assert_eq!(parsed_list.len(), list.len());
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 80,
        .. ProptestConfig::default()
    })]
    #[test]
    fn prop_dictionary_key_ordering(dict in simple_string_dict_strategy()) {
        let yaml_str = serde_yaml::to_string(&dict).unwrap();
        let formatted = format_yaml_string(&yaml_str).unwrap();

        let parsed: Value = serde_yaml::from_str(&formatted).unwrap();
        if let Value::Mapping(map) = parsed {
            let actual_keys: Vec<String> = map.keys().map(value_sort_key).collect();
            let mut expected_keys = actual_keys.clone();
            expected_keys.sort();
            prop_assert_eq!(actual_keys, expected_keys);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 120,
        .. ProptestConfig::default()
    })]
    #[test]
    fn prop_arbitrary_string_descriptions(text in text_string(0, 1000)) {
        let data = value_from_pairs(vec![
            ("description".to_string(), string_value(text.clone())),
        ]);

        let yaml_str = serde_yaml::to_string(&data).unwrap();
        let formatted_once = format_yaml_string(&yaml_str).unwrap();
        let formatted_twice = format_yaml_string(&formatted_once).unwrap();
        prop_assert!(formatted_once == formatted_twice);

        let parsed: Value = serde_yaml::from_str(&formatted_twice).unwrap();
        let root = parsed.as_mapping().unwrap();
        let output = mapping_get(root, "description")
            .and_then(Value::as_str)
            .unwrap();

        let orig_norm = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let out_norm = output.split_whitespace().collect::<Vec<_>>().join(" ");
        prop_assert_eq!(orig_norm, out_norm);
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 120,
        .. ProptestConfig::default()
    })]
    #[test]
    fn prop_format_dict_matches_string(value in mapping_strategy()) {
        prop_assume!(!has_literal_block_bug(&value));

        let dict_output = format_yaml_dict(&value).unwrap();

        let from_string = format_yaml_string(&serde_yaml::to_string(&value).unwrap()).unwrap();

        let parsed_dict: Value = serde_yaml::from_str(&dict_output).unwrap();
        let parsed_string: Value = serde_yaml::from_str(&from_string).unwrap();
        prop_assert!(approx_equal(&parsed_dict, &parsed_string));
    }
}
