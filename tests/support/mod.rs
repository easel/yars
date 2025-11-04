#![allow(dead_code)]

use serde_yaml::value::Mapping;
use serde_yaml::{Number, Value};

const FLOAT_REL_TOL: f64 = 1e-14;
const FLOAT_ABS_TOL: f64 = 1e-10;

pub fn parse_yaml(text: &str) -> Value {
    serde_yaml::from_str(text).expect("YAML should parse")
}

pub fn approx_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Number(x), Value::Number(y)) => approx_number(x, y),
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Sequence(xs), Value::Sequence(ys)) => {
            xs.len() == ys.len()
                && xs
                    .iter()
                    .zip(ys.iter())
                    .all(|(x, y)| approx_equal(x, y))
        }
        (Value::Mapping(xs), Value::Mapping(ys)) => approx_mapping(xs, ys),
        (Value::Tagged(x), Value::Tagged(y)) => x.tag == y.tag && approx_equal(&x.value, &y.value),
        _ => false,
    }
}

fn approx_mapping(a: &Mapping, b: &Mapping) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (key, value) in a {
        match b.get(key) {
            Some(other) if approx_equal(value, other) => {}
            _ => return false,
        }
    }
    true
}

fn approx_number(a: &Number, b: &Number) -> bool {
    match (number_to_f64(a), number_to_f64(b)) {
        (Some(x), Some(y)) => approx_f64(x, y),
        _ => a == b,
    }
}

fn number_to_f64(num: &Number) -> Option<f64> {
    if let Some(i) = num.as_i64() {
        Some(i as f64)
    } else if let Some(u) = num.as_u64() {
        Some(u as f64)
    } else {
        num.as_f64()
    }
}

fn approx_f64(a: f64, b: f64) -> bool {
    if a.is_nan() && b.is_nan() {
        true
    } else if !a.is_finite() || !b.is_finite() {
        a == b
    } else {
        (a - b).abs() <= FLOAT_ABS_TOL.max(FLOAT_REL_TOL * a.abs()).max(FLOAT_REL_TOL * b.abs())
    }
}

pub fn value_from_pairs(entries: Vec<(String, Value)>) -> Value {
    let mut mapping = Mapping::new();
    for (key, value) in entries {
        mapping.insert(Value::String(key), value);
    }
    Value::Mapping(mapping)
}

pub fn string_value<S: Into<String>>(value: S) -> Value {
    Value::String(value.into())
}

pub fn bool_value(value: bool) -> Value {
    Value::Bool(value)
}

pub fn int_value(value: i64) -> Value {
    Value::Number(Number::from(value))
}

pub fn sequence_value(items: Vec<Value>) -> Value {
    Value::Sequence(items)
}

pub fn has_literal_block_bug(value: &Value) -> bool {
    match value {
        Value::String(text) => string_has_literal_block_bug(text),
        Value::Mapping(map) => {
            if map.keys().any(|key| match key {
                Value::String(text) => is_yaml_boolean_keyword(text),
                _ => false,
            }) {
                return true;
            }
            map.values().any(has_literal_block_bug)
        }
        Value::Sequence(seq) => seq.iter().any(has_literal_block_bug),
        Value::Tagged(tagged) => has_literal_block_bug(&tagged.value),
        _ => false,
    }
}

fn string_has_literal_block_bug(text: &str) -> bool {
    if (text.starts_with('\n') || text.ends_with('\n')) && !text.trim().is_empty() {
        return true;
    }

    let special_chars = [':', '-', '#', '[', ']', '{', '}', '|', '>', ';', '!', '&', '*'];
    if special_chars.iter().any(|c| text.contains(&format!("\n{c}"))) {
        return true;
    }

    text.as_bytes()
        .windows(2)
        .any(|pair| pair[0] == b'#' && pair[1].is_ascii_alphanumeric())
}

fn is_yaml_boolean_keyword(text: &str) -> bool {
    matches!(
        text.to_ascii_lowercase().as_str(),
        "yes" | "no" | "on" | "off" | "true" | "false"
    )
}
