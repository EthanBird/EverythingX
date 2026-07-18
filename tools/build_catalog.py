#!/usr/bin/env python3
"""Build EverythingX source observations from pinned local snapshots.

The builder intentionally does not merge records across sources. Its output is
an evidence-preserving fact layer, not a canonical format registry.
"""

from __future__ import annotations

import argparse
import ast
import hashlib
import json
import re
import sys
import xml.etree.ElementTree as ET
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SOURCE_ROOT = ROOT.parent / "research" / "raw"


def local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def clean_text(element: ET.Element | None) -> str:
    if element is None:
        return ""
    return " ".join("".join(element.itertext()).split())


def direct_child(element: ET.Element, name: str) -> ET.Element | None:
    return next((item for item in element if local_name(item.tag) == name), None)


def descendants(element: ET.Element, name: str) -> list[ET.Element]:
    return [item for item in element.iter() if local_name(item.tag) == name]


def unique(values: Iterable[str]) -> list[str]:
    return sorted({value.strip() for value in values if value and value.strip()})


def slug(value: str) -> str:
    value = value.strip().lower()
    value = re.sub(r"[^a-z0-9._/+:-]+", "-", value)
    return value.strip("-") or "unknown"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def external_id(scheme: str, value: str) -> dict[str, str]:
    return {"scheme": scheme, "value": value}


def evidence(snapshot: str, locator: str) -> dict[str, str]:
    return {"snapshot": snapshot, "locator": locator}


def infer_from_media_type(media_type: str) -> dict[str, Any] | None:
    """Return deliberately coarse, explicitly heuristic facets."""
    value = media_type.lower()
    top, _, subtype = value.partition("/")
    mode: list[str] = []
    family: list[str] = []
    confidence = 0.72

    top_map = {
        "image": (["perceptual"], ["raster-image"]),
        "audio": (["perceptual"], ["audio-signal"]),
        "video": (["perceptual", "composite"], ["timed-media"]),
        "text": (["symbolic"], ["plain-text"]),
        "font": (["symbolic"], ["font"]),
        "model": (["symbolic"], ["geometry-3d"]),
        "message": (["symbolic", "composite"], ["message"]),
        "multipart": (["composite"], ["package"]),
        "haptics": (["perceptual"], ["domain-record"]),
    }
    if top in top_map:
        mode, family = top_map[top]
    elif top == "application":
        confidence = 0.55
        if any(token in subtype for token in ("json", "xml", "yaml", "toml", "cbor", "msgpack")):
            mode, family = ["symbolic"], ["structured-text"]
        elif any(token in subtype for token in ("zip", "tar", "archive", "compressed")):
            mode, family = ["composite"], ["archive"]
        elif any(token in subtype for token in ("sqlite", "database", "sql")):
            mode, family = ["symbolic", "stateful"], ["relational-database"]
        elif any(token in subtype for token in ("pdf", "postscript", "word", "document", "opendocument")):
            mode, family = ["symbolic", "composite"], ["page-description", "document-flow"]
        elif any(token in subtype for token in ("wasm", "executable", "java-vm", "x-sharedlib")):
            mode, family = ["behavioral"], ["executable"]
        elif any(token in subtype for token in ("font", "woff")):
            mode, family = ["symbolic"], ["font"]
        else:
            return None
    else:
        return None

    return {
        "method": "heuristic",
        "confidence": confidence,
        "values": {"mode_of_being": mode, "semantic_family": family},
    }


