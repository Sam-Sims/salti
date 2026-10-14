# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- An `auto` theme, now the default. At startup it checks whether the terminal is light or dark, and picks `everforest-light` or `everforest-dark` to match.
- Two new themes: `everforest-light` and `solarized-dark`.
- A `--theme` flag and `SALTI_THEME` variable to always start with a given theme, which skips the light/dark check.
- Live theme previews in `set-theme` as you type or tab through the names. `Enter` applies the theme.
- A keybinds popup, opened with `?` or `keybinds`, with one tab per category. `Left`/`Right` switch tabs.
- Reverse complement translation, with frames `-1`, `-2` and `-3` in `set-translation-frame`, bound to `Alt+4`, `Alt+5` and `Alt+6`.
- A `d` keybind that cycles the diff mode through off, reference and consensus, skipping reference when none is set. The new mode is shown as a notification.
- Typing a bare number in the command palette (`:42`) jumps to that position.
- A `--skip-update-check` flag, which does the same as the `SALTI_SKIP_UPDATE_CHECK` variable.
  - To always skip the check, use an alias: `alias salti="salti --skip-update-check"`.

### Changed

- **Breaking:** `toggle-translate` is now `toggle-translation-overlay`. The `t` keybind is unchanged.
- **Breaking:** `reload-as-protein` is now `toggle-protein-view`. The `Shift+T` keybind is unchanged.
- **Breaking:** `clear-filter` is now `clear-all-filters`, to make clear that it removes every filter.
- **Breaking:** `toggle-protein-view` no longer takes a reading frame. Set the frame with `set-translation-frame`.
- **Breaking:** The `terminal-default` theme is now `ansi`, and has been tweaked to stay readable on light and dark terminals.
- **Breaking:** A `filter-rows` regex containing spaces must be quoted (`filter-rows 'a b'`)
- **Breaking:** Alignments containing non-ASCII characters fail to load with an error.
- **Breaking:** Invalid GFF files fail to load with an error.
- **Breaking:** An open popup (the command palette, the minimap or the keybinds popup) takes all keyboard and mouse input.
  - `Esc` closes any popup, including the minimap. `m` opens the minimap but no longer closes it.
  - Other global keys do nothing while a popup is open. For example, with the minimap open, `q` no longer quits and `:` no longer opens the palette.
  - Clicks outside a popup are ignored.
  - In the minimap, arrow keys still scroll the alignment, but `Alt+Left`/`Alt+Right` no longer scroll the sequence names.
- The protein view (`Shift+T`) translates as you scroll, like the translation overlay, instead of storing a translated copy of the alignment.
- Forward frames are shown as `+1`, `+2` and `+3`. `set-translation-frame 1` still works.
- Leaving the protein view returns to the first nucleotide of the codon at the left edge of the screen.
- The command palette has been reworked to make it feel more fluid:
  - The cursor moves with `Left`/`Right`, `Home`/`Ctrl+A` and `End`/`Ctrl+E`. `Ctrl+U` deletes before the cursor, `Ctrl+K` after it, and `Ctrl+W` the word before it.
  - Completed names that contain spaces are quoted.
  - `Tab` on the only matching directory lists its contents, as does `Enter` on a selected directory.
  - A command given too many arguments shows an error, instead of ignoring the extra ones.
- The mouse acts on the pane under the pointer. Mouse actions on the alignment rows only work inside that pane.
- Filters that hide everything show `No columns to display` or `No sequences to display`, instead of a blank pane.
- `set-sequence-type` can't be used in the protein view, and can't set a non-DNA type while the translation overlay is on.
- Translating an alignment that isn't DNA shows an error suggesting `set-sequence-type`.
- Clearer error messages when the translation overlay, the protein view and column filters conflict.
- A failed alignment load is shown as an error notification naming the file. Any alignment already open stays open.
- FASTA files are validated off the UI thread.
- Notifications clear themselves instead of staying until the next key press. Errors show for 3s and other notifications for 1s, then fade out.
- Cysteine (C) has its own protein colour, following Clustal's groups.
- The minimap colours each block by the consensus of the columns it covers, using the current consensus method.
- The GFF pane's scrollbar matches the alignment's, and scrollbars no longer draw a background.
- Column stats (consensus, conservation, etc.) are only calculated for the part of the alignment on screen.
- Consensus, conservation and column filtering are faster, especially on alignments with many sequences.
- salti only redraws when something changes, up to about 60 times a second, instead of always 120 times a second.
- `SALTI_SKIP_UPDATE_CHECK` skips the check for any value except `false`, `no`, `off`, `0` or empty. Previously only `true` worked.

### Removed

- **Breaking:** `-p`/`--position`. Use `jump-position` once the alignment has loaded.

### Fixed

