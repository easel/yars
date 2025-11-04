# yars-yaml-formatter

Rust port of the tablespec YAML formatter with an idempotent pipeline, CLI entry point, and property-based regression tests.

## Usage

Format one or more files in-place:

```bash
cargo run --bin yars_format -- path/to/file.yaml
```

Check whether formatting changes would be required (without writing):

```bash
cargo run --bin yars_format -- --check path/to/file.yaml
```

Generate shell completions:

```bash
cargo run --bin yars_format -- --generate-completions bash > yars-format.bash
```

## Installation

Fetch the latest prebuilt binary and install to `~/.local/bin`:

```bash
curl -sSfL https://raw.githubusercontent.com/easel/yars/main/install.sh | bash
```

Options:

- `--version vX.Y.Z` installs a specific release tag.
- `--install-dir /path/to/bin` chooses a custom destination.
- `--force` reinstalls even if the requested version is already present.

Re-running the script upgrades to the latest release automatically.

## Development

Run the full test suite (unit, fuzz/property, and CLI integration):

```bash
cargo test
```

## License

Licensed under the Apache License, Version 2.0. See [`LICENSE`](LICENSE) for details.
