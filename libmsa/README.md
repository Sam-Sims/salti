# libmsa

`libmsa` is the alignment library that powers `salti`. At the moment it is an internal crate.

The lib works on alignments that are already in memory. You pass in a set of `RawSequence` values and it builds a
validated `Alignment`. That alignment can:

- expose rows and columns while keeping track of visible and absolute positions
- detect whether data might be DNA, protein, or generic, with overrides
- filter rows by index or name pattern, and hide columns by gap fraction
- calculate consensus, conservation, and gap fraction for visible columns
- translate DNA alignments in any of the three forward reading frames

Although this crate is not designed to be a general-purpose MSA library, it is intended to be flexible enough to support
a variety of MSA operations.

There are some current limits (some obvious, maybe some not so much):

- All sequences must already be aligned to the same length.
- Translation only works for DNA alignments, and only when the column view has not been filtered.
- Conservation is only defined for DNA and protein alignments.
- Consensus ties are resolved at random, which means tied columns are not deterministic.

