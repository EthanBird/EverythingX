#!/usr/bin/env python3
"""Dependency-free integrity checks for the EverythingX foundation artifact."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path
from typing import Any


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


def main() -> int:
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

    operator_required = {
        "operator_id", "contract_version", "implementation", "family", "layers",
        "arity", "inputs", "outputs", "preconditions", "effects", "invariants",
        "computability", "loss", "algebra", "execution", "dependencies", "security", "evidence"
    }
    operator_ids: set[str] = set()
    for path in sorted((ROOT / "operators").glob("*/operator.json")):
        record = load_json(path)
        require_fields(record, operator_required, str(path.relative_to(ROOT)))
        operator_id = record.get("operator_id", "")
        if not operator_id.startswith("exop:"):
            raise ValueError(f"{path}: invalid operator_id")
        if operator_id in operator_ids:
            raise ValueError(f"{path}: duplicate operator_id {operator_id}")
        operator_ids.add(operator_id)

    summary = load_json(ROOT / "catalog" / "summary.json")
    if summary["observation_count"] != len(sources):
        raise ValueError("catalog summary observation_count does not match NDJSON")
    if summary["catalog_sha256"] != digest(source_path):
        raise ValueError("catalog summary SHA-256 does not match NDJSON")

    print(
        json.dumps(
            {
                "status": "ok",
                "json_files": len(json_paths),
                "source_records": len(source_ids),
                "canonical_seeds": len(canonical_ids),
                "operator_manifests": len(operator_ids),
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as error:
        print(f"validation failed: {error}", file=sys.stderr)
        raise SystemExit(1)