def parse_iana(path: Path) -> Iterable[dict[str, Any]]:
    root = ET.parse(path).getroot()
    for registry in (item for item in root if local_name(item.tag) == "registry"):
        top_level = registry.attrib.get("id", "unknown")
        for record in (item for item in registry if local_name(item.tag) == "record"):
            file_node = direct_child(record, "file")
            name_node = direct_child(record, "name")
            media_type = clean_text(file_node)
            if not media_type:
                name = clean_text(name_node)
                media_type = f"{top_level}/{name}" if name else ""
            if not media_type:
                continue
            xrefs = [
                {key: value for key, value in item.attrib.items() if value}
                for item in record
                if local_name(item.tag) == "xref"
            ]
            result: dict[str, Any] = {
                "record_id": f"src:iana:{media_type.lower()}",
                "source": "iana",
                "external_ids": [external_id("media-type", media_type.lower())],
                "names": [media_type],
                "version": None,
                "media_types": [media_type.lower()],
                "extensions": [],
                "filenames": [],
                "signatures": [],
                "source_classes": {"top_level": top_level, "references": xrefs},
                "relations": [],
                "evidence": evidence("iana-media-types", media_type),
            }
            inferred = infer_from_media_type(media_type)
            if inferred:
                result["inferred_facets"] = inferred
            yield result


def parse_pronom(path: Path) -> Iterable[dict[str, Any]]:
    root = ET.parse(path).getroot()
    signature_meta: dict[str, dict[str, Any]] = {}
    for item in descendants(root, "InternalSignature"):
        signature_id = item.attrib.get("ID", "")
        if signature_id:
            signature_meta[signature_id] = {
                "kind": "pronom-internal-signature",
                "id": signature_id,
                "specificity": item.attrib.get("Specificity", ""),
                "byte_sequence_count": len(descendants(item, "ByteSequence")),
                "pattern_fragment_count": len(descendants(item, "Sequence")),
            }

    for item in descendants(root, "FileFormat"):
        puid = item.attrib.get("PUID", "")
        name = item.attrib.get("Name", puid or "Unnamed PRONOM format")
        version = item.attrib.get("Version") or None
        media_types = unique((item.attrib.get("MIMEType", "")).split(","))
        extensions = unique(clean_text(node).lstrip(".").lower() for node in item if local_name(node.tag) == "Extension")
        signature_ids = [clean_text(node) for node in item if local_name(node.tag) == "InternalSignatureID"]
        priorities = [clean_text(node) for node in item if local_name(node.tag) == "HasPriorityOverFileFormatID"]
        ids = [external_id("pronom-puid", puid)] if puid else []
        if item.attrib.get("ID"):
            ids.append(external_id("droid-file-format-id", item.attrib["ID"]))
        names = [name]
        if version:
            names.append(f"{name} {version}")
        result: dict[str, Any] = {
            "record_id": f"src:pronom:{puid or item.attrib.get('ID', slug(name))}",
            "source": "pronom",
            "external_ids": ids,
            "names": unique(names),
            "version": version,
            "media_types": media_types,
            "extensions": extensions,
            "filenames": [],
            "signatures": [signature_meta.get(sig, {"kind": "pronom-internal-signature", "id": sig}) for sig in signature_ids],
            "source_classes": {},
            "relations": [{"type": "has_priority_over_droid_id", "target": target} for target in priorities],
            "evidence": evidence("pronom-droid-v124", puid or item.attrib.get("ID", name)),
        }
        if media_types:
            inferred = infer_from_media_type(media_types[0])
            if inferred:
                result["inferred_facets"] = inferred
        yield result


def signifier_values(group: ET.Element, child_name: str) -> list[str]:
    child = direct_child(group, child_name)
    return unique(clean_text(item) for item in descendants(child, "sigValue")) if child is not None else []


