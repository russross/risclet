import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import ts from "typescript";

const source = await readFile(new URL("../disk-prefetch.ts", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } });
const { DiskPrefetch } = await import(`data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`);
const configUrl = "https://example.test/demo/image.cfg";

// Held response bodies expose concurrency and cancellation independently of request headers.
test("prefetch resolves relative paths, overlaps two bodies, and abandons remaining blocks", async () => {
    const original = globalThis.fetch;
    const requests = [];
    const pending = [];
    const prefetch = new DiskPrefetch();
    globalThis.fetch = async (url, { signal }) => {
        requests.push(String(url));
        if (String(url) === configUrl) return Response.json({ drive0: { file: "disk/blk.txt" } });
        if (String(url).endsWith("blk.txt")) return new Response("{ block_size: 256, n_block: 5, }");
        return { arrayBuffer: () => new Promise((resolve, reject) => {
            pending.push(resolve);
            signal.addEventListener("abort", () => reject(signal.reason), { once: true });
        }) };
    };
    const tick = () => new Promise(resolve => setImmediate(resolve));
    try {
        const complete = prefetch.start(configUrl);
        await tick();
        assert.deepEqual(requests.slice(2), [
            "https://example.test/demo/disk/blk000000000.bin",
            "https://example.test/demo/disk/blk000000001.bin",
        ]);
        pending[0](new ArrayBuffer(0));
        await tick();
        assert.equal(requests.at(-1), "https://example.test/demo/disk/blk000000002.bin");
        prefetch.stop();
        await complete;
        await prefetch.start(configUrl);
        assert.equal(requests.length, 5);
    } finally { globalThis.fetch = original; }
});

// Optional warming tolerates failed blocks and malformed or unavailable metadata.
test("prefetch continues after block errors and ignores metadata failures or early boot", async () => {
    const original = globalThis.fetch;
    try {
        let blocks = 0;
        globalThis.fetch = async url => {
            if (String(url) === configUrl) return Response.json({ drive0: { file: "disk/blk.txt" } });
            if (String(url).endsWith("blk.txt")) return new Response("{ block_size: 256, n_block: 4, }");
            blocks += 1;
            if (blocks === 1) throw new Error("network failure");
            return new Response("block");
        };
        await new DiskPrefetch().start(configUrl);
        assert.equal(blocks, 4);
        for (const response of [Response.json({}), new Response("unavailable", { status: 503 })]) {
            globalThis.fetch = async () => response;
            await new DiskPrefetch().start(configUrl);
        }
        globalThis.fetch = async () => { throw new Error("offline"); };
        await new DiskPrefetch().start(configUrl);
        globalThis.fetch = () => { assert.fail("boot must prevent all prefetch requests"); };
        const stopped = new DiskPrefetch();
        stopped.stop();
        await stopped.start(configUrl);
    } finally { globalThis.fetch = original; }
});
