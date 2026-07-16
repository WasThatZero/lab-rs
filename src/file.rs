//! a parsed .lab: an ordered sequence of [`Label`]s

use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::str::FromStr;

use crate::error::{ParseError, ReadError, ScaleError};
use crate::label::{secs_to_units, units_to_secs, Label};

/// a parsed `.lab` file, derefs to `Vec<Label>`
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct LabFile {
    /// the labels in file order
    pub labels: Vec<Label>,
}

impl LabFile {
    /// creates an empty label file
    pub fn new() -> Self {
        Self::default()
    }

    /// reads and parses a .lab from a reader
    pub fn from_reader<R: Read>(reader: R) -> Result<Self, ReadError> {
        let mut labels = Vec::new();
        for (i, line) in BufReader::new(reader).lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            labels.push(Label::parse_line(&line, i + 1)?);
        }
        Ok(LabFile { labels })
    }

    /// reads and parses the .lab at `path`
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ReadError> {
        Self::from_reader(std::fs::File::open(path)?)
    }

    /// writes the labels to a writer in .lab format
    pub fn write_to<W: Write>(&self, mut writer: W) -> std::io::Result<()> {
        for label in &self.labels {
            writeln!(writer, "{label}")?;
        }
        Ok(())
    }

    /// writes the labels to the file at `path`, replacing its contents
    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        self.write_to(std::fs::File::create(path)?)
    }

    /// earliest start time across all labels, in 100ns units
    pub fn start(&self) -> Option<u64> {
        self.labels.iter().filter_map(|l| l.start).min()
    }

    /// latest end time across all labels, in 100ns units
    pub fn end(&self) -> Option<u64> {
        self.labels.iter().filter_map(|l| l.end).max()
    }

    /// total time span (earliest start to latest end) in 100ns units
    pub fn duration(&self) -> Option<u64> {
        match (self.start(), self.end()) {
            (Some(s), Some(e)) => Some(e.saturating_sub(s)),
            _ => None,
        }
    }

    /// total time span in seconds
    pub fn duration_secs(&self) -> Option<f64> {
        self.duration().map(units_to_secs)
    }

    /// returns the label whose span contains `time` (in 100ns units)
    /// label matches when `start <= time < end`
    pub fn label_at(&self, time: u64) -> Option<&Label> {
        self.labels.iter().find(|l| l.contains(time))
    }

    /// returns the label whose span contains the time `secs` seconds
    pub fn label_at_secs(&self, secs: f64) -> Option<&Label> {
        self.label_at(secs_to_units(secs))
    }

    /// returns labels overlapping `time` (in 100ns units)
    /// label matches when `start <= time < end`
    pub fn labels_at(&self, time: u64) -> Vec<&Label> {
        self.labels.iter().filter(|l| l.contains(time)).collect()
    }

    /// returns labels overlapping the time `secs` in seconds
    /// label matches when `start <= time < end`
    pub fn labels_at_secs(&self, secs: f64) -> Vec<&Label> {
        self.labels_at(secs_to_units(secs))
    }

    /// finds pairs of consecutive labels that overlap or are out of order
    /// adjacent labels sharing a boundary (`a.end == b.start`) are fine
    /// returns (index of the second label, first label, second label) per problematic pair
    pub fn overlapping_pairs(&self) -> Vec<(usize, &Label, &Label)> {
        let mut overlaps = Vec::new();
        for (i, pair) in self.labels.windows(2).enumerate() {
            let (a, b) = (&pair[0], &pair[1]);
            if let (Some(a_end), Some(b_start)) = (a.end, b.start) {
                if b_start < a_end {
                    overlaps.push((i + 1, a, b));
                }
            }
        }
        overlaps
    }

    /// shifts every time by `offset` in 100ns units, clamping at zero
    pub fn shift(&mut self, offset: i64) {
        let apply = |t: u64| -> u64 {
            if offset >= 0 {
                t.saturating_add(offset as u64)
            } else {
                t.saturating_sub(offset.unsigned_abs())
            }
        };
        for label in &mut self.labels {
            label.start = label.start.map(apply);
            label.end = label.end.map(apply);
        }
    }

    /// shifts every time by `secs` seconds, clamping at zero
    pub fn shift_secs(&mut self, secs: f64) {
        let offset = (secs * crate::UNITS_PER_SECOND as f64).round() as i64;
        self.shift(offset);
    }

    /// multiplies every time by a non-negative finite `factor`, rounding to
    /// the nearest unit
    pub fn scale(&mut self, factor: f64) -> Result<(), ScaleError> {
        if !factor.is_finite() {
            return Err(ScaleError::NonFiniteFactor);
        }
        if factor < 0.0 {
            return Err(ScaleError::NegativeFactor);
        }
        if factor == 1.0 {
            return Ok(());
        }
        let apply = |t: u64| secs_to_units(units_to_secs(t) * factor);
        for label in &mut self.labels {
            label.start = label.start.map(apply);
            label.end = label.end.map(apply);
        }
        Ok(())
    }

    /// merges consecutive labels with identical text, merged labels lose
    /// their scores
    pub fn merge_adjacent(&mut self) {
        let mut merged: Vec<Label> = Vec::with_capacity(self.labels.len());
        for label in self.labels.drain(..) {
            match merged.last_mut() {
                Some(prev)
                    if prev.text == label.text
                        && (matches!((prev.end, label.start), (Some(end), Some(start)) if end == start)
                            || matches!((prev.end, label.start), (None, _) | (_, None))) =>
                {
                    prev.end = label.end.or(prev.end);
                    prev.score = None;
                }
                _ => merged.push(label),
            }
        }
        self.labels = merged;
    }

    /// returns a message for each gap, overlap, or out-of-order pair of
    /// consecutive labels
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for (i, pair) in self.labels.windows(2).enumerate() {
            let (a, b) = (&pair[0], &pair[1]);
            let (Some(a_end), Some(b_start)) = (a.end, b.start) else {
                continue;
            };
            if b_start < a_end {
                let msg = if b.end.is_some_and(|b_end| b_end <= a_end) {
                    "is out of order with"
                } else {
                    "overlaps"
                };
                problems.push(format!(
                    "label {} (`{}`) {} label {} (`{}`)",
                    i + 2,
                    b.text,
                    msg,
                    i + 1,
                    a.text,
                ));
            } else if b_start > a_end {
                problems.push(format!(
                    "gap of {:.4}s between label {} (`{}`) and label {} (`{}`)",
                    units_to_secs(b_start - a_end),
                    i + 1,
                    a.text,
                    i + 2,
                    b.text,
                ));
            }
        }
        problems
    }
}