- `U` in DNA alignments is coloured like `T`. Previously it had no colour.
- Pinned sequences count towards the consensus, conservation and column filters.
- Pinning a sequence no longer lowers the sequence count in the status bar.
- Keys no longer sometimes register twice on Windows.
- `:` opens the command palette properly on Windows.
- `filter-rows` with no argument only clears the row filter. Previously it also removed `filter-gaps` and `filter-constant` filters.
- Dragging over sequences selects the rows in the order on screen. Previously it selected rows in file order, which could include pinned sequences or ones hidden by `filter-rows`.
- The translation overlay (`t`) shows a partial codon at the end of a sequence as `X`, as the protein view does. Previously it was blank.
- `set-diff-mode reference` shows an error when no reference is set, and clearing the reference turns the reference diff off.
- Changing filters, the sequence type or the reading frame clears the selection. Previously it was kept, even when it covered hidden rows or columns.
- `jump-position` shows an error when there is no visible column at or after the position.
- `pin-sequence` and `jump-sequence` no longer suggest pinned sequences when autocompleting.
- The minimap no longer opens when no alignment is loaded.
- Quitting stops a running alignment load straight away instead of waiting for it to finish.
- While an error is shown, other notifications (such as the update notice) no longer replace it. Only a new error does.
- Notifications stay visible while the minimap is open.
- Protein gaps and unknown residues (`X`) use the DNA gap and `N` colours, so gaps and `X` look different.
- Consensus ties are no longer broken at random. The residue with the lowest ASCII value wins.
- salti exits if its terminal input closes, instead of hanging.

## [0.9.0] - 2026-05-13

### Added

- `filter-constant` command to hide highly conserved columns by threshold - ignores gaps and unknown symbols in DNA and protein alignments.
- `reload-as-protein` command and `Shift+T` shortcut to toggle a full protein-translated alignment view alongside the existing quick translation overlay (`t`).
- Keyboard shortcuts for translation frame selection: `Alt+1`, `Alt+2`, and `Alt+3`.
- Experimental GFF3 annotation support via `load-gff`, including:
  - a global feature minimap
  - a local feature track above the alignment
  - hoverable feature details in the `Feature Info` pane
  - mouse drag panning in the GFF pane
  - `jump-feature` command to jump to named features from a loaded GFF file
  - GFF annotations follow the current coordinate mode:
    - in quick translation they remain in nucleotide space
    - in reloaded protein mode they are projected into protein columns using the active reading frame
    - when columns are filtered, features collapse to the remaining visible columns and may disappear entirely if fully filtered out

### Changed

- Translation handling is now split more cleanly between quick overlay translation and full protein reload, with reading frame changes updating both modes correctly.
- Sequence type detection is now deterministic.
- Significant under-the-hood performance improvements in `libmsa`, including faster translation.
- Refactored the UI into panes and layers to simplify, and migrated them so each component implements the ratatui widget trait.
- Some UI changes to make the interface a bit sleeker - dropping obvious titles in panes etc.

### Fixed

- Mouse selection highlighting in the `terminal-default` theme using background colour making selection unusable.
- Ruler rendering bugs that could happen with filtered columns and translated views.

## [0.8.0] - 2026-02-26

### Added

- Installer bash script for easy installation on Linux.
- `filter-gaps` command to remove columns that contain a % of gaps. Translation is disabled if a gap filter is active
  and vice-versa
- `libmsa` lib crate to handle alignment parsing and manipulation
- mouse selection now selects the full AA codon in the translated view
- Ruler markers to indicate filtered columns. An arrow on the ruler will point to a location where 1 or core columns
  have been removed due to filters. `~` characters will mark a region where lots of columns have been filtered and would
  be too crowded to render lots of arrows. You might expect big jumps in absolute positions in these regions.
- Total length is now displayed on the top status bar
- Improved filter text in the bottom status bar, which shows the number of filtered rows/columns and the filter
  parameters (e.g. gap percentage or regex pattern)
- Human panic messages

### Changed

- Moved core alignment functions to `libmsa`
- Each status bar has separation of concerns for a more consistent UI. The top bar is for "global" concerns, i.e number
  of alignments, visible region etc
    - The bottom bar is for "local" concerns, i.e the current selected position and sequence
- Under the hood changes to how the caching worked - now cache maintains blocks of 5000nts instead of moving window +
  region. This simplifies the handling and makes it easier to cache new metrics in the future.
- Renamed `set-filter` to `filter-rows` to be explicit what it does
- Renamed `full` alignment type to `generic`
- Renamed `aa` type to `protein`
- `jump-position` will jump to next visible position if the provided position is filtered out
- You can now middle mouse pan while minimap is open
- Most app errors now raise notifications on the bottom bar

### Removed

- Removed musl builds due to the complexity of supporting OpenSSL

### Fixed

- Further omptimised dist build profile
- Removed the need for vendored OpenSSL
    - Together with the previous optimisations, this reduces the dist binary size from 13mb to 6.9mb
- Conservation in translated view now shows conservation of the amino acid in that position

## [0.7.1] - 2026-02-21

### Fixed

- Optimised release build settings according to ratatui recommendations. This reduces the binary size from 110mb to
  10mb (whoops)

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
- Conservation bar chart rendering in the consensus pane. The calculation algorithm is the same as Jbrowse MSA - that
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
- Regex-based sequence filtering (`filter-rows`, `clear-filter`).
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
