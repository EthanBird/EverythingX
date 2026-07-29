#!/usr/bin/env python3
"""Materialize GIF87a/GIF89a and ICO/CUR Raster Wave C Capsules."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path

import scaffold_raster_wave_a as raster


ROOT = Path(__file__).resolve().parents[1]
RASTER = ROOT / "tools" / "templates" / "raster_wave_a_capsule.rs"
PNG_NATIVE = ROOT / "tools" / "templates" / "png_native.rs"
PNG_GLUE = ROOT / "tools" / "templates" / "png_conversion_glue.rs"
LEGACY_NATIVE = ROOT / "tools" / "templates" / "gif_ico_native.rs"
LEGACY_GLUE = ROOT / "tools" / "templates" / "gif_ico_conversion_glue.rs"
VALIDATE = ROOT / "tools" / "templates" / "gif_ico_validate_capsule.rs"
RENDER = ROOT / "tools" / "templates" / "gif_render_capsule.rs"
NOTICE = raster.NOTICE

GIF = raster.Representation(
    "gif", "Gif", "imagefmt:gif89a-still", "GIF87a/GIF89a exact-palette still raster", "GIF89a specification"
)
ICO_SOURCE = raster.Representation(
    "ico-best", "Ico", "imagefmt:ico", "best-ranked Windows ICO member", "Microsoft ICO directory and DIB structures"
)
ICO_TARGET = raster.Representation(
    "ico", "Ico", "imagefmt:ico", "single-member PNG-backed Windows ICO", "Microsoft ICO directory and PNG member structures"
)
CUR_SOURCE = raster.Representation(
    "cur-best", "Cur", "imagefmt:cur", "best-ranked Windows CUR member", "Microsoft CUR directory and DIB structures"
)
CUR_TARGET = raster.Representation(
    "cur", "Cur", "imagefmt:cur", "single-member PNG-backed Windows CUR", "Microsoft CUR directory and PNG member structures"
)
PNG = raster.Representation("png", "Png", "exfmt:image:png", "Portable Network Graphics", "W3C PNG Third Edition")
COMMON = (*raster.REPRESENTATIONS, PNG)
ICON_COMMON = tuple(value for value in COMMON if value.slug in {"png", "bmp", "qoi"})


@dataclass(frozen=True)
class DirectSpec:
    source: raster.Representation
    target: raster.Representation

    @property
    def name(self) -> str:
        return f"{self.source.slug}-to-{self.target.slug}"


DIRECT = tuple(
    [DirectSpec(GIF, target) for target in COMMON]
    + [DirectSpec(source, GIF) for source in COMMON]
    + [DirectSpec(ICO_SOURCE, target) for target in ICON_COMMON]
    + [DirectSpec(source, ICO_TARGET) for source in ICON_COMMON]
    + [DirectSpec(CUR_SOURCE, target) for target in ICON_COMMON]
    + [DirectSpec(source, CUR_TARGET) for source in ICON_COMMON]
)


@dataclass(frozen=True)
class ValidateSpec:
    name: str
    profile: str
    format_id: str
    label: str


VALIDATORS = (
    ValidateSpec("validate-gif", "Gif", "imagefmt:gif-animation", "GIF87a/GIF89a"),
    ValidateSpec("validate-ico", "Ico", "imagefmt:ico", "Windows ICO"),
    ValidateSpec("validate-cur", "Cur", "imagefmt:cur", "Windows CUR"),
)


@dataclass(frozen=True)
class RenderSpec:
    name: str
    operation: str
    strategy: str
    summary: str


RENDERS = (
    RenderSpec(
        "gif-render-frame-to-png",
        "Frame",
        "gif-composited-frame-selection",
        "Render one explicitly indexed composited GIF animation frame to PNG.",
    ),
    RenderSpec(
        "gif-animation-to-sprite-sheet-png",
        "SpriteSheet",
        "gif-horizontal-composited-sprite-sheet",
        "Render every composited GIF animation frame into a horizontal PNG sprite sheet.",
    ),
)


def json_text(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, indent=2) + "\n"


def defaults() -> dict[str, object]:
    return {
        "allow_sample_scaling": False,
        "cursor_hotspot_x": 0,
        "cursor_hotspot_y": 0,
        "max_frames": 10000,
        "max_input_bytes": 536870912,
        "max_members": 1024,
        "max_pixels": 100000000,
        "ppm_alpha": "reject",
        "preserve_unmarked_bmp_alpha": False,
        "strict_trailing_data": True,
        "tga_rle": True,
    }


def validate_defaults() -> dict[str, object]:
    return {
        "max_frames": 10000,
        "max_input_bytes": 536870912,
        "max_members": 1024,
        "max_pixels": 100000000,
    }


def render_defaults() -> dict[str, object]:
    return {
        "frame_index": 0,
        "max_frames": 10000,
        "max_input_bytes": 536870912,
        "max_pixels": 100000000,
    }


def evidence(capability_id: str) -> list[str]:
    path = ROOT / "registry" / "performance" / "baseline.json"
    if not path.is_file():
        return []
    measured = {
        row.get("capability_id")
        for row in json.loads(path.read_text(encoding="utf-8")).get("capabilities", [])
    }
    return [f"registry/performance/baseline.json#{capability_id}"] if capability_id in measured else []


def direct_capability(spec: DirectSpec) -> str:
    return f"capability:{spec.name}/legacy-raster-rgba8/native-portable"


def direct_scope(spec: DirectSpec) -> tuple[list[str], list[str]]:
    scope = [
        "Single visual raster with checked dimensions and bounded allocation",
        "RGBA8/RGB8 pixel coordinates and channel code values",
        f"Native parsing of {spec.source.label}",
        f"Native emission of {spec.target.label}",
    ]
    outside = [
        "ICC conversion and inferred transfer-function conversion",
        "Arbitrary metadata migration",
        "Lossy palette quantization",
    ]
    if spec.source.profile == "Gif":
        scope.extend(
            [
                "GIF global and local color tables",
                "GIF LZW, interlace, transparency and disposal methods 0 through 3",
                "Exactly one composited visual frame under this direct edge",
            ]
        )
        outside.append("Animated GIF frame selection; use the explicit frame or sprite-sheet Capsule")
    if spec.target.profile == "Gif":
        scope.extend(
            [
                "Exact palettes of at most 256 colors",
                "Opaque or binary-alpha pixels; transparent RGB must be zero",
            ]
        )
        outside.extend(["Partial alpha", "Palette quantization or dithering", "Animation authoring"])
    if spec.source.profile in {"Ico", "Cur"}:
        scope.extend(
            [
                "Complete ICO/CUR directory validation",
                "PNG members and uncompressed 1/4/8/24/32-bit DIB members",
                "Deterministic best-member selection by area then bit depth",
            ]
        )
        outside.append("Preservation of unselected icon or cursor members")
    if spec.target.profile in {"Ico", "Cur"}:
        scope.extend(
            [
                "One PNG-backed ICO/CUR member",
                "Dimensions from 1 through 256",
                "Explicit CUR hotspot coordinates with a runnable origin default",
            ]
        )
        outside.append("Multi-resolution icon aggregation")
    return scope, outside


def direct_manifest(spec: DirectSpec) -> str:
    capability = direct_capability(spec)
    cost = evidence(capability)
    scope, outside = direct_scope(spec)
    icon_selection = spec.source.profile in {"Ico", "Cur"}
    gif_target = spec.target.profile == "Gif"
    return json_text(
        {
            "capsule_id": f"capsule:{spec.name}",
            "version": "0.1.0",
            "name": spec.name,
            "summary": f"Dependency-free conversion from {spec.source.label} to {spec.target.label}.",
            "taxonomy": {
                "domain": "image",
                "primary_ir": "ir:raster",
                "secondary_irs": ["ir:container-graph", "ir:timed-media"],
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
                "scope": scope,
                "out_of_scope": outside,
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
                "strategy": "legacy-raster-rgba8",
                "backend": "native-portable",
                "options": defaults(),
                "policy": "strict",
            },
            "strategies": [
                {
                    "id": "legacy-raster-rgba8",
                    "guarantees": [
                        "No animated GIF is silently flattened by a still-image edge",
                        "No ICO/CUR member is selected without an explicit best-member capability name and report",
                        "GIF encoding rejects rather than quantizes pixels outside its exact palette domain",
                        "The complete source is validated before target bytes are committed",
                    ],
                    "tradeoffs": [
                        "ICO/CUR target emission is a normalized single PNG member",
                        "GIF exact-palette encoding favors proof simplicity over compression ratio",
                    ],
                }
            ],
            "backends": [
                {
                    "id": "native-portable",
                    "tier": "native-portable",
                    "default": True,
                    "dependencies": [],
                }
            ],
            "validation": {
                "specifications": [spec.source.specification, spec.target.specification],
                "conformance": ["src/lib.rs, src/legacy_native.rs and src/png_native.rs unit tests"],
                "differential": [],
                "properties": [
                    "Accepted pixel coordinates and RGBA8 code values round-trip exactly",
                    "All dimensions, directory ranges, LZW output and allocations are checked",
                    "Malformed input never produces partial committed output",
                ],
                "regression": [
                    "GIF LZW clear/end codes and interlace order",
                    "GIF animation rejection on still-image edges",
                    "ICO/CUR overlapping member ranges",
                    "ICO DIB XOR/AND orientation and alpha",
                ],
                "fuzz": ["Planned GIF block/LZW and ICO/CUR directory/DIB campaigns"],
                "benchmarks": cost or ["Pending controlled exbench baseline"],
            },
            "security": {
                "accepts_untrusted_input": True,
                "limits": [
                    "max_input_bytes defaults to 512 MiB",
                    "max_pixels defaults to 100 million",
                    "max_frames defaults to 10,000",
                    "max_members defaults to 1,024",
                ],
                "known_risks": ["Output I/O failure can occur while committing a fully validated result"],
            },
        }
    )


def loss_for(spec: DirectSpec) -> dict[str, str]:
    return {
        "pixels": "none",
        "coordinates": "none",
        "structure": "unbounded" if spec.source.profile in {"Ico", "Cur"} else "normalized",
        "metadata": "unbounded",
        "color-semantics": "conditional",
    }


def direct_adapter_manifest(spec: DirectSpec) -> str:
    capability = direct_capability(spec)
    cost = evidence(capability)
    preconditions = ["Input belongs to the declared native parser subset"]
    if spec.source.profile == "Gif":
        preconditions.append("GIF contains exactly one visual frame")
    if spec.target.profile == "Gif":
        preconditions.extend(
            [
                "Raster has at most 256 exact colors",
                "Alpha is opaque or binary and transparent RGB is zero",
            ]
        )
    return json_text(
        {
            "adapter_id": f"adapter:{spec.name}-static",
            "version": "0.1.0",
            "capsule": {"id": f"capsule:{spec.name}", "version_requirement": "^0.1.0"},
            "protocol": {"name": "everythingx-adapter-protocol", "version_requirement": "0.1"},
            "transport": {"kind": "static-rust", "entrypoint": "GeneratedLegacyRasterAdapter"},
            "capabilities": [
                {
                    "capability_id": capability,
                    "capsule_entrypoint": "convert",
                    "strategy": "legacy-raster-rgba8",
                    "backend": "native-portable",
                    "inputs": [spec.source.format_id],
                    "outputs": [spec.target.format_id],
                    "preconditions": preconditions,
                    "effects": [f"Produce {spec.target.label}"],
                    "invariants": ["Width", "Height", "Pixel coordinates", "Accepted RGBA8 code values"],
                    "computability": "conditional_exact",
                    "loss": loss_for(spec),
                    "default_options": defaults(),
                    "defaults_are_runnable": True,
                    "execution": {"streaming": False, "seek_required": False, "cost_evidence": cost},
                    "report_mapping": {
                        "unknown_fields_are_preserved": True,
                        "rules": [
                            "Dimensions, channels, alpha action and selection warnings map to capsule_report",
                            "static Adapter buffers protocol input and enforces output budget",
                        ],
                    },
                }
            ],
        }
    )


def pairs(values: dict[str, object]) -> str:
    return ",".join(
        f'("{key}".into(),"{str(value).lower() if isinstance(value, bool) else value}".into())'
        for key, value in sorted(values.items())
    )


def direct_adapter_source(spec: DirectSpec) -> str:
    module = spec.name.replace("-", "_")
    capability = direct_capability(spec)
    loss = loss_for(spec)
    level = {
        "none": "None",
        "bounded": "Bounded",
        "normalized": "Normalized",
        "unbounded": "Unbounded",
        # The protocol has no runtime Conditional variant. A manifest may state
        # a conditional loss precondition, but the observed invocation level is
        # Unknown until an input-specific analyzer resolves it.
        "conditional": "Unknown",
        "unknown": "Unknown",
    }
    return f'''#![forbid(unsafe_code)]
use std::collections::BTreeMap;use std::io::{{self,Read,Write}};
use everythingx_protocol::{{AdapterError,AdapterErrorKind,AdapterHandshake,CapabilityDescriptor,CapsuleIdentity,InvocationRequest,InvocationResult,InvocationStatus,LossLevel,Measurements,ProtocolVersion,Provenance,StaticAdapter}};use {module}::{{Error as CapsuleError,Options}};
pub const ADAPTER_ID:&str="adapter:{spec.name}-static";pub const CAPABILITY_ID:&str="{capability}";pub struct GeneratedLegacyRasterAdapter;
fn defaults()->BTreeMap<String,String>{{BTreeMap::from([{pairs(defaults())}])}}fn descriptor()->CapabilityDescriptor{{CapabilityDescriptor{{capability_id:CAPABILITY_ID.into(),source_formats:vec!["{spec.source.format_id}".into()],target_formats:vec!["{spec.target.format_id}".into()],strategy:"legacy-raster-rgba8".into(),backend:"native-portable".into(),default_options:defaults(),defaults_are_runnable:true,streaming:false,seek_required:false}}}}
struct Limited<'a>{{inner:&'a mut dyn Write,remaining:u64,exceeded:bool}}impl Write for Limited<'_>{{fn write(&mut self,b:&[u8])->io::Result<usize>{{if b.len()as u64>self.remaining{{self.exceeded=true;return Err(io::Error::other("output budget exceeded"));}}let n=self.inner.write(b)?;self.remaining-=n as u64;Ok(n)}}fn flush(&mut self)->io::Result<()>{{self.inner.flush()}}}}
impl StaticAdapter for GeneratedLegacyRasterAdapter{{fn handshake(&self)->AdapterHandshake{{AdapterHandshake{{protocol:ProtocolVersion::CURRENT,adapter_id:ADAPTER_ID.into(),adapter_version:"0.1.0".into(),capsule:CapsuleIdentity{{id:"capsule:{spec.name}".into(),version:"0.1.0".into(),content_hash:None}},capabilities:vec![descriptor()]}}}}fn invoke(&self,request:&InvocationRequest,input:&mut dyn Read,output:&mut dyn Write)->Result<InvocationResult,AdapterError>{{if request.capability_id!=CAPABILITY_ID{{return Err(AdapterError::new(AdapterErrorKind::UnsupportedCapability,"unsupported capability"));}}if request.options!=defaults(){{return Err(AdapterError::new(AdapterErrorKind::InvalidOptions,"version 0.1 static Adapter accepts its declared defaults"));}}let limit=request.resource_budget.max_memory_bytes/4;let mut bytes=Vec::new();input.take(limit.saturating_add(1)).read_to_end(&mut bytes).map_err(|e|AdapterError::new(AdapterErrorKind::Io,e.to_string()))?;if bytes.len()as u64>limit{{return Err(AdapterError::new(AdapterErrorKind::ResourceLimit,"input exceeds Adapter memory share"));}}let adapter_memory=bytes.len()as u64;let mut source=&bytes[..];let mut limited=Limited{{inner:output,remaining:request.resource_budget.max_output_bytes,exceeded:false}};let report={module}::convert(&mut source,&mut limited,&Options::default()).map_err(|error|match error{{CapsuleError::Io(io)if limited.exceeded=>AdapterError::new(AdapterErrorKind::ResourceLimit,io.to_string()),CapsuleError::Io(io)=>AdapterError::new(AdapterErrorKind::Io,io.to_string()),limited_error@(CapsuleError::InputTooLarge{{..}}|CapsuleError::PixelLimitExceeded{{..}})=>AdapterError::new(AdapterErrorKind::ResourceLimit,limited_error.to_string()),other=>AdapterError::new(AdapterErrorKind::InvalidInput,other.to_string())}})?;let peak=adapter_memory.saturating_add(report.peak_working_memory_bytes);if peak>request.resource_budget.max_memory_bytes{{return Err(AdapterError::new(AdapterErrorKind::ResourceLimit,"reported peak memory exceeds request budget"));}}let handshake=self.handshake();Ok(InvocationResult{{status:InvocationStatus::Succeeded,effects:BTreeMap::from([("format".into(),"{spec.target.format_id}".into())]),losses:BTreeMap::from([("pixels".into(),LossLevel::{level[loss["pixels"]]}),("coordinates".into(),LossLevel::{level[loss["coordinates"]]}),("structure".into(),LossLevel::{level[loss["structure"]]}),("metadata".into(),LossLevel::{level[loss["metadata"]]}),("color-semantics".into(),LossLevel::{level[loss["color-semantics"]]})]),measurements:Measurements{{input_bytes:Some(report.input_bytes),output_bytes:Some(report.output_bytes),peak_memory_bytes:Some(peak),..Measurements::default()}},capsule_report:BTreeMap::from([("width".into(),report.width.to_string()),("height".into(),report.height.to_string()),("pixels".into(),report.pixels.to_string()),("source_channels".into(),report.source_channels.to_string()),("target_channels".into(),report.target_channels.to_string()),("alpha_action".into(),report.alpha_action.into())]),warnings:report.warnings,provenance:Provenance{{capsule:handshake.capsule,adapter_id:handshake.adapter_id,adapter_version:handshake.adapter_version,capability_id:CAPABILITY_ID.into(),strategy:"legacy-raster-rgba8".into(),backend:"native-portable".into(),effective_options:defaults()}}}})}}}}
#[cfg(test)]mod tests{{use super::*;use everythingx_kernel::Kernel;#[test]fn kernel_invokes_runnable_defaults(){{let mut kernel=Kernel::default();kernel.register(Box::new(GeneratedLegacyRasterAdapter)).unwrap();let fixture={module}::conformance_fixture();let mut input=&fixture[..];let mut output=Vec::new();assert_eq!(kernel.invoke_defaults(ADAPTER_ID,CAPABILITY_ID,&mut input,&mut output).unwrap().status,InvocationStatus::Succeeded);assert!(!output.is_empty());}}}}
'''


def common_files(root: Path, name: str, description: str) -> dict[Path, str]:
    return {
        root / "Cargo.toml": f'''[package]
name = "{name}"
version = "0.1.0"
edition = "2024"
publish = false
license-file = "LICENSE"
description = "{description}"

[lib]
path = "src/lib.rs"

[dependencies]
''',
        root / "Cargo.lock": f'''# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = "{name}"
version = "0.1.0"
''',
        root / "LICENSE": NOTICE,
        root / "benches" / "README.md": "# Benchmarks\n\nCovered by the repository-wide release-mode Kernel/Adapter performance harness.\n",
        root / "fuzz" / "README.md": "# Fuzzing\n\nPlanned GIF block/LZW and ICO/CUR directory/DIB campaigns.\n",
    }


def adapter_cargo(root_depth_name: str) -> str:
    return f'''[package]
name = "everythingx-adapter-{root_depth_name}"
version = "0.1.0"
edition = "2024"
publish = false

[lib]
path = "src/lib.rs"

[dependencies]
everythingx-protocol = {{ path = "../../../../../../../kernel/ex-protocol" }}
{root_depth_name} = {{ path = "../.." }}

[dev-dependencies]
everythingx-kernel = {{ path = "../../../../../../../kernel/ex-kernel" }}
'''


def conversion_source() -> str:
    source = RASTER.read_text(encoding="utf-8")
    source = source.replace(
        "#![forbid(unsafe_code)]\n",
        "#![forbid(unsafe_code)]\n\nmod legacy_native;\nmod png_native;\n",
        1,
    )
    source = source.replace("    Pam,\n}", "    Pam,\n    Png,\n    Gif,\n    Ico,\n    Cur,\n}", 1)
    source = source.replace(
        '            Self::Pam => "pam",\n',
        '            Self::Pam => "pam",\n            Self::Png => "png",\n            Self::Gif => "gif",\n            Self::Ico => "ico",\n            Self::Cur => "cur",\n',
        1,
    )
    source = source.replace(
        "    pub tga_rle: bool,\n}",
        "    pub tga_rle: bool,\n    pub max_frames: u32,\n    pub max_members: u32,\n    pub cursor_hotspot_x: u16,\n    pub cursor_hotspot_y: u16,\n}",
        1,
    )
    source = source.replace(
        "            tga_rle: true,\n",
        "            tga_rle: true,\n            max_frames: 10_000,\n            max_members: 1_024,\n            cursor_hotspot_x: 0,\n            cursor_hotspot_y: 0,\n",
        1,
    )
    source = source.replace(
        "    IntegerOverflow(&'static str),\n    Io(io::Error),",
        "    IntegerOverflow(&'static str),\n    Png(String),\n    Legacy(String),\n    Io(io::Error),",
        1,
    )
    source = source.replace(
        '            Self::IntegerOverflow(context) => write!(f, "integer overflow while computing {context}"),\n',
        '            Self::IntegerOverflow(context) => write!(f, "integer overflow while computing {context}"),\n            Self::Png(message) => write!(f, "PNG codec error: {message}"),\n            Self::Legacy(message) => write!(f, "GIF/ICO/CUR codec error: {message}"),\n',
        1,
    )
    source = source.replace(
        "        Profile::Pam => decode_pam(bytes, options),\n",
        "        Profile::Pam => decode_pam(bytes, options),\n        Profile::Png => decode_png(bytes, options),\n        Profile::Gif => decode_gif(bytes, options),\n        Profile::Ico => decode_icon(bytes, options, legacy_native::IconKind::Icon),\n        Profile::Cur => decode_icon(bytes, options, legacy_native::IconKind::Cursor),\n",
        1,
    )
    source = source.replace(
        "        Profile::Pam => encode_pam(image),\n",
        "        Profile::Pam => encode_pam(image),\n        Profile::Png => encode_png(image),\n        Profile::Gif => encode_gif(image),\n        Profile::Ico => encode_icon(image, options, legacy_native::IconKind::Icon),\n        Profile::Cur => encode_icon(image, options, legacy_native::IconKind::Cursor),\n",
        1,
    )
    source = source.replace(
        "                a: if alpha && (x + y) % 3 == 0 { 96 } else { 255 },",
        "                a: if alpha && (x + y) % 3 == 0 { if SOURCE == Profile::Gif || TARGET == Profile::Gif { 0 } else { 96 } } else { 255 },",
    )
    source = source.replace(
        "            pixels.push(Pixel {\n                r: x.wrapping_mul(37).wrapping_add(y.wrapping_mul(11)) as u8,",
        "            let gif_transparent = alpha && (x + y) % 3 == 0 && (SOURCE == Profile::Gif || TARGET == Profile::Gif);\n            pixels.push(Pixel {\n                r: if gif_transparent { 0 } else { x.wrapping_mul(37).wrapping_add(y.wrapping_mul(11)) as u8 },",
        1,
    )
    source = source.replace(
        "                g: x.wrapping_mul(3).wrapping_add(y.wrapping_mul(29)) as u8,\n                b: x.wrapping_mul(17).wrapping_add(y.wrapping_mul(5)) as u8,",
        "                g: if gif_transparent { 0 } else { x.wrapping_mul(3).wrapping_add(y.wrapping_mul(29)) as u8 },\n                b: if gif_transparent { 0 } else { x.wrapping_mul(17).wrapping_add(y.wrapping_mul(5)) as u8 },",
        1,
    )
    source = source.replace(
        "Error::InvalidSignature(_) | Error::Truncated(_)",
        "Error::InvalidSignature(_) | Error::Truncated(_) | Error::Png(_) | Error::Legacy(_)",
    )
    source = source.replace(
        'strategy: "rgba8-code-value-exact",',
        'strategy: "legacy-raster-rgba8",',
    )
    source = source.replace(
        'assert_eq!(report.strategy, "rgba8-code-value-exact");',
        'assert_eq!(report.strategy, "legacy-raster-rgba8");',
    )
    source = source.replace(
        'assert_eq!(report.alpha_action, "preserved");',
        'assert!(report.alpha_action.starts_with("preserved"));',
    )
    return source + PNG_GLUE.read_text(encoding="utf-8") + LEGACY_GLUE.read_text(encoding="utf-8")


def direct_files(spec: DirectSpec, source: str, native: str, png: str) -> dict[Path, str]:
    root = ROOT / "capsules" / "image" / "raster" / "direct" / spec.name
    files = common_files(root, spec.name, f"{spec.source.label} to {spec.target.label}")
    files.update(
        {
            root / "README.md": f"""# {spec.name}

