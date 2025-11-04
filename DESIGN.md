# Rust YAML Formatter Design

This document captures the implementation strategy for the Rust formatter that must
mirror the behaviour of the existing Python implementation (`tablespec.formatting.yaml_formatter`).

## High-level pipeline

1. **Parsing** – Use `serde_yaml` to parse UTF-8 text into `serde_yaml::Value`. Before parsing we skip a leading document marker (`---` / `---\n`) to match the reference semantics. If parsing yields `Value::Null` we return the original text untouched.
2. **Validation** – Reject documents whose root is a sequence with the reference error message (“Top-level lists are not supported…”). All other parse errors are wrapped in `YAMLFormatError`.
3. **Normalization** – Recursively sort every mapping by the string representation of the key (stable ordering) while leaving sequence order intact. This produces a deterministic intermediate tree.
4. **String classification** – Walk the tree and classify scalar strings into:
   - `LiteralBlock` → contains `\n`, has no leading/trailing whitespace, and contains no disallowed control characters (C0 except `\t`, `\n`, `\r`, plus `0x7F` and `0x80–0x9F`).
   - `InlinePlain` → newline-free tokens matching `[A-Za-z0-9._/-]+` and not reserved YAML keywords (`true`, `false`, `null`, `yes`, `no`, `on`, `off`, `~`) or numeric-looking literals.
   - `InlineQuoted` → everything else; rendered as JSON-style double-quoted strings via `serde_json::to_string` to guarantee escaping of control characters, Unicode, and backslash sequences.
5. **Emission** – Generate YAML text manually (not through `serde_yaml::Serializer`) so we can enforce:
   - Mapping indent = 2 spaces.
   - Sequence indent = 4 spaces with hyphen offset aligned to mapping indent.
   - Literal block scalars rendered as `|-` with the block body indented by 2 spaces from the parent key/sequence item.
   - Empty mappings rendered as `{}` and empty sequences as `[]`.
   - Unlimited line width for inline scalars (no soft wrapping).
6. **Outputs**
   - `format_yaml_string(&str) -> Result<String, YAMLFormatError>` – in-memory formatting from text.
   - `format_yaml_dict<T: Serialize>(&T) -> Result<String, YAMLFormatError>` – format a Rust structure (requires mapping at the root).
   - `format_yaml_file(Path, check_only)` and `format_yaml_files(&[Path], check_only)` – filesystem helpers that mirror the Python behaviour (write only when content changes; aggregate errors without aborting).

## Error handling

`YAMLFormatError` (implemented with `thiserror`) wraps:
- Top-level sequence rejection.
- File IO failures.
- Parse/emit failures (with contextual message `Error formatting YAML: …`).

## Testing strategy

Port the behavioural coverage from the Python suite into `tests/formatter.rs`, focusing on:
- Multiline string handling, indentation, and idempotence.
- Dictionary sorting, list order preservation, and nested structures.
- Escape sequence round-tripping and control-character safeguards.
- Error cases (invalid YAML, top-level lists, missing files).
- Known limitations (NEL `\x85` non-idempotence) documented via targeted tests.

Golden samples from the Python formatter will be captured in fixture files to assert byte-for-byte parity where feasible, while property-style tests will ensure the Rust formatter round trips correctly through `serde_yaml`.

