#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::{self, Cursor, Read, Write};

use everythingx_protocol::{
    AdapterError, AdapterErrorKind, AdapterHandshake, CapabilityDescriptor, CapsuleIdentity,
    InvocationRequest, InvocationResult, InvocationStatus, LossLevel, Measurements,
    ProtocolVersion, Provenance, StaticAdapter,
};
use wav_pcm_to_aiff::{Error as CapsuleError, MetadataPolicy, Options};

pub const ADAPTER_ID: &str = "adapter:wav-pcm-to-aiff-static";
pub const CAPABILITY_ID: &str =
    "capability:wav-pcm-to-aiff/pcm-exact/native-portable";

pub struct WavPcmToAiffAdapter;

fn defaults() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("buffer_size".into(), "65536".into()),
        ("max_channels".into(), "256".into()),
        ("max_metadata_bytes".into(), "1048576".into()),
        ("metadata".into(), "common-text".into()),
        ("strict_header_consistency".into(), "true".into()),
    ])
}

fn descriptor() -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability_id: CAPABILITY_ID.into(),
        source_formats: vec!["exfmt:audio:wav-pcm".into()],
        target_formats: vec!["exfmt:audio:aiff-pcm".into()],
        strategy: "pcm-exact".into(),
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
            "metadata" => {
                options.metadata = match value.as_str() {
                    "common-text" => MetadataPolicy::CommonText,
                    "discard" => MetadataPolicy::Discard,
                    _ => return Err(invalid_options("metadata must be common-text or discard")),
                }
            }
            "strict_header_consistency" => {
                options.strict_header_consistency = parse_bool(name, value)?;
            }
            "buffer_size" => {
                options.buffer_size = value
                    .parse()
                    .map_err(|_| invalid_options("buffer_size must be an unsigned integer"))?;
            }
            "max_metadata_bytes" => {
                options.max_metadata_bytes = value
                    .parse()
                    .map_err(|_| invalid_options("max_metadata_bytes must be an unsigned integer"))?;
            }
            "max_channels" => {
                options.max_channels = value
                    .parse()
                    .map_err(|_| invalid_options("max_channels must be an unsigned 16-bit integer"))?;
            }
            _ => return Err(invalid_options(format!("unknown option {name}"))),
        }
    }
    Ok(options)
}

fn options_map(options: &Options) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("buffer_size".into(), options.buffer_size.to_string()),
        ("max_channels".into(), options.max_channels.to_string()),
        (
            "max_metadata_bytes".into(),
            options.max_metadata_bytes.to_string(),
        ),
        (
            "metadata".into(),
            match options.metadata {
                MetadataPolicy::CommonText => "common-text",
                MetadataPolicy::Discard => "discard",
            }
            .into(),
        ),
        (
            "strict_header_consistency".into(),
            options.strict_header_consistency.to_string(),
        ),
    ])
}

fn estimated_core_memory(options: &Options) -> Result<u64, AdapterError> {
    (options.buffer_size as u64)
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(options.max_metadata_bytes))
        .ok_or_else(|| {
            AdapterError::new(
                AdapterErrorKind::ResourceLimit,
                "configured Capsule working memory overflows the resource counter",
            )
        })
}

