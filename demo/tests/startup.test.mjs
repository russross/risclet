import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import test from 'node:test';
import { runChromePage } from '../ui/tests/chrome.mjs';
import { observeApp } from './app-fixture.mjs';

// A transient first WASM failure is retryable through the existing Boot VM control.
test('startup failure reports once and Boot VM retries with a complete workspace', async () => {
    const directory = await mkdtemp(join(tmpdir(), 'risclet-startup-'));
    let requests = 0;
    try {
        await runChromePage(`<!doctype html><iframe src="/risclet/index.html" style="width:1200px;height:850px"></iframe><script type="module">
const frame = document.querySelector('iframe');
const pause = () => new Promise(resolve => setTimeout(resolve, 50));
const check = (condition, message) => { if (!condition) throw new Error(message); };
async function until(condition, message) {
    const deadline = performance.now() + 30000;
    while (!condition()) { if (performance.now() > deadline) throw new Error(message); await pause(); }
}
try {
    await until(() => frame.contentWindow.testAlerts?.length, 'startup error was not reported');
    const app = frame.contentWindow;
    const doc = frame.contentDocument;
    const boot = doc.getElementById('vm-boot-button');
    check(app.testAlerts.length === 1 && app.testAlerts[0].includes('503'), 'startup failure reported incorrectly');
    check(!boot.disabled && boot.textContent === 'Boot VM', 'failed preparation has no retry control');
    boot.click();
    await until(() => app.testRuntime?.started && doc.querySelector('.file-tree li.file'), 'retry did not boot and populate widgets');
    check(doc.querySelector('.cm-content').textContent.includes('sort'), 'retry did not restore the selected editor file');
    check(app.testRuntime.filesystem('default').listFiles().includes('sort.s'), 'retry lost workspace files');
    check(app.testAlerts.length === 1 && app.testErrors.length === 0, 'retry reported additional errors');
    await fetch('/result?status=pass');
} catch (error) { await fetch('/result?status=' + encodeURIComponent(error.stack)); }
</script>`, directory, {
            root:resolve(import.meta.dirname, '../dist'), basePath:'/risclet', timeoutMs:40000,
            transform:observeApp,
            response(url) {
                if (url.pathname.endsWith('/riscbox.wasm') && ++requests === 1) return {status:503, body:'temporarily unavailable'};
            },
        });
        assert.equal(requests, 2);
    } finally { await rm(directory, {recursive:true, force:true, maxRetries:10, retryDelay:100}); }
});
