//! Rust implementation of the YAML formatter described in `SPEC.md`.
//!
//! The API mirrors the Python reference implementation:
//! - `format_yaml_string`
//! - `format_yaml_dict`
//! - `format_yaml_file`
//! - `format_yaml_files`

use serde::Serialize;
use serde_yaml::value::{Mapping, TaggedValue};
use serde_yaml::Value;
use std::borrow::Cow;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use thiserror::Error;

/// Error type emitted by the formatter.
#[derive(Debug, Error)]
pub enum YamlFormatError {
    #[error("Error formatting YAML: {0}")]
    Format(String),
    #[error(
        "Top-level lists are not supported by the YAML formatter. \
UMF files should always have a dictionary at the root level with \
'column:' and 'validations:' keys. If you're seeing this error, your YAML \
file may be structured incorrectly."
    )]
    TopLevelList,
    #[error("File not found: {0}")]
    MissingFile(String),
    #[error("Failed to read {0}: {1}")]
    ReadFailure(String, String),
    #[error("Failed to write {0}: {1}")]
    WriteFailure(String, String),
}

/// Type alias with the Python-style name.
pub type YAMLFormatError = YamlFormatError;

/// Format YAML text. Returns the original string when the parsed document is `null`.
pub fn format_yaml_string(input: &str) -> Result<String, YamlFormatError> {
    let to_parse = strip_leading_marker(input);
    match serde_yaml::from_str::<Value>(to_parse) {
        Ok(Value::Null) => Ok(input.to_owned()),
        Ok(Value::Sequence(_)) => Err(YamlFormatError::TopLevelList),
        Ok(Value::Mapping(map)) => {
            let sorted = sort_value(Value::Mapping(map));
            emit_yaml(&sorted)
        }
        Ok(other) => emit_yaml(&sort_value(other)),
        Err(err) => Err(YamlFormatError::Format(err.to_string())),
    }
}

/// Format a serializable structure whose root must be a mapping.
pub fn format_yaml_dict<T>(data: &T) -> Result<String, YamlFormatError>
where
    T: Serialize,
{
    match serde_yaml::to_value(data) {
        Ok(Value::Mapping(map)) => {
            let sorted = sort_value(Value::Mapping(map));
            emit_yaml(&sorted)
        }
        Ok(Value::Null) => Ok(String::new()),
        Ok(Value::Sequence(_)) => Err(YamlFormatError::TopLevelList),
        Ok(other) => Err(YamlFormatError::Format(format!(
            "Expected dict, got {}",
            describe_value(&other)
        ))),
        Err(err) => Err(YamlFormatError::Format(err.to_string())),
    }
}

/// Format a file in-place. Returns whether a change was (or would be) made.
pub fn format_yaml_file(path: &Path, check_only: bool) -> Result<bool, YamlFormatError> {
    if !path.exists() {
        return Err(YamlFormatError::MissingFile(path.display().to_string()));
    }

    let original = fs::read_to_string(path)
        .map_err(|err| YamlFormatError::ReadFailure(path.display().to_string(), err.to_string()))?;
    let formatted = format_yaml_string(&original)?;

    let changed = original != formatted;
    if changed && !check_only {
        fs::write(path, formatted)
            .map_err(|err| YamlFormatError::WriteFailure(path.display().to_string(), err.to_string()))?;
    }

    Ok(changed)
}

/// Format multiple files, aggregating errors.
pub fn format_yaml_files<P>(paths: P, check_only: bool) -> (usize, usize, Vec<String>)
where
    P: IntoIterator,
    P::Item: AsRef<Path>,
{
    let mut changed = 0usize;
    let mut errors = 0usize;
    let mut messages = Vec::new();

    for path in paths {
        match format_yaml_file(path.as_ref(), check_only) {
            Ok(true) => changed += 1,
            Ok(false) => {}
            Err(err) => {
                errors += 1;
                messages.push(err.to_string());
            }
        }
    }

    (changed, errors, messages)
}

// --- Normalisation helpers -------------------------------------------------

fn strip_leading_marker(input: &str) -> &str {
    let trimmed = input.trim_start();
    if let Some(rest) = trimmed.strip_prefix("---\n") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("---") {
        rest
    } else {
        trimmed
    }
}

