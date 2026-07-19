#!/usr/bin/env python3
"""Materialize the independent PCM Wave A container Capsule batch."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TEMPLATE = ROOT / "tools" / "templates" / "pcm_container_capsule.rs"
RAW_TEMPLATE = ROOT / "tools" / "templates" / "pcm_raw_transform_capsule.rs"
CAPSULES = ROOT / "capsules" / "audio" / "sampled-audio" / "direct"
RAW_CAPSULES = ROOT / "capsules" / "audio" / "sampled-audio" / "transform"
NOTICE = """Required Notice: Copyright © 2026 EthanBird. All rights reserved.

This Conversion Capsule is licensed under the PolyForm Noncommercial License
1.0.0 included at the EverythingX repository root:
https://polyformproject.org/licenses/noncommercial/1.0.0
"""


@dataclass(frozen=True)
class Spec:
    name: str
    source_profile: str
    target_profile: str
    source_format: str
    target_format: str
    source_label: str
    target_label: str


@dataclass(frozen=True)
class RawSpec:
    name: str
    mode: str
    operator_kind: str
    summary: str


SPECS = (
    Spec("wav-pcm-to-caf-pcm", "Wav", "Caf", "exfmt:audio:wav-pcm", "exfmt:audio:caf-pcm", "RIFF/WAVE PCM", "Core Audio Format PCM"),
    Spec("caf-pcm-to-wav-pcm", "Caf", "Wav", "exfmt:audio:caf-pcm", "exfmt:audio:wav-pcm", "Core Audio Format PCM", "RIFF/WAVE PCM"),
    Spec("wav-pcm-to-au-pcm", "Wav", "Au", "exfmt:audio:wav-pcm", "exfmt:audio:au-pcm", "RIFF/WAVE PCM", "Sun AU/SND PCM"),
    Spec("au-pcm-to-wav-pcm", "Au", "Wav", "exfmt:audio:au-pcm", "exfmt:audio:wav-pcm", "Sun AU/SND PCM", "RIFF/WAVE PCM"),
    Spec("wav-pcm-to-rf64-pcm", "Wav", "Rf64", "exfmt:audio:wav-pcm", "exfmt:audio:rf64-pcm", "RIFF/WAVE PCM", "RF64 PCM"),
    Spec("rf64-pcm-to-wav-pcm", "Rf64", "Wav", "exfmt:audio:rf64-pcm", "exfmt:audio:wav-pcm", "RF64 PCM", "RIFF/WAVE PCM"),
    Spec("wav-pcm-to-bw64-pcm", "Wav", "Bw64", "exfmt:audio:wav-pcm", "exfmt:audio:bw64-pcm", "RIFF/WAVE PCM", "BW64 PCM"),
    Spec("bw64-pcm-to-wav-pcm", "Bw64", "Wav", "exfmt:audio:bw64-pcm", "exfmt:audio:wav-pcm", "BW64 PCM", "RIFF/WAVE PCM"),
    Spec("wav-pcm-to-wave64-pcm", "Wav", "Wave64", "exfmt:audio:wav-pcm", "exfmt:audio:wave64-pcm", "RIFF/WAVE PCM", "Sony Wave64 PCM"),
    Spec("wave64-pcm-to-wav-pcm", "Wave64", "Wav", "exfmt:audio:wave64-pcm", "exfmt:audio:wav-pcm", "Sony Wave64 PCM", "RIFF/WAVE PCM"),
    Spec("wav-pcm-to-bwf-pcm", "Wav", "Bwf", "exfmt:audio:wav-pcm", "exfmt:audio:bwf-pcm", "RIFF/WAVE PCM", "Broadcast WAVE PCM"),
    Spec("bwf-pcm-to-wav-pcm", "Bwf", "Wav", "exfmt:audio:bwf-pcm", "exfmt:audio:wav-pcm", "Broadcast WAVE PCM", "RIFF/WAVE PCM"),
)

RAW_SPECS = (
    RawSpec("raw-pcm-trim", "Trim", "trim", "Frame-exact trimming of parameterized raw interleaved PCM."),
    RawSpec("raw-pcm-reverse", "Reverse", "reverse", "Frame-order reversal of parameterized raw interleaved PCM."),
    RawSpec("raw-pcm-channel-map", "ChannelMap", "channel-map", "Explicit projection, duplication and reordering of raw PCM channels."),
    RawSpec("raw-pcm-endian-signedness-normalize", "Normalize", "normalize", "Byte-order and integer signedness normalization of raw PCM samples."),
)


def json_text(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, indent=2) + "\n"


def manifest(spec: Spec) -> str:
    defaults = {
        "strict_header_consistency": True,
        "buffer_size": 65536,
        "max_channels": 256,
    }
    value = {
        "capsule_id": f"capsule:{spec.name}",
        "version": "0.1.0",
        "name": spec.name,
        "summary": f"Zero-dependency integer PCM conversion from {spec.source_label} to {spec.target_label}.",
        "taxonomy": {
            "domain": "audio",
            "primary_ir": "ir:sampled-audio",
            "secondary_irs": ["ir:container-graph"],
            "operator_kind": "convert",
            "operator_role": "direct",
        },
        "license": {
            "expression": "PolyForm-Noncommercial-1.0.0",
            "file": "LICENSE",
            "commercial_authorization_required": True,
        },
        "repository": f"https://github.com/EthanBird/EverythingX/tree/main/capsules/audio/sampled-audio/direct/{spec.name}",
        "independence": {
            "standalone_cargo_build": True,
            "everythingx_optional": True,
            "external_path_dependencies": False,
            "copy_out_tested": True,
        },
        "conversion": {
            "source": [spec.source_format],
            "target": [spec.target_format],
            "arity": {"inputs": {"min": 1, "max": 1}, "outputs": {"min": 1, "max": 1}},
            "scope": [
                "8/16/24/32-bit interleaved integer PCM",
                "Exact integer sample rates and frame-aligned data",
                f"Native parsing of {spec.source_label}",
                f"Native emission of {spec.target_label}",
            ],
            "out_of_scope": [
                "Floating point, compressed, planar or DSD audio",
                "Arbitrary metadata migration beyond target-profile structural requirements",
                "Sample-rate, channel-layout or bit-depth conversion",
            ],
        },
        "api": {
            "language": "rust",
            "crate": spec.name,
            "entrypoint": "convert",
            "owns_options_error_report": True,
            "streaming": True,
            "seek_required": True,
        },
        "defaults": {
            "runnable": True,
            "strategy": "pcm-exact",
            "backend": "native-portable",
            "options": defaults,
            "policy": "strict",
        },
        "strategies": [{
            "id": "pcm-exact",
            "guarantees": [
                "Integer PCM sample levels, frame order and channel order are retained",
                "Container sizes, padding and byte order are generated natively",
                "Eight-bit signedness is normalized to the target convention",
            ],
            "tradeoffs": ["Non-structural source metadata is not migrated in version 0.1"],
        }],
        "backends": [{"id": "native-portable", "tier": "native-portable", "default": True, "dependencies": []}],
        "validation": {
            "specifications": [spec.source_label, spec.target_label],
            "conformance": ["src/lib.rs unit tests"],
            "differential": [],
            "properties": [
                "16-bit PCM round-trips through independent target parsing",
                "8-bit sample levels survive signedness convention changes",
                "Strict defaults reject trailing or inconsistent container sizes",
            ],
            "regression": ["Wrong signatures are rejected", "PCM frames remain aligned"],
            "fuzz": ["Planned: source chunk graph, size and fragmented-reader campaigns"],
            "benchmarks": ["Planned: widths, channels and multi-gigabyte streaming corpus"],
        },
        "security": {
            "accepts_untrusted_input": True,
            "limits": [
                "buffer_size is bounded to 16 MiB",
                "channels default to a maximum of 256",
                "all chunk offsets and target sizes use checked arithmetic",
            ],
            "known_risks": ["Seek or output I/O failure can occur after the target header is written"],
        },
    }
    return json_text(value)


def adapter_manifest(spec: Spec) -> str:
    defaults = {"strict_header_consistency": True, "buffer_size": 65536, "max_channels": 256}
    value = {
        "adapter_id": f"adapter:{spec.name}-static",
        "version": "0.1.0",
        "capsule": {"id": f"capsule:{spec.name}", "version_requirement": "^0.1.0"},
        "protocol": {"name": "everythingx-adapter-protocol", "version_requirement": "0.1"},
        "transport": {"kind": "static-rust", "entrypoint": "GeneratedPcmAdapter"},
        "capabilities": [{
            "capability_id": f"capability:{spec.name}/pcm-exact/native-portable",
            "capsule_entrypoint": "convert",
            "strategy": "pcm-exact",
            "backend": "native-portable",
            "inputs": [spec.source_format],
            "outputs": [spec.target_format],
            "preconditions": [f"Input is supported integer PCM in {spec.source_label}"],
            "effects": [f"Output is {spec.target_label}"],
            "invariants": ["Sample rate", "Frame count", "Channel count", "PCM sample sequence"],
            "computability": "semantic_lossless",
            "loss": {"payload": "none", "temporal": "none", "structure": "normalized", "metadata": "unbounded"},
            "default_options": defaults,
            "defaults_are_runnable": True,
            "execution": {"streaming": False, "seek_required": False, "cost_evidence": []},
            "report_mapping": {"unknown_fields_are_preserved": True, "rules": ["PCM facts map to capsule_report", "static Adapter buffers forward-only protocol input"]},
        }],
    }
    return json_text(value)


def adapter_source(spec: Spec) -> str:
    module = spec.name.replace("-", "_")
    adapter_type = "GeneratedPcmAdapter"
    return f'''#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::{{self, Cursor, Read, Write}};
use everythingx_protocol::{{AdapterError, AdapterErrorKind, AdapterHandshake, CapabilityDescriptor, CapsuleIdentity, InvocationRequest, InvocationResult, InvocationStatus, LossLevel, Measurements, ProtocolVersion, Provenance, StaticAdapter}};
use {module}::{{Error as CapsuleError, Options}};

pub const ADAPTER_ID:&str="adapter:{spec.name}-static";
pub const CAPABILITY_ID:&str="capability:{spec.name}/pcm-exact/native-portable";
pub struct {adapter_type};
fn defaults()->BTreeMap<String,String>{{BTreeMap::from([("buffer_size".into(),"65536".into()),("max_channels".into(),"256".into()),("strict_header_consistency".into(),"true".into())])}}
fn descriptor()->CapabilityDescriptor{{CapabilityDescriptor{{capability_id:CAPABILITY_ID.into(),source_formats:vec!["{spec.source_format}".into()],target_formats:vec!["{spec.target_format}".into()],strategy:"pcm-exact".into(),backend:"native-portable".into(),default_options:defaults(),defaults_are_runnable:true,streaming:false,seek_required:false}}}}
struct Limited<'a>{{inner:&'a mut dyn Write,remaining:u64,exceeded:bool}}
impl Write for Limited<'_>{{fn write(&mut self,b:&[u8])->io::Result<usize>{{if b.len()as u64>self.remaining{{self.exceeded=true;return Err(io::Error::other("output budget exceeded"));}}let n=self.inner.write(b)?;self.remaining-=n as u64;Ok(n)}}fn flush(&mut self)->io::Result<()>{{self.inner.flush()}}}}
impl StaticAdapter for {adapter_type}{{
fn handshake(&self)->AdapterHandshake{{AdapterHandshake{{protocol:ProtocolVersion::CURRENT,adapter_id:ADAPTER_ID.into(),adapter_version:"0.1.0".into(),capsule:CapsuleIdentity{{id:"capsule:{spec.name}".into(),version:"0.1.0".into(),content_hash:None}},capabilities:vec![descriptor()]}}}}
fn invoke(&self,request:&InvocationRequest,input:&mut dyn Read,output:&mut dyn Write)->Result<InvocationResult,AdapterError>{{
if request.capability_id!=CAPABILITY_ID{{return Err(AdapterError::new(AdapterErrorKind::UnsupportedCapability,"unsupported capability"));}}if request.options!=defaults(){{return Err(AdapterError::new(AdapterErrorKind::InvalidOptions,"version 0.1 static Adapter accepts its declared defaults"));}}
let reserve=65_536u64;let limit=request.resource_budget.max_memory_bytes.saturating_sub(reserve);let mut bytes=Vec::new();input.take(limit.saturating_add(1)).read_to_end(&mut bytes).map_err(|e|AdapterError::new(AdapterErrorKind::Io,e.to_string()))?;if bytes.len()as u64>limit{{return Err(AdapterError::new(AdapterErrorKind::ResourceLimit,"input exceeds memory budget"));}}let input_memory=bytes.len()as u64;let mut cursor=Cursor::new(bytes);let mut limited=Limited{{inner:output,remaining:request.resource_budget.max_output_bytes,exceeded:false}};
let report={module}::convert(&mut cursor,&mut limited,&Options::default()).map_err(|error|match error{{CapsuleError::Io(e)if limited.exceeded=>AdapterError::new(AdapterErrorKind::ResourceLimit,e.to_string()),CapsuleError::Io(e)=>AdapterError::new(AdapterErrorKind::Io,e.to_string()),other=>AdapterError::new(AdapterErrorKind::InvalidInput,other.to_string())}})?;let handshake=self.handshake();
Ok(InvocationResult{{status:InvocationStatus::Succeeded,effects:BTreeMap::from([("format".into(),"{spec.target_format}".into())]),losses:BTreeMap::from([("payload".into(),LossLevel::None),("temporal".into(),LossLevel::None),("structure".into(),LossLevel::Normalized),("metadata".into(),LossLevel::Unbounded)]),measurements:Measurements{{input_bytes:Some(report.input_bytes),output_bytes:Some(report.output_bytes),peak_memory_bytes:Some(input_memory+report.peak_working_memory_bytes),..Measurements::default()}},capsule_report:BTreeMap::from([("channels".into(),report.channels.to_string()),("sample_rate".into(),report.sample_rate.to_string()),("container_bits_per_sample".into(),report.container_bits_per_sample.to_string()),("valid_bits_per_sample".into(),report.valid_bits_per_sample.to_string()),("sample_frames".into(),report.sample_frames.to_string())]),warnings:report.warnings,provenance:Provenance{{capsule:handshake.capsule,adapter_id:handshake.adapter_id,adapter_version:handshake.adapter_version,capability_id:CAPABILITY_ID.into(),strategy:"pcm-exact".into(),backend:"native-portable".into(),effective_options:defaults()}}}})
}}}}
#[cfg(test)]mod tests{{use super::*;use everythingx_kernel::Kernel;#[test]fn kernel_invokes_runnable_defaults(){{let mut kernel=Kernel::default();kernel.register(Box::new({adapter_type})).unwrap();let fixture={module}::conformance_fixture();let mut input=&fixture[..];let mut output=Vec::new();let result=kernel.invoke_defaults(ADAPTER_ID,CAPABILITY_ID,&mut input,&mut output).unwrap();assert_eq!(result.status,InvocationStatus::Succeeded);assert!(!output.is_empty());}}}}
'''


def files_for(spec: Spec, template: str) -> dict[Path, str]:
    root = CAPSULES / spec.name
    cargo = f'''[package]
name = "{spec.name}"
version = "0.1.0"
edition = "2024"
publish = false
license-file = "LICENSE"
description = "Standalone zero-dependency {spec.source_label} to {spec.target_label} PCM converter"

[lib]
path = "src/lib.rs"

[dependencies]
'''
    lock = f'''# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = "{spec.name}"
version = "0.1.0"
'''
    adapter_cargo = f'''[package]
name = "everythingx-adapter-{spec.name}"
version = "0.1.0"
edition = "2024"
publish = false

[lib]
path = "src/lib.rs"

[dependencies]
everythingx-protocol = {{ path = "../../../../../../../kernel/ex-protocol" }}
{spec.name} = {{ path = "../.." }}

[dev-dependencies]
everythingx-kernel = {{ path = "../../../../../../../kernel/ex-kernel" }}
'''
    readme = f'''# {spec.name}

Independent, zero-dependency Rust conversion from {spec.source_label} to
{spec.target_label}. The crate parses the source container, validates integer
PCM structure, emits the target container natively and streams sample frames
through a bounded buffer. Copy this directory elsewhere and it remains a
complete library with runnable defaults and unit tests.

Version 0.1 supports interleaved integer PCM at 8/16/24/32 bits. It preserves
sample rate, channel count, frame order and sample levels; unsupported metadata
is reported as a declared boundary rather than silently advertised as retained.
'''
    source = template.replace("__SOURCE__", spec.source_profile).replace("__TARGET__", spec.target_profile)
    return {
        root / "Cargo.toml": cargo,
        root / "Cargo.lock": lock,
        root / "LICENSE": NOTICE,
        root / "README.md": readme,
        root / "capsule.json": manifest(spec),
        root / "src" / "lib.rs": source,
        root / "benches" / "README.md": "# Benchmarks\n\nPlanned reproducible width, channel and large-stream corpus.\n",
        root / "fuzz" / "README.md": "# Fuzzing\n\nPlanned source-container chunk, size and fragmented-reader campaigns.\n",
        root / "everythingx" / "adapter.json": adapter_manifest(spec),
        root / "everythingx" / "adapter" / "Cargo.toml": adapter_cargo,
        root / "everythingx" / "adapter" / "src" / "lib.rs": adapter_source(spec),
    }


def raw_defaults(spec: RawSpec) -> dict[str, object]:
    values: dict[str, object] = {
        "channels": 2 if spec.mode == "ChannelMap" else 1,
        "bits_per_sample": 16,
        "input_endianness": "little",
        "output_endianness": "big" if spec.mode == "Normalize" else "little",
        "input_encoding": "signed",
        "output_encoding": "signed",
        "start_frame": 0,
        "frame_count": "all",
        "channel_map": "1,0" if spec.mode == "ChannelMap" else "0",
        "buffer_size": 65536,
        "max_channels": 256,
    }
    return values


def raw_manifest(spec: RawSpec) -> str:
    defaults = raw_defaults(spec)
    value = {
        "capsule_id": f"capsule:{spec.name}",
        "version": "0.1.0",
        "name": spec.name,
        "summary": spec.summary,
        "taxonomy": {
            "domain": "audio",
            "primary_ir": "ir:sampled-audio",
            "secondary_irs": ["ir:sequence"],
            "operator_kind": spec.operator_kind,
            "operator_role": "transform",
        },
        "license": {"expression": "PolyForm-Noncommercial-1.0.0", "file": "LICENSE", "commercial_authorization_required": True},
        "repository": f"https://github.com/EthanBird/EverythingX/tree/main/capsules/audio/sampled-audio/transform/{spec.name}",
        "independence": {"standalone_cargo_build": True, "everythingx_optional": True, "external_path_dependencies": False, "copy_out_tested": True},
        "conversion": {
            "source": ["exfmt:audio:raw-pcm"],
            "target": ["exfmt:audio:raw-pcm"],
            "arity": {"inputs": {"min": 1, "max": 1}, "outputs": {"min": 1, "max": 1}},
            "scope": ["8/16/24/32-bit interleaved integer PCM", "Explicit parameter-owned raw PCM semantics", spec.summary],
            "out_of_scope": ["Floating point, planar, companded or DSD audio", "Automatic inference of headerless PCM parameters"],
        },
        "api": {"language": "rust", "crate": spec.name, "entrypoint": "convert", "owns_options_error_report": True, "streaming": spec.mode != "Reverse", "seek_required": True},
        "defaults": {"runnable": True, "strategy": "frame-exact", "backend": "native-portable", "options": defaults, "policy": "strict"},
        "strategies": [{
            "id": "frame-exact",
            "guarantees": ["Only complete frames are accepted", "Integer sample bytes are transformed deterministically", "All parameters and output frame counts are reported"],
            "tradeoffs": ["Raw PCM interpretation depends on explicit parameters"],
        }],
        "backends": [{"id": "native-portable", "tier": "native-portable", "default": True, "dependencies": []}],
        "validation": {
            "specifications": ["Parameterized interleaved integer PCM contract"],
            "conformance": ["src/lib.rs unit tests"],
            "differential": [],
            "properties": ["Partial frames are rejected", "Default and custom semantics are independently asserted"],
            "regression": ["Buffer limits are validated", "Frame/channel boundaries remain intact"],
            "fuzz": ["Planned: parameter, alignment, length and reader-fragmentation campaigns"],
            "benchmarks": ["Planned: widths, channels and large-stream corpus"],
        },
        "security": {
            "accepts_untrusted_input": True,
            "limits": ["buffer_size is bounded to 16 MiB", "channels default to a maximum of 256", "all frame and byte arithmetic is checked"],
            "known_risks": ["Output I/O failure can occur after partial output"],
        },
    }
    return json_text(value)


def raw_adapter_manifest(spec: RawSpec) -> str:
    value = {
        "adapter_id": f"adapter:{spec.name}-static",
        "version": "0.1.0",
        "capsule": {"id": f"capsule:{spec.name}", "version_requirement": "^0.1.0"},
        "protocol": {"name": "everythingx-adapter-protocol", "version_requirement": "0.1"},
        "transport": {"kind": "static-rust", "entrypoint": "GeneratedRawPcmAdapter"},
        "capabilities": [{
            "capability_id": f"capability:{spec.name}/frame-exact/native-portable",
            "capsule_entrypoint": "convert",
            "strategy": "frame-exact",
            "backend": "native-portable",
            "inputs": ["exfmt:audio:raw-pcm"],
            "outputs": ["exfmt:audio:raw-pcm"],
            "preconditions": ["Input matches the explicit raw PCM parameters"],
            "effects": [spec.summary],
            "invariants": ["Complete integer sample values", "Declared frame semantics"],
            "computability": "exact",
            "loss": {"payload": "none", "temporal": "normalized", "structure": "normalized", "metadata": "not-present"},
            "default_options": raw_defaults(spec),
            "defaults_are_runnable": True,
            "execution": {"streaming": False, "seek_required": False, "cost_evidence": []},
            "report_mapping": {"unknown_fields_are_preserved": True, "rules": ["Frame and channel counts map to capsule_report", "static Adapter buffers protocol input for seek"]},
        }],
    }
    return json_text(value)


def raw_adapter_source(spec: RawSpec) -> str:
    module = spec.name.replace("-", "_")
    pairs = ",".join(f'("{key}".into(),"{str(value).lower() if isinstance(value, bool) else value}".into())' for key, value in sorted(raw_defaults(spec).items()))
    return f'''#![forbid(unsafe_code)]
use std::collections::BTreeMap;use std::io::{{self,Cursor,Read,Write}};
use everythingx_protocol::{{AdapterError,AdapterErrorKind,AdapterHandshake,CapabilityDescriptor,CapsuleIdentity,InvocationRequest,InvocationResult,InvocationStatus,LossLevel,Measurements,ProtocolVersion,Provenance,StaticAdapter}};
use {module}::{{Error as CapsuleError,Options}};
pub const ADAPTER_ID:&str="adapter:{spec.name}-static";pub const CAPABILITY_ID:&str="capability:{spec.name}/frame-exact/native-portable";pub struct GeneratedRawPcmAdapter;
fn defaults()->BTreeMap<String,String>{{BTreeMap::from([{pairs}])}}
fn descriptor()->CapabilityDescriptor{{CapabilityDescriptor{{capability_id:CAPABILITY_ID.into(),source_formats:vec!["exfmt:audio:raw-pcm".into()],target_formats:vec!["exfmt:audio:raw-pcm".into()],strategy:"frame-exact".into(),backend:"native-portable".into(),default_options:defaults(),defaults_are_runnable:true,streaming:false,seek_required:false}}}}
struct Limited<'a>{{inner:&'a mut dyn Write,remaining:u64,exceeded:bool}}impl Write for Limited<'_>{{fn write(&mut self,b:&[u8])->io::Result<usize>{{if b.len()as u64>self.remaining{{self.exceeded=true;return Err(io::Error::other("output budget exceeded"));}}let n=self.inner.write(b)?;self.remaining-=n as u64;Ok(n)}}fn flush(&mut self)->io::Result<()>{{self.inner.flush()}}}}
impl StaticAdapter for GeneratedRawPcmAdapter{{fn handshake(&self)->AdapterHandshake{{AdapterHandshake{{protocol:ProtocolVersion::CURRENT,adapter_id:ADAPTER_ID.into(),adapter_version:"0.1.0".into(),capsule:CapsuleIdentity{{id:"capsule:{spec.name}".into(),version:"0.1.0".into(),content_hash:None}},capabilities:vec![descriptor()]}}}}fn invoke(&self,request:&InvocationRequest,input:&mut dyn Read,output:&mut dyn Write)->Result<InvocationResult,AdapterError>{{if request.capability_id!=CAPABILITY_ID{{return Err(AdapterError::new(AdapterErrorKind::UnsupportedCapability,"unsupported capability"));}}if request.options!=defaults(){{return Err(AdapterError::new(AdapterErrorKind::InvalidOptions,"version 0.1 static Adapter accepts its declared defaults"));}}let limit=request.resource_budget.max_memory_bytes.saturating_sub(131_072);let mut bytes=Vec::new();input.take(limit.saturating_add(1)).read_to_end(&mut bytes).map_err(|e|AdapterError::new(AdapterErrorKind::Io,e.to_string()))?;if bytes.len()as u64>limit{{return Err(AdapterError::new(AdapterErrorKind::ResourceLimit,"input exceeds memory budget"));}}let memory=bytes.len()as u64;let mut cursor=Cursor::new(bytes);let mut limited=Limited{{inner:output,remaining:request.resource_budget.max_output_bytes,exceeded:false}};let report={module}::convert(&mut cursor,&mut limited,&Options::default()).map_err(|e|match e{{CapsuleError::Io(io)if limited.exceeded=>AdapterError::new(AdapterErrorKind::ResourceLimit,io.to_string()),CapsuleError::Io(io)=>AdapterError::new(AdapterErrorKind::Io,io.to_string()),other=>AdapterError::new(AdapterErrorKind::InvalidInput,other.to_string())}})?;let handshake=self.handshake();Ok(InvocationResult{{status:InvocationStatus::Succeeded,effects:BTreeMap::from([("format".into(),"exfmt:audio:raw-pcm".into())]),losses:BTreeMap::from([("payload".into(),LossLevel::None),("temporal".into(),LossLevel::Normalized),("structure".into(),LossLevel::Normalized),("metadata".into(),LossLevel::None)]),measurements:Measurements{{input_bytes:Some(report.input_bytes),output_bytes:Some(report.output_bytes),peak_memory_bytes:Some(memory+report.peak_working_memory_bytes),..Measurements::default()}},capsule_report:BTreeMap::from([("input_frames".into(),report.input_frames.to_string()),("output_frames".into(),report.output_frames.to_string()),("input_channels".into(),report.input_channels.to_string()),("output_channels".into(),report.output_channels.to_string()),("bits_per_sample".into(),report.bits_per_sample.to_string())]),warnings:report.warnings,provenance:Provenance{{capsule:handshake.capsule,adapter_id:handshake.adapter_id,adapter_version:handshake.adapter_version,capability_id:CAPABILITY_ID.into(),strategy:"frame-exact".into(),backend:"native-portable".into(),effective_options:defaults()}}}})}}}}
#[cfg(test)]mod tests{{use super::*;use everythingx_kernel::Kernel;#[test]fn kernel_invokes_runnable_defaults(){{let mut kernel=Kernel::default();kernel.register(Box::new(GeneratedRawPcmAdapter)).unwrap();let fixture={module}::conformance_fixture();let mut input=&fixture[..];let mut output=Vec::new();let result=kernel.invoke_defaults(ADAPTER_ID,CAPABILITY_ID,&mut input,&mut output).unwrap();assert_eq!(result.status,InvocationStatus::Succeeded);assert!(!output.is_empty());}}}}
'''


def files_for_raw(spec: RawSpec, template: str) -> dict[Path, str]:
    root = RAW_CAPSULES / spec.name
    cargo = f'''[package]
name = "{spec.name}"
version = "0.1.0"
edition = "2024"
publish = false
license-file = "LICENSE"
description = "Standalone zero-dependency {spec.summary}"

[lib]
path = "src/lib.rs"

[dependencies]
'''
    lock = f'''# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = "{spec.name}"
version = "0.1.0"
'''
    adapter_cargo = f'''[package]
name = "everythingx-adapter-{spec.name}"
version = "0.1.0"
edition = "2024"
publish = false

[lib]
path = "src/lib.rs"

[dependencies]
everythingx-protocol = {{ path = "../../../../../../../kernel/ex-protocol" }}
{spec.name} = {{ path = "../.." }}

[dev-dependencies]
everythingx-kernel = {{ path = "../../../../../../../kernel/ex-kernel" }}
'''
    readme = f'''# {spec.name}

{spec.summary} This is an independent, zero-dependency Rust crate. Raw PCM
interpretation is explicit in `Options`; defaults are runnable and tests cover
alignment failures, parameter validation, default behavior and custom behavior.
'''
    source = template.replace("__MODE__", spec.mode)
    return {
        root / "Cargo.toml": cargo,
        root / "Cargo.lock": lock,
        root / "LICENSE": NOTICE,
        root / "README.md": readme,
        root / "capsule.json": raw_manifest(spec),
        root / "src" / "lib.rs": source,
        root / "benches" / "README.md": "# Benchmarks\n\nPlanned reproducible width, channel and large-stream corpus.\n",
        root / "fuzz" / "README.md": "# Fuzzing\n\nPlanned parameter, alignment, length and fragmented-reader campaigns.\n",
        root / "everythingx" / "adapter.json": raw_adapter_manifest(spec),
        root / "everythingx" / "adapter" / "Cargo.toml": adapter_cargo,
        root / "everythingx" / "adapter" / "src" / "lib.rs": raw_adapter_source(spec),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    template = TEMPLATE.read_text(encoding="utf-8")
    raw_template = RAW_TEMPLATE.read_text(encoding="utf-8")
    expected: dict[Path, str] = {}
    for spec in SPECS:
        expected.update(files_for(spec, template))
    for spec in RAW_SPECS:
        expected.update(files_for_raw(spec, raw_template))
    stale = [path for path, content in expected.items() if not path.is_file() or path.read_text(encoding="utf-8") != content]
    if args.check:
        if stale:
            print("PCM Wave A scaffold is stale:")
            for path in stale:
                print(path.relative_to(ROOT))
            return 1
        print(f"PCM Wave A scaffold is current ({len(SPECS)} container + {len(RAW_SPECS)} raw PCM Capsules)")
        return 0
    for path, content in expected.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
    print(f"materialized {len(SPECS) + len(RAW_SPECS)} independent PCM Wave A Capsules")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
