use std::{cmp::Reverse, fmt, fs::File, io::BufReader, ops::Range, path::Path};

use anyhow::{Result, format_err};
use noodles_gff as gff;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gff {
    pub features: Vec<Feature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Feature {
    pub name: String,
    pub kind: FeatureType,
    pub range: Range<usize>,
    pub strand: Strand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureType {
    Gene,
}

impl FeatureType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gene => "gene",
        }
    }

    fn parse(feature_type: &[u8]) -> Option<Self> {
        if feature_type.eq_ignore_ascii_case(b"gene") {
            return Some(Self::Gene);
        }

        None
    }
}

impl fmt::Display for FeatureType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Strand {
    Forward,
    Reverse,
    Unknown,
}

impl Strand {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Forward => "Forward →",
            Self::Reverse => "Reverse ←",
            Self::Unknown => "Unknown strand",
        }
    }
}

impl fmt::Display for Strand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<gff::feature::record::Strand> for Strand {
    fn from(strand: gff::feature::record::Strand) -> Self {
        match strand {
            gff::feature::record::Strand::Forward => Self::Forward,
            gff::feature::record::Strand::Reverse => Self::Reverse,
            _ => Self::Unknown,
        }
    }
}

pub fn parse_gff(path: &Path) -> Result<Gff> {
    let file = File::open(path).map_err(|e| format_err!("failed to open gff file: {e}"))?;
    let mut reader = gff::io::Reader::new(BufReader::new(file));

    let mut features: Vec<Feature> = reader
        .record_bufs()
        .map(|result| {
            let record = result.map_err(|e| format_err!("failed to parse gff record: {e}"))?;
            let Some(kind) = FeatureType::parse(record.ty().as_ref()) else {
                return Ok(None);
            };
            let start = usize::from(record.start()) - 1;
            let end = usize::from(record.end());
            let name = extract_name(&record);
            if end <= start {
                return Err(format_err!("GFF feature {name} ends before it starts"));
            }

            Ok(Some(Feature {
                name,
                kind,
                range: start..end,
                strand: record.strand().into(),
            }))
        })
        .filter_map(Result::transpose)
        .collect::<Result<_>>()?;

    if features.is_empty() {
        return Err(format_err!("no supported features found in gff file"));
    }

    // GFFS are not always sorted
    features.sort_by_key(|feature| (feature.range.start, Reverse(feature.range.end)));

    Ok(Gff { features })
}

fn extract_name(record: &gff::feature::RecordBuf) -> String {
    const POSSIBLE_NAMES: [&[u8]; 4] = [b"Name", b"ID", b"gene_name", b"product"];

    // try get names in order of preference, or falls back to record type so at at least something is shown
    POSSIBLE_NAMES
        .iter()
        .filter_map(|tag| record.attributes().get(tag))
        .filter_map(|value| value.as_string())
        .find(|name| !name.is_empty())
        .map_or_else(|| record.ty().to_string(), |name| name.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use rstest::rstest;

    use super::*;

    fn write_gff(lines: &[&str]) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "##gff-version 3").unwrap();
        for line in lines {
            writeln!(file, "{}", line.replace(' ', "\t")).unwrap();
        }
        file
    }

    fn gene(name: &str, range: Range<usize>, strand: Strand) -> Feature {
        Feature {
            name: name.to_string(),
            kind: FeatureType::Gene,
            range,
            strand,
        }
    }

    #[rstest]
    #[case::forward("chr1 . gene 1 100 . + . Name=a", gene("a", 0..100, Strand::Forward))]
    #[case::reverse_single_base("chr1 . gene 5 5 . - . Name=a", gene("a", 4..5, Strand::Reverse))]
    #[case::unknown_strand("chr1 . gene 1 3 . ? . Name=a", gene("a", 0..3, Strand::Unknown))]
    #[case::type_ignores_case("chr1 . GENE 1 3 . + . Name=a", gene("a", 0..3, Strand::Forward))]
    fn parse_gff_works(#[case] line: &str, #[case] expected: Feature) {
        let file = write_gff(&[line]);

        assert_eq!(parse_gff(file.path()).unwrap().features, [expected]);
    }

    #[test]
    fn parse_gff_skips_unsupported_features() {
        let file = write_gff(&[
            "chr1 . CDS 1 3 . + . Name=cds",
            "chr1 . gene 1 3 . + . Name=a",
        ]);

        assert_eq!(
            parse_gff(file.path()).unwrap().features,
            [gene("a", 0..3, Strand::Forward)]
        );
    }

    #[test]
    fn parse_gff_sorts_by_start_then_longest_first() {
        let file = write_gff(&[
            "chr1 . gene 5 9 . + . Name=late",
            "chr1 . gene 1 3 . + . Name=short",
            "chr1 . gene 1 9 . + . Name=long",
        ]);

        let names: Vec<String> = parse_gff(file.path())
            .unwrap()
            .features
            .into_iter()
            .map(|feature| feature.name)
            .collect();

        assert_eq!(names, ["long", "short", "late"]);
    }

    #[rstest]
    #[case::name_first("product=p;gene_name=g;ID=i;Name=n", "n")]
    #[case::then_id("product=p;gene_name=g;ID=i", "i")]
    #[case::then_gene_name("product=p;gene_name=g", "g")]
    #[case::then_product("product=p", "p")]
    #[case::skips_empty("Name=;ID=i", "i")]
    #[case::falls_back_to_type(".", "gene")]
    fn parse_gff_name_works(#[case] attributes: &str, #[case] expected: &str) {
        let file = write_gff(&[&format!("chr1 . gene 1 3 . + . {attributes}")]);

        assert_eq!(parse_gff(file.path()).unwrap().features[0].name, expected);
    }

    #[rstest]
    #[case::ends_before_start(&["chr1 . gene 5 4 . + . Name=a"])]
    #[case::no_supported_features(&["chr1 . CDS 1 3 . + . Name=a"])]
    #[case::no_features(&[])]
    #[case::invalid_record(&["chr1 . gene x 3 . + . Name=a"])]
    fn parse_gff_rejects(#[case] lines: &[&str]) {
        let file = write_gff(lines);

        assert!(parse_gff(file.path()).is_err());
    }

    #[test]
    fn parse_gff_rejects_missing_file() {
        assert!(parse_gff(Path::new("missing.gff")).is_err());
    }
}
