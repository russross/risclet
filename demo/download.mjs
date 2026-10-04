import { createHash } from "node:crypto";
import { mkdir, rename, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

// Release metadata binds a versioned asset name to GitHub's upload checksum.
try {
    const [project, version, destination] = process.argv.slice(2);
    if (!["riscbox", "risclet"].includes(project) || !version || !destination) {
        throw new Error("usage: node download.mjs riscbox|risclet VERSION DESTINATION");
    }
    const headers = { Accept: "application/vnd.github+json" };
    if (process.env.GH_TOKEN) headers.Authorization = `Bearer ${process.env.GH_TOKEN}`;
    const metadata = await fetch(`https://api.github.com/repos/russross/${project}/releases/tags/v${version}`, { headers });
    if (!metadata.ok) throw new Error(`${project} v${version}: HTTP ${metadata.status}`);
    const release = await metadata.json();
    if (release.draft) throw new Error(`${project} v${version} has not been published`);
    const name = project === "riscbox" ? `riscbox-${version}.tar.gz` : "risclet-riscv64gc-unknown-linux-musl";
    const asset = release.assets.find(asset => asset.name === name);
    if (!asset || !/^sha256:[a-f0-9]{64}$/.test(asset.digest ?? "")) {
        throw new Error(`${project} v${version} is missing ${name} or its SHA-256 digest`);
    }

    // An interrupted or invalid download never occupies the final cache path.
    const response = await fetch(asset.browser_download_url);
    if (!response.ok) throw new Error(`${name}: HTTP ${response.status}`);
    const bytes = Buffer.from(await response.arrayBuffer());
    const digest = `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
    if (digest !== asset.digest) throw new Error(`${name}: SHA-256 checksum mismatch`);
    await mkdir(dirname(destination), { recursive: true });
    await writeFile(`${destination}.part`, bytes);
    await rename(`${destination}.part`, destination);
    console.log(`Downloaded and verified ${project} v${version}`);
} catch (error) {
    console.error(`Release download failed: ${error.message}`);
    process.exitCode = 1;
}
