# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-01-28

### Added
- Command palette overlay (`:`) that centralises all features and replaces previous popups
- Regex-based sequence filtering (`set-filter`, `clear-filter`).
- Sequence pinning controls (`pin-sequence`, `unpin-sequence`)
- Reference sequence controls (`set-reference`, `clear-reference`) and reference-diff rendering mode (`toggle-reference-diff`).
- Consensus-diff rendering mode (`toggle-consensus-diff`).
- Runtime alignment loading from within the app (`load-alignment` / `load`).
- Startup without an input file (app can launch idle and load later via command palette, see above).
- Mouse selection support, including Ctrl+drag box selection.
- Translation frame control (`set-translation-frame`) and sequence type override (`set-sequence-type`).
- Consensus method selection (`set-consensus-method` with `majority` and `majority-non-gap`).
- Theme system with `set-theme` command (currently `everforest-dark`).
- Optional debug logging via `--debug`, with logs written to `salti.log`

### Changed
- Migrated to `ratatui` `0.30.0`.
- Refactored architecture into modules: `core` (logic/state), `ui` (rendering), and `overlay` (overlays).

### Removed
- stdin alignment input support (`-` as file input).
- help and jump widgets in favour of command palette

## [0.2.0] - 2025-06-15

### Added
- stdin support for reading files
- Amino Acid colour schemes based on CLUSTAL colours
- Auto-detection of DNA/AA input file

## [0.1.0] - 2025-06-10

### Added
- Initial release
