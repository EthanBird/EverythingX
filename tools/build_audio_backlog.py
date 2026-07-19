#!/usr/bin/env python3
"""Expand the reviewed audio representation snapshot into candidate pair edges."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

from build_support_matrix import build_matrix


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "operators" / "audio" / "representations.json"
OUTPUT = ROOT / "operators" / "audio" / "backlog.json"


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path}: expected JSON object")
    return value


def hypothesis(source: dict[str, str], target: dict[str, str], domain: str) -> str:
    if domain != "sampled-audio":
        return "representability-and-semantics-require-review"
    source_fidelity = source["fidelity"]
    target_fidelity = target["fidelity"]
    source_lossless = source_fidelity in {"lossless", "companded"}
    target_lossless = target_fidelity in {"lossless", "companded"}
    if source_lossless and target_lossless:
        return "conditional-exact-for-supported-signal-domain"
    if source_lossless and not target_lossless:
        return "controlled-lossy-encode"
    if not source_lossless and target_lossless:
        return "decoded-signal-preservation-without-source-recovery"
    return "generation-loss-likely"


def digest_records(records: list[dict[str, Any]]) -> str:
    encoded = json.dumps(records, ensure_ascii=False, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


def build_backlog(include_materialized_pairs: bool = False) -> dict[str, Any]:
    source = load_json(SOURCE)
    media_index = load_json(ROOT / "catalog" / "indexes" / "media-types.json")
    audio_media_types = [
        {"media_type": name, "source_record_ids": media_index[name]}
        for name in sorted(media_index)
        if name.startswith("audio/")
    ]

    representation_ids: set[str] = set()
    format_to_representation: dict[str, str] = {}
    candidates: list[dict[str, Any]] = []
    domain_counts: dict[str, dict[str, int]] = {}
    candidate_sets: list[dict[str, Any]] = []
    for domain in source["domains"]:
        representations = domain["representations"]
        for representation in representations:
            representation_id = representation["id"]
            if representation_id in representation_ids:
                raise ValueError(f"duplicate audio representation {representation_id}")
            representation_ids.add(representation_id)
            if "format_id" in representation:
                format_to_representation[representation["format_id"]] = representation_id
        domain_candidate_count = 0
        if domain.get("pairwise_candidates"):
            for input_representation in representations:
                for output_representation in representations:
                    if input_representation["id"] == output_representation["id"]:
                        continue
                    source_name = input_representation["id"].removeprefix("audiofmt:")
                    target_name = output_representation["id"].removeprefix("audiofmt:")
                    candidates.append(
                        {
                            "candidate_id": f"audio-candidate:{source_name}-to-{target_name}",
                            "domain": domain["id"],
                            "kind": "convert",
                            "input": input_representation["id"],
                            "output": output_representation["id"],
                            "arity": "1:1",
                            "knowledge_status": "candidate",
                            "computability": "unknown",
                            "initial_hypothesis": hypothesis(
                                input_representation,
                                output_representation,
                                domain["id"],
                            ),
                            "implementation_status": "not-implemented",
                        }
                    )
                    domain_candidate_count += 1
        domain_counts[domain["id"]] = {
            "representations": len(representations),
            "ordered_pair_candidates": domain_candidate_count,
        }

    candidates.sort(key=lambda item: item["candidate_id"])
    implemented_pairs = []
    for pair in build_matrix()["logical_pairs"]:
        input_representation = format_to_representation.get(pair["source"])
        output_representation = format_to_representation.get(pair["target"])
        if input_representation is not None and output_representation is not None:
            implemented_pairs.append(
                {"input": input_representation, "output": output_representation}
            )
    for domain in source["domains"]:
        domain_candidates = [
            item for item in candidates if item["domain"] == domain["id"]
        ]
        candidate_sets.append(
            {
                "domain": domain["id"],
                "representation_ids": [
                    item["id"] for item in domain["representations"]
                ],
                "expansion": "all ordered pairs (input, output) where input != output",
                "defaults": {
                    "kind": "convert",
                    "arity": "1:1",
                    "knowledge_status": "candidate",
                    "computability": "unknown",
                    "implementation_status": "not-implemented",
                },
                "ordered_pair_candidates": len(domain_candidates),
                "materialized_sha256": digest_records(domain_candidates),
            }
        )

    result = {
        "schema_version": "0.1.0",
        "generated_from": "operators/audio/representations.json",
        "meaning": "Complete ordered-pair research queue for the reviewed audio representation snapshot; candidate does not mean computable.",
        "regeneration_rule": "Run python3 tools/build_audio_backlog.py after changing the audio representation universe.",
        "summary": {
            "observed_audio_media_type_labels": len(audio_media_types),
            "reviewed_representations": len(representation_ids),
            "operator_templates": len(source["operator_templates"]),
            "ordered_pair_candidates": len(candidates),
            "implemented_pair_candidates": len(implemented_pairs),
        },
        "domain_counts": domain_counts,
        "currently_implemented_audio_pairs": implemented_pairs,
        "observed_audio_media_types": audio_media_types,
        "operator_templates": source["operator_templates"],
        "pair_candidate_sets": candidate_sets,
    }
    if include_materialized_pairs:
        result["materialized_pair_candidates"] = candidates
    return result


def serialized(backlog: dict[str, Any]) -> str:
    return json.dumps(backlog, ensure_ascii=False, indent=2) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument(
        "--materialize",
        type=Path,
        help="Write every expanded pair candidate to the requested path.",
    )
    args = parser.parse_args()
    if args.materialize is not None:
        args.materialize.write_text(
            serialized(build_backlog(include_materialized_pairs=True)),
            encoding="utf-8",
        )
        print(f"wrote materialized audio backlog to {args.materialize}")
        return 0
    expected = serialized(build_backlog())
    if args.check:
        actual = OUTPUT.read_text(encoding="utf-8") if OUTPUT.is_file() else ""
        if actual != expected:
            print("audio backlog is stale; run python3 tools/build_audio_backlog.py")
            return 1
        print("audio backlog is current")
        return 0
    OUTPUT.write_text(expected, encoding="utf-8")
    print(f"wrote {OUTPUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
