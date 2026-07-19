#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::{self, Cursor, Read, Write};

use bmp_to_png::{
    CompressionStrategy, Error as CapsuleError, FilterStrategy, Options, UnmarkedAlpha,
};
use everythingx_protocol::{
    AdapterError, AdapterErrorKind, AdapterHandshake, CapabilityDescriptor, CapsuleIdentity,
    InvocationRequest, InvocationResult, InvocationStatus, LossLevel, Measurements,
    ProtocolVersion, Provenance, StaticAdapter,
};

pub const ADAPTER_ID: &str = "adapter:bmp-to-png-static";
pub const CAPABILITY_ID: &str =
    "capability:bmp-to-png/pixel-exact/native-portable";

pub struct BmpToPngAdapter;

fn defaults() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("compression".into(), "fixed-rle".into()),
        ("filter".into(), "adaptive".into()),
        ("idat_chunk_size".into(), "65536".into()),
        ("max_pixels".into(), "100000000".into()),
        ("strict_declared_file_size".into(), "true".into()),
        ("unmarked_alpha".into(), "opaque".into()),
    ])
}

fn descriptor() -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability_id: CAPABILITY_ID.into(),
        source_formats: vec!["exfmt:image:bmp-family".into()],
        target_formats: vec!["exfmt:image:png".into()],
        strategy: "pixel-exact".into(),
        backend: "native-portable".into(),
        default_options: defaults(),
        defaults_are_runnable: true,
        streaming: false,
        seek_required: false,
    }
}

fn invalid_options(message: impl Into<String>) -> AdapterError {
    AdapterError::new(AdapterErrorKind::InvalidOptions, message)
}

fn parse_bool(name: &str, value: &str) -> Result<bool, AdapterError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(invalid_options(format!("{name} must be true or false"))),
    }
}

fn parse_options(request: &InvocationRequest) -> Result<Options, AdapterError> {
    if request.capability_id != CAPABILITY_ID {
        return Err(AdapterError::new(
            AdapterErrorKind::UnsupportedCapability,
            format!("unsupported capability {}", request.capability_id),
        ));
    }
    let mut options = Options::default();
    for (name, value) in &request.options {
        match name.as_str() {
            "filter" => {
                options.filter = match value.as_str() {
                    "none" => FilterStrategy::None,
                    "sub" => FilterStrategy::Sub,
                    "up" => FilterStrategy::Up,
                    "average" => FilterStrategy::Average,
                    "paeth" => FilterStrategy::Paeth,
                    "adaptive" => FilterStrategy::Adaptive,
                    _ => return Err(invalid_options("filter must be none, sub, up, average, paeth, or adaptive")),
                }
            }
            "compression" => {
                options.compression = match value.as_str() {
                    "fixed-rle" => CompressionStrategy::FixedRle,
                    "store" => CompressionStrategy::Store,
                    _ => return Err(invalid_options("compression must be fixed-rle or store")),
                }
            }
            "unmarked_alpha" => {
                options.unmarked_alpha = match value.as_str() {
                    "opaque" => UnmarkedAlpha::Opaque,
                    "preserve" => UnmarkedAlpha::Preserve,
                    _ => return Err(invalid_options("unmarked_alpha must be opaque or preserve")),
                }
            }
            "idat_chunk_size" => {
                options.idat_chunk_size = value
                    .parse()
                    .map_err(|_| invalid_options("idat_chunk_size must be an unsigned integer"))?;
            }
            "max_pixels" => {
                options.max_pixels = value
                    .parse()
                    .map_err(|_| invalid_options("max_pixels must be an unsigned integer"))?;
            }
            "strict_declared_file_size" => {
                options.strict_declared_file_size = parse_bool(name, value)?;
            }
            _ => return Err(invalid_options(format!("unknown option {name}"))),
        }
    }
    if options.idat_chunk_size as u64 > request.resource_budget.max_memory_bytes {
        return Err(AdapterError::new(
            AdapterErrorKind::ResourceLimit,
            "idat_chunk_size exceeds request memory budget",
        ));
    }
    let rle_pixel_budget = request.resource_budget.max_memory_bytes / 2;
    if options.max_pixels > rle_pixel_budget {
        return Err(AdapterError::new(
            AdapterErrorKind::ResourceLimit,
            "max_pixels exceeds the Adapter's conservative RLE memory budget",
        ));
    }
    Ok(options)
}

