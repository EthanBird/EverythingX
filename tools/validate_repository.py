#!/usr/bin/env python3
"""Dependency-free integrity checks for the EverythingX foundation artifact."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

from build_support_matrix import build_matrix
from build_audio_backlog import build_backlog
from build_image_backlog import build_backlog as build_image_backlog
from build_operator_universe import build_operator_universe
from sync_edge_weights import build_documents as build_edge_weight_documents
from sync_edge_weights import check_documents as check_edge_weight_documents


ROOT = Path(__file__).resolve().parents[1]


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def load_ndjson(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as error:
            raise ValueError(f"{path}:{line_number}: {error}") from error
        if not isinstance(value, dict):
            raise ValueError(f"{path}:{line_number}: expected object")
        records.append(value)
    return records


def digest(path: Path) -> str:
    result = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def require_fields(record: dict[str, Any], fields: set[str], location: str) -> None:
    missing = sorted(fields - record.keys())
    if missing:
        raise ValueError(f"{location}: missing fields {missing}")


def ensure_unique(records: list[dict[str, Any]], key: str, location: str) -> set[str]:
    seen: set[str] = set()
    for index, record in enumerate(records, 1):
        value = record.get(key)
        if not isinstance(value, str) or not value:
            raise ValueError(f"{location}:{index}: invalid {key}")
        if value in seen:
            raise ValueError(f"{location}:{index}: duplicate {key} {value}")
        seen.add(value)
    return seen


def validate_facets(record: dict[str, Any], vocabulary: dict[str, Any], location: str) -> None:
    for facet_name, values in record.get("facets", {}).items():
        if facet_name not in vocabulary:
            raise ValueError(f"{location}: unknown facet {facet_name}")
        if not isinstance(values, list):
            raise ValueError(f"{location}: facet {facet_name} must be a list")
        allowed = vocabulary[facet_name].get("values", {})
        open_values = vocabulary[facet_name].get("open_values", False)
        for value in values:
            if value not in allowed and not open_values:
                raise ValueError(f"{location}: unknown {facet_name} value {value}")


def main(allow_performance_baseline_lag: bool = False) -> int:
    # Parse every authored JSON and schema before deeper checks.
    json_paths = sorted(ROOT.glob("**/*.json"))
    for path in json_paths:
        load_json(path)

    source_path = ROOT / "catalog" / "source_records.ndjson"
    sources = load_ndjson(source_path)
    source_required = {"record_id", "source", "external_ids", "names", "evidence"}
    for index, record in enumerate(sources, 1):
        require_fields(record, source_required, f"source_records.ndjson:{index}")
        if not record["record_id"].startswith("src:"):
            raise ValueError(f"source_records.ndjson:{index}: invalid record_id prefix")
        if not record["names"]:
            raise ValueError(f"source_records.ndjson:{index}: names must not be empty")
    source_ids = ensure_unique(sources, "record_id", "source_records.ndjson")

    canonical_path = ROOT / "canonical" / "seed.ndjson"
    canonical = load_ndjson(canonical_path)
    canonical_required = {"format_id", "kind", "names", "status", "facets", "mappings"}
    vocabulary = load_json(ROOT / "ontology" / "facets.json")["facets"]
    for index, record in enumerate(canonical, 1):
        require_fields(record, canonical_required, f"canonical/seed.ndjson:{index}")
        validate_facets(record, vocabulary, f"canonical/seed.ndjson:{index}")
        for mapping in record["mappings"]:
            target = mapping.get("source_record_id")
            if target not in source_ids:
                raise ValueError(f"canonical/seed.ndjson:{index}: missing source mapping {target}")
    canonical_ids = ensure_unique(canonical, "format_id", "canonical/seed.ndjson")

    capsule_required = {
        "capsule_id", "version", "name", "summary", "taxonomy", "license", "independence",
        "conversion", "api", "defaults", "strategies", "backends", "validation", "security"
    }
    capsule_ids: set[str] = set()
    production_capsule_ids: set[str] = set()
    adapter_ids: set[str] = set()
    capability_ids: set[str] = set()
    production_capability_ids: set[str] = set()
    capability_cost_evidence: dict[str, list[str]] = {}
    for path in sorted((ROOT / "capsules").rglob("capsule.json")):
        record = load_json(path)
        require_fields(record, capsule_required, str(path.relative_to(ROOT)))
        capsule_id = record.get("capsule_id", "")
        if not capsule_id.startswith("capsule:"):
            raise ValueError(f"{path}: invalid capsule_id")
        if capsule_id in capsule_ids:
            raise ValueError(f"{path}: duplicate capsule_id {capsule_id}")
        capsule_ids.add(capsule_id)
        taxonomy = record.get("taxonomy", {})
        relative_parts = path.parent.relative_to(ROOT / "capsules").parts
        is_production = not any(part.startswith("_") for part in relative_parts)
        if is_production:
            production_capsule_ids.add(capsule_id)
            expected_prefix = (
                taxonomy.get("domain"),
                str(taxonomy.get("primary_ir", "")).removeprefix("ir:"),
                taxonomy.get("operator_role"),
            )
            if relative_parts[:3] != expected_prefix:
                raise ValueError(
                    f"{path}: directory prefix {relative_parts[:3]} does not match taxonomy {expected_prefix}"
                )
        independence = record.get("independence", {})
        if independence.get("standalone_cargo_build") is not True:
            raise ValueError(f"{path}: standalone_cargo_build must be true")
        if independence.get("everythingx_optional") is not True:
            raise ValueError(f"{path}: everythingx_optional must be true")
        if independence.get("external_path_dependencies") is not False:
            raise ValueError(f"{path}: external_path_dependencies must be false")
        defaults = record.get("defaults", {})
        if defaults.get("runnable") is not True:
            raise ValueError(f"{path}: defaults.runnable must be true")
        strategy_ids = {item.get("id") for item in record.get("strategies", [])}
        backend_ids = {item.get("id") for item in record.get("backends", [])}
        if defaults.get("strategy") not in strategy_ids:
            raise ValueError(f"{path}: default strategy is not declared")
        if defaults.get("backend") not in backend_ids:
            raise ValueError(f"{path}: default backend is not declared")

        adapter_path = path.parent / "everythingx" / "adapter.json"
        if not adapter_path.is_file():
            continue
        adapter = load_json(adapter_path)
        require_fields(
            adapter,
            {"adapter_id", "version", "capsule", "protocol", "transport", "capabilities"},
            str(adapter_path.relative_to(ROOT)),
        )
        adapter_id = adapter.get("adapter_id", "")
        if not adapter_id.startswith("adapter:") or adapter_id in adapter_ids:
            raise ValueError(f"{adapter_path}: invalid or duplicate adapter_id {adapter_id}")
        adapter_ids.add(adapter_id)
        if adapter.get("capsule", {}).get("id") != capsule_id:
            raise ValueError(f"{adapter_path}: adapter Capsule ID does not match {capsule_id}")
        for capability in adapter.get("capabilities", []):
            capability_id = capability.get("capability_id", "")
            if not capability_id.startswith("capability:") or capability_id in capability_ids:
                raise ValueError(f"{adapter_path}: invalid or duplicate capability_id {capability_id}")
            capability_ids.add(capability_id)
            if is_production:
                production_capability_ids.add(capability_id)
                capability_cost_evidence[capability_id] = capability.get("execution", {}).get("cost_evidence", [])
            if capability.get("defaults_are_runnable") is not True:
                raise ValueError(f"{adapter_path}: {capability_id} defaults_are_runnable must be true")
            if capability.get("strategy") == defaults.get("strategy") and capability.get("backend") == defaults.get("backend"):
                if capability.get("default_options") != defaults.get("options"):
                    raise ValueError(f"{adapter_path}: {capability_id} default_options do not match Capsule defaults")

    summary = load_json(ROOT / "catalog" / "summary.json")
    if summary["observation_count"] != len(sources):
        raise ValueError("catalog summary observation_count does not match NDJSON")
    if summary["catalog_sha256"] != digest(source_path):
        raise ValueError("catalog summary SHA-256 does not match NDJSON")

    support_matrix = load_json(ROOT / "registry" / "support-matrix.json")
    if support_matrix != build_matrix():
        raise ValueError(
            "registry/support-matrix.json is stale; run tools/build_support_matrix.py"
        )

    audio_backlog = load_json(ROOT / "operators" / "audio" / "backlog.json")
    if audio_backlog != build_backlog():
        raise ValueError(
            "operators/audio/backlog.json is stale; run tools/build_audio_backlog.py"
        )

    image_backlog = load_json(ROOT / "operators" / "image" / "backlog.json")
    if image_backlog != build_image_backlog():
        raise ValueError(
            "operators/image/backlog.json is stale; run tools/build_image_backlog.py"
        )

    operator_backlog = load_json(ROOT / "operators" / "backlog.json")
    if operator_backlog != build_operator_universe():
        raise ValueError(
            "operators/backlog.json is stale; run tools/build_operator_universe.py"
        )

    performance = load_json(ROOT / "registry" / "performance" / "baseline.json")
    require_fields(
        performance,
        {"schema_version", "profile_id", "generated_at", "scope", "comparability", "environment", "method", "summary", "capabilities"},
        "registry/performance/baseline.json",
    )
    performance_rows = performance["capabilities"]
    measured_ids = ensure_unique(performance_rows, "capability_id", "registry/performance/baseline.json")
    measured_capsules = {row.get("capsule_id") for row in performance_rows}
    if allow_performance_baseline_lag:
        if not measured_ids.issubset(production_capability_ids):
            raise ValueError("performance baseline contains capabilities absent from production")
        if not measured_capsules.issubset(production_capsule_ids):
            raise ValueError("performance baseline contains Capsules absent from production")
    else:
        if measured_ids != production_capability_ids:
            raise ValueError("performance baseline capability coverage does not match production Adapter capabilities")
        if measured_capsules != production_capsule_ids:
            raise ValueError("performance baseline Capsule coverage does not match production Capsules")
    if performance.get("summary") != {"capabilities": len(measured_ids), "capsules": len(measured_capsules)}:
        raise ValueError("performance baseline summary does not match measured records")
    for row in performance_rows:
        capability_id = row["capability_id"]
        score = row.get("score", {}).get("overall_0_to_100")
        if not isinstance(score, (int, float)) or not 0 <= score <= 100:
            raise ValueError(f"performance baseline has invalid score for {capability_id}")
        model = row.get("cost_model", {})
        for field in ("fixed_latency_micros", "nanoseconds_per_input_byte", "peak_memory_bytes_per_input_byte", "output_bytes_per_input_byte"):
            value = model.get(field)
            if not isinstance(value, (int, float)) or value < 0:
                raise ValueError(f"performance baseline has invalid {field} for {capability_id}")
        expected_evidence = f"registry/performance/baseline.json#{capability_id}"
        if expected_evidence not in capability_cost_evidence.get(capability_id, []):
            raise ValueError(f"{capability_id} does not link its checked-in performance evidence")

    edge_weight_documents = build_edge_weight_documents(
        allow_baseline_lag=allow_performance_baseline_lag
    )
    edge_weight_errors = check_edge_weight_documents(edge_weight_documents)
    if edge_weight_errors:
        detail = "; ".join(edge_weight_errors[:10])
        if len(edge_weight_errors) > 10:
            detail += f"; and {len(edge_weight_errors) - 10} more"
        raise ValueError(
            "Capsule-local edge weights are missing or stale; "
            f"run tools/sync_edge_weights.py ({detail})"
        )

    print(
        json.dumps(
            {
                "status": "ok",
                "json_files": len(json_paths),
                "source_records": len(source_ids),
                "canonical_seeds": len(canonical_ids),
                "capsule_manifests": len(capsule_ids),
                "adapter_manifests": len(adapter_ids),
                "capabilities": len(capability_ids),
                "benchmarked_production_capsules": len(measured_capsules),
                "benchmarked_production_capabilities": len(measured_ids),
                "capsule_edge_weight_files": len(edge_weight_documents),
                "weighted_production_capabilities": sum(
                    len(document["capabilities"])
                    for document in edge_weight_documents.values()
                ),
                "supported_logical_pairs": support_matrix["summary"]["logical_source_target_pairs"],
                "audio_representations": audio_backlog["summary"]["reviewed_representations"],
                "audio_pair_candidates": audio_backlog["summary"]["ordered_pair_candidates"],
                "image_representations": image_backlog["summary"]["reviewed_representations"],
                "image_pair_candidates": image_backlog["summary"]["ordered_pair_candidates"],
                "object_ir_operator_positions": operator_backlog["summary"]["object_ir_operator_positions"],
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--allow-performance-baseline-lag",
        action="store_true",
        help="Bootstrap a larger benchmark run while requiring the checked-in baseline to remain a valid production subset.",
    )
    try:
        raise SystemExit(main(parser.parse_args().allow_performance_baseline_lag))
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as error:
        print(f"validation failed: {error}", file=sys.stderr)
        raise SystemExit(1)
