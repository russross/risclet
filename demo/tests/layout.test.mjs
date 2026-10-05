import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { runChromePage } from "../ui/tests/chrome.mjs";

// Fresh pages exercise the initial split at its default, expanded, and capped widths.
test("initial terminal sizing accounts for geometry and preserves later resizing", { timeout: 120_000 }, async () => {
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
        const surface = doc.querySelector('.terminal-surface');
        const cell = parseFloat(frame.contentWindow.getComputedStyle(surface).getPropertyValue('--term-cell-width'));
        const percentage = 100 * (pane.getBoundingClientRect().width + 4) / width;
        const columns = Math.floor(surface.clientWidth / cell);
        check(percentage >= 44.99 && percentage <= 70.01, 'split bounds at ' + width + ': ' + percentage);
        if (width === 2100) check(Math.abs(percentage - 45) < 0.01, 'wide page retains 45 percent');
        if (width === 1250) check(columns >= 80, 'expanded terminal has 80 columns: ' + columns);
        if (width === 900) check(Math.abs(percentage - 70) < 0.01, 'narrow page caps at 70 percent');

        // A later page resize keeps the chosen percentage instead of restoring 80 columns.
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