fn options_map(options: &Options) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "compression".into(),
            match options.compression {
                CompressionStrategy::FixedRle => "fixed-rle",
                CompressionStrategy::Store => "store",
            }
            .into(),
        ),
        (
            "filter".into(),
            match options.filter {
                FilterStrategy::None => "none",
                FilterStrategy::Sub => "sub",
                FilterStrategy::Up => "up",
                FilterStrategy::Average => "average",
                FilterStrategy::Paeth => "paeth",
                FilterStrategy::Adaptive => "adaptive",
            }
            .into(),
        ),
        ("idat_chunk_size".into(), options.idat_chunk_size.to_string()),
        ("max_pixels".into(), options.max_pixels.to_string()),
        (
            "strict_declared_file_size".into(),
            options.strict_declared_file_size.to_string(),
        ),
        (
            "unmarked_alpha".into(),
            match options.unmarked_alpha {
                UnmarkedAlpha::Opaque => "opaque",
                UnmarkedAlpha::Preserve => "preserve",
            }
            .into(),
        ),
    ])
}

fn read_bounded(
    input: &mut dyn Read,
    memory_budget: u64,
) -> Result<Vec<u8>, AdapterError> {
    let limit = memory_budget / 2;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| AdapterError::new(AdapterErrorKind::Io, error.to_string()))?;
        if read == 0 {
            break;
        }
        let next = (bytes.len() as u64)
            .checked_add(read as u64)
            .ok_or_else(|| AdapterError::new(AdapterErrorKind::ResourceLimit, "input size overflow"))?;
        if next > limit {
            return Err(AdapterError::new(
                AdapterErrorKind::ResourceLimit,
                "static Adapter input buffer exceeds half of the memory budget",
            ));
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(bytes)
}

struct LimitedWriter<'a> {
    inner: &'a mut dyn Write,
    remaining: u64,
    limit_exceeded: bool,
}