def parse_loc(directory: Path) -> Iterable[dict[str, Any]]:
    for path in sorted(directory.glob("*.xml")):
        root = ET.parse(path).getroot()
        record_id = root.attrib.get("id", path.stem)
        full_name = clean_text(next(iter(descendants(root, "fullName")), None))
        names = unique([root.attrib.get("titleName", ""), root.attrib.get("shortName", ""), full_name])
        external_ids = [external_id("loc-fdd", record_id)]
        extensions: list[str] = []
        media_types: list[str] = []
        signatures: list[dict[str, Any]] = []

        for group in descendants(root, "signifiersGroup"):
            extensions.extend(value.lstrip(".").lower() for value in signifier_values(group, "filenameExtension"))
            media_types.extend(value.lower() for value in signifier_values(group, "internetMediaType") if "/" in value)
            for magic in signifier_values(group, "magicNumbers"):
                signatures.append({"kind": "loc-described-magic", "value": magic})
            for other in (item for item in group if local_name(item.tag) == "other"):
                tag = clean_text(direct_child(other, "tag"))
                values = direct_child(other, "values")
                for value_node in descendants(values, "sigValue") if values is not None else []:
                    value = clean_text(value_node)
                    if value:
                        external_ids.append(external_id(slug(tag), value))

        relationships: list[dict[str, str]] = []
        for relationship in descendants(root, "relationship"):
            rel_type = clean_text(direct_child(relationship, "typeOfRelationship"))
            related = direct_child(relationship, "relatedTo")
            target_id = ""
            if related is not None:
                id_node = direct_child(related, "id")
                target_id = clean_text(id_node) or clean_text(related).split(" ", 1)[0]
            if rel_type and target_id:
                relationships.append({"type": rel_type, "target": target_id})

        source_classes = {
            "gdfr_genre": unique(clean_text(item) for item in descendants(root, "gdfrGenre")),
            "format_category": unique(clean_text(item) for item in descendants(root, "category")),
            "composition": unique(clean_text(item) for item in descendants(root, "gdfrComposition")),
            "form": unique(clean_text(item) for item in descendants(root, "gdfrForm")),
            "constraint": unique(clean_text(item) for item in descendants(root, "gdfrConstraint")),
            "basis": unique(clean_text(item) for item in descendants(root, "gdfrBasis")),
            "keywords": unique(clean_text(item) for item in descendants(root, "keyword")),
        }
        result: dict[str, Any] = {
            "record_id": f"src:loc:{record_id}",
            "source": "loc",
            "external_ids": external_ids,
            "names": names or [record_id],
            "version": None,
            "media_types": unique(media_types),
            "extensions": unique(extensions),
            "filenames": [],
            "signatures": signatures,
            "source_classes": source_classes,
            "relations": relationships,
            "evidence": evidence("loc-fdd-2026-07-19", record_id),
        }
        if media_types:
            inferred = infer_from_media_type(media_types[0])
            if inferred:
                result["inferred_facets"] = inferred
        yield result


def parse_glob(pattern: str) -> tuple[str | None, str | None]:
    if re.fullmatch(r"\*\.[A-Za-z0-9_+.-]+", pattern):
        return pattern[2:].lower(), None
    if not any(token in pattern for token in "*?[]"):
        return None, pattern
    return None, None


def parse_mime_database(path: Path, source: str, snapshot: str) -> Iterable[dict[str, Any]]:
    root = ET.parse(path).getroot()
    for item in descendants(root, "mime-type"):
        media_type = item.attrib.get("type", "")
        if not media_type:
            continue
        aliases = unique(child.attrib.get("type", "") for child in item if local_name(child.tag) == "alias")
        glob_patterns = unique(child.attrib.get("pattern", "") for child in item if local_name(child.tag) == "glob")
        extensions: list[str] = []
        filenames: list[str] = []
        for pattern in glob_patterns:
            extension, filename = parse_glob(pattern)
            if extension:
                extensions.append(extension)
            if filename:
                filenames.append(filename)
        relations = [
            {"type": "sub_class_of", "target": child.attrib["type"]}
            for child in item
            if local_name(child.tag) == "sub-class-of" and child.attrib.get("type")
        ]
        magic_count = len([child for child in item if local_name(child.tag) in {"magic", "treemagic", "root-XML"}])
        ids = [external_id("media-type", media_type.lower())]
        ids.extend(external_id("media-type-alias", alias.lower()) for alias in aliases)
        result: dict[str, Any] = {
            "record_id": f"src:{source}:{media_type.lower()}",
            "source": source,
            "external_ids": ids,
            "names": unique([media_type] + aliases),
            "version": None,
            "media_types": unique([media_type.lower()] + [alias.lower() for alias in aliases]),
            "extensions": unique(extensions),
            "filenames": unique(filenames),
            "signatures": [{"kind": "operational-detection-rules", "rule_count": magic_count}] if magic_count else [],
            "source_classes": {"glob_patterns": glob_patterns},
            "relations": relations,
            "evidence": evidence(snapshot, media_type),
        }
        inferred = infer_from_media_type(media_type)
        if inferred:
            result["inferred_facets"] = inferred
        yield result


