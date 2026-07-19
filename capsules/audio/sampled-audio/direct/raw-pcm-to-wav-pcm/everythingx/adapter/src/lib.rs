#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::{self, Cursor, Read, Write};

use everythingx_protocol::{
    AdapterError, AdapterErrorKind, AdapterHandshake, CapabilityDescriptor, CapsuleIdentity,
    InvocationRequest, InvocationResult, InvocationStatus, LossLevel, Measurements,
    ProtocolVersion, Provenance, StaticAdapter,
};
use raw_pcm_to_wav_pcm::{Endianness, Error as CapsuleError, IntegerEncoding, Options};

pub const ADAPTER_ID: &str = "adapter:raw-pcm-to-wav-pcm-static";
pub const CAPABILITY_ID: &str = "capability:raw-pcm-to-wav-pcm/pcm-exact/native-portable";

pub struct RawPcmToWavPcmAdapter;

fn defaults() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("bits_per_sample".into(), "16".into()),
        ("buffer_size".into(), "65536".into()),
        ("channels".into(), "1".into()),
        ("input_encoding".into(), "signed".into()),
        ("input_endianness".into(), "little".into()),
        ("max_channels".into(), "256".into()),
        ("sample_rate".into(), "44100".into()),
        ("strict_frame_alignment".into(), "true".into()),
    ])
}

fn descriptor() -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability_id: CAPABILITY_ID.into(),
        source_formats: vec!["exfmt:audio:raw-pcm".into()],
        target_formats: vec!["exfmt:audio:wav-pcm".into()],
        strategy: "pcm-exact".into(),
        backend: "native-portable".into(),
        default_options: defaults(),
        defaults_are_runnable: true,
        streaming: false,
        seek_required: false,
    }
}

fn invalid(message: impl Into<String>) -> AdapterError {
    AdapterError::new(AdapterErrorKind::InvalidOptions, message)
}

fn parse_options(request: &InvocationRequest) -> Result<Options, AdapterError> {
    if request.capability_id != CAPABILITY_ID {
        return Err(AdapterError::new(AdapterErrorKind::UnsupportedCapability, "unsupported capability"));
    }
    let mut options = Options::default();
    for (name, value) in &request.options {
        match name.as_str() {
            "channels" => options.channels = value.parse().map_err(|_| invalid("channels must be u16"))?,
            "sample_rate" => options.sample_rate = value.parse().map_err(|_| invalid("sample_rate must be u32"))?,
            "bits_per_sample" => options.bits_per_sample = value.parse().map_err(|_| invalid("bits_per_sample must be u16"))?,
            "input_endianness" => options.input_endianness = match value.as_str() {
                "little" => Endianness::Little,
                "big" => Endianness::Big,
                _ => return Err(invalid("input_endianness must be little or big")),
            },
            "input_encoding" => options.input_encoding = match value.as_str() {
                "signed" => IntegerEncoding::Signed,
                "unsigned" => IntegerEncoding::Unsigned,
                _ => return Err(invalid("input_encoding must be signed or unsigned")),
            },
            "strict_frame_alignment" => options.strict_frame_alignment = match value.as_str() {
                "true" => true,
                "false" => false,
                _ => return Err(invalid("strict_frame_alignment must be true or false")),
            },
            "buffer_size" => options.buffer_size = value.parse().map_err(|_| invalid("buffer_size must be usize"))?,
            "max_channels" => options.max_channels = value.parse().map_err(|_| invalid("max_channels must be u16"))?,
            _ => return Err(invalid(format!("unknown option {name}"))),
        }
    }
    Ok(options)
}

fn options_map(options: &Options) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("bits_per_sample".into(), options.bits_per_sample.to_string()),
        ("buffer_size".into(), options.buffer_size.to_string()),
        ("channels".into(), options.channels.to_string()),
        ("input_encoding".into(), match options.input_encoding { IntegerEncoding::Signed => "signed", IntegerEncoding::Unsigned => "unsigned" }.into()),
        ("input_endianness".into(), match options.input_endianness { Endianness::Little => "little", Endianness::Big => "big" }.into()),
        ("max_channels".into(), options.max_channels.to_string()),
        ("sample_rate".into(), options.sample_rate.to_string()),
        ("strict_frame_alignment".into(), options.strict_frame_alignment.to_string()),
    ])
}

