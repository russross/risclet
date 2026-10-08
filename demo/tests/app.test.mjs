import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { runChromePage } from "../ui/tests/chrome.mjs";
import { observeApp } from "./app-fixture.mjs";

test("deployed demo boots, synchronizes, switches, reboots, and recovers", { timeout: 240_000 }, async () => {
    const directory = await mkdtemp(join(tmpdir(), "risclet-app-"));
    const requests = [];
    const versions = await readFile(new URL("../build/versions", import.meta.url), "utf8");
    const version = versions.match(/^risclet=(.+)$/m)[1];
    try {
        await runChromePage(`<!doctype html><meta charset="utf-8"><iframe src="/risclet/index.html" style="width:1250px;height:850px"></iframe><script type="module">
const frame = document.querySelector('iframe');
const sleep = () => new Promise(resolve => setTimeout(resolve, 50));
const check = (condition, message) => { if (!condition) throw new Error(message); };
let app, doc, runtime;
async function until(condition, message) {
    const deadline = performance.now() + 45000;
    while (!await condition()) {
        if (performance.now() > deadline) throw new Error(message + ': ' + doc?.getElementById('status')?.textContent + '\\n' + app?.testOutput.slice(-2500));
        await sleep();
    }
}
function button(id) { return doc.getElementById(id); }
function click(id) { check(!button(id).disabled, id + ' is disabled'); button(id).click(); }
function fileText(path) { return new TextDecoder().decode(runtime.filesystem('default').readFile(path)); }
async function edit(text) {
    const content = doc.querySelector('.cm-content');
    content.focus();
    doc.execCommand('selectAll');
    doc.execCommand('insertText', false, text);
    await sleep();
}
let serial = 0;
// Command entry follows the painted prompt, after asynchronous parser replies.
async function paintedPrompt() {
    await new Promise(resolve => app.requestAnimationFrame(() => app.requestAnimationFrame(resolve)));
}
async function command(shell, expected) {
    await paintedPrompt();
    const marker = 'RESULT_' + ++serial;
    app.testOutput = '';
    const bytes = new app.TextEncoder().encode(shell + '; printf "\\\\n' + marker + '\\\\n"\\r');
    check(runtime.consoleInput(bytes) === bytes.length, 'console input accepted');
    await until(() => app.testOutput.replace(/\\r/g, '').includes('\\n' + marker + '\\n'), shell);
    check(app.testOutput.includes(expected), 'missing ' + expected + ': ' + app.testOutput);
}
async function prompt() {
    await until(() => app.testOutput.includes('risclet:~$'), 'risclet login');
    await paintedPrompt();
}
async function select(title) {
    const item = [...doc.querySelectorAll('.example-button')].find(button => button.textContent === title);
    item.click();
    await until(() => item.isConnected === false && [...doc.querySelectorAll('.example-button')].some(button => button.textContent === title && button.disabled), 'select ' + title);
    await until(() => !button('vm-boot-button').disabled, 'selection controls');
}
try {
    await until(() => frame.contentDocument?.getElementById('vm-boot-button'), 'page load');
    app = frame.contentWindow; doc = frame.contentDocument;
    await until(() => !frame.contentDocument.getElementById('vm-boot-button').disabled, 'initial workspace');
    frame.contentDocument.getElementById('vm-tab-button').click();
    await until(() => frame.contentWindow.testRuntime?.started, 'initial boot');
    app = frame.contentWindow; doc = frame.contentDocument; runtime = app.testRuntime;
    await prompt();
    await command('id; risclet --version', ${JSON.stringify(version)});
    check(app.testOutput.includes('uid=1000(risclet)'), 'guest user');
    check(!button('sync-button') && !button('vm-reset-button') && !button('status'), 'removed menu controls');
    await command('test ! -x /usr/bin/vim && test ! -x /usr/bin/micro && test ! -d /usr/share/zoneinfo && echo MINIMAL_ROOT_OK', ${JSON.stringify("\r\nMINIMAL_ROOT_OK\r\n")});
    await command(${JSON.stringify("awk '$2 == \"/\" {print $3, $4}' /proc/mounts")}, 'ext4 rw');
    await command(${JSON.stringify("awk '$2 == \"/home/risclet\" {print $1, $3}' /proc/mounts")}, ${JSON.stringify("\r\nshared 9p\r\n")});
    await command(${JSON.stringify("test -z \"$(grep -E ' (overlay|tmpfs) ' /proc/mounts)\" && echo NO_OVERLAYS")}, ${JSON.stringify("\r\nNO_OVERLAYS\r\n")});
    await command('echo writable > /tmp/root-write; sync; cat /tmp/root-write', ${JSON.stringify("\r\nwritable\r\n")});
    const original = fileText('insertion_sort.s');
    const fs = runtime.filesystem('default');

    // Blur and terminal interaction flush buffered edits before guest use.
    await edit(original.trimEnd() + '\\n# editor change');
    check(!fileText('insertion_sort.s').includes('# editor change'), 'edit stays buffered');
    doc.querySelector('#vm-terminal textarea').focus();
    await until(() => fileText('insertion_sort.s').includes('# editor change'), 'blur synchronization');
    await edit(original.trimEnd() + '\\n# interaction change');
    click('vm-tab-button');
    await until(() => fileText('insertion_sort.s').includes('# interaction change'), 'VM tab synchronization');
    doc.querySelector('#vm-terminal textarea').focus();
    await command("printf '\\n# guest change\\n' >> insertion_sort.s; printf 'guest file' > extra; ln extra alias; ln -s extra symbolic", 'RESULT_');
    await until(() => doc.querySelector('.cm-content').textContent.includes('# guest change'), 'guest write reaches editor');
    check(doc.activeElement.closest('#vm-terminal') !== null, 'guest refresh preserves terminal focus');

    // A guest reboot reports completion independently of request acceptance.
    const width = button('vm-boot-button').getBoundingClientRect().width;
    app.testOutput = '';
    click('vm-boot-button');
    await until(() => app.testEvents.includes('guest-reboot'), 'soft reboot callback');
    await prompt();
    await command('cat /tmp/root-write', ${JSON.stringify("\r\nwritable\r\n")});
    check(fileText('extra') === 'guest file', 'soft reboot retains workspace');
    check(button('vm-boot-button').getBoundingClientRect().width === width, 'reboot label width');

    // Stop request delivery to model an unresponsive guest, then use the same button.
    const requestReboot = runtime.requestReboot.bind(runtime);
    runtime.requestReboot = async () => {};
    click('vm-boot-button');
    await until(() => button('vm-boot-button').textContent === 'Reset VM', 'recovery label');
    check(button('vm-boot-button').getBoundingClientRect().width === width, 'reset label width');
    await edit(original.trimEnd() + '\\n# pending recovery edit');
    const beforeRecovery = fileText('insertion_sort.s');
    app.testOutput = '';
    click('vm-boot-button');
    await until(() => button('vm-boot-button').textContent === 'Reboot VM' && !button('vm-boot-button').disabled, 'forced recovery');
    await prompt();
    check(doc.querySelector('.cm-content').textContent.includes('# pending recovery edit'), 'recovery retains buffered text');
    check(fileText('insertion_sort.s') === beforeRecovery, 'recovery does not flush buffered edits');
    check(fileText('extra') === 'guest file', 'recovery retains workspace');
    runtime.requestReboot = requestReboot;

    // Switches cold-reset disks while saving complete namespaces in memory.
    const disk = runtime.block(0);
    await runtime.halt();
    check(disk.capacitySectors === 32768n, 'root disk is 16 MiB');
    const sector = disk.capacitySectors - 1n;
    const originalSector = (await disk.read(sector, 512)).slice();
    disk.write(sector, new app.Uint8Array(512).fill(0x5a));
    await select('Binary search');
    check(button('instructions-tab-button').textContent === 'README' && fs.listFiles().includes('README.md'), 'README instructions');
    check(!runtime.started, 'instructions defer boot');
    check(!fs.listFiles().includes('extra'), 'outgoing files absent from new example');
    check((await disk.read(sector, 512)).every((byte, index) => byte === originalSector[index]), 'switch discards disk overlay');

    // Rapid choices retire intermediate work without losing the final workspace.
    const choose = title => [...doc.querySelectorAll('.example-button')].find(button => button.textContent === title).click();
    choose('Insertion sort'); choose('Binary search'); choose('Insertion sort'); choose('Binary search');
    await until(() => !button('vm-boot-button').disabled && [...doc.querySelectorAll('.example-button')].some(button => button.textContent === 'Binary search' && button.disabled), 'rapid final selection');
    check(fs.listFiles().includes('binary_search.s') && !fs.listFiles().includes('insertion_sort.s'), 'rapid switch selected the wrong namespace');

    // Documentation errors stay inside their pane and do not block the editor or VM.
    const readme = fileText('README.md');
    fs.writeFile('README.md', new app.TextEncoder().encode('# Changed README\\n![missing](missing.png)'));
    await until(() => doc.querySelector('#instructions-tab-content [role=status]'), 'README error pane');
    check(!doc.querySelector('#instructions-tab-content h1') && !button('vm-boot-button').disabled, 'README error retained stale content or blocked controls');
    fs.writeFile('README.md', new app.TextEncoder().encode('# Safe README\\n<img src="/missing" onerror="window.readmeExecuted=true">\\n\\n[unsafe](javascript:alert(1))'));
    await until(() => doc.querySelector('#instructions-tab-content h1')?.textContent === 'Safe README', 'README recovery');
    check(!app.readmeExecuted && !doc.querySelector('#instructions-tab-content img'), 'guest README executed host HTML');
    check(!doc.querySelector('#instructions-tab-content a').hasAttribute('href'), 'README retained an unsafe link');
    fs.writeFile('README.md', new app.TextEncoder().encode(readme));
    click('vm-tab-button');
    app.testOutput = '';
    await prompt();
    await command('risclet --strict', 'target 23 -> index 9');
    check(app.testOutput.includes('target 100 -> index -1'), 'absent search result');
    await select('Quicksort');
    click('vm-tab-button');
    app.testOutput = '';
    await prompt();
    await command('risclet --strict', 'after:  [-6, -3, -1, 0, 1, 2, 4, 5, 7, 7, 8, 9]');
    await select('Guess the digit');
    click('vm-tab-button');
    app.testOutput = '';
    await prompt();
    await command("printf '2\\n9\\nxx\\n6\\n' | risclet --strict", 'Correct!');
    check(app.testOutput.includes('Too low.') && app.testOutput.includes('Too high.') && app.testOutput.includes('Enter exactly one digit.'), 'game responses');
    await select('Insertion sort');
    check(fileText('insertion_sort.s').includes('# pending recovery edit'), 'switch flushes and restores edits');
    click('vm-tab-button');
    app.testOutput = '';
    await prompt();
    await command('risclet --strict', 'after:  [-6, -3, -1, 0, 1, 2, 4, 5, 7, 7, 8, 9]');
    check(fileText('extra') === 'guest file' && fs.stat('extra').inode === fs.stat('alias').inode, 'switch restores guest files and hard links');
    check(fs.readlink('symbolic') === 'extra', 'switch restores symlinks');

    check(app.testErrors.length === 0, app.testErrors.join('\\n'));
    check(app.testAlerts.length === 0, app.testAlerts.join('\\n'));
    await fetch('/result?status=pass');
} catch (error) { await fetch('/result?status=' + encodeURIComponent((error.stack ?? String(error)) + '\\n' + app?.testErrors?.join('\\n'))); }
</script>`, directory, {
            root: resolve(import.meta.dirname, "../dist"), basePath: "/risclet", timeoutMs: 220_000,
            chromeArgs: ["--remote-debugging-port=0", "--window-size=1400,1000"],
            onRequest: url => requests.push(url.pathname),
            transform: observeApp,
        });
        assert.equal(requests.filter(path => /\/examples-[a-f0-9]+\.json\.gz$/.test(path)).length, 1);
        assert.equal(requests.filter(path => path.includes("/examples/")).length, 0);
    } finally { await rm(directory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 }); }
});
