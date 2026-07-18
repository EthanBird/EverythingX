#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;
use std::io::{Read, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

impl ProtocolVersion {
    pub const CURRENT: Self = Self { major: 0, minor: 1 };

    pub fn is_compatible_with(self, other: Self) -> bool {
        self.major == other.major && other.minor <= self.minor
    }
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self::CURRENT
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceBudget {
    pub timeout_millis: u64,
    pub max_memory_bytes: u64,
    pub max_temporary_bytes: u64,
    pub max_output_bytes: u64,
    pub parallelism: u16,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            timeout_millis: 300_000,
            max_memory_bytes: 512 * 1024 * 1024,
            max_temporary_bytes: 2 * 1024 * 1024 * 1024,
            max_output_bytes: 4 * 1024 * 1024 * 1024,
            parallelism: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapsuleIdentity {
    pub id: String,
    pub version: String,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDescriptor {
    pub capability_id: String,
    pub source_formats: Vec<String>,
    pub target_formats: Vec<String>,
    pub strategy: String,
    pub backend: String,
    pub default_options: BTreeMap<String, String>,
    pub defaults_are_runnable: bool,
    pub streaming: bool,
    pub seek_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterHandshake {
    pub protocol: ProtocolVersion,
    pub adapter_id: String,
    pub adapter_version: String,
    pub capsule: CapsuleIdentity,
    pub capabilities: Vec<CapabilityDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationRequest {
    pub capability_id: String,
    pub options: BTreeMap<String, String>,
    pub resource_budget: ResourceBudget,
    pub invariants: Vec<String>,
}

impl InvocationRequest {
    pub fn with_defaults(capability: &CapabilityDescriptor) -> Self {
        Self {
            capability_id: capability.capability_id.clone(),
            options: capability.default_options.clone(),
            resource_budget: ResourceBudget::default(),
            invariants: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationStatus {
    Succeeded,
    Rejected,
    ResourceLimit,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LossLevel {
    None,
    Normalized,
    Bounded,
    Unbounded,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Measurements {
    pub input_bytes: Option<u64>,
    pub output_bytes: Option<u64>,
    pub elapsed_micros: Option<u64>,
    pub peak_memory_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub capsule: CapsuleIdentity,
    pub adapter_id: String,
    pub adapter_version: String,
    pub capability_id: String,
    pub strategy: String,
    pub backend: String,
    pub effective_options: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationResult {
    pub status: InvocationStatus,
    pub effects: BTreeMap<String, String>,
    pub losses: BTreeMap<String, LossLevel>,
    pub measurements: Measurements,
    pub capsule_report: BTreeMap<String, String>,
    pub warnings: Vec<String>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterErrorKind {
    UnsupportedCapability,
    InvalidOptions,
    InvalidInput,
    ResourceLimit,
    Io,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterError {
    pub kind: AdapterErrorKind,
    pub message: String,
}

impl AdapterError {
    pub fn new(kind: AdapterErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for AdapterError {}

pub trait StaticAdapter: Send + Sync {
    fn handshake(&self) -> AdapterHandshake;

    fn invoke(
        &self,
        request: &InvocationRequest,
        input: &mut dyn Read,
        output: &mut dyn Write,
    ) -> Result<InvocationResult, AdapterError>;
}

