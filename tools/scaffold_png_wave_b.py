#!/usr/bin/env python3
"""Materialize 20 independent PNG Wave B conversion and transform Capsules."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path

import scaffold_raster_wave_a as raster


ROOT = Path(__file__).resolve().parents[1]
NATIVE = ROOT / "tools" / "templates" / "png_native.rs"
TRANSFORM = ROOT / "tools" / "templates" / "png_transform_capsule.rs"
GLUE = ROOT / "tools" / "templates" / "png_conversion_glue.rs"
RASTER_TEMPLATE = ROOT / "tools" / "templates" / "raster_wave_a_capsule.rs"
NOTICE = raster.NOTICE
PNG = raster.Representation("png", "Png", "exfmt:image:png", "Portable Network Graphics", "W3C PNG Third Edition and RFC 1950/1951")
RASTER_FORMATS = raster.REPRESENTATIONS
CONVERSIONS = tuple([raster.Spec(PNG, target) for target in RASTER_FORMATS] + [raster.Spec(source, PNG) for source in RASTER_FORMATS if source.slug != "bmp"])


@dataclass(frozen=True)
class TransformSpec:
    name: str
    operation: str
    role: str
    kind: str
    summary: str
    pixel_loss: str = "none"
    coordinate_loss: str = "none"


TRANSFORMS = (
    TransformSpec("validate-png", "Validate", "analyze", "validate", "Validate PNG structure, CRC, zlib, Deflate, filters and pixels, then copy the exact byte stream."),
    TransformSpec("normalize-png", "Normalize", "structural", "normalize", "Canonicalize any supported PNG into deterministic RGB/RGBA PNG."),
    TransformSpec("png-crop", "Crop", "transform", "project", "Crop a PNG to an explicit rectangle with a runnable full-image default.", "bounded", "normalized"),
    TransformSpec("png-pad", "Pad", "transform", "pad", "Pad a PNG with configurable 16-bit RGBA borders and a runnable zero-border default."),
    TransformSpec("png-flip-horizontal", "FlipHorizontal", "transform", "reorder", "Reflect PNG pixels across the vertical axis."),
    TransformSpec("png-flip-vertical", "FlipVertical", "transform", "reorder", "Reflect PNG pixels across the horizontal axis."),
    TransformSpec("png-rotate-90", "Rotate90", "transform", "reorder", "Rotate PNG pixels 90 degrees clockwise."),
    TransformSpec("png-rotate-180", "Rotate180", "transform", "reorder", "Rotate PNG pixels 180 degrees."),
    TransformSpec("png-rotate-270", "Rotate270", "transform", "reorder", "Rotate PNG pixels 270 degrees clockwise."),
    TransformSpec("png-alpha-premultiply", "AlphaPremultiply", "transform", "map", "Convert unassociated PNG RGB samples to premultiplied code values with defined integer rounding.", "bounded"),
    TransformSpec("png-alpha-unpremultiply", "AlphaUnpremultiply", "transform", "map", "Recover unassociated PNG RGB samples from premultiplied code values with saturation and defined alpha-zero behavior.", "bounded"),
)


def json_text(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, indent=2) + "\n"


def conversion_source() -> str:
    source = RASTER_TEMPLATE.read_text(encoding="utf-8")
    source = source.replace("#![forbid(unsafe_code)]\n", "#![forbid(unsafe_code)]\n\nmod png_native;\n", 1)
    source = source.replace("    Pam,\n}", "    Pam,\n    Png,\n}", 1)
    source = source.replace('            Self::Pam => "pam",\n', '            Self::Pam => "pam",\n            Self::Png => "png",\n', 1)
    source = source.replace("    IntegerOverflow(&'static str),\n    Io(io::Error),", "    IntegerOverflow(&'static str),\n    Png(String),\n    Io(io::Error),", 1)
    source = source.replace('            Self::IntegerOverflow(context) => write!(f, "integer overflow while computing {context}"),\n', '            Self::IntegerOverflow(context) => write!(f, "integer overflow while computing {context}"),\n            Self::Png(message) => write!(f, "PNG codec error: {message}"),\n', 1)
    source = source.replace("        Profile::Pam => decode_pam(bytes, options),\n", "        Profile::Pam => decode_pam(bytes, options),\n        Profile::Png => decode_png(bytes, options),\n", 1)
    source = source.replace("        Profile::Pam => encode_pam(image),\n", "        Profile::Pam => encode_pam(image),\n        Profile::Png => encode_png(image),\n", 1)
    source = source.replace("Error::InvalidSignature(_) | Error::Truncated(_)", "Error::InvalidSignature(_) | Error::Truncated(_) | Error::Png(_)")
    return source + GLUE.read_text(encoding="utf-8")


def defaults() -> dict[str, object]:
    return {
        "background_alpha": 0, "background_blue": 0, "background_green": 0, "background_red": 0,
        "crop_height": 0, "crop_width": 0, "crop_x": 0, "crop_y": 0, "filter": "adaptive",
        "max_inflate_bytes": 1073741824, "max_input_bytes": 536870912, "max_pixels": 100000000,
        "pad_bottom": 0, "pad_left": 0, "pad_right": 0, "pad_top": 0,
        "strict_crc": True, "strict_trailing_data": True,
    }


def capability_id(spec: TransformSpec) -> str:
    return f"capability:{spec.name}/png-native-canonical/native-portable"


def evidence(spec: TransformSpec) -> list[str]:
    path = ROOT / "registry" / "performance" / "baseline.json"
    if path.is_file():
        measured = {row.get("capability_id") for row in json.loads(path.read_text(encoding="utf-8")).get("capabilities", [])}
        if capability_id(spec) in measured:
            return [f"registry/performance/baseline.json#{capability_id(spec)}"]
    return []


def transform_manifest(spec: TransformSpec) -> str:
    cost = evidence(spec)
    return json_text({
        "capsule_id": f"capsule:{spec.name}", "version": "0.1.0", "name": spec.name, "summary": spec.summary,
        "taxonomy": {"domain": "image", "primary_ir": "ir:raster", "secondary_irs": ["ir:container-graph"], "operator_kind": spec.kind, "operator_role": spec.role},
        "license": {"expression": "PolyForm-Noncommercial-1.0.0", "file": "LICENSE", "commercial_authorization_required": True},
        "repository": f"https://github.com/EthanBird/EverythingX/tree/main/capsules/image/raster/{spec.role}/{spec.name}",
        "independence": {"standalone_cargo_build": True, "everythingx_optional": True, "external_path_dependencies": False, "copy_out_tested": True},
        "conversion": {"source": ["exfmt:image:png"], "target": ["exfmt:image:png"], "arity": {"inputs": {"min": 1, "max": 1}, "outputs": {"min": 1, "max": 1}},
            "scope": ["PNG color types 0, 2, 3, 4 and 6", "PNG bit depths 1, 2, 4, 8 and 16 where legal", "Stored, fixed-Huffman and dynamic-Huffman Deflate", "PNG filters 0 through 4 and Adam7 interlace", spec.summary],
            "out_of_scope": ["APNG frame graph operations", "ICC color conversion", "Preservation of arbitrary ancillary chunks after pixel-changing or normalization operations"]},
        "api": {"language": "rust", "crate": spec.name, "entrypoint": "convert", "owns_options_error_report": True, "streaming": False, "seek_required": False},
        "defaults": {"runnable": True, "strategy": "png-native-canonical", "backend": "native-portable", "options": defaults(), "policy": "strict"},
        "strategies": [{"id": "png-native-canonical", "guarantees": ["CRC and Adler-32 are verified", "All legal PNG sample layouts decode to RGBA16 before the operation", "16-bit samples remain 16-bit through canonical output", "The complete input is validated before output is committed"], "tradeoffs": ["Canonical pixel-changing output uses RGB/RGBA and strips ancillary metadata", "The native encoder currently favors deterministic stored Deflate over maximum compression"]}],
        "backends": [{"id": "native-portable", "tier": "native-portable", "default": True, "dependencies": []}],
        "validation": {"specifications": ["W3C PNG Third Edition", "RFC 1950 zlib", "RFC 1951 Deflate"], "conformance": ["src/lib.rs and src/png_native.rs unit tests"], "differential": [], "properties": ["Canonical output decodes independently", "Dimension and allocation arithmetic is checked", "Five PNG filters round-trip"], "regression": ["CRC corruption rejected", "Pixel limit enforced before allocation", "Malformed streams never write a partial output"], "fuzz": ["Planned chunk, Huffman, filter, interlace and transform-coordinate campaigns"], "benchmarks": cost or ["Pending controlled exbench baseline"]},
        "security": {"accepts_untrusted_input": True, "limits": ["max_pixels defaults to 100 million", "max_input_bytes defaults to 512 MiB", "max_inflate_bytes defaults to 1 GiB", "checked chunk, row, pass and output arithmetic"], "known_risks": ["Output I/O failure can occur while committing a fully validated result"]},
    })


def transform_adapter_manifest(spec: TransformSpec) -> str:
    cost = evidence(spec)
    return json_text({"adapter_id": f"adapter:{spec.name}-static", "version": "0.1.0", "capsule": {"id": f"capsule:{spec.name}", "version_requirement": "^0.1.0"}, "protocol": {"name": "everythingx-adapter-protocol", "version_requirement": "0.1"}, "transport": {"kind": "static-rust", "entrypoint": "GeneratedPngAdapter"}, "capabilities": [{"capability_id": capability_id(spec), "capsule_entrypoint": "convert", "strategy": "png-native-canonical", "backend": "native-portable", "inputs": ["exfmt:image:png"], "outputs": ["exfmt:image:png"], "preconditions": ["Input is a structurally valid PNG within declared limits"], "effects": [spec.summary], "invariants": ["PNG pixel sample precision", "Deterministic integer arithmetic"], "computability": "total_for_declared_subset", "loss": {"pixels": spec.pixel_loss, "coordinates": spec.coordinate_loss, "structure": "normalized" if spec.operation != "Validate" else "none", "metadata": "none" if spec.operation == "Validate" else "unbounded", "color-semantics": "none"}, "default_options": defaults(), "defaults_are_runnable": True, "execution": {"streaming": False, "seek_required": False, "cost_evidence": cost}, "report_mapping": {"unknown_fields_are_preserved": True, "rules": ["Dimensions, source PNG properties and operation map to capsule_report", "static Adapter enforces input and output memory shares"]}}]})


def pairs(value: dict[str, object]) -> str:
    return ",".join(f'("{key}".into(),"{str(item).lower() if isinstance(item, bool) else item}".into())' for key, item in sorted(value.items()))


def transform_adapter_source(spec: TransformSpec) -> str:
    module = spec.name.replace("-", "_")
    return f'''#![forbid(unsafe_code)]
use std::collections::BTreeMap;use std::io::{{self,Read,Write}};
use everythingx_protocol::{{AdapterError,AdapterErrorKind,AdapterHandshake,CapabilityDescriptor,CapsuleIdentity,InvocationRequest,InvocationResult,InvocationStatus,LossLevel,Measurements,ProtocolVersion,Provenance,StaticAdapter}};use {module}::{{Error as CapsuleError,Options}};
pub const ADAPTER_ID:&str="adapter:{spec.name}-static";pub const CAPABILITY_ID:&str="{capability_id(spec)}";pub struct GeneratedPngAdapter;
fn defaults()->BTreeMap<String,String>{{BTreeMap::from([{pairs(defaults())}])}}fn descriptor()->CapabilityDescriptor{{CapabilityDescriptor{{capability_id:CAPABILITY_ID.into(),source_formats:vec!["exfmt:image:png".into()],target_formats:vec!["exfmt:image:png".into()],strategy:"png-native-canonical".into(),backend:"native-portable".into(),default_options:defaults(),defaults_are_runnable:true,streaming:false,seek_required:false}}}}
struct Limited<'a>{{inner:&'a mut dyn Write,remaining:u64,exceeded:bool}}impl Write for Limited<'_>{{fn write(&mut self,b:&[u8])->io::Result<usize>{{if b.len()as u64>self.remaining{{self.exceeded=true;return Err(io::Error::other("output budget exceeded"));}}let n=self.inner.write(b)?;self.remaining-=n as u64;Ok(n)}}fn flush(&mut self)->io::Result<()>{{self.inner.flush()}}}}
impl StaticAdapter for GeneratedPngAdapter{{fn handshake(&self)->AdapterHandshake{{AdapterHandshake{{protocol:ProtocolVersion::CURRENT,adapter_id:ADAPTER_ID.into(),adapter_version:"0.1.0".into(),capsule:CapsuleIdentity{{id:"capsule:{spec.name}".into(),version:"0.1.0".into(),content_hash:None}},capabilities:vec![descriptor()]}}}}fn invoke(&self,request:&InvocationRequest,input:&mut dyn Read,output:&mut dyn Write)->Result<InvocationResult,AdapterError>{{if request.capability_id!=CAPABILITY_ID{{return Err(AdapterError::new(AdapterErrorKind::UnsupportedCapability,"unsupported capability"));}}if request.options!=defaults(){{return Err(AdapterError::new(AdapterErrorKind::InvalidOptions,"version 0.1 static Adapter accepts its declared defaults"));}}let limit=request.resource_budget.max_memory_bytes/4;let mut bytes=Vec::new();input.take(limit.saturating_add(1)).read_to_end(&mut bytes).map_err(|e|AdapterError::new(AdapterErrorKind::Io,e.to_string()))?;if bytes.len()as u64>limit{{return Err(AdapterError::new(AdapterErrorKind::ResourceLimit,"input exceeds Adapter memory share"));}}let adapter_memory=bytes.len()as u64;let mut source=&bytes[..];let mut limited=Limited{{inner:output,remaining:request.resource_budget.max_output_bytes,exceeded:false}};let report={module}::convert(&mut source,&mut limited,&Options::default()).map_err(|error|match error{{CapsuleError::Io(io)if limited.exceeded=>AdapterError::new(AdapterErrorKind::ResourceLimit,io.to_string()),CapsuleError::Io(io)=>AdapterError::new(AdapterErrorKind::Io,io.to_string()),limited_error@(CapsuleError::InputTooLarge{{..}}|CapsuleError::PixelLimitExceeded{{..}})=>AdapterError::new(AdapterErrorKind::ResourceLimit,limited_error.to_string()),other=>AdapterError::new(AdapterErrorKind::InvalidInput,other.to_string())}})?;let peak=adapter_memory.saturating_add(report.peak_working_memory_bytes);if peak>request.resource_budget.max_memory_bytes{{return Err(AdapterError::new(AdapterErrorKind::ResourceLimit,"reported peak memory exceeds request budget"));}}let handshake=self.handshake();Ok(InvocationResult{{status:InvocationStatus::Succeeded,effects:BTreeMap::from([("format".into(),"exfmt:image:png".into()),("operation".into(),report.operation.into())]),losses:BTreeMap::from([("pixels".into(),LossLevel::{"None" if spec.pixel_loss == "none" else "Bounded"}),("coordinates".into(),LossLevel::{"None" if spec.coordinate_loss == "none" else "Normalized"}),("structure".into(),LossLevel::{"None" if spec.operation == "Validate" else "Normalized"}),("metadata".into(),LossLevel::{"None" if spec.operation == "Validate" else "Unbounded"}),("color-semantics".into(),LossLevel::None)]),measurements:Measurements{{input_bytes:Some(report.input_bytes),output_bytes:Some(report.output_bytes),peak_memory_bytes:Some(peak),..Measurements::default()}},capsule_report:BTreeMap::from([("width".into(),report.width.to_string()),("height".into(),report.height.to_string()),("pixels".into(),report.pixels.to_string()),("source_bit_depth".into(),report.source_bit_depth.to_string()),("source_color_type".into(),report.source_color_type.to_string()),("source_interlaced".into(),report.source_interlaced.to_string())]),warnings:report.warnings,provenance:Provenance{{capsule:handshake.capsule,adapter_id:handshake.adapter_id,adapter_version:handshake.adapter_version,capability_id:CAPABILITY_ID.into(),strategy:"png-native-canonical".into(),backend:"native-portable".into(),effective_options:defaults()}}}})}}}}
#[cfg(test)]mod tests{{use super::*;use everythingx_kernel::Kernel;#[test]fn kernel_invokes_runnable_defaults(){{let mut kernel=Kernel::default();kernel.register(Box::new(GeneratedPngAdapter)).unwrap();let fixture={module}::conformance_fixture();let mut input=&fixture[..];let mut output=Vec::new();assert_eq!(kernel.invoke_defaults(ADAPTER_ID,CAPABILITY_ID,&mut input,&mut output).unwrap().status,InvocationStatus::Succeeded);assert!(!output.is_empty());}}}}
'''


def common_files(root: Path, name: str, description: str) -> dict[Path, str]:
    return {root / "Cargo.toml": f'''[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2024"\npublish = false\nlicense-file = "LICENSE"\ndescription = "{description}"\n\n[lib]\npath = "src/lib.rs"\n\n[dependencies]\n''', root / "Cargo.lock": f'''# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = "{name}"\nversion = "0.1.0"\n''', root / "LICENSE": NOTICE, root / "benches" / "README.md": "# Benchmarks\n\nCovered by the repository-wide release-mode Kernel/Adapter performance harness.\n", root / "fuzz" / "README.md": "# Fuzzing\n\nPlanned chunk, Huffman, filter, interlace and coordinate campaigns.\n"}


def transform_files(spec: TransformSpec, transform: str, native: str) -> dict[Path, str]:
    root = ROOT / "capsules" / "image" / "raster" / spec.role / spec.name
    files = common_files(root, spec.name, spec.summary)
    files.update({root / "README.md": f"# {spec.name}\n\n{spec.summary}\n\nThis is an independent, zero-dependency Rust crate. Copy this directory anywhere and `cargo test`; the `everythingx/` Adapter is optional. Runnable defaults are defined by `Options::default()`.\n", root / "capsule.json": transform_manifest(spec), root / "src" / "lib.rs": transform.replace("__OPERATION__", spec.operation), root / "src" / "png_native.rs": native, root / "everythingx" / "adapter.json": transform_adapter_manifest(spec), root / "everythingx" / "adapter" / "Cargo.toml": f'''[package]\nname = "everythingx-adapter-{spec.name}"\nversion = "0.1.0"\nedition = "2024"\npublish = false\n\n[lib]\npath = "src/lib.rs"\n\n[dependencies]\neverythingx-protocol = {{ path = "../../../../../../../kernel/ex-protocol" }}\n{spec.name} = {{ path = "../.." }}\n\n[dev-dependencies]\neverythingx-kernel = {{ path = "../../../../../../../kernel/ex-kernel" }}\n''', root / "everythingx" / "adapter" / "src" / "lib.rs": transform_adapter_source(spec)})
    return files


def main() -> int:
    parser=argparse.ArgumentParser();parser.add_argument("--check",action="store_true");args=parser.parse_args()
    native=NATIVE.read_text(encoding="utf-8");transform=TRANSFORM.read_text(encoding="utf-8");conversion=conversion_source();expected:dict[Path,str]={}
    for spec in CONVERSIONS:
        files=raster.files_for(spec,conversion);root=ROOT/"capsules"/"image"/"raster"/"direct"/spec.name;files[root/"src"/"png_native.rs"]=native;files[root/"README.md"]=f"# {spec.name}\n\nIndependent, zero-dependency Rust conversion from {spec.source.label} to {spec.target.label}. PNG parsing covers all legal color types and depths, all Deflate block types, five filters and Adam7. A 16-bit PNG source requires the explicit `allow_sample_scaling` option when targeting an 8-bit carrier.\n";expected.update(files)
    for spec in TRANSFORMS:expected.update(transform_files(spec,transform,native))
    stale=[path for path,content in expected.items() if not path.is_file() or path.read_text(encoding="utf-8")!=content]
    if args.check:
        if stale:
            print("PNG Wave B scaffold is stale:");[print(path.relative_to(ROOT)) for path in stale];return 1
        print(f"PNG Wave B scaffold is current ({len(CONVERSIONS)} conversions, {len(TRANSFORMS)} PNG operators)");return 0
    for path,content in expected.items():path.parent.mkdir(parents=True,exist_ok=True);path.write_text(content,encoding="utf-8")
    print(f"materialized {len(CONVERSIONS)+len(TRANSFORMS)} independent PNG Wave B Capsules");return 0


if __name__ == "__main__": raise SystemExit(main())
