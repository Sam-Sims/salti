# salti

A modern, multiple sequence alignment (MSA) viewer - built for the terminal.

## Overview

salti is a standalone, self-contained terminal user interface (TUI) application designed for the browsing and inspection of multiple sequence alignments in FASTA format. The application operates entirely within the terminal environment, making it advantageous for HPC environments and/or remote servers where graphical user interfaces are unavailable or impractical.

Aiming to be feature rich, fast and optimised for large alignments - salti is under active development.

## Current Features

- **High performance**: Able to load 10,500 mpox alignments in a few seconds
- **Consensus calculation**: Consensus is calcualted in the background for the visable alignment range
- **Navigation**: Quick navigation to specific alignment positions or sequence names
- **Fuzzy sequence name search**: Find sequences by name using fuzzy matching


## Planned Features
- **Amino Acid support** - Auto-detect and colour amino acids
- **Amino Acid translation** - Background translation of AA codons
- **Support other file formats** - Support other alignment file formats e.g phylip
- **Extended colour schemes** - e.g by identity
- **Screenshots** - Ability to export screenshots of current region
- **stdin** - Read from stdin
- **Filtering** - Filter records
- **Options** - Options widget to configure options in app

## Demo

## Installation

### Building from Source

salti currently requires building from source:

#### Prerequisites

Install the Rust toolchain following the official documentation: [Rust Installation Guide](https://www.rust-lang.org/tools/install)

#### Repository Setup

Clone the repository from GitHub:
```bash
git clone https://github.com/Sam-Sims/salti
cd salti
```

#### Compilation and Installation

Build:
```bash
cargo build --release
```

The compiled executable will be located at `salti/target/release/salti`.

## Usage

### Basic Operation

Launch salti with a FASTA alignment file:
```bash
salti alignment.fasta
```

Optionally specify an initial position (1-based indexing):
```bash
salti alignment.fasta --position 150
```

### Keyboard Controls

#### Application Controls
- `q` - Exit application
- `?` - Toggle help widget
- `j` - Toggle jump widget

#### Navigation Controls
- `↑`/`↓` - Scroll vertically through sequences (one line at a time)
- `←`/`→` - Scroll horizontally through alignment positions (one column at a time)
- Hold `Shift` to scroll 10 at a time
- `Home` - Jump to alignment start position
- `End` - Jump to alignment end position
- `c` - Cycle colour modes

### Consensus calculation
The consensus shows the most frequent nucleotide at each position, excluding gaps (`-`). Positions with no valid nucleotides display `*`.