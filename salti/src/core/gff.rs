use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use anyhow::{Result, format_err};
use noodles_gff as gff;

#[derive(Debug)]
pub struct Feature {
    pub name: String,
    pub feature_type: String,
    pub start: usize,
    pub end: usize,
    pub strand: char,
    pub lane: usize,
}

#[derive(Debug)]
pub struct Gff {
    pub features: Vec<Feature>,
    pub row_count: usize,
}

pub fn parse_gff(path: &Path) -> Result<Gff> {
    let file = File::open(path).map_err(|e| format_err!("failed to open gff file: {e}"))?;
    let mut reader = gff::io::Reader::new(BufReader::new(file));

    let mut features = Vec::new();

    for result in reader.record_bufs() {
        let record = result.map_err(|e| format_err!("failed to parse gff record: {e}"))?;

        if !is_supported_feature_type(record.ty().as_ref()) {
            continue;
        }

        let name = extract_name(&record);
        let feature_type = record.ty().to_string();
        let start = usize::from(record.start()).saturating_sub(1);
        let end = usize::from(record.end()).saturating_sub(1);
        let strand = strand_char(record.strand());

        features.push(Feature {
            name,
            feature_type,
            start,
            end,
            strand,
            lane: 0,
        });
    }

    if features.is_empty() {
        return Err(format_err!("no supported features found in gff file"));
    }

    let row_count = assign_row(&mut features);

    Ok(Gff {
        features,
        row_count,
    })
}

fn is_supported_feature_type(feature_type: &[u8]) -> bool {
    feature_type.eq_ignore_ascii_case(b"gene")
        || feature_type.eq_ignore_ascii_case(b"pseudogene")
        || feature_type.eq_ignore_ascii_case(b"five_prime_UTR")
        || feature_type.eq_ignore_ascii_case(b"three_prime_UTR")
        || feature_type.eq_ignore_ascii_case(b"long_terminal_repeat")
        || feature_type.eq_ignore_ascii_case(b"repeat_region")
}

fn extract_name(record: &gff::feature::RecordBuf) -> String {
    let attributes = record.attributes();
    for tag in &[b"Name" as &[u8], b"ID", b"gene_name", b"product"] {
        if let Some(value) = attributes.get(tag)
            && let Some(s) = value.as_string()
        {
            let text = s.to_string();
            if !text.is_empty() {
                return text;
            }
        }
    }
    record.ty().to_string()
}

fn strand_char(strand: gff::feature::record::Strand) -> char {
    match strand {
        gff::feature::record::Strand::Forward => '+',
        gff::feature::record::Strand::Reverse => '-',
        gff::feature::record::Strand::None => '.',
        gff::feature::record::Strand::Unknown => '?',
    }
}

fn assign_row(features: &mut [Feature]) -> usize {
    features.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));
    let mut lane_ends: Vec<usize> = Vec::new();

    for feature in features.iter_mut() {
        let mut assigned = false;
        for (lane_idx, lane_end) in lane_ends.iter_mut().enumerate() {
            if feature.start > *lane_end {
                feature.lane = lane_idx;
                *lane_end = feature.end;
                assigned = true;
                break;
            }
        }
        if !assigned {
            feature.lane = lane_ends.len();
            lane_ends.push(feature.end);
        }
    }

    lane_ends.len().max(1)
}





#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

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
        assert_eq!(model.features[0].feature_type, "gene");
        assert_eq!(model.features[0].start, 0);
        assert_eq!(model.features[0].end, 99);
        assert_eq!(model.features[0].strand, '+');
    }

    #[test]
    fn assign_lanes_handles_overlapping_features() {
        let gff = write_gff(
            "##gff-version 3\n\
            chr1\t.\tgene\t1\t100\t.\t+\t.\tID=g1;Name=gene1\n\
            chr1\t.\tgene\t50\t150\t.\t+\t.\tID=g2;Name=gene2\n\
            chr1\t.\tgene\t120\t200\t.\t+\t.\tID=g3;Name=gene3\n",
        );
        let model = parse_gff(gff.path()).unwrap();

        assert_eq!(model.row_count, 2);
        // Gene1 and Gene2 overlap, so they should be in different lanes.
        let gene1 = model.features.iter().find(|f| f.name == "gene1").unwrap();
        let gene2 = model.features.iter().find(|f| f.name == "gene2").unwrap();
        assert_ne!(gene1.lane, gene2.lane);
        // Gene3 starts after Gene1 ends, so it can share Gene1's lane.
        let gene3 = model.features.iter().find(|f| f.name == "gene3").unwrap();
        assert_eq!(gene3.lane, gene1.lane);
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

    #[test]
    fn parse_gff_accepts_requested_non_gene_feature_types() {
        let gff = write_gff(
            "##gff-version 3\n\
            chr1\t.\tpseudogene\t1\t10\t.\t+\t.\tID=ps1;Name=pseudogene1\n\
            chr1\t.\tfive_prime_UTR\t11\t20\t.\t+\t.\tID=utr5;Name=five_prime_UTR1\n\
            chr1\t.\tthree_prime_UTR\t21\t30\t.\t+\t.\tID=utr3;Name=three_prime_UTR1\n\
            chr1\t.\tlong_terminal_repeat\t31\t40\t.\t+\t.\tID=ltr;Name=long_terminal_repeat1\n\
            chr1\t.\trepeat_region\t41\t50\t.\t+\t.\tID=rep;Name=repeat_region1\n",
        );
        let model = parse_gff(gff.path()).unwrap();

        let feature_types: Vec<_> = model
            .features
            .iter()
            .map(|feature| feature.feature_type.as_str())
            .collect();
        assert_eq!(
            feature_types,
            vec![
                "pseudogene",
                "five_prime_UTR",
                "three_prime_UTR",
                "long_terminal_repeat",
                "repeat_region",
            ]
        );
    }
}
