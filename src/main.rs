//! the `lab` command-line tool

use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use lab_rs::{units_to_secs, LabFile};

#[derive(Parser)]
#[command(name = "lab", version, about = "Toolkit for HTK .lab label files")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show a summary of a .lab file
    Info {
        /// The .lab file to inspect
        file: PathBuf,
    },
    /// Print labels with times in seconds
    Cat {
        /// The .lab file to print
        file: PathBuf,
        /// Print raw 100ns units instead of seconds
        #[arg(long)]
        raw: bool,
    },
    /// Convert a .lab file to another format
    Convert {
        /// The .lab file to convert
        file: PathBuf,
        /// Output format
        #[arg(long, value_enum)]
        to: Format,
        /// Write to this file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Shift all times by an offset in seconds
    Shift {
        /// The .lab file to shift
        file: PathBuf,
        /// Offset in seconds (may be negative, clamps at zero)
        #[arg(allow_hyphen_values = true)]
        offset: f64,
        /// Rewrite the file in place instead of printing to stdout
        #[arg(short, long)]
        in_place: bool,
    },
    /// Scale all times by a factor
    Scale {
        /// The .lab file to scale
        file: PathBuf,
        /// Scale factor (e.g. 2.0 doubles all times)
        factor: f64,
        /// Rewrite the file in place instead of printing to stdout
        #[arg(short, long)]
        in_place: bool,
    },
    /// Merge consecutive labels with identical text
    Merge {
        /// The .lab file to merge
        file: PathBuf,
        /// Rewrite the file in place instead of printing to stdout
        #[arg(short, long)]
        in_place: bool,
    },
}

#[derive(Copy, Clone, ValueEnum)]
enum Format {
    /// JSON array of label objects
    Json,
    /// Tab-separated start/end (seconds) and text
    Tsv,
    /// Audacity label track (importable via File > Import > Labels)
    Audacity,
    /// Normalized .lab output
    Lab,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Command::Info { file } => {
            let lab = LabFile::from_path(&file)?;
            println!("file:     {}", file.display());
            println!("labels:   {}", lab.len());
            match lab.duration_secs() {
                Some(d) => println!("duration: {d:.4}s"),
                None => println!("duration: unknown (labels have no times)"),
            }
            let unique: BTreeSet<&str> = lab.iter().map(|l| l.text.as_str()).collect();
            println!(
                "unique:   {} ({})",
                unique.len(),
                unique.into_iter().collect::<Vec<_>>().join(", ")
            );
            let problems = lab.validate();
            if problems.is_empty() {
                println!("checks:   ok");
            } else {
                println!("checks:   {} problem(s)", problems.len());
                for p in &problems {
                    println!("  - {p}");
                }
            }
        }
        Command::Cat { file, raw } => {
            let lab = LabFile::from_path(&file)?;
            let stdout = std::io::stdout().lock();
            if raw {
                lab.write_to(stdout)?;
            } else {
                write_seconds(stdout, &lab)?;
            }
        }
        Command::Convert { file, to, output } => {
            let lab = LabFile::from_path(&file)?;
            let mut out: Vec<u8> = Vec::new();
            match to {
                Format::Json => {
                    serde_json::to_writer_pretty(&mut out, &lab)?;
                    out.push(b'\n');
                }
                Format::Tsv => write_seconds(&mut out, &lab)?,
                Format::Audacity => {
                    for l in &lab {
                        let start = l.start.map(units_to_secs).unwrap_or(0.0);
                        let end = l.end.map(units_to_secs).unwrap_or(start);
                        writeln!(out, "{start:.7}\t{end:.7}\t{}", l.text)?;
                    }
                }
                Format::Lab => lab.write_to(&mut out)?,
            }
            match output {
                Some(path) => std::fs::write(path, out)?,
                None => std::io::stdout().write_all(&out)?,
            }
        }
        Command::Shift {
            file,
            offset,
            in_place,
        } => {
            let mut lab = LabFile::from_path(&file)?;
            lab.shift_secs(offset);
            emit(&lab, &file, in_place)?;
        }
        Command::Scale {
            file,
            factor,
            in_place,
        } => {
            let mut lab = LabFile::from_path(&file)?;
            lab.scale(factor)?;
            emit(&lab, &file, in_place)?;
        }
        Command::Merge { file, in_place } => {
            let mut lab = LabFile::from_path(&file)?;
            lab.merge_adjacent();
            emit(&lab, &file, in_place)?;
        }
    }
    Ok(())
}

// tab-separated start/end in seconds, then text and any score
fn write_seconds<W: Write>(mut w: W, lab: &LabFile) -> std::io::Result<()> {
    for l in lab {
        match l.start.map(units_to_secs) {
            Some(s) => write!(w, "{s:.7}\t")?,
            None => write!(w, "\t")?,
        }
        match l.end.map(units_to_secs) {
            Some(e) => write!(w, "{e:.7}\t")?,
            None => write!(w, "\t")?,
        }
        w.write_all(l.text.as_bytes())?;
        if let Some(score) = l.score {
            write!(w, "\t{score}")?;
        }
        writeln!(w)?;
    }
    Ok(())
}

fn emit(lab: &LabFile, file: &std::path::Path, in_place: bool) -> std::io::Result<()> {
    if in_place {
        lab.save(file)
    } else {
        lab.write_to(std::io::stdout().lock())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_output_preserves_100ns_ticks() {
        let lab: LabFile = "0 1 x\n1 2 y\n".parse().unwrap();
        let mut out = Vec::new();
        write_seconds(&mut out, &lab).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "0.0000000\t0.0000001\tx\n0.0000001\t0.0000002\ty\n"
        );
    }
}
