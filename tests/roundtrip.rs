use lab_rs::{LabFile, Label, ParseErrorKind, ReadError, UNITS_PER_SECOND};

const ALIGNMENT: &str = "\
0 2500000 sil
2500000 4200000 hh
4200000 6100000 eh
6100000 7800000 l
7800000 9200000 ow
9200000 12000000 sil
";

#[test]
fn parse_manipulate_write() {
    let mut lab: LabFile = ALIGNMENT.parse().unwrap();
    assert_eq!(lab.len(), 6);
    assert_eq!(lab.duration_secs(), Some(1.2));
    assert!(lab.validate().is_empty());
    assert_eq!(lab.label_at_secs(0.5).unwrap().text, "eh");

    lab.shift_secs(0.1);
    assert_eq!(lab[0].start, Some(1_000_000));
    lab.shift_secs(-0.1);
    assert_eq!(lab.to_string(), ALIGNMENT);
}

#[test]
fn file_io_round_trip() {
    let dir = std::env::temp_dir().join("lab-rs-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("roundtrip.lab");

    let lab: LabFile = ALIGNMENT.parse().unwrap();
    lab.save(&path).unwrap();
    let reread = LabFile::from_path(&path).unwrap();
    assert_eq!(reread, lab);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn tolerant_of_messy_whitespace() {
    let lab: LabFile = "  0\t100   a  \r\n\r\n100 200 b\r\n".parse().unwrap();
    assert_eq!(lab.len(), 2);
    assert_eq!(lab[0], Label::new(0, 100, "a"));
}

#[test]
fn read_error_carries_parse_details() {
    let err = LabFile::from_reader("0 10 a\n30 20 b\n".as_bytes()).unwrap_err();
    match err {
        ReadError::Parse(e) => {
            assert_eq!(e.line, 2);
            assert!(matches!(e.kind, ParseErrorKind::StartAfterEnd { .. }));
        }
        other => panic!("expected parse error, got {other:?}"),
    }
}

#[cfg(feature = "serde")]
#[test]
fn serde_json_round_trip() {
    let lab: LabFile = ALIGNMENT.parse().unwrap();
    let json = serde_json::to_string(&lab).unwrap();
    let back: LabFile = serde_json::from_str(&json).unwrap();
    assert_eq!(back, lab);
}

#[test]
fn label_secs_accessors() {
    let l = Label::new(10, 20, "test");
    assert_eq!(l.start_secs(), Some(10.0 / UNITS_PER_SECOND as f64));
    assert_eq!(l.end_secs(), Some(20.0 / UNITS_PER_SECOND as f64));
    assert_eq!(l.duration_secs(), Some(10.0 / UNITS_PER_SECOND as f64));

    let bare = Label { start: None, end: None, text: "test".to_string(), score: None };
    assert_eq!(bare.start_secs(), None);
    assert_eq!(bare.end_secs(), None);
    assert_eq!(bare.duration_secs(), None);
}

#[test]
fn labfile_labels_at_methods() {
    let lab: LabFile = "0 10 a\n10 20 b\n20 30 c\n".parse().unwrap();
    assert_eq!(lab.label_at(5).unwrap().text, "a");
    assert_eq!(lab.label_at(15).unwrap().text, "b");
    // intervals are half-open, so a shared boundary belongs to the later label
    assert_eq!(lab.label_at(20).unwrap().text, "c");
    assert_eq!(lab.label_at(30), None);

    assert_eq!(lab.labels_at(5).len(), 1);
    assert_eq!(lab.labels_at(5)[0].text, "a");
    assert_eq!(lab.labels_at(15).len(), 1);
    assert_eq!(lab.labels_at(15)[0].text, "b");
    assert_eq!(lab.labels_at(20).len(), 1);
    assert_eq!(lab.labels_at(20)[0].text, "c");
    assert_eq!(lab.labels_at(30).len(), 0);

    let secs = |units: u64| units as f64 / UNITS_PER_SECOND as f64;
    assert_eq!(lab.labels_at_secs(secs(5)).len(), 1);
    assert_eq!(lab.labels_at_secs(secs(15)).len(), 1);
    assert_eq!(lab.labels_at_secs(secs(20)).len(), 1);
    assert_eq!(lab.labels_at_secs(secs(30)).len(), 0);
}

#[test]
fn labfile_overlapping_pairs() {
    let lab: LabFile = "0 10 a\n5 20 b\n20 30 c\n".parse().unwrap();
    let overlaps = lab.overlapping_pairs();
    assert_eq!(overlaps.len(), 1);
    assert_eq!(overlaps[0].0, 1);
    assert_eq!(overlaps[0].1.text, "a");
    assert_eq!(overlaps[0].2.text, "b");

    // adjacent labels sharing a boundary are not overlaps
    let clean: LabFile = "0 10 a\n10 20 b\n".parse().unwrap();
    assert!(clean.overlapping_pairs().is_empty());
}
