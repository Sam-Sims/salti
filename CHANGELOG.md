# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.0] - 2026-02-21

### Added

- Self update check on startup, with a notification in the status bar if an update is available. This checks the latest
  release on crates.io and compares it to the current version. It does not automatically download or install updates.
- `check-update` command to manually trigger an update check.
- Under the hood a more general notification system, which can be used for other types of notifications in the future.
  Currently it is only used for update notifications and command palette errors.

### Changed

- Command errors use the new notification system for display errors.

### Fixed

- `cargo clippy` errors.

## [0.6.0] - 2026-02-21

### Added

- Minimap for easy panning along the length of the alignment. Press `m` to open and click and drag to move around. The
  minimap colours are created by sampling the alignment in regions and picking the most common colour.
- `Full` sequence type. This adds support for the full, renderable ASCII alphabet - which enables support for arbitrary
  alignments. Salti will still try and infer the file type, but now samples both NT characters and AA characters. The
  most prevalent "wins". If none meet the threshold it falls back to the `Full` mode. `Full` is also manually
  toggleable.
    - This introduced a new colour palette for full, which is hard-set across every theme. In full mode only the UI
      elements will change with the theme.
    - Note in `Full` mode conservation + translation is disabled.

### Fixed

- `toggle-translate` now returns a visible error message when sequence type is not `DNA`

## [0.5.1] - 2026-02-17

### Fixed

- musl builds failing due to openssl dependency, fixed by using `native-tls` crate

## [0.5.0] - 2026-02-17

### Added

- `set-diff-mode` command that replaces the previous 2 commands `toggle-reference-diff` and `toggle-consensus-diff`. It
  supports the same 3 modes as before: `off`, `reference`, and `consensus`.
- `tokyo-night` theme, available via `set-theme tokyo-night`.
- `solarized-light` theme, available via `set-theme solarized-light`.
- `terminal-default` theme, which uses terminal-provided ANSI colours and defaults, available via
  `set-theme terminal-default`.
- Support loading alignments over HTTP/HTTPS, by providing a URL to the `load` command, e.g.
  `:load https://example.com/alignment.fasta`.
- Support for compressed input files - just provide the gzipped file as input, e.g. `alignment.fasta.gz`.
    - Supported compression formats are `gz`, `zstd`, `lzma`, `bz2` and `bgz`
- Support for loading alignments via SSH e.g `:load ssh://user@host/path/to/alignment.fasta`

### Changed

- Switch from `needletail` to `paraseq` for fasta parsing. This enables a lot of great features such as compressed file
  support (thanks to niffler too!), and native HTTP/HTTPS and SSH loading.
    - Although it supports parallel processing - salti uses the single threaded API at the moment, for the above
      features.

### Removed

- `toggle-reference-diff` - see Added above
- `toggle-consensus-diff` - see Added above

## [0.4.0] - 2026-02-16

### Added

- Middle-mouse drag panning in the alignment pane.
- Conservation bar chart rendering in the consensus pane. The calculation algorithim is the same as Jbrowse MSA - that
  is to say that it is calculated as shannon entropy of the column, normalised to [0,1] by max entropy for the column's
  symbol count (e.g. 4 for DNA, 20 for AA).

### Changed

- Reworked async handling into a more unified system for both consensus and conservation.
- File loading is now an App command, not Core.
- Rendering now redraws on state changes only, rather than every frame tick.
- Refined the UI, removed the outer frame border, and converted status lines into dedicated top/bottom bars. This
  removed the double borders around the app, and hopefully gives a cleaner feel.

### Fixed

- Mouse selection now functions correctly in the pinned rows area (previously pinned sequences were not selectable).

## [0.3.0] - 2026-02-12

### Added

- Command palette overlay (`:`) that centralises all features and replaces previous popups
- Regex-based sequence filtering (`set-filter`, `clear-filter`).
- Sequence pinning controls (`pin-sequence`, `unpin-sequence`)
- Reference sequence controls (`set-reference`, `clear-reference`) and reference-diff rendering mode (
  `toggle-reference-diff`).
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
