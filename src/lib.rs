//! read, write, and manipulate HTK .lab label files
//!
//! ```
//! use lab_rs::LabFile;
//!
//! let lab: LabFile = "0 2500000 sil\n2500000 4200000 hh\n".parse()?;
//! assert_eq!(lab.label_at_secs(0.3).unwrap().text, "hh");
//! # Ok::<(), lab_rs::ParseError>(())
//! ```
//!
//! times are integers in units of 100ns, parsing is tolerant: start/end
//! times are optional, a trailing numeric score is allowed, and blank
//! lines are ignored
//!
//! feature flags: `serde` adds Serialize/Deserialize, `cli` builds the
//! `lab` binary

#![warn(missing_docs)]

mod error;
mod file;
mod label;

pub use error::{ParseError, ParseErrorKind, ReadError, ScaleError};
pub use file::LabFile;
pub use label::{secs_to_units, units_to_secs, Label, UNITS_PER_SECOND};
