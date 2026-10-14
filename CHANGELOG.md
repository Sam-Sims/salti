# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- A `--skip-update-check` flag, which does the same as the `SALTI_SKIP_UPDATE_CHECK` variable.
  - To always skip the check without setting the variable, use an alias: `alias salti="salti --skip-update-check"`.
- A keybinds help popup, opened with `?` or the `keybinds` command. It lists every key binding, with one tab per category.
  - `Left`/`Right` switch tabs.
- A `d` keybind that cycles the diff mode through off, reference and consensus. Reference is skipped when no reference is set.
  - The new mode is shown as a notification in the bottom right.
- An `auto` theme, which is now the default.
  - At startup it checks whether the terminal uses a light or dark theme, and picks `everforest-light` or `everforest-dark` to match.
- A `--theme` flag and `SALTI_THEME` variable to always start salti with a given theme.
  - This skips the startup check of the terminal light/dark mode.
- Two new themes: `everforest-light` and `solarized-dark`.
- `set-theme` previews each theme as you type or tab through the names. It is not applied until `Enter` pressed.
- Typing a bare number in the command palette (`:42`) jumps to that position.
- Reverse complement translation, with frames `-1`, `-2` and `-3` in `set-translation-frame` and bound to `Alt+4`, `Alt+5` and `Alt+6`.

### Changed

#### Commands

- `toggle-translate` has been renamed to`toggle-translation-overlay`.
  - The `t` keybind is unchanged.
- `reload-as-protein` has been renamed to `toggle-protein-view`.
  - The `Shift+T` keybind is unchanged.
- `toggle-protein-view` no longer takes a reading frame. Set the frame separately with `set-translation-frame`.
- `clear-filter` is now `clear-all-filters`, to make clear that it removes every filter.

#### Filters

- Filters that hide everything show an error in alignment pane.
  - Previously, a column filter that hid every column, or a `filter-rows` regex that matched no sequences left the alignment pane blank.
  - Now the pane shows `No columns to display` or `No sequences to display`.

#### Translation and protein view

- Forward frames are displayed as `+1`, `+2` and `+3` however `set-translation-frame 1` still works.
- The protein view (`Shift+T`) no longer keeps a second, translated copy of the alignment in memory.
  - Previously, turning it on translated every sequence and stored both copies. Changing the reading frame translated everything again. In practice this was still very fast, but used extra memory, especially on large alignments.
  - Now it only translates the columns on screen as you scroll, the same way the translation overlay (`t`) does.
  - On large alignments, switching to the protein view and changing the reading frame are now instant (instead of near-instant), and the protein view uses no extra memory.
- Leaving the protein view returns to the first nucleotide of the codon at the left edge of the screen.
- `set-sequence-type` can't be used in the protein view, and can't set a non DNA type while the translation overlay is on.
- Translating an alignment that isn't DNA shows an error suggesting `set-sequence-type`.
- Clearer error messages when the translation overlay, the protein view and column filters conflict.

#### Selection and mouse

- The mouse now acts on the pane under the pointer, and an open popup takes all mouse input.
  - As a consequence mouse interaction intended for inside the alignment rows, only works within the alignment rows pane.

#### Command palette and popups

- The command palette has been reworked to feel more fluid:
  - The cursor can now be moved with shell editing keys:
    - `Left`/`Right` move the cursor one character.
    - `Home`/`Ctrl+A` move to the start of the line, and `End`/`Ctrl+E` to the end.
    - `Ctrl+U` deletes everything before the cursor, and `Ctrl+K` everything after it.
    - `Ctrl+W` deletes the word before the cursor.
  - Completed names that contain spaces are quoted automatically.
  - Completing a file path with directories is more fluid with `Tab` on the only matching directory listing its contents straight away.
    - Alternatively use `Enter` on a selected directory to list contents.
  - Commands given too many arguments now show an error, instead of using the first one and ignoring the rest.
  - `set-theme` previews themes live (see Added).
