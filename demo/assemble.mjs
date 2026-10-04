import { createHash } from "node:crypto";
import { cp, mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { join } from "node:path";
import { bundleExamples } from "./bundle.mjs";

// Fetch-loaded assets get immutable names; page-linked scripts keep regular names.
async function asset(name, extension, bytes) {
    const hash = createHash("sha256").update(bytes).digest("hex").slice(0, 16);
    const filename = `${name}-${hash}.${extension}`;
    await writeFile(join("dist", filename), bytes);
    return filename;
}

try {
    await rm("dist", { recursive: true, force: true });
    await mkdir("dist");
    const release = "build/riscbox";
    const entries = await readdir(release);
    const bios = entries.find(name => /^fw_dynamic\.bin-.*\.gz$/.test(name));
    const kernel = entries.find(name => /^linux-.*\.gz$/.test(name));
    if (!bios || !kernel) throw new Error("Riscbox release has no boot payloads");
    await cp(release, "dist/riscbox", { recursive: true });

    // The release's splitter owns disk chunk naming and its manifest format.
    const split = execFileSync(join(release, "splitimg.py"), ["build/rootfs.erofs", "dist"], { encoding: "utf8" });
    const drive = split.trim().split(" ")[0];
    await writeFile("dist/riscbox.cfg", JSON.stringify({
        version: 1, machine: "riscv64", memory_size: 256,
        bios: `riscbox/${bios}`, kernel: `riscbox/${kernel}`,
        cmdline: "root=/dev/vda ro rootfstype=erofs console=hvc0 quiet loglevel=0",
        console: "virtio", uart_output: true,
        drive0: { file: `${drive}/blk.txt` },
        fs0: { server: "default", tag: "risclet" },
    }, null, 2) + "\n");
    const examples = await asset("examples", "json.gz", await bundleExamples("examples"));

    // Lazy chunks and terminal WASM retain hashes while the entry script is stable.
    const ui = (await readdir("build/ui")).filter(name => name !== ".built");
    for (const name of ui) await cp(join("build/ui", name), join("dist", name));
    if (!ui.includes("app.js")) throw new Error("Application bundle is missing");
    await cp("web/style.css", "dist/style.css");
    await cp("web/favicon.svg", "dist/favicon.svg");
    const html = (await readFile("web/index.html", "utf8")).replace("{{examples}}", examples);
    await writeFile("dist/index.html", html);
    await writeFile("dist/.nojekyll", "");
    await cp("build/versions", "dist/versions.txt");
} catch (error) {
    console.error(`Demo assembly failed: ${error.message}`);
    process.exitCode = 1;
}
