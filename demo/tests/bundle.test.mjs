import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";
import ts from "../ui/node_modules/typescript/lib/typescript.js";
const execute = promisify(execFile);
async function bundleExamples(directory) {
    const output = join(directory, ".bundle.json.gz");
    await execute("python3", [fileURLToPath(new URL("../scripts/bundle.py", import.meta.url)), directory, output]);
    return readFile(output);
}

const source = await readFile(new URL("../ui/examples.ts", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } });
const { parseExamples } = await import(`data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`);

// Bundling and decoding preserve binary bytes, empty files, and nested paths.
test("one deterministic bundle contains every source byte", async () => {
    const directory = await mkdtemp(join(tmpdir(), "risclet-bundle-"));
    try {
        await mkdir(join(directory, "binary/nested"), { recursive: true });
        const bytes = Buffer.from([0, 255, 128, 13, 10]);
        await writeFile(join(directory, "binary/nested/data"), bytes);
        await writeFile(join(directory, "binary/empty"), "");
        await writeFile(join(directory, "examples.json"), JSON.stringify([
            { id: "binary", title: "Binary", editable: "empty", files: ["nested/data", "empty"] },
        ]));
        const bundle = await bundleExamples(directory);
        assert.deepEqual(await bundleExamples(directory), bundle);
        const examples = parseExamples(JSON.parse(gunzipSync(bundle)));
        assert.deepEqual(Buffer.from(examples[0].files.get("nested/data")), bytes);
        assert.equal(examples[0].files.get("empty").length, 0);
    } finally { await rm(directory, { recursive: true, force: true }); }
});

test("bundle validation rejects partial or ambiguous workspaces", () => {
    const file = { path: "main.s", size: 1, content: "AA==" };
    const example = { id: "demo", title: "Demo", editable: "main.s", files: [file] };
    const parse = examples => parseExamples({ version: 1, examples });
    assert.throws(() => parse([example, example]), /Duplicate example IDs/);
    assert.throws(() => parse([{ ...example, files: [file, file] }]), /Duplicate bundled path/);
    assert.throws(() => parse([{ ...example, files: [{ ...file, path: "../main.s" }] }]), /Invalid example path/);
    assert.throws(() => parse([{ ...example, files: [{ ...file, size: 2 }] }]), /Incorrect bundled file size/);
    assert.throws(() => parse([{ ...example, files: [file, { ...file, path: "main.s/child" }] }]), /also a directory/);
    assert.throws(() => parse([{ ...example, documentation: "missing.md" }]), /Missing documentation/);
    assert.throws(() => parse([]), /Invalid examples bundle/);
});
