#!/usr/bin/env python3
"""Require the requested published Risclet release and its demo binary."""

import json
import sys

from release_version import SemanticVersion


def published_version(release: dict[str, object], version: str) -> str:
    SemanticVersion.parse(version)
    # Exact tag matching keeps Cargo's version authoritative, including build metadata.
    if release.get("tag_name") != f"v{version}":
        raise ValueError(f"release tag does not match Cargo version {version}")
    if release.get("draft") is not False or not release.get("published_at"):
        raise ValueError(f"release v{version} is not published")

    # The demo needs the published static RISC-V binary rather than a local build.
    assets = release.get("assets")
    if not isinstance(assets, list) or not any(
        isinstance(asset, dict)
        and asset.get("name") == "risclet-riscv64gc-unknown-linux-musl"
        for asset in assets
    ):
        raise ValueError(f"release v{version} has no RISC-V Linux binary")
    return version


def main() -> None:
    try:
        if len(sys.argv) != 2:
            raise ValueError("usage: demo_version.py VERSION")
        release = json.load(sys.stdin)
        if not isinstance(release, dict):
            raise ValueError("expected release metadata")
        print(published_version(release, sys.argv[1]))
    except (ValueError, TypeError, AttributeError) as error:
        raise SystemExit(f"demo version: {error}") from None


if __name__ == "__main__":
    main()