- A `filter-rows` regex containing spaces must now be quoted (`filter-rows 'a b'`), because the line is now split into arguments the same way for every command.
- Popups (the command palette, the minimap and the new keybinds help) now share one system, which changes how they handle input:
  - An open popup takes all keyboard and mouse input.
  - `Esc` closes any popup, including the minimap.
  - Other global keys do nothing while a popup is open. For example, with the minimap open, `q` no longer quits and `:` no longer opens the palette.
  - Clicks outside a popup are ignored.
  - `m` opens the minimap but no longer closes it.
  - In the minimap, arrow keys (with or without `Shift`) still scroll the alignment, but `Alt+Left`/`Alt+Right` no longer scroll the sequence names.

#### Loading and status bar

- Fasta files are now validated off of the UI thread.
- Invalid GFF files will fail to load with an error.
- A failed alignment load is shown as an error notification naming the file, and any alignment already open stays open.

#### Notifications

- Notifications now clear themselves after a few seconds, instead of staying until the next key press.
  - Errors show for 3s and fade out, other notifications for 1s, then fade out.

#### Themes and colours

- `terminal-default` is now `ansi`, and has been tweaked to stay readable on both light and dark terminals.
- Protein colours now fully follow Clustal's groups: cysteine (C) has its own colour instead of the hydrophobic one.
- The GFF pane's scrollbar now looks the same as the alignment's.
- Scrollbars no longer draw a background.
- The minimap now colours each block by the consensus of the columns it covers, using the current consensus method.
  - Previously it used the most common character in a sample of 10 sequences, with gaps always counted.
  - With the default `majority-non-gap` method, gappy regions now show the most common residue instead of the gap colour. Use `set-consensus-method majority` to see gaps.

#### General

- Internal changes to how `libmsa` handles residues.
- Column stats (consensus, conservation, etc.) are now only calculated for the part of the alignment on screen.
- Consensus, conservation and column filtering are faster, especially on alignments with many sequences.
- Alignments containing non-ASCII characters now fail to load with an error.
- salti now only redraws when something changes, up to about 60 times a second, instead of always redrawing 120 times a second.
- `SALTI_SKIP_UPDATE_CHECK` now skips the check for any value except `false`, `no`, `off`, `0` or empty. Previously only `true` worked.

### Removed

- `-p`/`--position`. Use `jump-position` once the alignment has loaded.

### Fixed

- `U` in DNA alignments is now coloured the same as `T`. Previously it had no colour.
- Pinned sequences now count towards the consensus, conservation and column filters.
- Pinning a sequence no longer lowers the sequence count in the status bar.
- Keys no longer sometimes register twice on Windows.
- `:` now opens the command palette properly on Windows.
- `filter-rows` with no argument now only clears the row filter.
  - Previously it did the same as `clear-filter`, so it also removed any `filter-gaps` or `filter-constant` filters.
- Dragging over sequences now selects exactly the rows you see.
  - Previously a drag selected rows in file order, so it could include pinned sequences or ones hidden by `filter-rows`.
  - Now it follows the order on screen, including when you drag from the pinned sequences into the rest.
- The translation overlay (`t`) now shows a partial codon at the end of a sequence as `X`.
  - Previously the overlay left the last one or two nucleotides blank, while the protein view showed an `X`.
- `set-diff-mode reference` now shows an error when no reference is set, and clearing the reference turns the reference diff off.
- `pin-sequence` and `jump-sequence` no longer suggest pinned sequences when autocompleting.
- `jump-position` now shows an error when there is no visible column at or after the position.
- Changing filters, the sequence type or the reading frame now clears the selection.
  - Previously the selection was kept, even when a filter hid the rows or columns it covered.
- The minimap no longer opens when no alignment is loaded.
- Quitting stops a running alignment load straight away instead of waiting for it to finish.
- While an error is shown, other notifications (such as the update notice) no longer replace it. Only a new error does.
- Notifications stay visible while the minimap is open.
- Protein gaps and unknown residues (`X`) now use the same colours as DNA gaps and `N`, so gaps and `X` look different from each other.
- Consensus ties are no longer broken at random. The residue with the lowest ASCII value now wins.
- salti now exits if its terminal input closes, instead of hanging.

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