Independent, zero-dependency Rust conversion from {spec.source.label} to
{spec.target.label}. GIF parsing covers LZW, interlace, global/local palettes,
transparency and animation disposal. ICO/CUR parsing validates the complete
directory and supports PNG plus common uncompressed DIB members.

Still-image GIF edges reject animation. ICO/CUR read edges explicitly select
the best member by area then bit depth and report that choice. GIF target edges
reject quantization, partial alpha and palettes above 256 colors.
""",
            root / "capsule.json": direct_manifest(spec),
            root / "src" / "lib.rs": source.replace("__SOURCE__", spec.source.profile).replace(
                "__TARGET__", spec.target.profile
            ),
            root / "src" / "png_native.rs": png,
            root / "src" / "legacy_native.rs": native,
            root / "everythingx" / "adapter.json": direct_adapter_manifest(spec),
            root / "everythingx" / "adapter" / "Cargo.toml": adapter_cargo(spec.name),
            root / "everythingx" / "adapter" / "src" / "lib.rs": direct_adapter_source(spec),
        }
    )
    return files


def validate_capability(spec: ValidateSpec) -> str:
    return f"capability:{spec.name}/strict-full-structure-validation/native-portable"


def validate_manifest(spec: ValidateSpec) -> str:
    capability = validate_capability(spec)
    cost = evidence(capability)
    return json_text(
        {
            "capsule_id": f"capsule:{spec.name}",
            "version": "0.1.0",
            "name": spec.name,
            "summary": f"Strictly validate {spec.label} and copy the exact byte stream.",
            "taxonomy": {
                "domain": "image",
                "primary_ir": "ir:raster",
                "secondary_irs": ["ir:container-graph", "ir:timed-media"],
                "operator_kind": "validate",
                "operator_role": "analyze",
            },
            "license": {
                "expression": "PolyForm-Noncommercial-1.0.0",
                "file": "LICENSE",
                "commercial_authorization_required": True,
            },
            "repository": f"https://github.com/EthanBird/EverythingX/tree/main/capsules/image/raster/analyze/{spec.name}",
            "independence": {
                "standalone_cargo_build": True,
                "everythingx_optional": True,
                "external_path_dependencies": False,
                "copy_out_tested": True,
            },
            "conversion": {
                "source": [spec.format_id],
                "target": [spec.format_id],
                "arity": {"inputs": {"min": 1, "max": 1}, "outputs": {"min": 1, "max": 1}},
                "scope": [
                    "Complete block or member directory traversal",
                    "GIF LZW/frame composition or every ICO/CUR PNG/DIB member decode",
                    "Exact byte-for-byte output after validation",
                ],
                "out_of_scope": ["Repair of malformed structures", "ICC color conversion"],
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
                "strategy": "strict-full-structure-validation",
                "backend": "native-portable",
                "options": validate_defaults(),
                "policy": "strict",
            },
            "strategies": [
                {
                    "id": "strict-full-structure-validation",
                    "guarantees": [
                        "Every declared GIF frame or ICO/CUR member is decoded",
                        "Ranges, dimensions, checksums and allocation arithmetic are checked",
                        "Successful output is byte-for-byte identical to input",
                    ],
                    "tradeoffs": ["Validation materializes decoded rasters to prove payload integrity"],
                }
            ],
            "backends": [
                {
                    "id": "native-portable",
                    "tier": "native-portable",
                    "default": True,
                    "dependencies": [],
                }
            ],
            "validation": {
                "specifications": ["GIF89a", "Microsoft ICO/CUR and DIB structures", "W3C PNG Third Edition"],
                "conformance": ["src/lib.rs unit tests"],
                "differential": [],
                "properties": ["Successful validation copies exact bytes", "Truncation fails before output"],
                "regression": ["Frame/member limits", "Overlapping icon ranges", "Malformed LZW"],
                "fuzz": ["Planned full-structure campaign"],
                "benchmarks": cost or ["Pending controlled exbench baseline"],
            },
            "security": {
                "accepts_untrusted_input": True,
                "limits": [
                    "512 MiB input",
                    "100 million pixels",
                    "10,000 GIF frames",
                    "1,024 ICO/CUR members",
                ],
                "known_risks": ["Validation cost scales with decoded frames or members"],
            },
        }
    )


def validate_adapter_manifest(spec: ValidateSpec) -> str:
    capability = validate_capability(spec)
    return json_text(
        {
            "adapter_id": f"adapter:{spec.name}-static",
            "version": "0.1.0",
            "capsule": {"id": f"capsule:{spec.name}", "version_requirement": "^0.1.0"},
            "protocol": {"name": "everythingx-adapter-protocol", "version_requirement": "0.1"},
            "transport": {"kind": "static-rust", "entrypoint": "GeneratedLegacyValidateAdapter"},
            "capabilities": [
                {
                    "capability_id": capability,
                    "capsule_entrypoint": "convert",
                    "strategy": "strict-full-structure-validation",
                    "backend": "native-portable",
                    "inputs": [spec.format_id],
                    "outputs": [spec.format_id],
                    "preconditions": ["Input is structurally valid within declared limits"],
                    "effects": ["Validate all frames or members and copy exact bytes"],
                    "invariants": ["All source bytes", "All decoded pixels"],
                    "computability": "total_for_declared_subset",
                    "loss": {
                        "pixels": "none",
                        "coordinates": "none",
                        "structure": "none",
                        "metadata": "none",
                        "color-semantics": "none",
                    },
                    "default_options": validate_defaults(),
                    "defaults_are_runnable": True,
                    "execution": {
                        "streaming": False,
                        "seek_required": False,
                        "cost_evidence": evidence(capability),
                    },
                    "report_mapping": {
                        "unknown_fields_are_preserved": True,
                        "rules": ["Frame/member counts and selected dimensions map to capsule_report"],
                    },
                }
            ],
        }
    )


def validate_adapter_source(spec: ValidateSpec) -> str:
    module = spec.name.replace("-", "_")
    capability = validate_capability(spec)
    return f'''#![forbid(unsafe_code)]
use std::collections::BTreeMap;use std::io::{{self,Read,Write}};use everythingx_protocol::{{AdapterError,AdapterErrorKind,AdapterHandshake,CapabilityDescriptor,CapsuleIdentity,InvocationRequest,InvocationResult,InvocationStatus,LossLevel,Measurements,ProtocolVersion,Provenance,StaticAdapter}};use {module}::{{Error as CapsuleError,Options}};
pub const ADAPTER_ID:&str="adapter:{spec.name}-static";pub const CAPABILITY_ID:&str="{capability}";pub struct GeneratedLegacyValidateAdapter;fn defaults()->BTreeMap<String,String>{{BTreeMap::from([{pairs(validate_defaults())}])}}fn descriptor()->CapabilityDescriptor{{CapabilityDescriptor{{capability_id:CAPABILITY_ID.into(),source_formats:vec!["{spec.format_id}".into()],target_formats:vec!["{spec.format_id}".into()],strategy:"strict-full-structure-validation".into(),backend:"native-portable".into(),default_options:defaults(),defaults_are_runnable:true,streaming:false,seek_required:false}}}}
struct Limited<'a>{{inner:&'a mut dyn Write,remaining:u64,exceeded:bool}}impl Write for Limited<'_>{{fn write(&mut self,b:&[u8])->io::Result<usize>{{if b.len()as u64>self.remaining{{self.exceeded=true;return Err(io::Error::other("output budget exceeded"));}}let n=self.inner.write(b)?;self.remaining-=n as u64;Ok(n)}}fn flush(&mut self)->io::Result<()>{{self.inner.flush()}}}}
impl StaticAdapter for GeneratedLegacyValidateAdapter{{fn handshake(&self)->AdapterHandshake{{AdapterHandshake{{protocol:ProtocolVersion::CURRENT,adapter_id:ADAPTER_ID.into(),adapter_version:"0.1.0".into(),capsule:CapsuleIdentity{{id:"capsule:{spec.name}".into(),version:"0.1.0".into(),content_hash:None}},capabilities:vec![descriptor()]}}}}fn invoke(&self,request:&InvocationRequest,input:&mut dyn Read,output:&mut dyn Write)->Result<InvocationResult,AdapterError>{{if request.capability_id!=CAPABILITY_ID{{return Err(AdapterError::new(AdapterErrorKind::UnsupportedCapability,"unsupported capability"));}}if request.options!=defaults(){{return Err(AdapterError::new(AdapterErrorKind::InvalidOptions,"static Adapter accepts declared defaults"));}}let limit=request.resource_budget.max_memory_bytes/4;let mut bytes=Vec::new();input.take(limit.saturating_add(1)).read_to_end(&mut bytes).map_err(|e|AdapterError::new(AdapterErrorKind::Io,e.to_string()))?;if bytes.len()as u64>limit{{return Err(AdapterError::new(AdapterErrorKind::ResourceLimit,"input exceeds Adapter memory share"));}}let adapter_memory=bytes.len()as u64;let mut source=&bytes[..];let mut limited=Limited{{inner:output,remaining:request.resource_budget.max_output_bytes,exceeded:false}};let report={module}::convert(&mut source,&mut limited,&Options::default()).map_err(|error|match error{{CapsuleError::Io(io)if limited.exceeded=>AdapterError::new(AdapterErrorKind::ResourceLimit,io.to_string()),CapsuleError::Io(io)=>AdapterError::new(AdapterErrorKind::Io,io.to_string()),CapsuleError::InputTooLarge{{..}}=>AdapterError::new(AdapterErrorKind::ResourceLimit,error.to_string()),other=>AdapterError::new(AdapterErrorKind::InvalidInput,other.to_string())}})?;let peak=adapter_memory.saturating_add(report.peak_working_memory_bytes);if peak>request.resource_budget.max_memory_bytes{{return Err(AdapterError::new(AdapterErrorKind::ResourceLimit,"reported peak memory exceeds budget"));}}let handshake=self.handshake();Ok(InvocationResult{{status:InvocationStatus::Succeeded,effects:BTreeMap::from([("validated".into(),"true".into())]),losses:BTreeMap::from([("pixels".into(),LossLevel::None),("coordinates".into(),LossLevel::None),("structure".into(),LossLevel::None),("metadata".into(),LossLevel::None),("color-semantics".into(),LossLevel::None)]),measurements:Measurements{{input_bytes:Some(report.input_bytes),output_bytes:Some(report.output_bytes),peak_memory_bytes:Some(peak),..Measurements::default()}},capsule_report:BTreeMap::from([("width".into(),report.width.to_string()),("height".into(),report.height.to_string()),("frames_or_members".into(),report.frames_or_members.to_string()),("png_members".into(),report.png_members.to_string()),("dib_members".into(),report.dib_members.to_string())]),warnings:report.warnings,provenance:Provenance{{capsule:handshake.capsule,adapter_id:handshake.adapter_id,adapter_version:handshake.adapter_version,capability_id:CAPABILITY_ID.into(),strategy:"strict-full-structure-validation".into(),backend:"native-portable".into(),effective_options:defaults()}}}})}}}}
#[cfg(test)]mod tests{{use super::*;use everythingx_kernel::Kernel;#[test]fn kernel_invokes_runnable_defaults(){{let mut kernel=Kernel::default();kernel.register(Box::new(GeneratedLegacyValidateAdapter)).unwrap();let fixture={module}::conformance_fixture();let mut input=&fixture[..];let mut output=Vec::new();assert_eq!(kernel.invoke_defaults(ADAPTER_ID,CAPABILITY_ID,&mut input,&mut output).unwrap().status,InvocationStatus::Succeeded);assert_eq!(output,fixture);}}}}
'''


def validate_files(spec: ValidateSpec, template: str, native: str, png: str) -> dict[Path, str]:
    root = ROOT / "capsules" / "image" / "raster" / "analyze" / spec.name
    files = common_files(root, spec.name, f"Strict {spec.label} validation")
    files.update(
        {
            root / "README.md": f"# {spec.name}\n\nStrictly validates every declared frame or member of a {spec.label} file, then copies the exact byte stream. The standalone crate has no dependencies.\n",
            root / "capsule.json": validate_manifest(spec),
            root / "src" / "lib.rs": template.replace("__FORMAT__", spec.profile),
            root / "src" / "legacy_native.rs": native,
            root / "src" / "png_native.rs": png,
            root / "everythingx" / "adapter.json": validate_adapter_manifest(spec),
            root / "everythingx" / "adapter" / "Cargo.toml": adapter_cargo(spec.name),
            root / "everythingx" / "adapter" / "src" / "lib.rs": validate_adapter_source(spec),
        }
    )
    return files


def render_capability(spec: RenderSpec) -> str:
    return f"capability:{spec.name}/{spec.strategy}/native-portable"


def render_manifest(spec: RenderSpec) -> str:
    capability = render_capability(spec)
    sprite = spec.operation == "SpriteSheet"
    return json_text(
        {
            "capsule_id": f"capsule:{spec.name}",
            "version": "0.1.0",
            "name": spec.name,
            "summary": spec.summary,
            "taxonomy": {
                "domain": "image",
                "primary_ir": "ir:raster",
                "secondary_irs": ["ir:timed-media"],
                "operator_kind": "render",
                "operator_role": "bridge",
            },
            "license": {
                "expression": "PolyForm-Noncommercial-1.0.0",
                "file": "LICENSE",
                "commercial_authorization_required": True,
            },
            "repository": f"https://github.com/EthanBird/EverythingX/tree/main/capsules/image/raster/bridge/{spec.name}",
            "independence": {
                "standalone_cargo_build": True,
                "everythingx_optional": True,
                "external_path_dependencies": False,
                "copy_out_tested": True,
            },
            "conversion": {
                "source": ["imagefmt:gif-animation"],
                "target": ["exfmt:image:png"],
                "arity": {"inputs": {"min": 1, "max": 1}, "outputs": {"min": 1, "max": 1}},
                "scope": [
                    "GIF global/local palettes, LZW, interlace, transparency and disposal 0 through 3",
                    "Composited visual frames",
                    "Explicit indexed frame selection" if not sprite else "All frames in chronological left-to-right order",
                ],
                "out_of_scope": [
                    "Preservation of animation timing in a still PNG",
                    "GIF plain-text rendering extension",
                    "ICC conversion",
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
                "strategy": spec.strategy,
                "backend": "native-portable",
                "options": render_defaults(),
                "policy": "strict",
            },
            "strategies": [
                {
                    "id": spec.strategy,
                    "guarantees": [
                        "Frame disposal and transparency are composited before rendering",
                        "No animation is mistaken for an ordinary still-image carrier conversion",
                        "PNG output is CRC/Adler protected and independently decodable",
                    ],
                    "tradeoffs": [
                        "Timing and loop metadata are reported but not representable in still PNG",
                        "Sprite sheets grow horizontally with frame count" if sprite else "One frame is selected explicitly",
                    ],
                }
            ],
            "backends": [
                {
                    "id": "native-portable",
                    "tier": "native-portable",
                    "default": True,
                    "dependencies": [],
                }
            ],
            "validation": {
                "specifications": ["GIF89a", "W3C PNG Third Edition", "RFC 1950/1951"],
                "conformance": ["Two-frame direct tests in src/lib.rs"],
                "differential": [],
                "properties": ["Output dimensions encode the selected frame set", "PNG decodes independently"],
                "regression": ["Out-of-range frame index", "Truncated GIF", "Sprite-sheet pixel limit"],
                "fuzz": ["Planned animation block, LZW, disposal and dimension campaigns"],
                "benchmarks": evidence(capability) or ["Pending controlled exbench baseline"],
            },
            "security": {
                "accepts_untrusted_input": True,
                "limits": ["512 MiB input", "100 million output pixels", "10,000 frames"],
                "known_risks": ["Full animation compositing materializes decoded frame rasters"],
            },
        }
    )


def render_adapter_manifest(spec: RenderSpec) -> str:
    capability = render_capability(spec)
    return json_text(
        {
            "adapter_id": f"adapter:{spec.name}-static",
            "version": "0.1.0",
            "capsule": {"id": f"capsule:{spec.name}", "version_requirement": "^0.1.0"},
            "protocol": {"name": "everythingx-adapter-protocol", "version_requirement": "0.1"},
            "transport": {"kind": "static-rust", "entrypoint": "GeneratedGifRenderAdapter"},
            "capabilities": [
                {
                    "capability_id": capability,
                    "capsule_entrypoint": "convert",
                    "strategy": spec.strategy,
                    "backend": "native-portable",
                    "inputs": ["imagefmt:gif-animation"],
                    "outputs": ["exfmt:image:png"],
                    "preconditions": ["GIF animation is valid within frame and pixel limits"],
                    "effects": [spec.summary],
                    "invariants": ["Composited frame pixel code values", "Frame chronological order"],
                    "computability": "total_for_declared_subset",
                    "loss": {
                        "pixels": "none",
                        "coordinates": "normalized",
                        "structure": "unbounded",
                        "metadata": "unbounded",
                        "color-semantics": "conditional",
                    },
                    "default_options": render_defaults(),
                    "defaults_are_runnable": True,
                    "execution": {
                        "streaming": False,
                        "seek_required": False,
                        "cost_evidence": evidence(capability),
                    },
                    "report_mapping": {
                        "unknown_fields_are_preserved": True,
                        "rules": ["Source frame count, selection and dimensions map to capsule_report"],
                    },
                }
            ],
        }
    )


def render_adapter_source(spec: RenderSpec) -> str:
    module = spec.name.replace("-", "_")
    capability = render_capability(spec)
    return f'''#![forbid(unsafe_code)]
use std::collections::BTreeMap;use std::io::{{self,Read,Write}};use everythingx_protocol::{{AdapterError,AdapterErrorKind,AdapterHandshake,CapabilityDescriptor,CapsuleIdentity,InvocationRequest,InvocationResult,InvocationStatus,LossLevel,Measurements,ProtocolVersion,Provenance,StaticAdapter}};use {module}::{{Error as CapsuleError,Options}};
pub const ADAPTER_ID:&str="adapter:{spec.name}-static";pub const CAPABILITY_ID:&str="{capability}";pub struct GeneratedGifRenderAdapter;fn defaults()->BTreeMap<String,String>{{BTreeMap::from([{pairs(render_defaults())}])}}fn descriptor()->CapabilityDescriptor{{CapabilityDescriptor{{capability_id:CAPABILITY_ID.into(),source_formats:vec!["imagefmt:gif-animation".into()],target_formats:vec!["exfmt:image:png".into()],strategy:"{spec.strategy}".into(),backend:"native-portable".into(),default_options:defaults(),defaults_are_runnable:true,streaming:false,seek_required:false}}}}
struct Limited<'a>{{inner:&'a mut dyn Write,remaining:u64,exceeded:bool}}impl Write for Limited<'_>{{fn write(&mut self,b:&[u8])->io::Result<usize>{{if b.len()as u64>self.remaining{{self.exceeded=true;return Err(io::Error::other("output budget exceeded"));}}let n=self.inner.write(b)?;self.remaining-=n as u64;Ok(n)}}fn flush(&mut self)->io::Result<()>{{self.inner.flush()}}}}
impl StaticAdapter for GeneratedGifRenderAdapter{{fn handshake(&self)->AdapterHandshake{{AdapterHandshake{{protocol:ProtocolVersion::CURRENT,adapter_id:ADAPTER_ID.into(),adapter_version:"0.1.0".into(),capsule:CapsuleIdentity{{id:"capsule:{spec.name}".into(),version:"0.1.0".into(),content_hash:None}},capabilities:vec![descriptor()]}}}}fn invoke(&self,request:&InvocationRequest,input:&mut dyn Read,output:&mut dyn Write)->Result<InvocationResult,AdapterError>{{if request.capability_id!=CAPABILITY_ID{{return Err(AdapterError::new(AdapterErrorKind::UnsupportedCapability,"unsupported capability"));}}if request.options!=defaults(){{return Err(AdapterError::new(AdapterErrorKind::InvalidOptions,"static Adapter accepts declared defaults"));}}let limit=request.resource_budget.max_memory_bytes/4;let mut bytes=Vec::new();input.take(limit.saturating_add(1)).read_to_end(&mut bytes).map_err(|e|AdapterError::new(AdapterErrorKind::Io,e.to_string()))?;if bytes.len()as u64>limit{{return Err(AdapterError::new(AdapterErrorKind::ResourceLimit,"input exceeds Adapter memory share"));}}let adapter_memory=bytes.len()as u64;let mut source=&bytes[..];let mut limited=Limited{{inner:output,remaining:request.resource_budget.max_output_bytes,exceeded:false}};let report={module}::convert(&mut source,&mut limited,&Options::default()).map_err(|error|match error{{CapsuleError::Io(io)if limited.exceeded=>AdapterError::new(AdapterErrorKind::ResourceLimit,io.to_string()),CapsuleError::Io(io)=>AdapterError::new(AdapterErrorKind::Io,io.to_string()),limited_error@(CapsuleError::InputTooLarge{{..}}|CapsuleError::PixelLimitExceeded{{..}})=>AdapterError::new(AdapterErrorKind::ResourceLimit,limited_error.to_string()),other=>AdapterError::new(AdapterErrorKind::InvalidInput,other.to_string())}})?;let peak=adapter_memory.saturating_add(report.peak_working_memory_bytes);if peak>request.resource_budget.max_memory_bytes{{return Err(AdapterError::new(AdapterErrorKind::ResourceLimit,"reported peak memory exceeds budget"));}}let handshake=self.handshake();Ok(InvocationResult{{status:InvocationStatus::Succeeded,effects:BTreeMap::from([("format".into(),"exfmt:image:png".into()),("operation".into(),"{spec.strategy}".into())]),losses:BTreeMap::from([("pixels".into(),LossLevel::None),("coordinates".into(),LossLevel::Normalized),("structure".into(),LossLevel::Unbounded),("metadata".into(),LossLevel::Unbounded),("color-semantics".into(),LossLevel::Unknown)]),measurements:Measurements{{input_bytes:Some(report.input_bytes),output_bytes:Some(report.output_bytes),peak_memory_bytes:Some(peak),..Measurements::default()}},capsule_report:BTreeMap::from([("source_width".into(),report.source_width.to_string()),("source_height".into(),report.source_height.to_string()),("source_frames".into(),report.source_frames.to_string()),("width".into(),report.width.to_string()),("height".into(),report.height.to_string())]),warnings:report.warnings,provenance:Provenance{{capsule:handshake.capsule,adapter_id:handshake.adapter_id,adapter_version:handshake.adapter_version,capability_id:CAPABILITY_ID.into(),strategy:"{spec.strategy}".into(),backend:"native-portable".into(),effective_options:defaults()}}}})}}}}
#[cfg(test)]mod tests{{use super::*;use everythingx_kernel::Kernel;#[test]fn kernel_invokes_runnable_defaults(){{let mut kernel=Kernel::default();kernel.register(Box::new(GeneratedGifRenderAdapter)).unwrap();let fixture={module}::conformance_fixture();let mut input=&fixture[..];let mut output=Vec::new();assert_eq!(kernel.invoke_defaults(ADAPTER_ID,CAPABILITY_ID,&mut input,&mut output).unwrap().status,InvocationStatus::Succeeded);assert!(!output.is_empty());}}}}
'''


def render_files(spec: RenderSpec, template: str, native: str, png: str) -> dict[Path, str]:
    root = ROOT / "capsules" / "image" / "raster" / "bridge" / spec.name
    files = common_files(root, spec.name, spec.summary)
    files.update(
        {
            root / "README.md": f"# {spec.name}\n\n{spec.summary}\n\nThe independent zero-dependency crate fully composes GIF frame transparency and disposal before emitting PNG.\n",
            root / "capsule.json": render_manifest(spec),
            root / "src" / "lib.rs": template.replace("__OPERATION__", spec.operation),
            root / "src" / "legacy_native.rs": native,
            root / "src" / "png_native.rs": png,
            root / "everythingx" / "adapter.json": render_adapter_manifest(spec),
            root / "everythingx" / "adapter" / "Cargo.toml": adapter_cargo(spec.name),
            root / "everythingx" / "adapter" / "src" / "lib.rs": render_adapter_source(spec),
        }
    )
    return files


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    conversion = conversion_source()
    native = LEGACY_NATIVE.read_text(encoding="utf-8")
    png = PNG_NATIVE.read_text(encoding="utf-8")
    validate = VALIDATE.read_text(encoding="utf-8")
    render = RENDER.read_text(encoding="utf-8")
    expected: dict[Path, str] = {}
    for spec in DIRECT:
        expected.update(direct_files(spec, conversion, native, png))
    for spec in VALIDATORS:
        expected.update(validate_files(spec, validate, native, png))
    for spec in RENDERS:
        expected.update(render_files(spec, render, native, png))
    stale = [
        path
        for path, content in expected.items()
        if not path.is_file() or path.read_text(encoding="utf-8") != content
    ]
    if args.check:
        if stale:
            print("GIF/ICO Wave C scaffold is stale:")
            for path in stale:
                print(path.relative_to(ROOT))
            return 1
        print(
            f"GIF/ICO Wave C scaffold is current "
            f"({len(DIRECT)} conversions, {len(VALIDATORS)} validators, {len(RENDERS)} animation renderers)"
        )
        return 0
    for path, content in expected.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
    print(f"materialized {len(DIRECT) + len(VALIDATORS) + len(RENDERS)} GIF/ICO Wave C Capsules")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
