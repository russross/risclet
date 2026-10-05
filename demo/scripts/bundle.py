#!/usr/bin/env python3
"""Pack example workspaces into the browser's versioned JSON format."""

import argparse
import base64
import gzip
import json
from pathlib import Path


def relative_path(value: object) -> str:
    if not isinstance(value, str) or "\\" in value or "\0" in value:
        raise ValueError(f"invalid example path: {value!r}")
    if any(part in ("", ".", "..") for part in value.split("/")):
        raise ValueError(f"invalid example path: {value!r}")
    return value


def bundle_examples(directory: Path) -> bytes:
    descriptions = json.loads((directory / "examples.json").read_text())
    if not isinstance(descriptions, list) or not descriptions:
        raise ValueError("examples.json must contain a nonempty array")
    examples: list[dict[str, object]] = []
    ids: set[str] = set()
    # Validate namespaces before opening paths or publishing any output.
    for description in descriptions:
        if not isinstance(description, dict) or not isinstance(description.get("title"), str):
            raise ValueError("invalid example description")
        identifier = relative_path(description.get("id"))
        if "/" in identifier or identifier in ids:
            raise ValueError(f"invalid or duplicate example ID: {identifier}")
        ids.add(identifier)
        names = description.get("files")
        if not isinstance(names, list):
            raise ValueError(f"missing files: {identifier}")
        paths = [relative_path(name) for name in names]
        if len(set(paths)) != len(paths):
            raise ValueError(f"duplicate example path: {identifier}")
        for path in paths:
            parents = Path(path).parents
            if any(str(parent) in paths for parent in parents):
                raise ValueError(f"file is also a directory: {identifier}/{path}")
        editable = relative_path(description.get("editable"))
        if editable not in paths:
            raise ValueError(f"missing editable file: {identifier}/{editable}")
        example: dict[str, object] = {"id": identifier, "title": description["title"], "editable": editable}
        if "documentation" in description:
            documentation = relative_path(description["documentation"])
            if documentation not in paths:
                raise ValueError(f"missing documentation file: {identifier}/{documentation}")
            example["documentation"] = documentation

        # Base64 preserves arbitrary bytes; a fixed gzip timestamp makes bundles stable.
        files: list[dict[str, object]] = []
        for path in paths:
            source = directory / identifier / path
            if not source.resolve().is_relative_to((directory / identifier).resolve()):
                raise ValueError(f"example symlink escapes workspace: {identifier}/{path}")
            content = source.read_bytes()
            files.append({"path": path, "size": len(content), "content": base64.b64encode(content).decode("ascii")})
        example["files"] = files
        examples.append(example)
    data = json.dumps({"version": 1, "examples": examples}, separators=(",", ":")).encode()
    return gzip.compress(data, compresslevel=9, mtime=0)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    try:
        data = bundle_examples(args.directory)
        temporary = args.output.with_name(args.output.name + ".part")
        temporary.write_bytes(data)
        temporary.replace(args.output)
    except (OSError, ValueError, KeyError) as error:
        raise SystemExit(f"example bundle: {error}") from None


if __name__ == "__main__":
    main()
