# Agent Notes

## Project Snapshot
- Repository: `https://github.com/easel/yars`
- Language: Rust (workspace initialized with `cargo init`)
- License: Apache 2.0 (`LICENSE`, `NOTICE`, Cargo metadata)
- CLI Binary: `yars-format`
- Installer: `install.sh` (detects OS/arch, fetches GitHub Release assets, supports updates)
- CI: `.github/workflows/ci.yml` (build + clippy + tests on `master`)
- Release automation: `.github/workflows/release.yml` (multi-platform archives on tags `v*`)

## Current State (post-initial commit)
- Library functionality mirrors reference Python YAML formatter.
- Extensive property tests and integration tests (`tests/`).
- CLI supports `--check`, `--verbose`, `--generate-completions`.
- `README.md` documents usage, installation, license.

## Open Actions / Follow-ups
1. Push repository to GitHub:
   ```bash
   git remote add origin https://github.com/easel/yars.git
   git push -u origin master
   ```
2. Authenticate `gh` for further automation (`gh auth login`), if needed.
3. Monitor GitHub Actions runs after the first push:
   - `CI` workflow should pass on stable + nightly.
   - `Release` workflow triggers on tags (verify artifacts once a tag is pushed).

## Tips for Future Agents
- Use `cargo test` before committing—CI runs clippy/tests across all targets.
- Release archives expect a tag starting with `v`; the installer handles `--version vX.Y.Z`.
- Keep licenses updated when adding third-party code.
- Update this document when roles/ownership change.
