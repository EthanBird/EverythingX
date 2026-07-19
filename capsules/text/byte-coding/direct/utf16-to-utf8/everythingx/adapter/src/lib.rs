#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::{self, Read, Write};

use everythingx_protocol::{
    AdapterError, AdapterErrorKind, AdapterHandshake, CapabilityDescriptor, CapsuleIdentity,
    InvocationRequest, InvocationResult, InvocationStatus, LossLevel, Measurements,
    ProtocolVersion, Provenance, StaticAdapter,
};
use utf16_to_utf8::{
    ByteOrder, Endianness, Error as CapsuleError, InvalidSequencePolicy, Options,
};

pub const ADAPTER_ID: &str = "adapter:utf16-to-utf8-static";
pub const STRICT_CAPABILITY: &str = "capability:utf16-to-utf8/strict/native-portable";
pub const REPLACE_CAPABILITY: &str =
    "capability:utf16-to-utf8/replace-invalid/native-portable";

pub struct Utf16ToUtf8Adapter;

fn strict_defaults() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("buffer_size".into(), "65536".into()),
        ("default_endianness".into(), "little".into()),
        ("emit_utf8_bom".into(), "false".into()),
        ("input_endianness".into(), "auto".into()),
        ("invalid_sequence".into(), "error".into()),
    ])
}

fn replace_defaults() -> BTreeMap<String, String> {
    let mut values = strict_defaults();
    values.insert("invalid_sequence".into(), "replace".into());
    values
}

fn capability(
    capability_id: &str,
    strategy: &str,
    defaults: BTreeMap<String, String>,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability_id: capability_id.into(),
        source_formats: vec!["exfmt:text:utf-16".into()],
        target_formats: vec!["exfmt:text:utf-8".into()],
        strategy: strategy.into(),
        backend: "native-portable".into(),
        default_options: defaults,
        defaults_are_runnable: true,
        streaming: true,
        seek_required: false,
    }
}

fn parse_bool(name: &str, value: &str) -> Result<bool, AdapterError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(AdapterError::new(
            AdapterErrorKind::InvalidOptions,
            format!("{name} must be true or false"),
        )),
    }
}

fn parse_options(request: &InvocationRequest) -> Result<Options, AdapterError> {
    let expected_policy = match request.capability_id.as_str() {
        STRICT_CAPABILITY => InvalidSequencePolicy::Error,
        REPLACE_CAPABILITY => InvalidSequencePolicy::Replace,
        _ => {
            return Err(AdapterError::new(
                AdapterErrorKind::UnsupportedCapability,
                format!("unsupported capability {}", request.capability_id),
            ));
        }
    };
    let mut options = Options {
        invalid_sequence: expected_policy,
        ..Options::default()
    };
    for (name, value) in &request.options {
        match name.as_str() {
            "input_endianness" => {
                options.input_endianness = match value.as_str() {
                    "auto" => Endianness::Auto,
                    "little" => Endianness::Little,
                    "big" => Endianness::Big,
                    _ => {
                        return Err(AdapterError::new(
                            AdapterErrorKind::InvalidOptions,
                            "input_endianness must be auto, little, or big",
                        ));
                    }
                }
            }
            "default_endianness" => {
                options.default_endianness = match value.as_str() {
                    "little" => ByteOrder::Little,
                    "big" => ByteOrder::Big,
                    _ => {
                        return Err(AdapterError::new(
                            AdapterErrorKind::InvalidOptions,
                            "default_endianness must be little or big",
                        ));
                    }
                }
            }
            "invalid_sequence" => {
                let requested = match value.as_str() {
                    "error" => InvalidSequencePolicy::Error,
                    "replace" => InvalidSequencePolicy::Replace,
                    _ => {
                        return Err(AdapterError::new(
                            AdapterErrorKind::InvalidOptions,
                            "invalid_sequence must be error or replace",
                        ));
                    }
                };
                if requested != expected_policy {
                    return Err(AdapterError::new(
                        AdapterErrorKind::InvalidOptions,
                        "invalid_sequence conflicts with selected capability strategy",
                    ));
                }
                options.invalid_sequence = requested;
            }
            "emit_utf8_bom" => options.emit_utf8_bom = parse_bool(name, value)?,
            "buffer_size" => {
                options.buffer_size = value.parse().map_err(|_| {
                    AdapterError::new(
                        AdapterErrorKind::InvalidOptions,
                        "buffer_size must be an unsigned integer",
                    )
                })?;
            }
            _ => {
                return Err(AdapterError::new(
                    AdapterErrorKind::InvalidOptions,
                    format!("unknown option {name}"),
                ));
            }
        }
    }
    if options.buffer_size as u64 > request.resource_budget.max_memory_bytes {
        return Err(AdapterError::new(
            AdapterErrorKind::ResourceLimit,
            "buffer_size exceeds request memory budget",
        ));
    }
    Ok(options)
}

