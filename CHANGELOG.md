# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-02-16

### Added
- Middle-mouse drag panning in the alignment pane.
- Conservation bar chart rendering in the consensus pane. The calculation algorithim is the same as Jbrowse MSA - that is to say that it is calculated as shannon entropy of the column, normalised to [0,1] by max entropy for the column's symbol count (e.g. 4 for DNA, 20 for AA). 

### Changed
- Reworked async handling into a more unified system for both consensus and conservation.
- File loading is now an App command, not Core.
- Rendering now redraws on state changes only, rather than every frame tick.
- Refined the UI, removed the outer frame border, and converted status lines into dedicated top/bottom bars. This removed the double borders around the app, and hopefully gives a cleaner feel.

### Fixed
- Mouse selection now functions correctly in the pinned rows area (previously pinned sequences were not selectable).

## [0.3.0] - 2026-02-12

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
