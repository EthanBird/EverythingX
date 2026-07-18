#![forbid(unsafe_code)]

use std::io::{self, Read, Write};

#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub buffer_size: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            buffer_size: 64 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub strategy: &'static str,
    pub backend: &'static str,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    InvalidOptions(&'static str),
    Io(io::Error),
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub fn convert<R: Read, W: Write>(
    input: &mut R,
    output: &mut W,
    options: &Options,
) -> Result<Report, Error> {
    let buffer_size = options.buffer_size;
    if buffer_size == 0 {
        return Err(Error::InvalidOptions("buffer_size must be greater than zero"));
    }
    if buffer_size > 16 * 1024 * 1024 {
        return Err(Error::InvalidOptions("buffer_size exceeds 16 MiB"));
    }

    let mut buffer = vec![0_u8; buffer_size];
    let mut total = 0_u64;
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        output.write_all(&buffer[..count])?;
        total += count as u64;
    }

    Ok(Report {
        bytes_read: total,
        bytes_written: total,
        strategy: "exact",
        backend: "native",
        warnings: Vec::new(),
    })
}