fn sort_value(value: Value) -> Value {
    match value {
        Value::Mapping(map) => {
            let mut entries = Vec::with_capacity(map.len());
            for (key, val) in map {
                entries.push((key_sort_key(&key), key, sort_value(val)));
            }
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut sorted = Mapping::with_capacity(entries.len());
            for (_, key, val) in entries {
                sorted.insert(key, val);
            }
            Value::Mapping(sorted)
        }
        Value::Sequence(items) => {
            Value::Sequence(items.into_iter().map(sort_value).collect::<Vec<_>>())
        }
        other => other,
    }
}

fn key_sort_key(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(true) => "true".to_owned(),
        Value::Bool(false) => "false".to_owned(),
        Value::Number(num) => num.to_string(),
        Value::String(text) => text.clone(),
        Value::Tagged(tagged) => format!("{}:{}", tagged.tag, key_sort_key(&tagged.value)),
        Value::Sequence(_) | Value::Mapping(_) => serde_yaml::to_string(value)
            .unwrap_or_else(|_| format!("{value:?}"))
            .replace('\n', " ")
            .trim()
            .to_owned(),
    }
}

// --- Emission --------------------------------------------------------------

fn emit_yaml(value: &Value) -> Result<String, YamlFormatError> {
    let mut formatter = Formatter::new();
    formatter.write_root(value)?;
    Ok(formatter.finish())
}

struct Formatter {
    buf: String,
}

impl Formatter {
    fn new() -> Self {
        Self { buf: String::new() }
    }

    fn finish(mut self) -> String {
        if !self.buf.ends_with('\n') && !self.buf.is_empty() {
            self.buf.push('\n');
        }
        self.buf
    }

    fn write_root(&mut self, value: &Value) -> Result<(), YamlFormatError> {
        match value {
            Value::Mapping(map) => self.write_mapping(map, 0),
            other => self.write_value(other, 0, Position::Root),
        }
    }

    fn write_mapping(&mut self, map: &Mapping, indent: usize) -> Result<(), YamlFormatError> {
        if map.is_empty() {
            self.buf.push_str("{}");
            return Ok(());
        }

        let mut iter = map.iter();
        if let Some((key, value)) = iter.next() {
            self.write_indentation(indent);
            self.write_key(key)?;
            self.buf.push(':');
            self.write_value_after_colon(value, indent)?;
        }

        for (key, value) in iter {
            self.buf.push('\n');
            self.write_indentation(indent);
            self.write_key(key)?;
            self.buf.push(':');
            self.write_value_after_colon(value, indent)?;
        }

        Ok(())
    }

    fn write_sequence(&mut self, items: &[Value], indent: usize) -> Result<(), YamlFormatError> {
        if items.is_empty() {
            self.buf.push_str("[]");
            return Ok(());
        }

        let mut first = true;
        for item in items {
            if !first {
                self.buf.push('\n');
            }
            first = false;
            self.write_indentation(indent);
            self.buf.push('-');
            self.write_sequence_item(item, indent)?;
        }
        Ok(())
    }

    fn write_sequence_item(&mut self, value: &Value, indent: usize) -> Result<(), YamlFormatError> {
        match value {
            Value::Mapping(map) => {
                if map.is_empty() {
                    self.buf.push_str(" {}");
                } else {
                    self.buf.push(' ');
                    self.write_inline_mapping(map, indent + 2)?;
                }
            }
            Value::Sequence(seq) => {
                if seq.is_empty() {
                    self.buf.push_str(" []");
                } else {
                    self.buf.push('\n');
                    self.write_sequence(seq, indent + 2)?;
                }
            }
            Value::String(text) => {
                if should_use_literal_block(text) {
                    self.buf.push_str(" |-\n");
                    self.write_literal_block(text, indent + 2);
                } else {
                    self.buf.push(' ');
                    self.write_inline_string(text)?;
                }
            }
            Value::Tagged(tagged) => {
                self.buf.push(' ');
                self.write_tagged(tagged, indent + 2)?;
            }
            scalar => {
                self.buf.push(' ');
                self.write_scalar(scalar)?;
            }
        }
        Ok(())
    }

    fn write_inline_mapping(
        &mut self,
        map: &Mapping,
        indent: usize,
    ) -> Result<(), YamlFormatError> {
        let mut iter = map.iter();
        if let Some((key, value)) = iter.next() {
            self.write_key(key)?;
            self.buf.push(':');
            self.write_value_after_colon(value, indent)?;
        } else {
            self.buf.push_str("{}");
            return Ok(());
        }

        for (key, value) in iter {
            self.buf.push('\n');
            self.write_indentation(indent);
            self.write_key(key)?;
            self.buf.push(':');
            self.write_value_after_colon(value, indent)?;
        }

        Ok(())
    }

