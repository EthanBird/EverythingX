#!/usr/bin/env python3
"""Expand Object IRs, operator kinds and semantic families into research cells."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "operators" / "operator-basis.json"
OUTPUT = ROOT / "operators" / "backlog.json"


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path}: expected JSON object")
    return value


def digest_records(records: list[dict[str, str]]) -> str:
    encoded = json.dumps(records, ensure_ascii=False, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


def build_operator_universe(include_materialized_positions: bool = False) -> dict[str, Any]:
    basis = load_json(SOURCE)
    facets = load_json(ROOT / "ontology" / "facets.json")
    semantic_families = facets["facets"]["semantic_family"]["values"]
    ir_cells: list[dict[str, str]] = []
    family_cells: list[dict[str, str]] = []
    kind_count = 0

    for operator_family in basis["families"]:
        kind_count += len(operator_family["kinds"])
        for object_ir in basis["object_irs"]:
            for kind in operator_family["kinds"]:
                ir_name = object_ir["id"].removeprefix("ir:")
                ir_cells.append(
                    {
                        "candidate_id": f"operator-position:{ir_name}/{kind}",
                        "object_ir": object_ir["id"],
                        "layer": object_ir["layer"],
                        "operator_family": operator_family["id"],
                        "kind": kind,
                        "applicability": "unknown",
                        "knowledge_status": "candidate",
                    }
                )
        for semantic_family in semantic_families:
            catalog_status = (
                "family-catalog-present"
                if semantic_family == "audio-signal"
                else "family-catalog-not-created"
            )
            family_cells.append(
                {
                    "research_cell_id": (
                        f"family-operator-cell:{semantic_family}/{operator_family['id']}"
                    ),
                    "semantic_family": semantic_family,
                    "operator_family": operator_family["id"],
                    "catalog_status": catalog_status,
                    "review_status": "not-reviewed",
                }
            )

    ir_cells.sort(key=lambda item: item["candidate_id"])
    family_cells.sort(key=lambda item: item["research_cell_id"])
    result = {
        "schema_version": "0.1.0",
        "generated_from": [
            "operators/operator-basis.json",
            "ontology/facets.json",
        ],
        "meaning": "Exhaustive research positions for the current finite operator basis; a position may later be marked not-applicable or impossible.",
        "summary": {
            "object_irs": len(basis["object_irs"]),
            "operator_families": len(basis["families"]),
            "operator_kinds": kind_count,
            "object_ir_operator_positions": len(ir_cells),
            "semantic_families": len(semantic_families),
            "semantic_family_operator_cells": len(family_cells),
            "family_catalogs_present": 1,
        },
        "object_ir_operator_set": {
            "expansion": "every object IR crossed with every operator kind",
            "object_ir_ids": [item["id"] for item in basis["object_irs"]],
            "operator_families": [
                {"id": item["id"], "kinds": item["kinds"]}
                for item in basis["families"]
            ],
            "defaults": {
                "applicability": "unknown",
                "knowledge_status": "candidate",
            },
            "position_count": len(ir_cells),
            "materialized_sha256": digest_records(ir_cells),
        },
        "semantic_family_operator_set": {
            "expansion": "every semantic family crossed with every operator family",
            "semantic_families": sorted(semantic_families),
            "operator_family_ids": [item["id"] for item in basis["families"]],
            "catalog_status_overrides": {
                "audio-signal": "family-catalog-present"
            },
            "default_catalog_status": "family-catalog-not-created",
            "default_review_status": "not-reviewed",
            "cell_count": len(family_cells),
            "materialized_sha256": digest_records(family_cells),
        },
    }
    if include_materialized_positions:
        result["materialized_object_ir_operator_positions"] = ir_cells
        result["materialized_semantic_family_operator_cells"] = family_cells
    return result


def serialized(backlog: dict[str, Any]) -> str:
    return json.dumps(backlog, ensure_ascii=False, indent=2) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument(
        "--materialize",
        type=Path,
        help="Write every expanded research position to the requested path.",
    )
    args = parser.parse_args()
    if args.materialize is not None:
        args.materialize.write_text(
            serialized(build_operator_universe(include_materialized_positions=True)),
            encoding="utf-8",
        )
        print(f"wrote materialized operator backlog to {args.materialize}")
        return 0
    expected = serialized(build_operator_universe())
    if args.check:
        actual = OUTPUT.read_text(encoding="utf-8") if OUTPUT.is_file() else ""
        if actual != expected:
            print("operator backlog is stale; run python3 tools/build_operator_universe.py")
            return 1
        print("operator backlog is current")
        return 0
    OUTPUT.write_text(expected, encoding="utf-8")
    print(f"wrote {OUTPUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
