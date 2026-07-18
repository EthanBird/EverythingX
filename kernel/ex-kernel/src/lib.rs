#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;
use std::io::{Read, Write};

use everythingx_protocol::{
    AdapterError, AdapterHandshake, CapabilityDescriptor, InvocationRequest, InvocationResult,
    ProtocolVersion, ResourceBudget, StaticAdapter,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelConfig {
    pub protocol: ProtocolVersion,
    pub default_resource_budget: ResourceBudget,
    pub require_runnable_defaults: bool,
}

impl Default for KernelConfig {
    fn default() -> Self {
        Self {
            protocol: ProtocolVersion::CURRENT,
            default_resource_budget: ResourceBudget::default(),
            require_runnable_defaults: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectCapability {
    pub adapter_id: String,
    pub descriptor: CapabilityDescriptor,
}

#[derive(Debug)]
pub enum KernelError {
    DuplicateAdapter(String),
    IncompatibleProtocol {
        adapter_id: String,
        kernel: ProtocolVersion,
        adapter: ProtocolVersion,
    },
    MissingRunnableDefaults {
        adapter_id: String,
        capability_id: String,
    },
    UnknownAdapter(String),
    UnknownCapability {
        adapter_id: String,
        capability_id: String,
    },
    Adapter(AdapterError),
}

impl fmt::Display for KernelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateAdapter(id) => write!(formatter, "adapter already registered: {id}"),
            Self::IncompatibleProtocol {
                adapter_id,
                kernel,
                adapter,
            } => write!(
                formatter,
                "adapter {adapter_id} protocol {adapter:?} is incompatible with kernel {kernel:?}"
            ),
            Self::MissingRunnableDefaults {
                adapter_id,
                capability_id,
            } => write!(
                formatter,
                "adapter {adapter_id} capability {capability_id} has no runnable defaults"
            ),
            Self::UnknownAdapter(id) => write!(formatter, "unknown adapter: {id}"),
            Self::UnknownCapability {
                adapter_id,
                capability_id,
            } => write!(
                formatter,
                "adapter {adapter_id} does not expose capability {capability_id}"
            ),
            Self::Adapter(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for KernelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Adapter(error) => Some(error),
            _ => None,
        }
    }
}

impl From<AdapterError> for KernelError {
    fn from(value: AdapterError) -> Self {
        Self::Adapter(value)
    }
}

struct RegisteredAdapter {
    handshake: AdapterHandshake,
    implementation: Box<dyn StaticAdapter>,
}

pub struct Kernel {
    config: KernelConfig,
    adapters: BTreeMap<String, RegisteredAdapter>,
}

impl Kernel {
    pub fn new(config: KernelConfig) -> Self {
        Self {
            config,
            adapters: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, adapter: Box<dyn StaticAdapter>) -> Result<(), KernelError> {
        let handshake = adapter.handshake();
        if self.adapters.contains_key(&handshake.adapter_id) {
            return Err(KernelError::DuplicateAdapter(handshake.adapter_id));
        }
        if !self.config.protocol.is_compatible_with(handshake.protocol) {
            return Err(KernelError::IncompatibleProtocol {
                adapter_id: handshake.adapter_id,
                kernel: self.config.protocol,
                adapter: handshake.protocol,
            });
        }
        if self.config.require_runnable_defaults {
            for capability in &handshake.capabilities {
                if !capability.defaults_are_runnable {
                    return Err(KernelError::MissingRunnableDefaults {
                        adapter_id: handshake.adapter_id.clone(),
                        capability_id: capability.capability_id.clone(),
                    });
                }
            }
        }
        self.adapters.insert(
            handshake.adapter_id.clone(),
            RegisteredAdapter {
                handshake,
                implementation: adapter,
            },
        );
        Ok(())
    }

    pub fn direct_capabilities(&self, source: &str, target: &str) -> Vec<DirectCapability> {
        let mut matches = Vec::new();
        for registered in self.adapters.values() {
            for capability in &registered.handshake.capabilities {
                if capability.source_formats.iter().any(|value| value == source)
                    && capability.target_formats.iter().any(|value| value == target)
                {
                    matches.push(DirectCapability {
                        adapter_id: registered.handshake.adapter_id.clone(),
                        descriptor: capability.clone(),
                    });
                }
            }
        }
        matches
    }

    pub fn invoke_defaults(
        &self,
        adapter_id: &str,
        capability_id: &str,
        input: &mut dyn Read,
        output: &mut dyn Write,
    ) -> Result<InvocationResult, KernelError> {
        let registered = self
            .adapters
            .get(adapter_id)
            .ok_or_else(|| KernelError::UnknownAdapter(adapter_id.to_owned()))?;
        let capability = registered
            .handshake
            .capabilities
            .iter()
            .find(|item| item.capability_id == capability_id)
            .ok_or_else(|| KernelError::UnknownCapability {
                adapter_id: adapter_id.to_owned(),
                capability_id: capability_id.to_owned(),
            })?;
        if self.config.require_runnable_defaults && !capability.defaults_are_runnable {
            return Err(KernelError::MissingRunnableDefaults {
                adapter_id: adapter_id.to_owned(),
                capability_id: capability_id.to_owned(),
            });
        }
        let mut request = InvocationRequest::with_defaults(capability);
        request.resource_budget = self.config.default_resource_budget.clone();
        registered
            .implementation
            .invoke(&request, input, output)
            .map_err(KernelError::Adapter)
    }

    pub fn invoke(
        &self,
        adapter_id: &str,
        request: &InvocationRequest,
        input: &mut dyn Read,
        output: &mut dyn Write,
    ) -> Result<InvocationResult, KernelError> {
        let registered = self
            .adapters
            .get(adapter_id)
            .ok_or_else(|| KernelError::UnknownAdapter(adapter_id.to_owned()))?;
        if !registered
            .handshake
            .capabilities
            .iter()
            .any(|item| item.capability_id == request.capability_id)
        {
            return Err(KernelError::UnknownCapability {
                adapter_id: adapter_id.to_owned(),
                capability_id: request.capability_id.clone(),
            });
        }
        registered
            .implementation
            .invoke(request, input, output)
            .map_err(KernelError::Adapter)
    }
}

impl Default for Kernel {
    fn default() -> Self {
        Self::new(KernelConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use everythingx_protocol::{
        AdapterErrorKind, CapsuleIdentity, InvocationStatus, LossLevel, Measurements, Provenance,
    };

    struct EchoAdapter;

    fn descriptor(defaults_are_runnable: bool) -> CapabilityDescriptor {
        CapabilityDescriptor {
            capability_id: "capability:test/echo".into(),
            source_formats: vec!["test:bytes".into()],
            target_formats: vec!["test:bytes".into()],
            strategy: "exact".into(),
            backend: "native".into(),
            default_options: BTreeMap::new(),
            defaults_are_runnable,
            streaming: true,
            seek_required: false,
        }
    }

    impl StaticAdapter for EchoAdapter {
        fn handshake(&self) -> AdapterHandshake {
            AdapterHandshake {
                protocol: ProtocolVersion::CURRENT,
                adapter_id: "adapter:test-echo".into(),
                adapter_version: "0.1.0".into(),
                capsule: CapsuleIdentity {
                    id: "capsule:test-echo".into(),
                    version: "0.1.0".into(),
                    content_hash: None,
                },
                capabilities: vec![descriptor(true)],
            }
        }

        fn invoke(
            &self,
            request: &InvocationRequest,
            input: &mut dyn Read,
            output: &mut dyn Write,
        ) -> Result<InvocationResult, AdapterError> {
            let copied = std::io::copy(input, output).map_err(|error| {
                AdapterError::new(AdapterErrorKind::Io, error.to_string())
            })?;
            Ok(InvocationResult {
                status: InvocationStatus::Succeeded,
                effects: BTreeMap::from([("byte_identity".into(), "true".into())]),
                losses: BTreeMap::from([("payload".into(), LossLevel::None)]),
                measurements: Measurements {
                    input_bytes: Some(copied),
                    output_bytes: Some(copied),
                    ..Measurements::default()
                },
                capsule_report: BTreeMap::new(),
                warnings: Vec::new(),
                provenance: Provenance {
                    capsule: self.handshake().capsule,
                    adapter_id: "adapter:test-echo".into(),
                    adapter_version: "0.1.0".into(),
                    capability_id: request.capability_id.clone(),
                    strategy: "exact".into(),
                    backend: "native".into(),
                    effective_options: request.options.clone(),
                },
            })
        }
    }

    #[test]
    fn invokes_registered_adapter_with_defaults() {
        let mut kernel = Kernel::default();
        kernel.register(Box::new(EchoAdapter)).unwrap();
        let mut input = &b"kernel"[..];
        let mut output = Vec::new();
        let result = kernel
            .invoke_defaults(
                "adapter:test-echo",
                "capability:test/echo",
                &mut input,
                &mut output,
            )
            .unwrap();
        assert_eq!(output, b"kernel");
        assert_eq!(result.status, InvocationStatus::Succeeded);
    }

    #[test]
    fn rejects_adapter_without_runnable_defaults() {
        struct BadDefaults;
        impl StaticAdapter for BadDefaults {
            fn handshake(&self) -> AdapterHandshake {
                let mut handshake = EchoAdapter.handshake();
                handshake.adapter_id = "adapter:bad-defaults".into();
                handshake.capabilities = vec![descriptor(false)];
                handshake
            }

            fn invoke(
                &self,
                _request: &InvocationRequest,
                _input: &mut dyn Read,
                _output: &mut dyn Write,
            ) -> Result<InvocationResult, AdapterError> {
                Err(AdapterError::new(
                    AdapterErrorKind::Internal,
                    "must not be called",
                ))
            }
        }

        let mut kernel = Kernel::default();
        let error = kernel.register(Box::new(BadDefaults)).unwrap_err();
        assert!(matches!(error, KernelError::MissingRunnableDefaults { .. }));
    }
}