fn read_bounded(input: &mut dyn Read, limit: u64) -> Result<Vec<u8>, AdapterError> {
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
            .ok_or_else(|| {
                AdapterError::new(AdapterErrorKind::ResourceLimit, "input size overflow")
            })?;
        if next > limit {
            return Err(AdapterError::new(
                AdapterErrorKind::ResourceLimit,
                "static Adapter input plus Capsule working memory exceeds the request budget",
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

impl StaticAdapter for WavPcmToAiffAdapter {
    fn handshake(&self) -> AdapterHandshake {
        AdapterHandshake {
            protocol: ProtocolVersion::CURRENT,
            adapter_id: ADAPTER_ID.into(),
            adapter_version: "0.1.0".into(),
            capsule: CapsuleIdentity {
                id: "capsule:wav-pcm-to-aiff".into(),
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
        let core_memory = estimated_core_memory(&options)?;
        let input_limit = request
            .resource_budget
            .max_memory_bytes
            .checked_sub(core_memory)
            .ok_or_else(|| {
                AdapterError::new(
                    AdapterErrorKind::ResourceLimit,
                    "configured Capsule working memory exceeds the request budget",
                )
            })?;
        let source = read_bounded(input, input_limit)?;
        let source_bytes = source.len() as u64;
        let mut source = Cursor::new(source);
        let mut limited = LimitedWriter {
            inner: output,
            remaining: request.resource_budget.max_output_bytes,
            limit_exceeded: false,
        };
        let report = match wav_pcm_to_aiff::convert(&mut source, &mut limited, &options) {
            Ok(report) => report,
            Err(CapsuleError::Io(error)) if limited.limit_exceeded => {
                return Err(AdapterError::new(
                    AdapterErrorKind::ResourceLimit,
                    error.to_string(),
                ));
            }
            Err(CapsuleError::Io(error)) => {
                return Err(AdapterError::new(AdapterErrorKind::Io, error.to_string()));
            }
            Err(error) => {
                return Err(AdapterError::new(
                    AdapterErrorKind::InvalidInput,
                    error.to_string(),
                ));
            }
        };
        let adapter_peak_memory = source_bytes
            .checked_add(report.peak_working_memory_bytes)
            .ok_or_else(|| {
                AdapterError::new(AdapterErrorKind::ResourceLimit, "peak memory size overflow")
            })?;
        if adapter_peak_memory > request.resource_budget.max_memory_bytes {
            return Err(AdapterError::new(
                AdapterErrorKind::ResourceLimit,
                "static Adapter and Capsule working memory exceed the request budget",
            ));
        }

        let handshake = self.handshake();
        Ok(InvocationResult {
            status: InvocationStatus::Succeeded,
            effects: BTreeMap::from([
                ("format".into(), "aiff".into()),
                ("sample_encoding".into(), "signed-integer-pcm".into()),
                ("byte_order".into(), "big-endian".into()),
            ]),
            losses: BTreeMap::from([
                ("payload".into(), LossLevel::None),
                ("temporal".into(), LossLevel::None),
                ("structure".into(), LossLevel::Normalized),
                // Only the explicitly mapped LIST/INFO subset can be retained.
                ("metadata".into(), LossLevel::Bounded),
            ]),
            measurements: Measurements {
                input_bytes: Some(report.input_bytes),
                output_bytes: Some(report.output_bytes),
                peak_memory_bytes: Some(adapter_peak_memory),
                ..Measurements::default()
            },
            capsule_report: BTreeMap::from([
                ("channels".into(), report.channels.to_string()),
                ("sample_rate".into(), report.sample_rate.to_string()),
                (
                    "container_bits_per_sample".into(),
                    report.container_bits_per_sample.to_string(),
                ),
                (
                    "valid_bits_per_sample".into(),
                    report.valid_bits_per_sample.to_string(),
                ),
                ("sample_frames".into(), report.sample_frames.to_string()),
                (
                    "source_audio_bytes".into(),
                    report.source_audio_bytes.to_string(),
                ),
                (
                    "output_audio_bytes".into(),
                    report.output_audio_bytes.to_string(),
                ),
                (
                    "source_data_chunks".into(),
                    report.source_data_chunks.to_string(),
                ),
                (
                    "wave_format_extensible".into(),
                    report.wave_format_extensible.to_string(),
                ),
                (
                    "metadata_chunks_found".into(),
                    report.metadata_chunks_found.to_string(),
                ),
                (
                    "metadata_chunks_preserved".into(),
                    report.metadata_chunks_preserved.to_string(),
                ),
            ]),
            warnings: report.warnings,
            provenance: Provenance {
                capsule: handshake.capsule,
                adapter_id: handshake.adapter_id,
                adapter_version: handshake.adapter_version,
                capability_id: CAPABILITY_ID.into(),
                strategy: "pcm-exact".into(),
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

    fn one_frame_wave() -> Vec<u8> {
        let mut wav = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0".to_vec();
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8_000_u32.to_le_bytes());
        wav.extend_from_slice(&8_000_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8_u16.to_le_bytes());
        wav.extend_from_slice(b"data\x01\0\0\0\x80\0");
        let riff_size = wav.len() as u32 - 8;
        wav[4..8].copy_from_slice(&riff_size.to_le_bytes());
        wav
    }

    #[test]
    fn kernel_invokes_capsule_with_runnable_defaults() {
        let mut kernel = Kernel::default();
        kernel.register(Box::new(WavPcmToAiffAdapter)).unwrap();
        let mut input = &one_frame_wave()[..];
        let mut output = Vec::new();
        let result = kernel
            .invoke_defaults(ADAPTER_ID, CAPABILITY_ID, &mut input, &mut output)
            .unwrap();
        assert_eq!(&output[..4], b"FORM");
        assert_eq!(&output[8..12], b"AIFF");
        assert_eq!(result.provenance.effective_options, defaults());
        assert_eq!(result.losses["payload"], LossLevel::None);
        assert_eq!(result.capsule_report["sample_frames"], "1");
    }
}