def unquote_yaml_scalar(value: str) -> str:
    value = value.strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
        try:
            return str(ast.literal_eval(value))
        except (ValueError, SyntaxError):
            return value[1:-1]
    return value


def parse_linguist(path: Path) -> Iterable[dict[str, Any]]:
    current: dict[str, Any] | None = None
    list_key: str | None = None

    def emit(data: dict[str, Any] | None) -> dict[str, Any] | None:
        if not data or not data.get("name"):
            return None
        language_id = str(data.get("language_id", "")).strip()
        record_key = language_id or slug(data["name"])
        category = str(data.get("type", "unknown"))
        family_map = {
            "programming": (["symbolic", "behavioral"], ["source-code"]),
            "markup": (["symbolic"], ["structured-text"]),
            "data": (["symbolic"], ["structured-text"]),
            "prose": (["symbolic"], ["plain-text"]),
        }
        modes, families = family_map.get(category, (["symbolic"], ["unknown"]))
        ids = [external_id("github-linguist-name", data["name"])]
        if language_id:
            ids.append(external_id("github-linguist-language-id", language_id))
        aliases = data.get("aliases", [])
        extensions = [value.lstrip(".").lower() for value in data.get("extensions", [])]
        return {
            "record_id": f"src:linguist:{record_key}",
            "source": "linguist",
            "external_ids": ids,
            "names": unique([data["name"]] + aliases),
            "version": None,
            "media_types": [],
            "extensions": unique(extensions),
            "filenames": unique(data.get("filenames", [])),
            "signatures": [],
            "source_classes": {"linguist_type": category, "group": data.get("group")},
            "relations": ([{"type": "language_group", "target": data["group"]}] if data.get("group") else []),
            "inferred_facets": {
                "method": "heuristic",
                "confidence": 0.82,
                "values": {"mode_of_being": modes, "semantic_family": families},
            },
            "evidence": evidence("github-linguist-2026-07-19", data["name"]),
        }

    for raw_line in path.read_text(encoding="utf-8").splitlines():
        if not raw_line.strip() or raw_line.lstrip().startswith("#"):
            continue
        if not raw_line.startswith((" ", "\t")) and raw_line.rstrip().endswith(":"):
            record = emit(current)
            if record:
                yield record
            current = {"name": unquote_yaml_scalar(raw_line.rstrip()[:-1])}
            list_key = None
            continue
        if current is None:
            continue
        field_match = re.match(r"^  ([a-zA-Z0-9_]+):(?:\s*(.*))?$", raw_line)
        if field_match:
            key, value = field_match.group(1), (field_match.group(2) or "").strip()
            if value:
                current[key] = unquote_yaml_scalar(value)
                list_key = None
            else:
                current[key] = []
                list_key = key
            continue
        item_match = re.match(r"^  -\s+(.*)$", raw_line)
        if item_match and list_key:
            current[list_key].append(unquote_yaml_scalar(item_match.group(1)))

    record = emit(current)
    if record:
        yield record


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", type=Path, default=DEFAULT_SOURCE_ROOT)
    parser.add_argument("--output-root", type=Path, default=ROOT / "catalog")
    args = parser.parse_args()
    source_root = args.source_root.resolve()
    output_root = args.output_root.resolve()

    expected = {
        "iana": source_root / "iana-media-types.xml",
        "pronom": source_root / "pronom-droid-signatures-v124.xml",
        "loc": source_root / "loc-fdd" / "fddXML",
        "tika": source_root / "apache-tika-mimetypes.xml",
        "freedesktop": source_root / "freedesktop-shared-mime.xml.in",
        "linguist": source_root / "github-linguist-languages.yml",
    }
    missing = [str(path) for path in expected.values() if not path.exists()]
    if missing:
        print("Missing required source snapshots:\n- " + "\n- ".join(missing), file=sys.stderr)
        return 2

    producers = [
        parse_iana(expected["iana"]),
        parse_pronom(expected["pronom"]),
        parse_loc(expected["loc"]),
        parse_mime_database(expected["tika"], "tika", "apache-tika-2026-07-19"),
        parse_mime_database(expected["freedesktop"], "freedesktop", "freedesktop-shared-mime-info-2026-07-19"),
        parse_linguist(expected["linguist"]),
    ]

    records: list[dict[str, Any]] = []
    seen: set[str] = set()
    duplicate_ordinals: Counter[str] = Counter()
    for producer in producers:
        for record in producer:
            base_record_id = record["record_id"]
            duplicate_ordinals[base_record_id] += 1
            record_id = base_record_id
            if record_id in seen:
                # A registry may intentionally expose two observations with the
                # same label. Preserve both rather than silently merging them.
                record_id = f"{base_record_id}#{duplicate_ordinals[base_record_id]}"
                record["record_id"] = record_id
            seen.add(record_id)
            records.append(record)
    records.sort(key=lambda item: item["record_id"])

    output_root.mkdir(parents=True, exist_ok=True)
    ndjson_path = output_root / "source_records.ndjson"
    with ndjson_path.open("w", encoding="utf-8") as handle:
        for record in records:
            handle.write(json.dumps(record, ensure_ascii=False, separators=(",", ":")) + "\n")

    by_media_type: dict[str, list[str]] = defaultdict(list)
    by_extension: dict[str, list[str]] = defaultdict(list)
    by_external_id: dict[str, list[str]] = defaultdict(list)
    for record in records:
        for value in record.get("media_types", []):
            by_media_type[value.lower()].append(record["record_id"])
        for value in record.get("extensions", []):
            by_extension[value.lower().lstrip(".")].append(record["record_id"])
        for value in record.get("external_ids", []):
            by_external_id[f"{value['scheme']}:{value['value']}"] .append(record["record_id"])

    for index in (by_media_type, by_extension, by_external_id):
        for key in index:
            index[key] = sorted(set(index[key]))

    write_json(output_root / "indexes" / "media-types.json", dict(sorted(by_media_type.items())))
    write_json(output_root / "indexes" / "extensions.json", dict(sorted(by_extension.items())))
    write_json(output_root / "indexes" / "external-ids.json", dict(sorted(by_external_id.items())))

    source_counts = Counter(record["source"] for record in records)
    summary = {
        "snapshot_id": "everythingx-source-baseline-2026-07-19",
        "observation_count": len(records),
        "source_counts": dict(sorted(source_counts.items())),
        "distinct_media_type_labels": len(by_media_type),
        "distinct_extensions": len(by_extension),
        "distinct_external_identifiers": len(by_external_id),
        "records_with_signatures": sum(bool(record.get("signatures")) for record in records),
        "records_with_relations": sum(bool(record.get("relations")) for record in records),
        "records_with_heuristic_facets": sum("inferred_facets" in record for record in records),
        "catalog_sha256": sha256(ndjson_path),
        "interpretation": "Counts are source observations. They include overlap and must not be reported as a count of unique canonical file formats."
    }
    write_json(output_root / "summary.json", summary)
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
