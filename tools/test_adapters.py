#!/usr/bin/env python3
"""Discover and test every optional EverythingX Adapter."""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    cargo = shutil.which("cargo")
    if cargo is None:
        print("cargo is not installed", file=sys.stderr)
        return 2

    manifests = sorted(
        (ROOT / "capsules").rglob("everythingx/adapter/Cargo.toml")
    )
    if not manifests:
        print("No Adapter manifests found.", file=sys.stderr)
        return 2

    for manifest in manifests:
        relative = manifest.relative_to(ROOT)
        print(f"TEST {relative}", flush=True)
        result = subprocess.run(
            [cargo, "test", "--manifest-path", str(manifest)],
            cwd=ROOT,
            check=False,
        )
        if result.returncode != 0:
            return result.returncode
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