fn options_map(options: &Options) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("buffer_size".into(), options.buffer_size.to_string()),
        (
            "default_endianness".into(),
            match options.default_endianness {
                ByteOrder::Little => "little",
                ByteOrder::Big => "big",
            }
            .into(),
        ),
        ("emit_utf8_bom".into(), options.emit_utf8_bom.to_string()),
        (
            "input_endianness".into(),
            match options.input_endianness {
                Endianness::Auto => "auto",
                Endianness::Little => "little",
                Endianness::Big => "big",
            }
            .into(),
        ),
        (
            "invalid_sequence".into(),
            match options.invalid_sequence {
                InvalidSequencePolicy::Error => "error",
                InvalidSequencePolicy::Replace => "replace",
            }
            .into(),
        ),
    ])
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

impl StaticAdapter for Utf16ToUtf8Adapter {
    fn handshake(&self) -> AdapterHandshake {
        AdapterHandshake {
            protocol: ProtocolVersion::CURRENT,
            adapter_id: ADAPTER_ID.into(),
            adapter_version: "0.1.0".into(),
            capsule: CapsuleIdentity {
                id: "capsule:utf16-to-utf8".into(),
                version: "0.1.0".into(),
                content_hash: None,
            },
            capabilities: vec![
                capability(STRICT_CAPABILITY, "strict", strict_defaults()),
                capability(
                    REPLACE_CAPABILITY,
                    "replace-invalid",
                    replace_defaults(),
                ),
            ],
        }
    }

    fn invoke(
        &self,
        request: &InvocationRequest,
        input: &mut dyn Read,
        output: &mut dyn Write,
    ) -> Result<InvocationResult, AdapterError> {
        let options = parse_options(request)?;
        let mut limited = LimitedWriter {
            inner: output,
            remaining: request.resource_budget.max_output_bytes,
            limit_exceeded: false,
        };
        let conversion = utf16_to_utf8::convert(input, &mut limited, &options);
        let report = match conversion {
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

        let handshake = self.handshake();
        let capability = handshake
            .capabilities
            .iter()
            .find(|item| item.capability_id == request.capability_id)
            .expect("capability validated while parsing options");
        let capability_id = capability.capability_id.clone();
        let strategy = capability.strategy.clone();
        let backend = capability.backend.clone();
        let mut losses = BTreeMap::from([
            ("structure".into(), LossLevel::Normalized),
            ("payload".into(), LossLevel::None),
        ]);
        if report.replacement_count > 0 {
            losses.insert("payload".into(), LossLevel::Bounded);
        }
        Ok(InvocationResult {
            status: InvocationStatus::Succeeded,
            effects: BTreeMap::from([
                ("encoding".into(), "utf-8".into()),
                ("valid_utf8".into(), "true".into()),
            ]),
            losses,
            measurements: Measurements {
                input_bytes: Some(report.input_bytes),
                output_bytes: Some(report.output_bytes),
                ..Measurements::default()
            },
            capsule_report: BTreeMap::from([
                (
                    "detected_endianness".into(),
                    match report.detected_endianness {
                        ByteOrder::Little => "little",
                        ByteOrder::Big => "big",
                    }
                    .into(),
                ),
                ("input_bom_consumed".into(), report.input_bom_consumed.to_string()),
                (
                    "decoded_scalar_values".into(),
                    report.decoded_scalar_values.to_string(),
                ),
                ("replacement_count".into(), report.replacement_count.to_string()),
            ]),
            warnings: report.warnings,
            provenance: Provenance {
                capsule: handshake.capsule,
                adapter_id: handshake.adapter_id,
                adapter_version: handshake.adapter_version,
                capability_id,
                strategy,
                backend,
                effective_options: options_map(&options),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use everythingx_kernel::Kernel;

    #[test]
    fn kernel_invokes_capsule_with_runnable_defaults() {
        let mut kernel = Kernel::default();
        kernel.register(Box::new(Utf16ToUtf8Adapter)).unwrap();
        let mut input = &[0x48, 0x00, 0x69, 0x00][..];
        let mut output = Vec::new();
        let result = kernel
            .invoke_defaults(ADAPTER_ID, STRICT_CAPABILITY, &mut input, &mut output)
            .unwrap();
        assert_eq!(output, b"Hi");
        assert_eq!(result.provenance.effective_options, strict_defaults());
        assert_eq!(result.losses["payload"], LossLevel::None);
    }

    #[test]
    fn replacement_capability_reports_bounded_loss() {
        let mut kernel = Kernel::default();
        kernel.register(Box::new(Utf16ToUtf8Adapter)).unwrap();
        let mut input = &[0x00, 0xD8, 0x41, 0x00][..];
        let mut output = Vec::new();
        let result = kernel
            .invoke_defaults(ADAPTER_ID, REPLACE_CAPABILITY, &mut input, &mut output)
            .unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "�A");
        assert_eq!(result.losses["payload"], LossLevel::Bounded);
    }
}
