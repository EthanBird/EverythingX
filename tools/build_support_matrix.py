#!/usr/bin/env python3
"""Build the checked-in matrix of conversions implemented by real Capsules."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "registry" / "support-matrix.json"


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path}: expected JSON object")
    return value


def build_matrix() -> dict[str, Any]:
    capsules: list[dict[str, Any]] = []
    capabilities: list[dict[str, Any]] = []
    logical_pairs: set[tuple[str, str]] = set()

    for manifest_path in sorted((ROOT / "capsules").glob("*/capsule.json")):
        if manifest_path.parent.name.startswith("_"):
            continue
        manifest = load_json(manifest_path)
        capsule_id = manifest["capsule_id"]
        conversion = manifest["conversion"]
        adapter_path = manifest_path.parent / "everythingx" / "adapter.json"
        adapter = load_json(adapter_path) if adapter_path.is_file() else None
        capsule_capabilities: list[str] = []

        if adapter is not None:
            for capability in adapter.get("capabilities", []):
                capability_id = capability["capability_id"]
                capsule_capabilities.append(capability_id)
                inputs = capability["inputs"]
                outputs = capability["outputs"]
                for source in inputs:
                    for target in outputs:
                        logical_pairs.add((source, target))
                capabilities.append(
                    {
                        "capability_id": capability_id,
                        "capsule_id": capsule_id,
                        "inputs": inputs,
                        "outputs": outputs,
                        "strategy": capability["strategy"],
                        "backend": capability["backend"],
                        "computability": capability["computability"],
                        "loss": capability["loss"],
                        "defaults_are_runnable": capability["defaults_are_runnable"],
                        "streaming": capability["execution"]["streaming"],
                        "seek_required": capability["execution"]["seek_required"],
                    }
                )

        capsules.append(
            {
                "capsule_id": capsule_id,
                "version": manifest["version"],
                "crate": manifest["api"]["crate"],
                "inputs": conversion["source"],
                "outputs": conversion["target"],
                "arity": conversion["arity"],
                "standalone": manifest["independence"]["standalone_cargo_build"],
                "copy_out_tested": manifest["independence"]["copy_out_tested"],
                "capability_ids": sorted(capsule_capabilities),
            }
        )

    capabilities.sort(key=lambda item: item["capability_id"])
    pairs = [
        {"source": source, "target": target}
        for source, target in sorted(logical_pairs)
    ]
    return {
        "schema_version": "0.1.0",
        "meaning": "Implemented conversion support derived from non-template Capsule and Adapter manifests.",
        "update_rule": "Any Capsule or Adapter capability change must regenerate this file with tools/build_support_matrix.py.",
        "summary": {
            "standalone_capsules": len(capsules),
            "adapter_capabilities": len(capabilities),
            "logical_source_target_pairs": len(pairs),
        },
        "logical_pairs": pairs,
        "capsules": capsules,
        "capabilities": capabilities,
    }


def serialized(matrix: dict[str, Any]) -> str:
    return json.dumps(matrix, ensure_ascii=False, indent=2) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="Fail when registry/support-matrix.json is not current.",
    )
    args = parser.parse_args()
    expected = serialized(build_matrix())
    if args.check:
        actual = OUTPUT.read_text(encoding="utf-8") if OUTPUT.is_file() else ""
        if actual != expected:
            print("support matrix is stale; run python3 tools/build_support_matrix.py")
            return 1
        print("support matrix is current")
        return 0
    OUTPUT.write_text(expected, encoding="utf-8")
    print(f"wrote {OUTPUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
