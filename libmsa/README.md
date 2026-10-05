# libmsa

`libmsa` is the alignment library that powers `salti`.

The lib works on alignments that are already in memory. Generally you pass in a `Vec<Sequence>` and it builds a validated
`Alignment` which:

- detects whether the data is DNA, protein, or generic
- reads cells through a `Grid`, either as stored or translated in any of the three forward reading frames

and can:

- calculate consensus and conservation over any set of rows and columns
- finds the columns that pass a gap or constant-column filter

Although this crate is not designed to be a general-purpose MSA library, it is intended to be flexible enough to support
a variety of MSA operations.

There are some current limits (some obvious, maybe some not so much):

- All sequences must already be aligned to the same length.
- Translation only makes sense for DNA alignments.
- Conservation is only defined for DNA and protein alignments.
- Consensus ties go to the lowest ASCII byte.
