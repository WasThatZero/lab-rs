//! a single label (one line of a `.lab` file)

use std::fmt;
use std::str::FromStr;

use crate::error::{ParseError, ParseErrorKind};

/// number of HTK time units (100ns) in one second
pub const UNITS_PER_SECOND: u64 = 10_000_000;

/// a single labelled segment, times are in 100ns units as stored in the
/// file, use the `*_secs` methods to work in seconds
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Label {
    /// start time in 100ns units
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub start: Option<u64>,
    /// end time in 100ns units
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub end: Option<u64>,
    /// the label text, e.g. a phone or word
    pub text: String,
    /// optional score following the label text
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub score: Option<f64>,
}

impl Label {
    /// creates a label spanning `start..end` in 100ns units
    pub fn new(start: u64, end: u64, text: impl Into<String>) -> Self {
        Label {
            start: Some(start),
            end: Some(end),
            text: text.into(),
            score: None,
        }
    }

    /// creates a label spanning `start..end` given in seconds
    pub fn from_secs(start: f64, end: f64, text: impl Into<String>) -> Self {
        Label::new(secs_to_units(start), secs_to_units(end), text)
    }

    /// start time in seconds
    pub fn start_secs(&self) -> Option<f64> {
        self.start.map(units_to_secs)
    }

    /// end time in seconds
    pub fn end_secs(&self) -> Option<f64> {
        self.end.map(units_to_secs)
    }

    /// duration in 100ns units, if both times are present
    pub fn duration(&self) -> Option<u64> {
        match (self.start, self.end) {
            (Some(s), Some(e)) => Some(e.saturating_sub(s)),
            _ => None,
        }
    }

    /// duration in seconds, if both times are present
    pub fn duration_secs(&self) -> Option<f64> {
        self.duration().map(units_to_secs)
    }

    /// returns `true` if `time` (in 100ns units) falls within `start..end`
    pub fn contains(&self, time: u64) -> bool {
        matches!((self.start, self.end), (Some(s), Some(e)) if s <= time && time < e)
    }

    // parses one non-empty line, line_no is 1-indexed for error messages
    pub(crate) fn parse_line(line: &str, line_no: usize) -> Result<Self, ParseError> {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        debug_assert!(!tokens.is_empty());

        let mut idx = 0;
        let mut start = None;
        let mut end = None;
        // leading integer tokens are times, but a line must keep at least
        // one token for the label text itself
        while idx < tokens.len() - 1 && idx < 2 {
            match parse_time(tokens[idx], line_no)? {
                Some(t) if idx == 0 => start = Some(t),
                Some(t) => end = Some(t),
                None => break,
            }
            idx += 1;
        }

        if let (Some(s), Some(e)) = (start, end) {
            if s > e {
                return Err(ParseError {
                    line: line_no,
                    kind: ParseErrorKind::StartAfterEnd { start: s, end: e },
                });
            }
        }

        let rest = &tokens[idx..];
        let (text_tokens, score) = match rest.split_last() {
            // a trailing numeric token after the label name is a score
            Some((last, init)) if !init.is_empty() => match last.parse::<f64>() {
                Ok(score) => (init, Some(score)),
                Err(_) => (rest, None),
            },
            _ => (rest, None),
        };

        Ok(Label {
            start,
            end,
            text: text_tokens.join(" "),
            score,
        })
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(s) = self.start {
            write!(f, "{s} ")?;
        }
        if let Some(e) = self.end {
            write!(f, "{e} ")?;
        }
        f.write_str(&self.text)?;
        if let Some(score) = self.score {
            write!(f, " {score}")?;
        }
        Ok(())
    }
}

impl FromStr for Label {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Label::parse_line(s, 1)
    }
}

/// converts a time in 100ns units to seconds
pub fn units_to_secs(units: u64) -> f64 {
    units as f64 / UNITS_PER_SECOND as f64
}

/// converts a time in seconds to 100ns units, rounding to nearest
pub fn secs_to_units(secs: f64) -> u64 {
    (secs * UNITS_PER_SECOND as f64).round().max(0.0) as u64
}

// Ok(None) means the token is label text, not a time, digit-only tokens
// that overflow u64 are an error rather than silently becoming text
fn parse_time(token: &str, line_no: usize) -> Result<Option<u64>, ParseError> {
    match token.parse::<u64>() {
        Ok(t) => Ok(Some(t)),
        Err(_) if token.bytes().all(|b| b.is_ascii_digit()) => Err(ParseError {
            line: line_no,
            kind: ParseErrorKind::InvalidTime(token.to_string()),
        }),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_line() {
        let l: Label = "0 23823130 pau".parse().unwrap();
        assert_eq!(l, Label::new(0, 23823130, "pau"));
    }

    #[test]
    fn parses_score() {
        let l: Label = "0 100 a -42.5".parse().unwrap();
        assert_eq!(l.score, Some(-42.5));
        assert_eq!(l.text, "a");
    }

    #[test]
    fn parses_label_only() {
        let l: Label = "sil".parse().unwrap();
        assert_eq!((l.start, l.end), (None, None));
        assert_eq!(l.text, "sil");
    }

    #[test]
    fn parses_single_time() {
        let l: Label = "100 sil".parse().unwrap();
        assert_eq!((l.start, l.end), (Some(100), None));
    }

    #[test]
    fn numeric_only_line_is_a_label() {
        // the last token is always the label text, even if numeric
        let l: Label = "100 200".parse().unwrap();
        assert_eq!((l.start, l.end), (Some(100), None));
        assert_eq!(l.text, "200");
    }

    #[test]
    fn rejects_start_after_end() {
        let err = "200 100 a".parse::<Label>().unwrap_err();
        assert_eq!(
            err.kind,
            ParseErrorKind::StartAfterEnd {
                start: 200,
                end: 100
            }
        );
    }

    #[test]
    fn rejects_overflowing_time() {
        let err = "99999999999999999999999 100 a"
            .parse::<Label>()
            .unwrap_err();
        assert!(matches!(err.kind, ParseErrorKind::InvalidTime(_)));
    }

    #[test]
    fn display_round_trips() {
        for line in ["0 23823130 pau", "100 sil", "sil", "0 100 a -42.5"] {
            let l: Label = line.parse().unwrap();
            assert_eq!(l.to_string(), line);
        }
    }

    #[test]
    fn seconds_conversion() {
        let l = Label::from_secs(1.0, 2.5, "a");
        assert_eq!(l.start, Some(10_000_000));
        assert_eq!(l.end, Some(25_000_000));
        assert_eq!(l.duration_secs(), Some(1.5));
        assert!(l.contains(15_000_000));
        assert!(!l.contains(25_000_000));
    }
}
