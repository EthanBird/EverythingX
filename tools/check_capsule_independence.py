#!/usr/bin/env python3
"""Check that Conversion Capsules do not depend on the EverythingX repository."""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path
from typing import Any, Iterable


ROOT = Path(__file__).resolve().parents[1]
FORBIDDEN_CORE_TOKENS = (
    "ex_core",
    "ex_kernel",
    "ex_registry",
    "everythingx::",
    "EverythingXOperator",
)


def walk_dict(value: Any) -> Iterable[dict[str, Any]]:
    if isinstance(value, dict):
        yield value
        for child in value.values():
            yield from walk_dict(child)
    elif isinstance(value, list):
        for child in value:
            yield from walk_dict(child)


def is_within(path: Path, root: Path) -> bool:
    try:
        path.resolve().relative_to(root.resolve())
        return True
    except ValueError:
        return False


def check_path_dependencies(capsule_root: Path) -> list[str]:
    errors: list[str] = []
    integration = capsule_root / "everythingx"
    for manifest in capsule_root.rglob("Cargo.toml"):
        # Adapter code is allowed to depend on ex-protocol/ex-kernel. The
        # independence boundary applies to the Capsule core after the entire
        # everythingx/ directory is deleted.
        if integration.exists() and is_within(manifest, integration):
            continue
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
        for table in walk_dict(data):
            path_value = table.get("path")
            if not isinstance(path_value, str):
                continue
            resolved = (manifest.parent / path_value).resolve()
            if not is_within(resolved, capsule_root):
                errors.append(
                    f"{manifest.relative_to(capsule_root)}: path dependency escapes Capsule root: {path_value}"
                )
    return errors


def check_core_source(capsule_root: Path) -> list[str]:
    errors: list[str] = []
    integration = capsule_root / "everythingx"
    for path in capsule_root.rglob("*.rs"):
        if integration.exists() and is_within(path, integration):
            continue
        text = path.read_text(encoding="utf-8")
        for token in FORBIDDEN_CORE_TOKENS:
            if token in text:
                errors.append(f"{path.relative_to(capsule_root)}: forbidden core token {token!r}")
    return errors


def copy_out_and_test(capsule_root: Path, run_cargo: bool) -> list[str]:
    if not run_cargo:
        return []
    cargo = shutil.which("cargo")
    if cargo is None:
        return ["cargo is not installed; copy-out build was requested but could not run"]

    with tempfile.TemporaryDirectory(prefix="everythingx-capsule-") as temporary:
        copied = Path(temporary) / capsule_root.name
        shutil.copytree(capsule_root, copied)
        integration = copied / "everythingx"
        if integration.exists():
            shutil.rmtree(integration)
        result = subprocess.run(
            [cargo, "test", "--locked"],
            cwd=copied,
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode != 0:
            return [
                "copy-out cargo test failed after deleting everythingx/:\n"
                + result.stdout
                + result.stderr
            ]
    return []


def check_capsule(capsule_root: Path, run_cargo: bool) -> list[str]:
    errors: list[str] = []
    for required in ("Cargo.toml", "capsule.json", "README.md", "LICENSE", "src/lib.rs"):
        if not (capsule_root / required).is_file():
            errors.append(f"missing required file: {required}")
    errors.extend(check_path_dependencies(capsule_root))
    errors.extend(check_core_source(capsule_root))
    errors.extend(copy_out_and_test(capsule_root, run_cargo))
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("capsules", nargs="*", type=Path)
    parser.add_argument("--cargo", action="store_true", help="Copy out, delete everythingx/, and run cargo test --locked")
    args = parser.parse_args()
    roots = args.capsules or sorted(
        path.parent for path in (ROOT / "capsules").rglob("capsule.json")
    )
    roots = [path.resolve() for path in roots if path.is_dir()]
    if not roots:
        print("No Capsule directories found.", file=sys.stderr)
        return 2

    failures = 0
    for capsule_root in roots:
        errors = check_capsule(capsule_root, args.cargo)
        if errors:
            failures += 1
            print(f"FAIL {capsule_root}")
            for error in errors:
                print(f"  - {error}")
        else:
            print(f"OK   {capsule_root}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
