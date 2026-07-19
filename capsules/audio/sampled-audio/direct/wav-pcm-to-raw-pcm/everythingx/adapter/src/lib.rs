#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::{self, Cursor, Read, Write};

use everythingx_protocol::{AdapterError, AdapterErrorKind, AdapterHandshake, CapabilityDescriptor, CapsuleIdentity, InvocationRequest, InvocationResult, InvocationStatus, LossLevel, Measurements, ProtocolVersion, Provenance, StaticAdapter};
use wav_pcm_to_raw_pcm::{Endianness, Error as CapsuleError, IntegerEncoding, Options};

pub const ADAPTER_ID: &str = "adapter:wav-pcm-to-raw-pcm-static";
pub const CAPABILITY_ID: &str = "capability:wav-pcm-to-raw-pcm/pcm-exact/native-portable";
pub struct WavPcmToRawPcmAdapter;

fn defaults() -> BTreeMap<String, String> { BTreeMap::from([
    ("buffer_size".into(), "65536".into()), ("max_channels".into(), "256".into()),
    ("output_encoding".into(), "signed".into()), ("output_endianness".into(), "little".into()),
    ("strict_header_consistency".into(), "true".into()),
]) }

fn descriptor() -> CapabilityDescriptor { CapabilityDescriptor {
    capability_id: CAPABILITY_ID.into(), source_formats: vec!["exfmt:audio:wav-pcm".into()], target_formats: vec!["exfmt:audio:raw-pcm".into()],
    strategy: "pcm-exact".into(), backend: "native-portable".into(), default_options: defaults(), defaults_are_runnable: true, streaming: false, seek_required: false,
} }

fn bad(message: impl Into<String>) -> AdapterError { AdapterError::new(AdapterErrorKind::InvalidOptions, message) }
fn parse(request: &InvocationRequest) -> Result<Options, AdapterError> {
    if request.capability_id != CAPABILITY_ID { return Err(AdapterError::new(AdapterErrorKind::UnsupportedCapability, "unsupported capability")); }
    let mut options = Options::default();
    for (name, value) in &request.options { match name.as_str() {
        "output_endianness" => options.output_endianness = match value.as_str() { "little" => Endianness::Little, "big" => Endianness::Big, _ => return Err(bad("output_endianness must be little or big")) },
        "output_encoding" => options.output_encoding = match value.as_str() { "signed" => IntegerEncoding::Signed, "unsigned" => IntegerEncoding::Unsigned, _ => return Err(bad("output_encoding must be signed or unsigned")) },
        "strict_header_consistency" => options.strict_header_consistency = match value.as_str() { "true" => true, "false" => false, _ => return Err(bad("strict_header_consistency must be true or false")) },
        "buffer_size" => options.buffer_size = value.parse().map_err(|_| bad("buffer_size must be usize"))?,
        "max_channels" => options.max_channels = value.parse().map_err(|_| bad("max_channels must be u16"))?,
        _ => return Err(bad(format!("unknown option {name}"))),
    }} Ok(options)
}
fn option_map(o: &Options) -> BTreeMap<String, String> { BTreeMap::from([
    ("buffer_size".into(), o.buffer_size.to_string()), ("max_channels".into(), o.max_channels.to_string()),
    ("output_encoding".into(), match o.output_encoding { IntegerEncoding::Signed => "signed", IntegerEncoding::Unsigned => "unsigned" }.into()),
    ("output_endianness".into(), match o.output_endianness { Endianness::Little => "little", Endianness::Big => "big" }.into()),
    ("strict_header_consistency".into(), o.strict_header_consistency.to_string()),
]) }

struct LimitedWriter<'a> { inner: &'a mut dyn Write, remaining: u64, exceeded: bool }
impl Write for LimitedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() as u64 > self.remaining { self.exceeded = true; return Err(io::Error::other("output budget exceeded")); }
        let written = self.inner.write(bytes)?; self.remaining -= written as u64; Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> { self.inner.flush() }
}

