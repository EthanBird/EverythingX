#!/usr/bin/env python3
"""Run and score the end-to-end benchmark for every registered capability."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "benchmarks" / "capability-bench" / "Cargo.toml"
PROFILE_ID = "exbench:ci-default-v1"


def expected_coverage() -> dict[str, int]:
    capsules = 0
    capabilities = 0
    for manifest in sorted((ROOT / "capsules").rglob("capsule.json")):
        if any(part.startswith("_") for part in manifest.relative_to(ROOT / "capsules").parts):
            continue
        capsules += 1
        adapter = manifest.parent / "everythingx" / "adapter.json"
        value = json.loads(adapter.read_text(encoding="utf-8"))
        capabilities += len(value["capabilities"])
    return {"capsules": capsules, "capabilities": capabilities}


def percentile_record(parts: list[str]) -> dict[str, Any]:
    if len(parts) != 16:
        raise ValueError(f"malformed CAPABILITY row with {len(parts)} fields")
    return {
        "capsule_id": parts[2],
        "capability_id": parts[3],
        "strategy": parts[4],
        "backend": parts[5],
        "source_format": parts[6],
        "small_input_bytes": int(parts[7]),
        "small_output_bytes": int(parts[8]),
        "small_p50_ns": int(parts[9]),
        "small_p95_ns": int(parts[10]),
        "large_input_bytes": int(parts[11]),
        "large_output_bytes": int(parts[12]),
        "large_p50_ns": int(parts[13]),
        "large_p95_ns": int(parts[14]),
        "reported_peak_memory_bytes": int(parts[15]),
    }


def parse_raw(text: str) -> tuple[dict[str, int], list[dict[str, Any]], dict[str, int]]:
    calibration = None
    rows: list[dict[str, Any]] = []
    summary = None
    for line in text.splitlines():
        if not line.startswith("EXBENCH\t"):
            continue
        parts = line.split("\t")
        if parts[1] == "CALIBRATION":
            if len(parts) != 5:
                raise ValueError("malformed CALIBRATION row")
            calibration = {"input_bytes": int(parts[2]), "p50_ns": int(parts[3]), "p95_ns": int(parts[4])}
        elif parts[1] == "CAPABILITY":
            rows.append(percentile_record(parts))
        elif parts[1] == "SUMMARY":
            if len(parts) != 4:
                raise ValueError("malformed SUMMARY row")
            summary = {"capabilities": int(parts[2]), "capsules": int(parts[3])}
    if calibration is None or summary is None:
        raise ValueError("benchmark output is missing calibration or summary")
    capability_ids = [row["capability_id"] for row in rows]
    if len(capability_ids) != len(set(capability_ids)):
        raise ValueError("benchmark output contains duplicate capability IDs")
    capsules = {row["capsule_id"] for row in rows}
    if summary != {"capabilities": len(rows), "capsules": len(capsules)}:
        raise ValueError(f"benchmark summary mismatch: {summary}, observed {len(capsules)}/{len(rows)}")
    return calibration, rows, summary


def component_scores(row: dict[str, Any], calibration_mib_s: float) -> dict[str, float]:
    throughput = row["large_input_bytes"] / (row["large_p50_ns"] / 1_000_000_000) / (1024 * 1024)
    efficiency = min(1.0, throughput / calibration_mib_s)
    latency_us = row["small_p50_ns"] / 1000
    memory_ratio = row["reported_peak_memory_bytes"] / max(1, row["large_input_bytes"])
    stability_ratio = row["large_p95_ns"] / max(1, row["large_p50_ns"])
    return {
        "throughput": 100 * math.sqrt(max(0.0, efficiency)),
        "latency": 100 / (1 + latency_us / 1000),
        "memory": 100 / max(1.0, memory_ratio),
        "stability": 100 / max(1.0, stability_ratio),
    }


def performance_score(components: dict[str, float]) -> float:
    weights = {"throughput": 0.55, "latency": 0.20, "memory": 0.15, "stability": 0.10}
    logarithm = sum(weights[name] * math.log(max(1.0, components[name]) / 100) for name in weights)
    return 100 * math.exp(logarithm)


def score_row(row: dict[str, Any], calibration_mib_s: float) -> dict[str, Any]:
    small_bytes = row["small_input_bytes"]
    large_bytes = row["large_input_bytes"]
    delta_bytes = max(1, large_bytes - small_bytes)
    delta_ns = row["large_p50_ns"] - row["small_p50_ns"]
    ns_per_byte = max(0.0, delta_ns / delta_bytes)
    if ns_per_byte == 0:
        ns_per_byte = row["large_p50_ns"] / max(1, large_bytes)
    fixed_ns = max(0.0, row["small_p50_ns"] - ns_per_byte * small_bytes)
    throughput_mib_s = large_bytes / (row["large_p50_ns"] / 1_000_000_000) / (1024 * 1024)
    components = component_scores(row, calibration_mib_s)
    return {
        "capsule_id": row["capsule_id"],
        "capability_id": row["capability_id"],
        "strategy": row["strategy"],
        "backend": row["backend"],
        "source_format": row["source_format"],
        "workloads": {
            "small": {
                "input_bytes": small_bytes,
                "output_bytes": row["small_output_bytes"],
                "p50_micros": round(row["small_p50_ns"] / 1000, 3),
                "p95_micros": round(row["small_p95_ns"] / 1000, 3),
            },
            "large": {
                "input_bytes": large_bytes,
                "output_bytes": row["large_output_bytes"],
                "p50_micros": round(row["large_p50_ns"] / 1000, 3),
                "p95_micros": round(row["large_p95_ns"] / 1000, 3),
                "throughput_mib_s": round(throughput_mib_s, 3),
                "reported_peak_memory_bytes": row["reported_peak_memory_bytes"],
            },
        },
        "cost_model": {
            "fixed_latency_micros": round(fixed_ns / 1000, 6),
            "nanoseconds_per_input_byte": round(ns_per_byte, 9),
            "peak_memory_bytes_per_input_byte": round(row["reported_peak_memory_bytes"] / max(1, large_bytes), 9),
            "output_bytes_per_input_byte": round(row["large_output_bytes"] / max(1, large_bytes), 9),
        },
        "score": {
            "overall_0_to_100": round(performance_score(components), 3),
            "components_0_to_100": {name: round(value, 3) for name, value in components.items()},
            "use": "Same-profile ranking and tie-breaking only; graph search should prefer the raw cost model.",
        },
    }


def harness_hash() -> str:
    digest = hashlib.sha256()
    for path in [
        ROOT / "benchmarks" / "capability-bench" / "src" / "main.rs",
        ROOT / "benchmarks" / "capability-bench" / "src" / "generated_adapters.rs",
    ]:
        digest.update(path.read_bytes())
    return digest.hexdigest()


def command_output(command: list[str]) -> str:
    result = subprocess.run(command, cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
    if result.returncode != 0:
        sys.stdout.write(result.stdout)
        raise RuntimeError(f"command failed with status {result.returncode}: {' '.join(command)}")
    return result.stdout


def build_report(raw: str) -> dict[str, Any]:
    calibration, rows, summary = parse_raw(raw)
    expected = expected_coverage()
    if summary != expected:
        raise ValueError(f"expected repository coverage {expected}, got {summary}")
    calibration_mib_s = calibration["input_bytes"] / (calibration["p50_ns"] / 1_000_000_000) / (1024 * 1024)
    evidence = [score_row(row, calibration_mib_s) for row in sorted(rows, key=lambda item: item["capability_id"])]
    return {
        "schema_version": "0.1.0",
        "profile_id": PROFILE_ID,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "scope": "End-to-end Kernel default invocation including Adapter buffering and Capsule execution.",
        "comparability": "Scores are comparable only when profile_id, harness_sha256 and environment class match.",
        "environment": {
            "os": platform.platform(),
            "architecture": platform.machine(),
            "python": platform.python_version(),
            "rustc": command_output(["rustc", "-Vv"]).strip(),
            "github_runner_image": os.environ.get("ImageOS", "local-or-unknown"),
            "github_run_id": os.environ.get("GITHUB_RUN_ID"),
            "commit_sha": os.environ.get("GITHUB_SHA"),
        },
        "method": {
            "harness_sha256": harness_hash(),
            "fixture_profile": "deterministic synthetic valid inputs; 16 KiB small and approximately 4 MiB large",
            "warmup_iterations": 2,
            "small_samples": 11,
            "large_samples": 7,
            "percentiles": [50, 95],
            "calibration": {
                "operation": "4 MiB Vec clone",
                "input_bytes": calibration["input_bytes"],
                "p50_micros": round(calibration["p50_ns"] / 1000, 3),
                "p95_micros": round(calibration["p95_ns"] / 1000, 3),
                "throughput_mib_s": round(calibration_mib_s, 3),
            },
            "score_formula": "geometric_mean(throughput^.55, latency^.20, memory^.15, stability^.10), calibrated to same-run Vec clone",
        },
        "summary": summary,
        "capabilities": evidence,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--raw-input", type=Path, help="Score previously captured EXBENCH rows instead of running Cargo.")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--print-report", action="store_true")
    args = parser.parse_args()
    if args.raw_input:
        raw = args.raw_input.read_text(encoding="utf-8")
    else:
        raw = command_output(["cargo", "run", "--release", "--manifest-path", str(MANIFEST)])
        sys.stdout.write("\n".join(line for line in raw.splitlines() if line.startswith("EXBENCH\t")) + "\n")
    report = build_report(raw)
    rendered = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
        print(f"wrote performance report for {report['summary']['capsules']} Capsules/{report['summary']['capabilities']} capabilities")
    if args.print_report:
        print("EXBENCH_REPORT_BEGIN")
        print(rendered, end="")
        print("EXBENCH_REPORT_END")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, ValueError, KeyError, ZeroDivisionError) as error:
        print(f"benchmark failed: {error}", file=sys.stderr)
        raise SystemExit(1)
