# YAML Formatter Specification

The reference implementation lives in `../pulseflow/refactor/yaml-formatter/packages/tablespec/src/tablespec/formatting/yaml_formatter.py` with tests in `packages/tablespec/tests/unit/test_yaml_formatter.py`. The Rust formatter must reproduce the same observable behaviour. The essential requirements distilled from that suite are:

- **Primary functions**
  - `format_yaml_string(str) -> str`: parse YAML text, format, and return a string. Leading document markers (`---` or `---\n`) are stripped prior to parsing; empty/`None` documents are returned unchanged.
  - `format_yaml_dict(dict) -> str`: format an in-memory mapping into YAML. Input must be a mapping; otherwise raise `YAMLFormatError`.
  - `format_yaml_file(Path, check_only=False) -> bool`: format a file in-place (write only when the formatted text differs) or, in check mode, report whether changes would occur.
  - `format_yaml_files(List[Path], check_only=False) -> (changed_count, error_count, List[str])`: batch helper that aggregates the previous behaviour over many files.
  - All functions raise `YAMLFormatError` on parsing/formatting problems (including unsupported root structures).
- **CLI**
  - `yars-format [OPTIONS] <FILE>...` provides a standalone formatter that mirrors the library behaviour.
    - `--check` exits with status `1` when any file would change, `0` when everything is already formatted, and `2` on errors.
    - `-v/--verbose` lists each file with a per-file status and line delta.
    - `--generate-completions <shell>` writes completion scripts for `bash`, `zsh`, `fish`, `powershell`, or `elvish` to stdout and performs no formatting.

- **Parsing & validation**
  - Only standard YAML mappings at the document root are accepted. Any top-level list must raise `YAMLFormatError` with a message mentioning the limitation; nested lists remain valid.
  - YAML that parses to `None` is treated as blank and returned verbatim.
  - Input is considered UTF‑8 text; formatter must preserve Unicode and escape sequences exactly.

- **Structure normalisation**
  - Mappings are sorted recursively by key using their string representation; sequence order is preserved exactly (no sorting of lists or list contents).
  - All nested mappings follow the same sorting rule (depth-first).
  - Scalar values flow through unchanged except where string formatting rules apply.

- **String formatting rules**
  - Strings containing literal newline characters (`\n`) become literal block scalars (`|-`) so that human-entered line breaks survive. The body of the block is emitted verbatim (no extra wrapping).
  - Strings lacking newlines stay on a single line (quoted or plain as needed) even when they are long; there is no automatic soft-wrapping.
  - Strings with leading or trailing whitespace, or any control characters that YAML cannot represent in literal blocks (C0 control chars except tab/newline/carriage return, DEL `0x7F`, C1 control block `0x80–0x9F`), must remain quoted scalars to preserve semantics.
  - Escape sequences (e.g. `\x1f`, `\u2026`, backslash continuation lines) must round-trip without modification.

- **Emission settings**
  - Mapping indentation: 2 spaces; sequence indentation: 4 spaces with an offset of 2 (`indent(mapping=2, sequence=4, offset=2)`).
  - `format_yaml_string` uses an effectively unlimited line width (4096) to avoid reflowing quoted scalars. `format_yaml_dict` targets 72-character width for readability.
  - Quotes are not preserved from the original input (`preserve_quotes = False`); output is canonical for the formatter.
  - Document start/end markers are not emitted.

- **Idempotence and validity**
  - Re-running the formatter on its own output must return identical text for supported inputs.
  - Output must parse successfully with a standard YAML loader and round-trip to the same Python/Rust data structures for all supported scenarios.
  - Known limitation: documents containing the Unicode NEL character (`\x85`) inside scalars are documented as a non-idempotent edge case in the legacy implementation; we should match the current behaviour (no crashes) but need not fix the upstream yamlfix issue.

- **Error cases**
  - Invalid YAML input raises `YAMLFormatError`.
  - Missing files in file-formatting helpers raise `YAMLFormatError`.
  - Batch formatter aggregates errors without aborting the remaining files.

The Rust implementation should expose equivalent entry points and be accompanied by automated tests that mirror the Python suite’s coverage wherever practical, ensuring byte-for-byte parity and semantics preservation.
