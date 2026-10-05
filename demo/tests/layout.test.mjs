import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { runChromePage } from "../ui/tests/chrome.mjs";

// Fresh pages retain the same pane proportions across viewport widths.
test("initial layout retains fixed pane proportions across viewport widths", { timeout: 120_000 }, async () => {
    const directory = await mkdtemp(join(tmpdir(), "risclet-layout-"));
    try {
        await runChromePage(`<!doctype html><script type="module">
const check = (condition, message) => { if (!condition) throw new Error(message); };
const pause = () => new Promise(resolve => setTimeout(resolve, 50));
try {
    for (const width of [2100, 1250, 900]) {
        const frame = document.createElement('iframe');
        frame.style.cssText = 'width:' + width + 'px;height:850px';
        frame.src = '/risclet/index.html?example=reduction';
        document.body.append(frame);
        const deadline = performance.now() + 30000;
        while (!frame.contentDocument?.querySelector('#instructions-tab-content h1')) {
            if (performance.now() > deadline) throw new Error('page load at ' + width);
            await pause();
        }
        const doc = frame.contentDocument;
        const pane = doc.getElementById('info-pane');
        doc.getElementById('instructions-tab-content').classList.remove('active');
        doc.getElementById('vm-tab-content').classList.add('active');
        await pause();
        const mainWidth = doc.getElementById('main-content').clientWidth;
        for (const [id, expected, gutter] of [['file-tree-pane', 10, 4], ['editor-pane', 45, 8], ['info-pane', 45, 4]]) {
            const percentage = 100 * (doc.getElementById(id).getBoundingClientRect().width + gutter) / mainWidth;
            check(Math.abs(percentage - expected) < 0.1, id + ' proportion at ' + width + ': ' + percentage);
        }

        // A later page resize preserves the assigned pane proportions.
        const assignedWidth = pane.style.width;
        frame.style.width = '800px';
        await pause();
        check(pane.style.width === assignedWidth, 'later resize preserves split');
        frame.remove();
    }
    await fetch('/result?status=pass');
} catch (error) { await fetch('/result?status=' + encodeURIComponent(error.stack ?? String(error))); }
</script>`, directory, { root: resolve(import.meta.dirname, "../dist"), basePath: "/risclet", timeoutMs: 110_000 });
    } finally { await rm(directory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 }); }
});
