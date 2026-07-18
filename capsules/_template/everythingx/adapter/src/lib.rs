#![forbid(unsafe_code)]

use conversion_capsule_template::{convert, Error, Options, Report};

/// Draft glue only. The production ex-protocol types will live in this adapter,
/// never in the standalone Capsule public API.
pub fn invoke(input: &[u8], options: &Options) -> Result<(Vec<u8>, Report), Error> {
    let mut reader = input;
    let mut output = Vec::new();
    let report = convert(&mut reader, &mut output, options)?;
    Ok((output, report))
}

