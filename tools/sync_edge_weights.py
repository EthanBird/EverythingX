#!/usr/bin/env python3
"""Materialize measured graph-edge weights inside every production Capsule."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
BASELINE = ROOT / "registry" / "performance" / "baseline.json"
EDGE_WEIGHT_NAME = "edge-weight.json"
SCHEMA_ID = "https://everythingx.dev/schema/edge-weight.schema.json"


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path}: expected JSON object")
    return value


def canonical_sha256(value: Any) -> str:
    payload = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def is_production_manifest(path: Path) -> bool:
    relative_parts = path.relative_to(ROOT / "capsules").parts
    return not any(part.startswith("_") for part in relative_parts)


def production_capsules() -> list[tuple[Path, dict[str, Any], dict[str, Any]]]:
    capsules: list[tuple[Path, dict[str, Any], dict[str, Any]]] = []
    for manifest_path in sorted((ROOT / "capsules").rglob("capsule.json")):
        if not is_production_manifest(manifest_path):
            continue
        adapter_path = manifest_path.parent / "everythingx" / "adapter.json"
        if not adapter_path.is_file():
            raise ValueError(f"{manifest_path}: production Capsule has no Adapter manifest")
        capsules.append((manifest_path, load_json(manifest_path), load_json(adapter_path)))
    return capsules


def compact_rustc(value: Any) -> str:
    first_line = str(value or "unknown").splitlines()[0].strip()
    return first_line or "unknown"


def profile_document(baseline: dict[str, Any]) -> dict[str, Any]:
    environment = baseline["environment"]
    method = baseline["method"]
    return {
        "profile_id": baseline["profile_id"],
        "generated_at": baseline["generated_at"],
        "source_report": "registry/performance/baseline.json",
        "scope": baseline["scope"],
        "comparability": baseline["comparability"],
        "harness_sha256": method["harness_sha256"],
        "environment_fingerprint_sha256": canonical_sha256(environment),
        "environment_class": {
            "os": environment["os"],
            "architecture": environment["architecture"],
            "rustc": compact_rustc(environment.get("rustc")),
            "github_runner_image": environment.get("github_runner_image", "unknown"),
        },
        "evidence_commit_sha": environment["commit_sha"],
        "evidence_run_id": str(environment["github_run_id"]),
    }


def planner_contract() -> dict[str, Any]:
    return {
        "kind": "environment-bound-empirical-vector",
        "input_variable": "N = input byte length",
        "selection_order": [
            "semantic and resource hard constraints",
            "raw edge-weight vector at the concrete input size",
            "derived load only for same-profile equivalent-edge tie-breaking",
        ],
        "path_composition": {
            "latency": "sum sequential edge estimates",
            "peak_memory": "maximum live-set estimate; never blindly sum edge peaks",
            "output_bytes": "feed each edge estimate into the next edge input",
        },
    }


def capability_weight(row: dict[str, Any], adapter_capability: dict[str, Any]) -> dict[str, Any]:
    capability_id = row["capability_id"]
    model = row["cost_model"]
    score = round(float(row["score"]["overall_0_to_100"]), 3)
    load = round(100.0 - score, 3)
    if load == -0.0:
        load = 0.0
    return {
        "capability_id": capability_id,
        "strategy": row["strategy"],
        "backend": row["backend"],
        "inputs": adapter_capability["inputs"],
        "outputs": adapter_capability["outputs"],
        "edge_weight": {
            "kind": "empirical-multidimensional",
            "fixed_latency_micros": model["fixed_latency_micros"],
            "nanoseconds_per_input_byte": model["nanoseconds_per_input_byte"],
            "peak_memory_bytes_per_input_byte": model["peak_memory_bytes_per_input_byte"],
            "output_bytes_per_input_byte": model["output_bytes_per_input_byte"],
            "estimated_latency_micros": "fixed_latency_micros + nanoseconds_per_input_byte * N / 1000",
            "estimated_peak_memory_bytes": "peak_memory_bytes_per_input_byte * N",
            "estimated_output_bytes": "output_bytes_per_input_byte * N",
            "performance_score_0_to_100": score,
            "load_0_to_100": load,
            "load_direction": "higher-is-more-expensive",
        },
        "observations": row["workloads"],
        "source_evidence": f"registry/performance/baseline.json#{capability_id}",
    }


def build_documents(*, allow_baseline_lag: bool = False) -> dict[Path, dict[str, Any]]:
    baseline = load_json(BASELINE)
    profile = profile_document(baseline)
    rows = baseline["capabilities"]
    rows_by_id: dict[str, dict[str, Any]] = {}
    for row in rows:
        capability_id = row["capability_id"]
        if capability_id in rows_by_id:
            raise ValueError(f"duplicate baseline capability {capability_id}")
        rows_by_id[capability_id] = row

    documents: dict[Path, dict[str, Any]] = {}
    discovered_capabilities: set[str] = set()
    measured_capabilities: set[str] = set()
    for manifest_path, manifest, adapter in production_capsules():
        capsule_id = manifest["capsule_id"]
        adapter_capabilities = {
            capability["capability_id"]: capability
            for capability in adapter.get("capabilities", [])
        }
        discovered_capabilities.update(adapter_capabilities)
        weighted: list[dict[str, Any]] = []
        for capability_id, adapter_capability in sorted(adapter_capabilities.items()):
            row = rows_by_id.get(capability_id)
            if row is None:
                if allow_baseline_lag:
                    continue
                raise ValueError(f"baseline has no measurement for {capability_id}")
            if row.get("capsule_id") != capsule_id:
                raise ValueError(f"{capability_id}: baseline Capsule ID does not match {capsule_id}")
            if row.get("strategy") != adapter_capability.get("strategy"):
                raise ValueError(f"{capability_id}: baseline strategy does not match Adapter")
            if row.get("backend") != adapter_capability.get("backend"):
                raise ValueError(f"{capability_id}: baseline backend does not match Adapter")
            if row.get("source_format") not in adapter_capability.get("inputs", []):
                raise ValueError(f"{capability_id}: baseline source format is not an Adapter input")
            measured_capabilities.add(capability_id)
            weighted.append(capability_weight(row, adapter_capability))

        if not weighted:
            if allow_baseline_lag:
                continue
            raise ValueError(f"{manifest_path}: no measured capability")

        repository_path = str(manifest_path.parent.relative_to(ROOT))
        documents[manifest_path.parent / EDGE_WEIGHT_NAME] = {
            "$schema": SCHEMA_ID,
            "schema_version": "0.1.0",
            "meaning": (
                "Environment-bound empirical graph-edge load for this standalone Capsule; "
                "semantic loss and hard constraints are evaluated separately."
            ),
            "capsule": {
                "id": capsule_id,
                "version": manifest["version"],
                "repository_path": repository_path,
            },
            "profile": profile,
            "planner_contract": planner_contract(),
            "capabilities": weighted,
        }

    unknown_rows = set(rows_by_id) - discovered_capabilities
    if unknown_rows:
        raise ValueError(f"baseline contains unknown production capabilities: {sorted(unknown_rows)}")
    if not allow_baseline_lag and measured_capabilities != discovered_capabilities:
        missing = sorted(discovered_capabilities - measured_capabilities)
        raise ValueError(f"baseline does not cover all production capabilities: {missing}")
    return documents


def serialized(document: dict[str, Any]) -> str:
    return json.dumps(document, ensure_ascii=False, indent=2) + "\n"


def actual_weight_paths() -> set[Path]:
    return set((ROOT / "capsules").rglob(EDGE_WEIGHT_NAME))


def check_documents(documents: dict[Path, dict[str, Any]]) -> list[str]:
    errors: list[str] = []
    expected_paths = set(documents)
    actual_paths = actual_weight_paths()
    for path in sorted(expected_paths - actual_paths):
        errors.append(f"missing {path.relative_to(ROOT)}")
    for path in sorted(actual_paths - expected_paths):
        errors.append(f"unexpected {path.relative_to(ROOT)}")
    for path in sorted(expected_paths & actual_paths):
        if path.read_text(encoding="utf-8") != serialized(documents[path]):
            errors.append(f"stale {path.relative_to(ROOT)}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="Fail if a Capsule edge-weight.json is missing, stale or unexpected.",
    )
    args = parser.parse_args()
    documents = build_documents()
    if args.check:
        errors = check_documents(documents)
        if errors:
            for error in errors:
                print(error)
            print("Capsule edge weights are stale; run python3 tools/sync_edge_weights.py")
            return 1
        capabilities = sum(len(document["capabilities"]) for document in documents.values())
        print(f"Capsule edge weights are current: {len(documents)} files, {capabilities} capabilities")
        return 0

    for path, document in documents.items():
        path.write_text(serialized(document), encoding="utf-8")
    capabilities = sum(len(document["capabilities"]) for document in documents.values())
    print(f"wrote {len(documents)} Capsule edge-weight files for {capabilities} capabilities")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
