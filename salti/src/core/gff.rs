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
            let start = usize::from(record.start())
                .checked_sub(1)
                .ok_or_else(|| format_err!("gff feature start must be one-based"))?;
            let end = usize::from(record.end());

            Ok(Some(Feature {
                name: extract_name(&record),
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

    // try get names in order of preference, or falls back to record type so something is shown
    POSSIBLE_NAMES
        .iter()
        .filter_map(|tag| record.attributes().get(*tag))
        .filter_map(|value| value.as_string())
        .find(|name| !name.is_empty())
        .map_or_else(|| record.ty().to_string(), |name| name.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn write_gff(content: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn parse_gff_extracts_supported_features_only() {
        let gff = write_gff(
            "##gff-version 3\n\
            chr1\t.\tgene\t1\t100\t.\t+\t.\tID=gene1;Name=gene1\n\
            chr1\t.\tCDS\t20\t80\t.\t+\t.\tID=cds1;Name=CDS1\n",
        );
        let model = parse_gff(gff.path()).unwrap();

        assert_eq!(model.features.len(), 1);
        assert_eq!(model.features[0].name, "gene1");
        assert_eq!(model.features[0].kind, FeatureType::Gene);
        assert_eq!(model.features[0].range, 0..100);
        assert_eq!(model.features[0].strand, Strand::Forward);
    }

    #[test]
    fn parse_gff_sorts_features_by_start() {
        let gff = write_gff(
            "##gff-version 3\n\
            chr1\t.\tgene\t50\t150\t.\t+\t.\tID=g2;Name=gene2\n\
            chr1\t.\tgene\t1\t100\t.\t+\t.\tID=g1;Name=gene1\n",
        );
        let model = parse_gff(gff.path()).unwrap();

        assert_eq!(model.features[0].name, "gene1");
        assert_eq!(model.features[1].name, "gene2");
    }

    #[test]
    fn parse_gff_falls_back_to_id_when_no_name() {
        let gff = write_gff(
            "##gff-version 3\n\
            chr1\t.\tgene\t1\t10\t.\t+\t.\tID=id1\n",
        );
        let model = parse_gff(gff.path()).unwrap();

        assert_eq!(model.features[0].name, "id1");
    }

    #[test]
    fn parse_gff_no_supported_features_returns_error() {
        let gff = write_gff(
            "##gff-version 3\n\
            chr1\t.\tCDS\t1\t10\t.\t+\t.\tID=cds1\n",
        );
        let result = parse_gff(gff.path());

        assert_eq!(
            result.unwrap_err().to_string(),
            "no supported features found in gff file"
        );
    }
}