impl Write for LimitedWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.len() as u64 > self.remaining {
            self.limit_exceeded = true;
            return Err(io::Error::other("output exceeds resource budget"));
        }
        let written = self.inner.write(buffer)?;
        self.remaining -= written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl StaticAdapter for BmpToPngAdapter {
    fn handshake(&self) -> AdapterHandshake {
        AdapterHandshake {
            protocol: ProtocolVersion::CURRENT,
            adapter_id: ADAPTER_ID.into(),
            adapter_version: "0.1.0".into(),
            capsule: CapsuleIdentity {
                id: "capsule:bmp-to-png".into(),
                version: "0.1.0".into(),
                content_hash: None,
            },
            capabilities: vec![descriptor()],
        }
    }

    fn invoke(
        &self,
        request: &InvocationRequest,
        input: &mut dyn Read,
        output: &mut dyn Write,
    ) -> Result<InvocationResult, AdapterError> {
        let options = parse_options(request)?;
        let source = read_bounded(input, request.resource_budget.max_memory_bytes)?;
        let mut source = Cursor::new(source);
        let mut limited = LimitedWriter {
            inner: output,
            remaining: request.resource_budget.max_output_bytes,
            limit_exceeded: false,
        };
        let report = match bmp_to_png::convert(&mut source, &mut limited, &options) {
            Ok(report) => report,
            Err(CapsuleError::Io(error)) if limited.limit_exceeded => {
                return Err(AdapterError::new(AdapterErrorKind::ResourceLimit, error.to_string()));
            }
            Err(CapsuleError::Io(error)) => {
                return Err(AdapterError::new(AdapterErrorKind::Io, error.to_string()));
            }
            Err(error) => {
                return Err(AdapterError::new(AdapterErrorKind::InvalidInput, error.to_string()));
            }
        };
        if report.peak_working_memory_bytes > request.resource_budget.max_memory_bytes {
            return Err(AdapterError::new(
                AdapterErrorKind::ResourceLimit,
                "reported Capsule working memory exceeds request budget",
            ));
        }

        let handshake = self.handshake();
        Ok(InvocationResult {
            status: InvocationStatus::Succeeded,
            effects: BTreeMap::from([
                ("format".into(), "png".into()),
                ("pixel_order".into(), "top-down".into()),
                ("color_type".into(), report.output_color_type.into()),
            ]),
            losses: BTreeMap::from([
                ("payload".into(), LossLevel::None),
                ("structure".into(), LossLevel::Normalized),
                ("metadata".into(), LossLevel::Bounded),
            ]),
            measurements: Measurements {
                input_bytes: Some(report.input_bytes),
                output_bytes: Some(report.output_bytes),
                peak_memory_bytes: Some(report.peak_working_memory_bytes),
                ..Measurements::default()
            },
            capsule_report: BTreeMap::from([
                ("width".into(), report.width.to_string()),
                ("height".into(), report.height.to_string()),
                ("source_bits_per_pixel".into(), report.source_bits_per_pixel.to_string()),
                ("source_compression".into(), report.source_compression.into()),
                ("source_top_down".into(), report.source_top_down.to_string()),
                ("palette_entries".into(), report.palette_entries.to_string()),
                ("alpha_preserved".into(), report.alpha_preserved.to_string()),
                ("compression".into(), report.compression.into()),
                ("filter".into(), report.filter.into()),
            ]),
            warnings: report.warnings,
            provenance: Provenance {
                capsule: handshake.capsule,
                adapter_id: handshake.adapter_id,
                adapter_version: handshake.adapter_version,
                capability_id: CAPABILITY_ID.into(),
                strategy: "pixel-exact".into(),
                backend: "native-portable".into(),
                effective_options: options_map(&options),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use everythingx_kernel::Kernel;

    fn one_pixel_bmp() -> Vec<u8> {
        let mut bmp = Vec::new();
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&58_u32.to_le_bytes());
        bmp.extend_from_slice(&0_u32.to_le_bytes());
        bmp.extend_from_slice(&54_u32.to_le_bytes());
        bmp.extend_from_slice(&40_u32.to_le_bytes());
        bmp.extend_from_slice(&1_i32.to_le_bytes());
        bmp.extend_from_slice(&1_i32.to_le_bytes());
        bmp.extend_from_slice(&1_u16.to_le_bytes());
        bmp.extend_from_slice(&24_u16.to_le_bytes());
        bmp.extend_from_slice(&0_u32.to_le_bytes());
        bmp.extend_from_slice(&4_u32.to_le_bytes());
        bmp.extend_from_slice(&[0_u8; 16]);
        bmp.extend_from_slice(&[3, 2, 1, 0]);
        bmp
    }

    #[test]
    fn kernel_invokes_capsule_with_runnable_defaults() {
        let mut kernel = Kernel::default();
        kernel.register(Box::new(BmpToPngAdapter)).unwrap();
        let mut input = &one_pixel_bmp()[..];
        let mut output = Vec::new();
        let result = kernel
            .invoke_defaults(ADAPTER_ID, CAPABILITY_ID, &mut input, &mut output)
            .unwrap();
        assert_eq!(&output[..8], b"\x89PNG\r\n\x1A\n");
        assert_eq!(result.provenance.effective_options, defaults());
        assert_eq!(result.losses["payload"], LossLevel::None);
        assert_eq!(result.capsule_report["width"], "1");
    }
}

