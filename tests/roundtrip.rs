use lab_rs::{LabFile, Label, ParseErrorKind, ReadError};

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
