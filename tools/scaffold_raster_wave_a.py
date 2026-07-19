#!/usr/bin/env python3
"""Materialize the complete five-format Raster Wave A directed mesh."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TEMPLATE = ROOT / "tools" / "templates" / "raster_wave_a_capsule.rs"
CAPSULES = ROOT / "capsules" / "image" / "raster" / "direct"
NOTICE = """Required Notice: Copyright © 2026 EthanBird. All rights reserved.

This Conversion Capsule is licensed under the PolyForm Noncommercial License
1.0.0 included at the EverythingX repository root:
https://polyformproject.org/licenses/noncommercial/1.0.0
"""


@dataclass(frozen=True)
class Representation:
    slug: str
    profile: str
    format_id: str
    label: str
    specification: str


REPRESENTATIONS = (
    Representation("bmp", "Bmp", "exfmt:image:bmp-family", "Windows BMP raster", "Microsoft Windows bitmap structures"),
    Representation("tga", "Tga", "exfmt:image:tga-raster", "Truevision TGA raster", "Truevision TGA File Format 2.0"),
    Representation("qoi", "Qoi", "exfmt:image:qoi", "Quite OK Image", "QOI Specification 1.0"),
    Representation("ppm", "Ppm", "exfmt:image:ppm", "Netpbm PPM", "Netpbm PPM specification"),
    Representation("pam", "Pam", "exfmt:image:pam", "Netpbm PAM", "Netpbm PAM specification"),
)


@dataclass(frozen=True)
class Spec:
    source: Representation
    target: Representation

    @property
    def name(self) -> str:
        return f"{self.source.slug}-to-{self.target.slug}"


SPECS = tuple(
    Spec(source, target)
    for source in REPRESENTATIONS
    for target in REPRESENTATIONS
    if source != target
)


def json_text(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, indent=2) + "\n"


def capability_id(spec: Spec) -> str:
    return f"capability:{spec.name}/rgba8-code-value-exact/native-portable"


def performance_evidence(spec: Spec) -> list[str]:
    baseline = ROOT / "registry" / "performance" / "baseline.json"
    if not baseline.is_file():
        return []
    measured = {
        row.get("capability_id")
        for row in json.loads(baseline.read_text(encoding="utf-8")).get("capabilities", [])
    }
    identifier = capability_id(spec)
    return [f"registry/performance/baseline.json#{identifier}"] if identifier in measured else []


def defaults() -> dict[str, object]:
    return {
        "allow_sample_scaling": False,
        "max_input_bytes": 536870912,
        "max_pixels": 100000000,
        "ppm_alpha": "reject",
        "preserve_unmarked_bmp_alpha": False,
        "strict_trailing_data": True,
        "tga_rle": True,
    }


def manifest(spec: Spec) -> str:
    evidence = performance_evidence(spec)
    value = {
        "capsule_id": f"capsule:{spec.name}",
        "version": "0.1.0",
        "name": spec.name,
        "summary": f"Zero-dependency RGBA8 code-value conversion from {spec.source.label} to {spec.target.label}.",
        "taxonomy": {
            "domain": "image",
            "primary_ir": "ir:raster",
            "secondary_irs": ["ir:container-graph"],
            "operator_kind": "convert",
            "operator_role": "direct",
        },
        "license": {
            "expression": "PolyForm-Noncommercial-1.0.0",
            "file": "LICENSE",
            "commercial_authorization_required": True,
        },
        "repository": f"https://github.com/EthanBird/EverythingX/tree/main/capsules/image/raster/direct/{spec.name}",
        "independence": {
            "standalone_cargo_build": True,
            "everythingx_optional": True,
            "external_path_dependencies": False,
            "copy_out_tested": True,
        },
        "conversion": {
            "source": [spec.source.format_id],
            "target": [spec.target.format_id],
            "arity": {"inputs": {"min": 1, "max": 1}, "outputs": {"min": 1, "max": 1}},
            "scope": [
                "Single-image RGBA8 or RGB8 visual rasters",
                "Top-to-bottom normalized coordinates and unassociated alpha",
                f"Native parsing of {spec.source.label}",
                f"Native emission of {spec.target.label}",
                "PPM/PAM MAXVAL 255 under runnable defaults",
            ],
            "out_of_scope": [
                "ICC transforms, chromatic adaptation and inferred transfer-function conversion",
                "Palette preservation and arbitrary metadata migration",
                "Animation, multi-image sequences, HDR and sample depths above eight bits under defaults",
                "Color-mapped TGA and BMP compression outside the declared Wave A subset",
            ],
        },
        "api": {
            "language": "rust",
            "crate": spec.name,
            "entrypoint": "convert",
            "owns_options_error_report": True,
            "streaming": False,
            "seek_required": False,
        },
        "defaults": {
            "runnable": True,
            "strategy": "rgba8-code-value-exact",
            "backend": "native-portable",
            "options": defaults(),
            "policy": "strict",
        },
        "strategies": [{
            "id": "rgba8-code-value-exact",
            "guarantees": [
                "Accepted RGB/RGBA eight-bit channel code values and pixel coordinates are retained exactly",
                "Non-opaque pixels are rejected rather than silently discarded when the target is PPM",
                "Container framing, row order, RLE and checks required by each endpoint are generated natively",
            ],
            "tradeoffs": [
                "Carrier representation and unsupported metadata are normalized",
                "Code-value preservation does not assert equivalence between unspecified source colorimetry",
            ],
        }],
        "backends": [{"id": "native-portable", "tier": "native-portable", "default": True, "dependencies": []}],
        "validation": {
            "specifications": [spec.source.specification, spec.target.specification],
            "conformance": ["src/lib.rs unit tests"],
            "differential": [],
            "properties": [
                "Opaque conformance pixels are decoded independently from emitted target bytes",
                "Alpha is either preserved exactly or rejected by the target contract",
                "Dimensions and pixel allocations use checked arithmetic and an explicit limit",
            ],
            "regression": [
                "Malformed signatures are rejected",
                "Pixel limits are enforced before allocation",
                "TGA origin and RLE are normalized without coordinate changes",
                "QOI index, diff, luma, run, RGB and RGBA chunks share one state model",
            ],
            "fuzz": ["Planned: per-source header, dimensions, packet stream, truncation and trailing-data campaigns"],
            "benchmarks": evidence or ["Pending controlled exbench baseline"],
        },
        "security": {
            "accepts_untrusted_input": True,
            "limits": [
                "max_pixels defaults to 100 million",
                "max_input_bytes defaults to 512 MiB",
                "all sizes, offsets and allocations use checked arithmetic",
                "the complete source is validated before target bytes are committed",
            ],
            "known_risks": ["Output I/O failure can occur while committing a fully encoded target buffer"],
        },
    }
    return json_text(value)


def adapter_manifest(spec: Spec) -> str:
    evidence = performance_evidence(spec)
    target_ppm = spec.target.slug == "ppm"
    value = {
        "adapter_id": f"adapter:{spec.name}-static",
        "version": "0.1.0",
        "capsule": {"id": f"capsule:{spec.name}", "version_requirement": "^0.1.0"},
        "protocol": {"name": "everythingx-adapter-protocol", "version_requirement": "0.1"},
        "transport": {"kind": "static-rust", "entrypoint": "GeneratedRasterAdapter"},
        "capabilities": [{
            "capability_id": capability_id(spec),
            "capsule_entrypoint": "convert",
            "strategy": "rgba8-code-value-exact",
            "backend": "native-portable",
            "inputs": [spec.source.format_id],
            "outputs": [spec.target.format_id],
            "preconditions": [
                "Input belongs to the declared Wave A parser subset",
                "PPM/PAM samples use MAXVAL 255 under defaults",
                "RGB channel code values have compatible intended color semantics",
            ] + (["All input pixels are opaque under the default PPM alpha policy"] if target_ppm else []),
            "effects": [f"Output is a normalized {spec.target.label} single image"],
            "invariants": ["Width", "Height", "Pixel coordinates", "Accepted RGB/RGBA8 code values"],
            "computability": "conditional_exact",
            "loss": {"pixels": "none", "coordinates": "none", "structure": "normalized", "metadata": "unbounded", "color-semantics": "conditional"},
            "default_options": defaults(),
            "defaults_are_runnable": True,
            "execution": {"streaming": False, "seek_required": False, "cost_evidence": evidence},
            "report_mapping": {
                "unknown_fields_are_preserved": True,
                "rules": ["Dimensions, channels and alpha facts map to capsule_report", "static Adapter buffers protocol input and enforces output budget"],
            },
        }],
    }
    return json_text(value)


def adapter_source(spec: Spec) -> str:
    module = spec.name.replace("-", "_")
    default_pairs = ",".join(
        f'("{key}".into(),"{str(value).lower() if isinstance(value, bool) else value}".into())'
        for key, value in sorted(defaults().items())
    )
    return f'''#![forbid(unsafe_code)]
use std::collections::BTreeMap;use std::io::{{self,Read,Write}};
use everythingx_protocol::{{AdapterError,AdapterErrorKind,AdapterHandshake,CapabilityDescriptor,CapsuleIdentity,InvocationRequest,InvocationResult,InvocationStatus,LossLevel,Measurements,ProtocolVersion,Provenance,StaticAdapter}};
use {module}::{{Error as CapsuleError,Options}};
pub const ADAPTER_ID:&str="adapter:{spec.name}-static";pub const CAPABILITY_ID:&str="{capability_id(spec)}";pub struct GeneratedRasterAdapter;
fn defaults()->BTreeMap<String,String>{{BTreeMap::from([{default_pairs}])}}
fn descriptor()->CapabilityDescriptor{{CapabilityDescriptor{{capability_id:CAPABILITY_ID.into(),source_formats:vec!["{spec.source.format_id}".into()],target_formats:vec!["{spec.target.format_id}".into()],strategy:"rgba8-code-value-exact".into(),backend:"native-portable".into(),default_options:defaults(),defaults_are_runnable:true,streaming:false,seek_required:false}}}}
struct Limited<'a>{{inner:&'a mut dyn Write,remaining:u64,exceeded:bool}}impl Write for Limited<'_>{{fn write(&mut self,b:&[u8])->io::Result<usize>{{if b.len()as u64>self.remaining{{self.exceeded=true;return Err(io::Error::other("output budget exceeded"));}}let n=self.inner.write(b)?;self.remaining-=n as u64;Ok(n)}}fn flush(&mut self)->io::Result<()>{{self.inner.flush()}}}}
impl StaticAdapter for GeneratedRasterAdapter{{fn handshake(&self)->AdapterHandshake{{AdapterHandshake{{protocol:ProtocolVersion::CURRENT,adapter_id:ADAPTER_ID.into(),adapter_version:"0.1.0".into(),capsule:CapsuleIdentity{{id:"capsule:{spec.name}".into(),version:"0.1.0".into(),content_hash:None}},capabilities:vec![descriptor()]}}}}fn invoke(&self,request:&InvocationRequest,input:&mut dyn Read,output:&mut dyn Write)->Result<InvocationResult,AdapterError>{{if request.capability_id!=CAPABILITY_ID{{return Err(AdapterError::new(AdapterErrorKind::UnsupportedCapability,"unsupported capability"));}}if request.options!=defaults(){{return Err(AdapterError::new(AdapterErrorKind::InvalidOptions,"version 0.1 static Adapter accepts its declared defaults"));}}let limit=request.resource_budget.max_memory_bytes/4;let mut bytes=Vec::new();input.take(limit.saturating_add(1)).read_to_end(&mut bytes).map_err(|e|AdapterError::new(AdapterErrorKind::Io,e.to_string()))?;if bytes.len()as u64>limit{{return Err(AdapterError::new(AdapterErrorKind::ResourceLimit,"input exceeds Adapter memory share"));}}let adapter_memory=bytes.len()as u64;let mut source=&bytes[..];let mut limited=Limited{{inner:output,remaining:request.resource_budget.max_output_bytes,exceeded:false}};let report={module}::convert(&mut source,&mut limited,&Options::default()).map_err(|error|match error{{CapsuleError::Io(io)if limited.exceeded=>AdapterError::new(AdapterErrorKind::ResourceLimit,io.to_string()),CapsuleError::Io(io)=>AdapterError::new(AdapterErrorKind::Io,io.to_string()),limited_error@(CapsuleError::InputTooLarge{{..}}|CapsuleError::PixelLimitExceeded{{..}})=>AdapterError::new(AdapterErrorKind::ResourceLimit,limited_error.to_string()),other=>AdapterError::new(AdapterErrorKind::InvalidInput,other.to_string())}})?;let peak=adapter_memory.saturating_add(report.peak_working_memory_bytes);if peak>request.resource_budget.max_memory_bytes{{return Err(AdapterError::new(AdapterErrorKind::ResourceLimit,"reported peak memory exceeds request budget"));}}let handshake=self.handshake();Ok(InvocationResult{{status:InvocationStatus::Succeeded,effects:BTreeMap::from([("format".into(),"{spec.target.format_id}".into())]),losses:BTreeMap::from([("pixels".into(),LossLevel::None),("coordinates".into(),LossLevel::None),("structure".into(),LossLevel::Normalized),("metadata".into(),LossLevel::Unbounded),("color-semantics".into(),LossLevel::Unknown)]),measurements:Measurements{{input_bytes:Some(report.input_bytes),output_bytes:Some(report.output_bytes),peak_memory_bytes:Some(peak),..Measurements::default()}},capsule_report:BTreeMap::from([("width".into(),report.width.to_string()),("height".into(),report.height.to_string()),("pixels".into(),report.pixels.to_string()),("source_channels".into(),report.source_channels.to_string()),("target_channels".into(),report.target_channels.to_string()),("non_opaque_pixels".into(),report.non_opaque_pixels.to_string()),("alpha_action".into(),report.alpha_action.into())]),warnings:report.warnings,provenance:Provenance{{capsule:handshake.capsule,adapter_id:handshake.adapter_id,adapter_version:handshake.adapter_version,capability_id:CAPABILITY_ID.into(),strategy:"rgba8-code-value-exact".into(),backend:"native-portable".into(),effective_options:defaults()}}}})}}}}
#[cfg(test)]mod tests{{use super::*;use everythingx_kernel::Kernel;#[test]fn kernel_invokes_runnable_defaults(){{let mut kernel=Kernel::default();kernel.register(Box::new(GeneratedRasterAdapter)).unwrap();let fixture={module}::conformance_fixture();let mut input=&fixture[..];let mut output=Vec::new();let result=kernel.invoke_defaults(ADAPTER_ID,CAPABILITY_ID,&mut input,&mut output).unwrap();assert_eq!(result.status,InvocationStatus::Succeeded);assert!(!output.is_empty());}}}}
'''


def files_for(spec: Spec, template: str) -> dict[Path, str]:
    root = CAPSULES / spec.name
    cargo = f'''[package]
name = "{spec.name}"
version = "0.1.0"
edition = "2024"
publish = false
license-file = "LICENSE"
description = "Standalone zero-dependency {spec.source.label} to {spec.target.label} conversion"

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

Independent, zero-dependency Rust conversion from {spec.source.label} to
{spec.target.label}. The directory can be copied out of EverythingX and built
or tested on its own. It contains its own parser, encoder, options, errors,
report, conformance fixture and runnable defaults; `everythingx/` is optional.

Version 0.1 targets the Raster Wave A RGBA8/RGB8 domain. It preserves accepted
pixel code values and coordinates exactly. PPM transparency is rejected by
default and requires an explicit lossy policy to discard or composite alpha.
'''
    source = template.replace("__SOURCE__", spec.source.profile).replace("__TARGET__", spec.target.profile)
    return {
        root / "Cargo.toml": cargo,
        root / "Cargo.lock": lock,
        root / "LICENSE": NOTICE,
        root / "README.md": readme,
        root / "capsule.json": manifest(spec),
        root / "src" / "lib.rs": source,
        root / "benches" / "README.md": "# Benchmarks\n\nCovered by the repository-wide release-mode Kernel/Adapter performance harness.\n",
        root / "fuzz" / "README.md": "# Fuzzing\n\nPlanned header, dimensions, packet stream, truncation and trailing-data campaigns.\n",
        root / "everythingx" / "adapter.json": adapter_manifest(spec),
        root / "everythingx" / "adapter" / "Cargo.toml": adapter_cargo,
        root / "everythingx" / "adapter" / "src" / "lib.rs": adapter_source(spec),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    template = TEMPLATE.read_text(encoding="utf-8")
    expected: dict[Path, str] = {}
    for spec in SPECS:
        expected.update(files_for(spec, template))
    stale = [path for path, content in expected.items() if not path.is_file() or path.read_text(encoding="utf-8") != content]
    if args.check:
        if stale:
            print("Raster Wave A scaffold is stale:")
            for path in stale: print(path.relative_to(ROOT))
            return 1
        print(f"Raster Wave A scaffold is current ({len(REPRESENTATIONS)} formats, {len(SPECS)} directed Capsules)")
        return 0
    for path, content in expected.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
    print(f"materialized {len(SPECS)} independent Raster Wave A Capsules")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