    fn write_value_after_colon(
        &mut self,
        value: &Value,
        indent: usize,
    ) -> Result<(), YamlFormatError> {
        match value {
            Value::Mapping(map) => {
                if map.is_empty() {
                    self.buf.push_str(" {}");
                } else {
                    self.buf.push('\n');
                    self.write_mapping(map, indent + 2)?;
                }
            }
            Value::Sequence(seq) => {
                if seq.is_empty() {
                    self.buf.push_str(" []");
                } else {
                    self.buf.push('\n');
                    self.write_sequence(seq, indent + 2)?;
                }
            }
            Value::String(text) => {
                if should_use_literal_block(text) {
                    self.buf.push_str(" |-\n");
                    self.write_literal_block(text, indent + 2);
                } else {
                    self.buf.push(' ');
                    self.write_inline_string(text)?;
                }
            }
            Value::Tagged(tagged) => {
                self.buf.push(' ');
                self.write_tagged(tagged, indent + 2)?;
            }
            scalar => {
                self.buf.push(' ');
                self.write_scalar(scalar)?;
            }
        }
        Ok(())
    }

    fn write_value(
        &mut self,
        value: &Value,
        indent: usize,
        position: Position,
    ) -> Result<(), YamlFormatError> {
        match value {
            Value::Mapping(map) => {
                if matches!(position, Position::Root) {
                    self.write_mapping(map, indent)
                } else {
                    self.write_mapping(map, indent + 2)
                }
            }
            Value::Sequence(seq) => self.write_sequence(seq, indent + 2),
            Value::String(text) => {
                if should_use_literal_block(text) {
                    self.write_literal_block(text, indent + 2);
                    Ok(())
                } else {
                    self.write_inline_string(text)
                }
            }
            Value::Tagged(tagged) => self.write_tagged(tagged, indent + 2),
            scalar => self.write_scalar(scalar),
        }
    }

    fn write_literal_block(&mut self, text: &str, indent: usize) {
        let indent_str = spaces(indent);
        let mut lines = text.split('\n').peekable();
        while let Some(line) = lines.next() {
            self.buf.push_str(&indent_str);
            self.buf.push_str(line);
            if lines.peek().is_some() {
                self.buf.push('\n');
            }
        }
    }

    fn write_inline_string(&mut self, text: &str) -> Result<(), YamlFormatError> {
        if is_plain_string(text) {
            self.buf.push_str(text);
            Ok(())
        } else {
            // Use JSON escaping for convenience (valid YAML double-quoted scalar).
            let encoded = serde_json::to_string(text).map_err(|err| YamlFormatError::Format(err.to_string()))?;
            self.buf.push_str(&encoded);
            Ok(())
        }
    }

    fn write_scalar(&mut self, value: &Value) -> Result<(), YamlFormatError> {
        match value {
            Value::Null => {
                self.buf.push_str("null");
                Ok(())
            }
            Value::Bool(true) => {
                self.buf.push_str("true");
                Ok(())
            }
            Value::Bool(false) => {
                self.buf.push_str("false");
                Ok(())
            }
            Value::Number(num) => {
                write!(self.buf, "{num}").map_err(|err| YamlFormatError::Format(err.to_string()))
            }
            Value::String(text) => self.write_inline_string(text),
            Value::Tagged(tagged) => self.write_tagged(tagged, 0),
            other => {
                let encoded = serde_yaml::to_string(other).map_err(|err| YamlFormatError::Format(err.to_string()))?;
                self.buf
                    .push_str(encoded.trim_end_matches('\n'));
                Ok(())
            }
        }
    }

    fn write_tagged(&mut self, tagged: &TaggedValue, indent: usize) -> Result<(), YamlFormatError> {
        let tag_repr = tagged.tag.to_string();
        self.buf.push_str(tag_repr.as_str());
        self.buf.push(' ');
        self.write_value(&tagged.value, indent, Position::Inline)
    }

