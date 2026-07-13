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

    // parses one line, line_no is 1-indexed for error messages
    pub(crate) fn parse_line(line: &str, line_no: usize) -> Result<Self, ParseError> {
        let tokens = tokenize(line, line_no)?;
        if tokens.is_empty() {
            return Err(invalid_label(line_no, "label line is empty"));
        }

        let mut idx = 0;
        let mut start = None;
        let mut end = None;
        if tokens.len() >= 3 && tokens[0].quoted && tokens[0].value.is_empty() && !tokens[1].quoted
        {
            if let Some(t) = parse_time(&tokens[1].value, line_no)? {
                end = Some(t);
                idx = 2;
            }
        }
        // leading integer tokens are times, but a line must keep at least
        // one token for the label text itself
        while idx < tokens.len() - 1 && idx < 2 && !tokens[idx].quoted {
            match parse_time(&tokens[idx].value, line_no)? {
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
            Some((last, init)) if !init.is_empty() && !last.quoted => {
                match last.value.parse::<f64>() {
                    Ok(score) if score.is_finite() => (init, Some(score)),
                    Err(_) => (rest, None),
                    _ => (rest, None),
                }
            }
            _ => (rest, None),
        };

        Ok(Label {
            start,
            end,
            text: text_tokens
                .iter()
                .map(|token| token.value.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            score,
        })
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(s) = self.start {
            write!(f, "{s} ")?;
        } else if let Some(e) = self.end {
            write!(f, "\"\" {e} ")?;
        }
        if self.start.is_some() {
            if let Some(e) = self.end {
                write!(f, "{e} ")?;
            }
        }
        write_text(f, &self.text)?;
        if let Some(score) = self.score {
            write!(f, " {score}")?;
        }
        Ok(())
    }
}

impl FromStr for Label {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let line = s.trim_end_matches(['\r', '\n']);
        if line.contains('\r') || line.contains('\n') {
            return Err(invalid_label(1, "a label must contain exactly one line"));
        }
        Label::parse_line(line, 1)
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

#[derive(Debug)]
struct Token {
    value: String,
    quoted: bool,
}

fn tokenize(line: &str, line_no: usize) -> Result<Vec<Token>, ParseError> {
    let mut tokens = Vec::new();
    let mut chars = line.chars().peekable();

    while chars.peek().is_some() {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        if chars.peek().is_none() {
            break;
        }

        let mut value = String::new();
        let mut quoted = false;
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                break;
            }
            chars.next();
            if c != '"' {
                value.push(c);
                continue;
            }

            quoted = true;
            let mut closed = false;
            while let Some(c) = chars.next() {
                match c {
                    '"' => {
                        closed = true;
                        break;
                    }
                    '\\' => match chars.next() {
                        Some('n') => value.push('\n'),
                        Some('r') => value.push('\r'),
                        Some('t') => value.push('\t'),
                        Some('\\') => value.push('\\'),
                        Some('"') => value.push('"'),
                        Some(other) => {
                            return Err(invalid_label(
                                line_no,
                                format!("unsupported escape sequence `\\{other}`"),
                            ));
                        }
                        None => return Err(invalid_label(line_no, "unterminated escape sequence")),
                    },
                    other => value.push(other),
                }
            }
            if !closed {
                return Err(invalid_label(line_no, "unterminated quoted label text"));
            }
        }
        tokens.push(Token { value, quoted });
    }

    Ok(tokens)
}

fn invalid_label(line: usize, message: impl Into<String>) -> ParseError {
    ParseError {
        line,
        kind: ParseErrorKind::InvalidLabel(message.into()),
    }
}

fn write_text(f: &mut fmt::Formatter<'_>, text: &str) -> fmt::Result {
    let needs_quotes = text.is_empty()
        || text
            .chars()
            .any(|c| c.is_whitespace() || c == '"' || c == '\\')
        || text.parse::<u64>().is_ok()
        || text.parse::<f64>().is_ok();
    if !needs_quotes {
        return f.write_str(text);
    }

    f.write_str("\"")?;
    for c in text.chars() {
        match c {
            '\n' => f.write_str("\\n")?,
            '\r' => f.write_str("\\r")?,
            '\t' => f.write_str("\\t")?,
            '\\' => f.write_str("\\\\")?,
            '"' => f.write_str("\\\"")?,
            other => write!(f, "{other}")?,
        }
    }
    f.write_str("\"")
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
    fn display_round_trips_ambiguous_text_and_end_only_labels() {
        let labels = [
            Label::new(0, 100, "word 123"),
            Label {
                start: None,
                end: Some(100),
                text: "word".into(),
                score: None,
            },
            Label {
                start: None,
                end: None,
                text: "a\nquoted \"label\"".into(),
                score: None,
            },
        ];

        for label in labels {
            assert_eq!(label.to_string().parse::<Label>().unwrap(), label);
        }
    }

    #[test]
    fn rejects_empty_and_multiline_single_labels() {
        let empty = "  ".parse::<Label>().unwrap_err();
        assert!(matches!(empty.kind, ParseErrorKind::InvalidLabel(_)));

        let multiline = "0 10 a\n10 20 b".parse::<Label>().unwrap_err();
        assert!(matches!(multiline.kind, ParseErrorKind::InvalidLabel(_)));
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
