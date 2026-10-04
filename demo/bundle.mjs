import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { gzipSync } from "node:zlib";

function relativePath(path) {
    if (typeof path !== "string" || path.includes("\\") || path.includes("\0")
        || path.split("/").some(part => !part || part === "." || part === "..")) {
        throw new Error(`Invalid example path: ${JSON.stringify(path)}`);
    }
    return path;
}

// Base64 preserves arbitrary file bytes in one portable, compressed JSON bundle.
export async function bundleExamples(directory) {
    const descriptions = JSON.parse(await readFile(join(directory, "examples.json"), "utf8"));
    const examples = [];
    const ids = new Set();
    for (const description of descriptions) {
        const id = relativePath(description.id);
        if (id.includes("/") || ids.has(id)) throw new Error(`Invalid or duplicate example ID: ${id}`);
        ids.add(id);
        const paths = new Set();
        const files = [];
        for (const path of description.files) {
            relativePath(path);
            if (paths.has(path)) throw new Error(`Duplicate example path: ${id}/${path}`);
            paths.add(path);
            const bytes = await readFile(join(directory, id, path));
            files.push({ path, size: bytes.length, content: bytes.toString("base64") });
        }
        if (!paths.has(description.editable)) throw new Error(`Missing editable file: ${id}`);
        if (description.documentation !== undefined && !paths.has(description.documentation)) {
            throw new Error(`Missing documentation file: ${id}`);
        }
        examples.push({ ...description, files });
    }
    return gzipSync(JSON.stringify({ version: 1, examples }), { level: 9 });
}
