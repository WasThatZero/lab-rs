# lab-rs

[![crates.io](https://img.shields.io/crates/v/lab-rs.svg)](https://crates.io/crates/lab-rs)
[![docs.rs](https://docs.rs/lab-rs/badge.svg)](https://docs.rs/lab-rs)
[![CI](https://github.com/WasThatZero/lab-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/WasThatZero/lab-rs/actions/workflows/ci.yml)

Toolkit and library for reading, writing, and manipulating HTK .lab label files

.lab files are plain-text label files used by [HTK](https://htk.eng.cam.ac.uk/) and throughout speech and singing-voice tooling for phoneme/word alignments. Each line is a labelled time segment, with times in units of 100 nanoseconds:

```text
0 2500000 sil
2500000 4200000 hh
4200000 6100000 eh
6100000 7800000 l
7800000 9200000 ow
9200000 12000000 sil
```

The parser handles the common subset of the HTK label format tolerantly: start and end times are optional, an optional numeric score may follow the label text, and blank lines and extra whitespace are ignored. Quoted text with `\\` escapes is supported and is emitted when needed to preserve ambiguous label text, normalized end-only labels use an empty quoted start marker (`"" <end> <text>`).

> I tried my best not to make *my* code look AI-generated since that's apparently something you need to worry about nowadays. Excuse the potential lack of comments and documentation outside this README and docs.rs. And yes, I am bitter about this.

## Library

```sh
cargo add lab-rs
```

The library has no dependencies by default. Enable the `serde` feature for `Serialize`/`Deserialize` on all types.

```rust
use lab_rs::LabFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut lab = LabFile::from_path("hello.lab")?;

    println!("{} labels", lab.len());
    lab.retain(|l| l.text != "sil");
    lab.shift_secs(0.1);
    lab.save("hello-edited.lab")?;

    Ok(())
}
```

`LabFile` dereferences to `Vec<Label>`, so the usual vector and slice methods work directly. It also provides helpers for looking up labels by time, shifting and scaling timestamps, merging adjacent labels, and validating a label sequence. See the [documentation](https://docs.rs/lab-rs) for the full interface.

## CLI

```sh
cargo install lab-rs --features cli
```

This installs a `lab` binary that lets you do things like:

```sh
lab info hello.lab              # label count, duration, unique labels, sanity checks
lab cat hello.lab               # print with times in seconds (--raw for 100ns units)
lab convert hello.lab --to json # also: tsv, audacity, lab
lab shift hello.lab -- -0.25    # shift all times by -0.25 s (stdout, or -i for in place)
lab scale hello.lab 2.0         # stretch all times by 2x
lab merge hello.lab             # merge consecutive identical labels
```