impl StaticAdapter for WavPcmToRawPcmAdapter {
    fn handshake(&self) -> AdapterHandshake { AdapterHandshake { protocol: ProtocolVersion::CURRENT, adapter_id: ADAPTER_ID.into(), adapter_version: "0.1.0".into(), capsule: CapsuleIdentity { id: "capsule:wav-pcm-to-raw-pcm".into(), version: "0.1.0".into(), content_hash: None }, capabilities: vec![descriptor()] } }
    fn invoke(&self, request: &InvocationRequest, input: &mut dyn Read, output: &mut dyn Write) -> Result<InvocationResult, AdapterError> {
        let options = parse(request)?;
        let mut bytes = Vec::new();
        input.take(request.resource_budget.max_memory_bytes.saturating_add(1)).read_to_end(&mut bytes).map_err(|e| AdapterError::new(AdapterErrorKind::Io, e.to_string()))?;
        if bytes.len() as u64 > request.resource_budget.max_memory_bytes { return Err(AdapterError::new(AdapterErrorKind::ResourceLimit, "input exceeds memory budget")); }
        let input_len = bytes.len() as u64;
        let mut cursor = Cursor::new(bytes);
        let mut limited = LimitedWriter { inner: output, remaining: request.resource_budget.max_output_bytes, exceeded: false };
        let report = wav_pcm_to_raw_pcm::convert(&mut cursor, &mut limited, &options).map_err(|error| match error {
            CapsuleError::Io(e) if limited.exceeded => AdapterError::new(AdapterErrorKind::ResourceLimit, e.to_string()),
            CapsuleError::Io(e) => AdapterError::new(AdapterErrorKind::Io, e.to_string()),
            other => AdapterError::new(AdapterErrorKind::InvalidInput, other.to_string()),
        })?;
        let handshake = self.handshake();
        Ok(InvocationResult {
            status: InvocationStatus::Succeeded,
            effects: BTreeMap::from([("format".into(), "raw-pcm".into()), ("parameterized".into(), "true".into())]),
            losses: BTreeMap::from([("payload".into(), LossLevel::None), ("temporal".into(), LossLevel::None), ("structure".into(), LossLevel::Unbounded), ("metadata".into(), LossLevel::Unbounded)]),
            measurements: Measurements { input_bytes: Some(report.input_bytes), output_bytes: Some(report.output_bytes), peak_memory_bytes: Some(input_len + report.peak_working_memory_bytes), ..Measurements::default() },
            capsule_report: BTreeMap::from([("channels".into(), report.channels.to_string()), ("sample_rate".into(), report.sample_rate.to_string()), ("container_bits_per_sample".into(), report.container_bits_per_sample.to_string()), ("valid_bits_per_sample".into(), report.valid_bits_per_sample.to_string()), ("sample_frames".into(), report.sample_frames.to_string()), ("source_data_chunks".into(), report.source_data_chunks.to_string())]),
            warnings: report.warnings,
            provenance: Provenance { capsule: handshake.capsule, adapter_id: handshake.adapter_id, adapter_version: handshake.adapter_version, capability_id: CAPABILITY_ID.into(), strategy: "pcm-exact".into(), backend: "native-portable".into(), effective_options: option_map(&options) },
        })
    }
}

#[cfg(test)] mod tests {
    use super::*; use everythingx_kernel::Kernel;
    #[test] fn kernel_defaults_extract_pcm() {
        let mut wave = b"RIFF\x26\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x44\xac\0\0\x88\x58\x01\0\x02\0\x10\0data\x02\0\0\0\x34\x12".to_vec();
        let riff_size = (wave.len() - 8) as u32;
        wave[4..8].copy_from_slice(&riff_size.to_le_bytes());
        let mut kernel = Kernel::default(); kernel.register(Box::new(WavPcmToRawPcmAdapter)).unwrap();
        let mut input = &wave[..]; let mut output = Vec::new();
        let result = kernel.invoke_defaults(ADAPTER_ID, CAPABILITY_ID, &mut input, &mut output).unwrap();
        assert_eq!(result.status, InvocationStatus::Succeeded); assert_eq!(output, [0x34, 0x12]);
    }
}
