use conversion_capsule_template::{convert, Options};

#[test]
fn copies_bytes_and_reports_counts() {
    let source = b"standalone capsule";
    let mut input = &source[..];
    let mut output = Vec::new();

    let report = convert(&mut input, &mut output, &Options::default()).unwrap();

    assert_eq!(output, source);
    assert_eq!(report.bytes_read, source.len() as u64);
    assert_eq!(report.bytes_written, source.len() as u64);
}