fn read_bounded(input: &mut dyn Read, limit: u64) -> Result<Vec<u8>, AdapterError> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 65_536];
    loop {
        let read = input.read(&mut buffer).map_err(|e| AdapterError::new(AdapterErrorKind::Io, e.to_string()))?;
        if read == 0 { break; }
        if (bytes.len() as u64).saturating_add(read as u64) > limit {
            return Err(AdapterError::new(AdapterErrorKind::ResourceLimit, "input exceeds memory budget"));
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(bytes)
}

struct LimitedWriter<'a> { inner: &'a mut dyn Write, remaining: u64, exceeded: bool }
impl Write for LimitedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() as u64 > self.remaining { self.exceeded = true; return Err(io::Error::other("output budget exceeded")); }
        let written = self.inner.write(bytes)?; self.remaining -= written as u64; Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> { self.inner.flush() }
}

impl StaticAdapter for RawPcmToWavPcmAdapter {
    fn handshake(&self) -> AdapterHandshake {
        AdapterHandshake {
            protocol: ProtocolVersion::CURRENT,
            adapter_id: ADAPTER_ID.into(), adapter_version: "0.1.0".into(),
            capsule: CapsuleIdentity { id: "capsule:raw-pcm-to-wav-pcm".into(), version: "0.1.0".into(), content_hash: None },
            capabilities: vec![descriptor()],
        }
    }

    fn invoke(&self, request: &InvocationRequest, input: &mut dyn Read, output: &mut dyn Write) -> Result<InvocationResult, AdapterError> {
        let options = parse_options(request)?;
        let source = read_bounded(input, request.resource_budget.max_memory_bytes.saturating_sub(options.buffer_size as u64))?;
        let source_len = source.len() as u64;
        let mut source = Cursor::new(source);
        let mut limited = LimitedWriter { inner: output, remaining: request.resource_budget.max_output_bytes, exceeded: false };
        let report = match raw_pcm_to_wav_pcm::convert(&mut source, &mut limited, &options) {
            Ok(report) => report,
            Err(CapsuleError::Io(error)) if limited.exceeded => return Err(AdapterError::new(AdapterErrorKind::ResourceLimit, error.to_string())),
            Err(CapsuleError::Io(error)) => return Err(AdapterError::new(AdapterErrorKind::Io, error.to_string())),
            Err(error) => return Err(AdapterError::new(AdapterErrorKind::InvalidInput, error.to_string())),
        };
        let handshake = self.handshake();
        Ok(InvocationResult {
            status: InvocationStatus::Succeeded,
            effects: BTreeMap::from([("format".into(), "wav-pcm".into()), ("byte_order".into(), "little-endian".into())]),
            losses: BTreeMap::from([("payload".into(), LossLevel::None), ("temporal".into(), LossLevel::None), ("structure".into(), LossLevel::Normalized), ("metadata".into(), LossLevel::None)]),
            measurements: Measurements { input_bytes: Some(report.input_bytes), output_bytes: Some(report.output_bytes), peak_memory_bytes: Some(source_len + report.peak_working_memory_bytes), ..Measurements::default() },
            capsule_report: BTreeMap::from([
                ("channels".into(), report.channels.to_string()), ("sample_rate".into(), report.sample_rate.to_string()),
                ("bits_per_sample".into(), report.bits_per_sample.to_string()), ("sample_frames".into(), report.sample_frames.to_string()),
                ("discarded_trailing_bytes".into(), report.discarded_trailing_bytes.to_string()),
            ]),
            warnings: report.warnings,
            provenance: Provenance { capsule: handshake.capsule, adapter_id: handshake.adapter_id, adapter_version: handshake.adapter_version, capability_id: CAPABILITY_ID.into(), strategy: "pcm-exact".into(), backend: "native-portable".into(), effective_options: options_map(&options) },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use everythingx_kernel::Kernel;

    #[test]
    fn kernel_defaults_wrap_raw_pcm() {
        let mut kernel = Kernel::default();
        kernel.register(Box::new(RawPcmToWavPcmAdapter)).unwrap();
        let mut input = &b"\x34\x12"[..];
        let mut output = Vec::new();
        let result = kernel.invoke_defaults(ADAPTER_ID, CAPABILITY_ID, &mut input, &mut output).unwrap();
        assert_eq!(result.status, InvocationStatus::Succeeded);
        assert_eq!(&output[..4], b"RIFF");
        assert_eq!(&output[44..46], b"\x34\x12");
    }
}