impl fmt::Display for LabFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for label in &self.labels {
            writeln!(f, "{label}")?;
        }
        Ok(())
    }
}

impl FromStr for LabFile {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut labels = Vec::new();
        for (i, line) in s.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            labels.push(Label::parse_line(line, i + 1)?);
        }
        Ok(LabFile { labels })
    }
}

impl std::ops::Deref for LabFile {
    type Target = Vec<Label>;

    fn deref(&self) -> &Self::Target {
        &self.labels
    }
}

impl std::ops::DerefMut for LabFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.labels
    }
}

impl FromIterator<Label> for LabFile {
    fn from_iter<I: IntoIterator<Item = Label>>(iter: I) -> Self {
        LabFile {
            labels: iter.into_iter().collect(),
        }
    }
}

impl IntoIterator for LabFile {
    type Item = Label;
    type IntoIter = std::vec::IntoIter<Label>;

    fn into_iter(self) -> Self::IntoIter {
        self.labels.into_iter()
    }
}

impl<'a> IntoIterator for &'a LabFile {
    type Item = &'a Label;
    type IntoIter = std::slice::Iter<'a, Label>;

    fn into_iter(self) -> Self::IntoIter {
        self.labels.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "0 1000 a\n1000 2500 b\n2500 4000 b\n";

    fn sample() -> LabFile {
        SAMPLE.parse().unwrap()
    }

    #[test]
    fn parses_and_round_trips() {
        let lab = sample();
        assert_eq!(lab.len(), 3);
        assert_eq!(lab.to_string(), SAMPLE);
    }

    #[test]
    fn skips_blank_lines_and_reports_line_numbers() {
        let lab: LabFile = "0 10 a\n\n  \n10 20 b\n".parse().unwrap();
        assert_eq!(lab.len(), 2);
        let err = "0 10 a\n\n20 10 b\n".parse::<LabFile>().unwrap_err();
        assert_eq!(err.line, 3);
    }

    #[test]
    fn duration_and_lookup() {
        let lab = sample();
        assert_eq!(lab.duration(), Some(4000));
        assert_eq!(lab.label_at(1500).unwrap().text, "b");
        assert_eq!(lab.label_at(4000), None);
    }

    #[test]
    fn shift_clamps_at_zero() {
        let mut lab = sample();
        lab.shift(-1500);
        assert_eq!(lab[0].start, Some(0));
        assert_eq!(lab[0].end, Some(0));
        assert_eq!(lab[1].start, Some(0));
        assert_eq!(lab[2].end, Some(2500));
    }

    #[test]
    fn scale_doubles_times() {
        let mut lab = sample();
        lab.scale(2.0).unwrap();
        assert_eq!(lab[2].end, Some(8000));
    }

    #[test]
    fn merges_adjacent_identical_labels() {
        let mut lab = sample();
        lab.merge_adjacent();
        assert_eq!(lab.len(), 2);
        assert_eq!(lab[1], Label::new(1000, 4000, "b"));
    }

    #[test]
    fn merge_does_not_cross_gaps_or_overlaps() {
        let mut gapped: LabFile = "0 10 a\n20 30 a\n".parse().unwrap();
        gapped.merge_adjacent();
        assert_eq!(gapped.len(), 2);

        let mut nested: LabFile = "0 100 a\n10 20 a\n".parse().unwrap();
        nested.merge_adjacent();
        assert_eq!(nested.len(), 2);
    }

    #[test]
    fn scale_rejects_invalid_factors() {
        let mut lab = sample();
        assert_eq!(lab.scale(f64::NAN), Err(ScaleError::NonFiniteFactor));
        assert_eq!(lab.scale(-1.0), Err(ScaleError::NegativeFactor));
        assert_eq!(lab, sample());
    }

    #[test]
    fn validate_finds_gaps_overlaps_and_disorder() {
        let lab: LabFile = "0 10 a\n5 20 b\n30 40 c\n32 35 d\n".parse().unwrap();
        let problems = lab.validate();
        assert_eq!(problems.len(), 3);
        assert!(problems[0].contains("overlaps"));
        assert!(problems[1].contains("gap"));
        assert!(problems[2].contains("out of order"));
        assert!(sample().validate().is_empty());
    }

    #[test]
    fn reader_and_writer_round_trip() {
        let lab = LabFile::from_reader(SAMPLE.as_bytes()).unwrap();
        let mut out = Vec::new();
        lab.write_to(&mut out).unwrap();
        assert_eq!(out, SAMPLE.as_bytes());
    }
}
