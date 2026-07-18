#!/usr/bin/env python3
"""Fetch locked upstream snapshots without mutating ontology or canonical data.

Mutable upstream URLs can change. By default, a checksum mismatch is a hard
failure and the downloaded candidate remains separate for review.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
import tempfile
import urllib.error
import urllib.request
import zipfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]


def digest(path: Path) -> str:
    result = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def download(url: str, destination: Path) -> None:
    request = urllib.request.Request(url, headers={"User-Agent": "EverythingX-Format-Foundation/0.1"})
    with urllib.request.urlopen(request, timeout=90) as response, destination.open("wb") as output:
        shutil.copyfileobj(response, output)


def archive_name(source: dict[str, Any]) -> str:
    if source["id"] == "loc-fdd":
        return "loc-fddXML.zip"
    return Path(source["local_input"]).name


def expected_hash(source: dict[str, Any]) -> str:
    return source.get("sha256") or source.get("archive_sha256") or ""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--destination", type=Path, required=True)
    parser.add_argument(
        "--accept-upstream-change",
        action="store_true",
        help="Store changed candidates and report hashes; does not edit sources.json.",
    )
    args = parser.parse_args()
    manifest = json.loads((ROOT / "sources" / "sources.json").read_text(encoding="utf-8"))
    destination = args.destination.resolve()
    destination.mkdir(parents=True, exist_ok=True)
    report: list[dict[str, Any]] = []

    with tempfile.TemporaryDirectory(prefix="everythingx-source-sync-") as temporary:
        temporary_root = Path(temporary)
        for source in manifest["sources"]:
            candidate = temporary_root / archive_name(source)
            print(f"fetching {source['id']} from {source['url']}", file=sys.stderr)
            download(source["url"], candidate)
            actual = digest(candidate)
            expected = expected_hash(source)
            matches = actual == expected
            report.append(
                {
                    "id": source["id"],
                    "url": source["url"],
                    "expected_sha256": expected,
                    "actual_sha256": actual,
                    "matches_lock": matches,
                }
            )
            if not matches and not args.accept_upstream_change:
                continue
            target = destination / archive_name(source)
            shutil.copy2(candidate, target)
            if source["id"] == "loc-fdd":
                extraction = destination / "loc-fdd"
                if extraction.exists():
                    raise RuntimeError(f"Refusing to replace existing extraction directory: {extraction}")
                extraction.mkdir(parents=True)
                with zipfile.ZipFile(target) as archive:
                    for info in archive.infolist():
                        resolved = (extraction / info.filename).resolve()
                        if extraction not in resolved.parents and resolved != extraction:
                            raise RuntimeError(f"Unsafe ZIP path: {info.filename}")
                    archive.extractall(extraction)

    print(json.dumps({"sources": report}, ensure_ascii=False, indent=2))
    if any(not item["matches_lock"] for item in report) and not args.accept_upstream_change:
        print("One or more sources changed; rerun with --accept-upstream-change only after review.", file=sys.stderr)
        return 3
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, urllib.error.URLError, zipfile.BadZipFile) as error:
        print(f"sync failed: {error}", file=sys.stderr)
        raise SystemExit(1)