    fn write_key(&mut self, key: &Value) -> Result<(), YamlFormatError> {
        match key {
            Value::String(text) if is_plain_key(text) => {
                self.buf.push_str(text);
                Ok(())
            }
            Value::String(text) => {
                let encoded = serde_json::to_string(text).map_err(|err| YamlFormatError::Format(err.to_string()))?;
                self.buf.push_str(&encoded);
                Ok(())
            }
            Value::Number(num) => {
                write!(self.buf, "{num}").map_err(|err| YamlFormatError::Format(err.to_string()))
            }
            Value::Bool(true) => {
                self.buf.push_str("true");
                Ok(())
            }
            Value::Bool(false) => {
                self.buf.push_str("false");
                Ok(())
            }
            Value::Null => {
                self.buf.push_str("null");
                Ok(())
            }
            Value::Tagged(tagged) => {
                let tag_repr = tagged.tag.to_string();
                self.buf.push_str(tag_repr.as_str());
                self.buf.push(' ');
                self.write_inline_string(&value_to_inline_string(&tagged.value)?)?;
                Ok(())
            }
            other => {
                let encoded = serde_yaml::to_string(other).map_err(|err| YamlFormatError::Format(err.to_string()))?;
                self.buf
                    .push_str(encoded.trim_end_matches('\n'));
                Ok(())
            }
        }
    }

    fn write_indentation(&mut self, indent: usize) {
        self.buf.push_str(&spaces(indent));
    }
}

#[derive(Clone, Copy)]
enum Position {
    Root,
    Inline,
}

fn should_use_literal_block(text: &str) -> bool {
    if !text.contains('\n') {
        return false;
    }
    if text.chars().any(is_disallowed_control) {
        return false;
    }
    if text
        .chars()
        .next()
        .is_some_and(|ch| ch.is_whitespace())
        || text
            .chars()
            .last()
            .is_some_and(|ch| ch.is_whitespace())
    {
        return false;
    }
    true
}

fn is_plain_string(text: &str) -> bool {
    if text.is_empty()
        || text.starts_with(char::is_whitespace)
        || text.ends_with(char::is_whitespace)
        || text.starts_with('-')
        || text.contains('\n')
        || is_reserved_keyword(text)
        || looks_like_number(text)
    {
        return false;
    }

    text.chars()
        .all(|ch| matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | '.' | '/'))
}

fn is_plain_key(text: &str) -> bool {
    is_plain_string(text)
}

fn is_disallowed_control(ch: char) -> bool {
    (ch < '\u{20}' && !matches!(ch, '\t' | '\n' | '\r'))
        || ch == '\u{7f}'
        || ('\u{80}'..='\u{9f}').contains(&ch)
}

fn is_reserved_keyword(text: &str) -> bool {
    matches!(
        text.to_ascii_lowercase().as_str(),
        "true" | "false" | "null" | "~" | "yes" | "no" | "on" | "off"
    )
}

fn looks_like_number(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    let trimmed = text.trim_start_matches('-');
    if trimmed.is_empty() {
        return false;
    }

    let numeric = trimmed.chars().all(|ch| ch.is_ascii_digit());
    if numeric {
        return true;
    }

    let mut has_decimal = false;
    let mut has_exp = false;
    let mut has_digits = false;
    for (idx, ch) in trimmed.chars().enumerate() {
        match ch {
            '0'..='9' => has_digits = true,
            '.' if !has_decimal && !has_exp => has_decimal = true,
            'e' | 'E' if !has_exp && has_digits => {
                has_exp = true;
                has_digits = false;
            }
            '+' | '-' if has_exp && idx > 0 && trimmed.as_bytes()[idx - 1].eq(&b'e') => {}
            _ => return false,
        }
    }

    has_digits
}

fn describe_value(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Sequence(_) => "list",
        Value::Mapping(_) => "dict",
        Value::Tagged(_) => "tagged",
    }
}

fn spaces(count: usize) -> Cow<'static, str> {
    static CACHE: [&str; 9] = ["", " ", "  ", "   ", "    ", "     ", "      ", "       ", "        "];
    if count < CACHE.len() {
        Cow::Borrowed(CACHE[count])
    } else {
        Cow::Owned(" ".repeat(count))
    }
}

fn value_to_inline_string(value: &Value) -> Result<String, YamlFormatError> {
    match value {
        Value::String(text) => Ok(text.clone()),
        _ => serde_yaml::to_string(value)
            .map(|s| s.trim_end_matches('\n').to_owned())
            .map_err(|err| YamlFormatError::Format(err.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_string_detection() {
        assert!(is_plain_string("client_member_id"));
        assert!(!is_plain_string("Short description"));
        assert!(!is_plain_string(" trailing"));
        assert!(!is_plain_string("true"));
        assert!(!is_plain_string("42"));
        assert!(!is_plain_string("3.14"));
        assert!(is_plain_string("Bronze.Raw"));
    }
}
