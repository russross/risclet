#!/usr/bin/env python3
"""Select the highest published Risclet version with a demo-compatible binary."""

import json
import sys

from release_version import SemanticVersion


def latest_version(pages: list[list[dict[str, object]]]) -> str:
    selected: SemanticVersion | None = None
    version = ""
    # Release dates and tag creation order do not determine semantic version precedence.
    for page in pages:
        for release in page:
            tag = release.get("tag_name")
            if release.get("draft") or not isinstance(tag, str) or not tag.startswith("v"):
                continue
            try:
                candidate = SemanticVersion.parse(tag[1:])
            except ValueError:
                continue
            assets = release.get("assets")
            if not isinstance(assets, list) or not any(
                isinstance(asset, dict) and asset.get("name") == "risclet-riscv64gc-unknown-linux-musl"
                for asset in assets
            ):
                continue
            if selected is None or candidate.has_higher_precedence_than(selected):
                selected = candidate
                version = tag[1:]
    if selected is None:
        raise ValueError("no published Risclet release has a RISC-V Linux binary")
    return version


def main() -> None:
    try:
        pages = json.load(sys.stdin)
        if not isinstance(pages, list) or any(not isinstance(page, list) for page in pages):
            raise ValueError("expected paginated release metadata")
        print(latest_version(pages))
    except (ValueError, TypeError, AttributeError) as error:
        raise SystemExit(f"demo version: {error}") from None


if __name__ == "__main__":
    main()
