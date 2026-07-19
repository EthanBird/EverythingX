#!/usr/bin/env python3
"""Measure Rust cold/warm build time and artifact footprint on an isolated target tree."""

from __future__ import annotations

import argparse
import json
import math
import os
import platform
import shutil
import statistics
import subprocess
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
BENCH_MANIFEST = ROOT / "benchmarks" / "capability-bench" / "Cargo.toml"
KERNEL_MANIFEST = ROOT / "kernel" / "Cargo.toml"


def run(command: list[str], *, target: Path | None = None) -> tuple[float, str]:
    environment = os.environ.copy()
    if target is not None:
        environment["CARGO_TARGET_DIR"] = str(target)
    started = time.perf_counter()
    result = subprocess.run(
        command,
        cwd=ROOT,
        env=environment,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    elapsed = time.perf_counter() - started
    if result.returncode != 0:
        print(result.stdout)
        raise RuntimeError(
            f"command failed with status {result.returncode}: {' '.join(command)}"
        )
    return elapsed, result.stdout


def output(command: list[str]) -> str:
    result = subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(f"command failed: {' '.join(command)}\n{result.stdout}")
    return result.stdout.strip()


def tree_size(path: Path) -> dict[str, int]:
    logical = 0
    allocated = 0
    files = 0
    if not path.exists():
        return {"logical_bytes": 0, "allocated_bytes": 0, "files": 0}
    for candidate in path.rglob("*"):
        if not candidate.is_file() or candidate.is_symlink():
            continue
        stat = candidate.stat()
        logical += stat.st_size
        allocated += getattr(stat, "st_blocks", math.ceil(stat.st_size / 512)) * 512
        files += 1
    return {"logical_bytes": logical, "allocated_bytes": allocated, "files": files}


def source_tree_size(path: Path, *, omit_everythingx: bool = False) -> int:
    total = 0
    for candidate in path.rglob("*"):
        if not candidate.is_file() or candidate.is_symlink():
            continue
        relative = candidate.relative_to(path)
        if any(part in {".git", "target", "__pycache__"} for part in relative.parts):
            continue
        if omit_everythingx and relative.parts and relative.parts[0] == "everythingx":
            continue
        total += candidate.stat().st_size
    return total


def artifact_breakdown(path: Path) -> dict[str, dict[str, int]]:
    groups: dict[str, dict[str, int]] = {}
    for candidate in path.rglob("*"):
        if not candidate.is_file() or candidate.is_symlink():
            continue
        relative = candidate.relative_to(path)
        suffix = candidate.suffix
        if suffix == ".rlib":
            group = "rlib"
        elif suffix == ".rmeta":
            group = "rmeta"
        elif suffix in {".o", ".obj"}:
            group = "object"
        elif suffix in {".d", ".dep-info"}:
            group = "dependency-info"
        elif suffix in {".json", ".bin"} or ".fingerprint" in relative.parts:
            group = "cargo-metadata"
        elif os.access(candidate, os.X_OK) and suffix == "":
            group = "executable"
        else:
            group = "other"
        record = groups.setdefault(group, {"logical_bytes": 0, "files": 0})
        record["logical_bytes"] += candidate.stat().st_size
        record["files"] += 1
    return dict(sorted(groups.items()))


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    if not ordered:
        return 0.0
    index = math.ceil(fraction * len(ordered)) - 1
    return ordered[max(0, min(index, len(ordered) - 1))]


def summary(values: list[float]) -> dict[str, float]:
    return {
        "min": round(min(values), 3),
        "median": round(statistics.median(values), 3),
        "p95": round(percentile(values, 0.95), 3),
        "max": round(max(values), 3),
        "sum": round(sum(values), 3),
    }


def production_capsules() -> list[Path]:
    manifests: list[Path] = []
    for manifest in sorted((ROOT / "capsules").rglob("Cargo.toml")):
        relative = manifest.parent.relative_to(ROOT / "capsules")
        if any(part.startswith("_") for part in relative.parts):
            continue
        if "everythingx" in relative.parts:
            continue
        if not (manifest.parent / "capsule.json").is_file():
            continue
        manifests.append(manifest)
    return manifests


def largest_rlib(target: Path) -> Path | None:
    candidates = list((target / "release" / "deps").glob("*.rlib"))
    return max(candidates, key=lambda path: path.stat().st_size) if candidates else None


def human_bytes(value: int) -> str:
    units = ["B", "KiB", "MiB", "GiB"]
    amount = float(value)
    for unit in units:
        if amount < 1024 or unit == units[-1]:
            return f"{amount:.2f} {unit}"
        amount /= 1024
    raise AssertionError("unreachable")


def markdown(report: dict[str, Any]) -> str:
    unified = report["measurements"]["unified_release"]
    kernel = report["measurements"]["kernel_test_compile"]
    standalone = report["measurements"]["standalone_capsules"]
    lines = [
        "# Rust build footprint",
        "",
        f"Generated: {report['generated_at']}",
        f"Commit: `{report['commit_sha']}`",
        f"Runner: `{report['environment']['os']}` / `{report['environment']['architecture']}` / {report['environment']['cpu_count']} CPUs",
        f"Toolchain: `{report['toolchain']['rustc']}`; `{report['toolchain']['cargo']}`",
        "",
        "## Outcome",
        "",
        "| Measurement | Value |",
        "|---|---:|",
        f"| Unified all-capability release cold build | {unified['cold_seconds']:.3f} s |",
        f"| Unified all-capability release no-op rebuild | {unified['no_op_seconds']:.3f} s |",
        f"| Kernel test compilation | {kernel['cold_seconds']:.3f} s |",
        f"| 104 independent Capsule release builds | {standalone['build_seconds']['sum']:.3f} s |",
        f"| Unified release target tree | {human_bytes(unified['target']['logical_bytes'])} |",
        f"| Unified linked benchmark executable | {human_bytes(unified['binary_bytes'])} |",
        f"| Stripped executable copy | {human_bytes(unified['stripped_binary_bytes']) if unified['stripped_binary_bytes'] is not None else 'unavailable'} |",
        f"| All isolated Capsule target trees | {human_bytes(standalone['target_logical_bytes_total'])} |",
        f"| All standalone Capsule `.rlib` artifacts | {human_bytes(standalone['rlib_bytes_total'])} |",
        f"| Active Rust sysroot | {human_bytes(report['toolchain']['sysroot']['logical_bytes'])} |",
        "",
        "The unified target size is developer build state, not a distributable package. It includes Cargo fingerprints, dependency metadata, libraries and the linked harness. The stripped executable is the closest number here to a monolithic release payload. A standalone Capsule is a library, so its `.rlib` is the relevant Rust artifact, not its entire isolated `target/` directory.",
        "",
        "## Independent Capsule distribution",
        "",
        f"Per-Capsule cold release time: min {standalone['build_seconds']['min']:.3f} s, median {standalone['build_seconds']['median']:.3f} s, p95 {standalone['build_seconds']['p95']:.3f} s, max {standalone['build_seconds']['max']:.3f} s.",
        "",
        "### Slowest builds",
        "",
        "| Capsule | Seconds | `.rlib` | Target tree |",
        "|---|---:|---:|---:|",
    ]
    for item in standalone["slowest"]:
        lines.append(
            f"| `{item['capsule']}` | {item['build_seconds']:.3f} | {human_bytes(item['rlib_bytes'])} | {human_bytes(item['target']['logical_bytes'])} |"
        )
    lines.extend([
        "",
        "### Largest standalone libraries",
        "",
        "| Capsule | `.rlib` | Source without Adapter | Seconds |",
        "|---|---:|---:|---:|",
    ])
    for item in standalone["largest_rlibs"]:
        lines.append(
            f"| `{item['capsule']}` | {human_bytes(item['rlib_bytes'])} | {human_bytes(item['source_bytes_without_adapter'])} | {item['build_seconds']:.3f} |"
        )
    lines.extend([
        "",
        "## Unified target breakdown",
        "",
        "| Class | Files | Logical size |",
        "|---|---:|---:|",
    ])
    for name, item in unified["artifact_breakdown"].items():
        lines.append(f"| {name} | {item['files']} | {human_bytes(item['logical_bytes'])} |")
    lines.extend([
        "",
        "## Method",
        "",
        "- Stable Rust with the minimal rustup profile on a fresh GitHub-hosted Ubuntu runner.",
        "- Every cold measurement uses a new `CARGO_TARGET_DIR` under the ephemeral runner temp directory.",
        "- The no-op rebuild repeats the identical release command against the populated unified target.",
        "- Each production Capsule is then built independently with its own isolated release target and locked manifest.",
        "- Logical byte counts sum file lengths; allocated byte counts use filesystem block accounting.",
        "- Network dependency download time is absent because the current production Capsules are dependency-free.",
        "",
    ])
    return "\n".join(lines)


def build_report(work_root: Path) -> dict[str, Any]:
    manifests = production_capsules()
    if len(manifests) != 104:
        raise ValueError(f"expected 104 production Capsules, found {len(manifests)}")

    unified_target = work_root / "unified-release"
    release_command = [
        "cargo", "build", "--release", "--quiet",
        "--manifest-path", str(BENCH_MANIFEST),
    ]
    cold_seconds, _ = run(release_command, target=unified_target)
    no_op_seconds, _ = run(release_command, target=unified_target)
    binary = unified_target / "release" / "everythingx-capability-bench"
    if not binary.is_file():
        raise ValueError(f"expected linked benchmark binary at {binary}")
    stripped_bytes: int | None = None
    strip = shutil.which("strip")
    if strip:
        stripped = work_root / "everythingx-capability-bench.stripped"
        shutil.copy2(binary, stripped)
        run([strip, "--strip-unneeded", str(stripped)])
        stripped_bytes = stripped.stat().st_size

    kernel_target = work_root / "kernel-test"
    kernel_seconds, _ = run(
        [
            "cargo", "test", "--no-run", "--locked", "--quiet",
            "--manifest-path", str(KERNEL_MANIFEST),
        ],
        target=kernel_target,
    )

    capsule_records: list[dict[str, Any]] = []
    individual_root = work_root / "standalone-capsules"
    for index, manifest in enumerate(manifests):
        capsule_dir = manifest.parent
        capsule_name = capsule_dir.name
        target = individual_root / f"{index:03d}-{capsule_name}"
        seconds, _ = run(
            [
                "cargo", "build", "--release", "--locked", "--quiet",
                "--manifest-path", str(manifest),
            ],
            target=target,
        )
        rlib = largest_rlib(target)
        if rlib is None:
            raise ValueError(f"{capsule_name}: release build produced no .rlib")
        record = {
            "capsule": str(capsule_dir.relative_to(ROOT / "capsules")),
            "build_seconds": round(seconds, 3),
            "source_bytes_without_adapter": source_tree_size(capsule_dir, omit_everythingx=True),
            "rlib_bytes": rlib.stat().st_size,
            "target": tree_size(target),
        }
        capsule_records.append(record)
        if (index + 1) % 10 == 0 or index + 1 == len(manifests):
            print(f"built {index + 1}/{len(manifests)} independent Capsules", flush=True)

    build_times = [float(item["build_seconds"]) for item in capsule_records]
    rust_sysroot = Path(output(["rustc", "--print", "sysroot"]))
    return {
        "schema_version": "0.1.0",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "commit_sha": os.environ.get("GITHUB_SHA", "local-or-unknown"),
        "environment": {
            "os": platform.platform(),
            "architecture": platform.machine(),
            "cpu_count": os.cpu_count(),
            "github_runner_image": os.environ.get("ImageOS", "local-or-unknown"),
            "github_run_id": os.environ.get("GITHUB_RUN_ID", "local-or-unknown"),
        },
        "toolchain": {
            "rustc": output(["rustc", "--version"]),
            "rustc_verbose": output(["rustc", "-Vv"]),
            "cargo": output(["cargo", "--version"]),
            "rustup": output(["rustup", "--version"]),
            "sysroot_path": str(rust_sysroot),
            "sysroot": tree_size(rust_sysroot),
        },
        "source": {
            "repository_logical_bytes": source_tree_size(ROOT),
            "production_capsules": len(manifests),
        },
        "measurements": {
            "unified_release": {
                "scope": "Kernel, protocol, all 104 Adapters and all 104 production Capsules linked into the capability benchmark executable.",
                "cold_seconds": round(cold_seconds, 3),
                "no_op_seconds": round(no_op_seconds, 3),
                "target": tree_size(unified_target),
                "artifact_breakdown": artifact_breakdown(unified_target),
                "binary_bytes": binary.stat().st_size,
                "stripped_binary_bytes": stripped_bytes,
            },
            "kernel_test_compile": {
                "scope": "Kernel workspace tests compiled but not executed.",
                "cold_seconds": round(kernel_seconds, 3),
                "target": tree_size(kernel_target),
            },
            "standalone_capsules": {
                "scope": "Every production Capsule release-built from its own manifest and isolated target directory, excluding its EverythingX Adapter.",
                "count": len(capsule_records),
                "build_seconds": summary(build_times),
                "target_logical_bytes_total": sum(item["target"]["logical_bytes"] for item in capsule_records),
                "target_allocated_bytes_total": sum(item["target"]["allocated_bytes"] for item in capsule_records),
                "rlib_bytes_total": sum(item["rlib_bytes"] for item in capsule_records),
                "slowest": sorted(capsule_records, key=lambda item: item["build_seconds"], reverse=True)[:10],
                "largest_rlibs": sorted(capsule_records, key=lambda item: item["rlib_bytes"], reverse=True)[:10],
                "capsules": capsule_records,
            },
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path, required=True)
    parser.add_argument("--markdown", type=Path, required=True)
    parser.add_argument(
        "--work-root",
        type=Path,
        help="Parent for fresh isolated target trees; defaults to RUNNER_TEMP or /tmp.",
    )
    args = parser.parse_args()
    parent = args.work_root or Path(os.environ.get("RUNNER_TEMP", tempfile.gettempdir()))
    parent.mkdir(parents=True, exist_ok=True)
    work_root = Path(tempfile.mkdtemp(prefix="everythingx-build-footprint-", dir=parent))
    print(f"isolated build root: {work_root}", flush=True)
    report = build_report(work_root)
    args.json.parent.mkdir(parents=True, exist_ok=True)
    args.markdown.parent.mkdir(parents=True, exist_ok=True)
    args.json.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    args.markdown.write_text(markdown(report), encoding="utf-8")
    print(f"wrote {args.json} and {args.markdown}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, ValueError, KeyError) as error:
        print(f"build-footprint measurement failed: {error}")
        raise SystemExit(1)
