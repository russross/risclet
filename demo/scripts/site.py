#!/usr/bin/env python3
"""Write runtime configuration or the site's content-addressed entry page."""

import argparse
import hashlib
import html
import json
from pathlib import Path


def write_changed(path: Path, content: bytes) -> None:
    if path.is_file() and path.read_bytes() == content:
        return
    temporary = path.with_name(path.name + ".part")
    temporary.write_bytes(content)
    temporary.replace(path)


def asset(source: Path) -> str:
    content = source.read_bytes()
    digest = hashlib.sha256(content).hexdigest()[:16]
    extension = "".join(source.suffixes)
    stem = source.name[:-len(extension)] if extension else source.name
    name = f"{stem}-{digest}{extension}"
    write_changed(Path("dist") / name, content)
    return name


def configuration(version: str) -> None:
    release = Path("build/riscbox")
    # Require exactly one payload of each kind rather than depending on directory order.
    bios = sorted(release.glob("fw_dynamic.bin-*.gz"))
    kernels = sorted(release.glob("linux-*.gz"))
    if len(bios) != 1 or len(kernels) != 1:
        raise ValueError("Riscbox release must contain one firmware and one kernel payload")
    runtime = f"riscbox-{version}"
    drive = Path("build/disk-name").read_text().strip()
    config = {
        "version": 1, "machine": "riscv64", "memory_size": 256,
        "bios": f"{runtime}/{bios[0].name}", "kernel": f"{runtime}/{kernels[0].name}",
        "cmdline": "root=/dev/vda rw rootfstype=ext4 console=hvc0 quiet loglevel=0",
        "console": "virtio", "uart_output": True,
        "drive0": {"file": f"{drive}/blk.txt"},
        "fs0": {"server": "default", "tag": "shared"},
    }
    write_changed(Path("build/riscbox.cfg"), (json.dumps(config, indent=2) + "\n").encode())


def publish(version: str) -> None:
    scripts = sorted(Path("build/ui").glob("app-*.js"))
    if len(scripts) != 1:
        raise ValueError("UI build must contain one application entry script")
    runtime = f"riscbox-{version}"
    replacements = {
        "examples": asset(Path("build/examples.json.gz")),
        "config": asset(Path("build/riscbox.cfg")),
        "style": asset(Path("web/style.css")),
        "app": scripts[0].name,
        "runtime": f"{runtime}/riscbox.js",
        "wasm": f"{runtime}/riscbox.wasm",
    }
    # Every linked mutable asset has a content name or an immutable release version.
    page = Path("web/index.html").read_text()
    for key, value in replacements.items():
        page = page.replace("{{" + key + "}}", html.escape(value, quote=True))
    if "{{" in page:
        raise ValueError("unresolved site template placeholder")
    write_changed(Path("dist/index.html"), page.encode())


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("config", "publish"))
    parser.add_argument("version")
    args = parser.parse_args()
    try:
        if args.action == "config":
            configuration(args.version)
        else:
            publish(args.version)
    except (OSError, ValueError) as error:
        raise SystemExit(f"site {args.action}: {error}") from None


if __name__ == "__main__":
    main()
