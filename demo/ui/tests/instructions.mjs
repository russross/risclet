import { InstructionsPane } from '../instructions.ts';

const encoder = new TextEncoder();
const check = (condition, message) => { if (!condition) throw new Error(message); };
export async function run() {
    const files = new Map();
    const fs = {
        listFiles: () => [...files.keys()],
        readFile: path => { if (!files.has(path)) throw new Error('Missing file: ' + path); return encoder.encode(files.get(path)); },
    };
    const host = document.createElement('section');
    const button = document.createElement('button');
    document.body.append(button, host);
    const pane = new InstructionsPane(host, button);
    const write = (path, text) => {
        files.set(path, text);
        pane.handleChange(fs, {kind:'write', path, aliases:[], source:'guest', origin:0n});
    };

    // Host HTML and navigation remain inert even when a guest rewrites its README.
    files.set('doc/README.md', '# Documentation\n<img src="/missing" onerror="window.readmeExecuted=true">\n\n[unsafe](javascript:alert(1))');
    pane.update(fs, 'doc/README.md');
    await new Promise(resolve => setTimeout(resolve, 50));
    check(!window.readmeExecuted && !host.querySelector('img'), 'raw HTML became executable');
    check(!host.querySelector('a').hasAttribute('href'), 'unsafe link survived safe rendering');
    check(host.querySelector('h1').textContent === 'Documentation', 'ordinary Markdown changed');

    // Missing dependencies replace stale content and recover when that dependency appears.
    write('doc/README.md', '# New documentation\n![diagram](diagram.svg)');
    check(host.querySelector('[role=status]')?.textContent.includes('Missing file'), 'missing image did not produce a local error');
    check(!host.querySelector('h1'), 'failed rendering retained previous content');
    write('doc/diagram.svg', '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" onload="window.readmeExecuted=true"><rect width="20" height="20"/></svg>');
    const image = host.querySelector('img');
    await image.decode();
    check(image.src.startsWith('data:image/svg+xml;base64,') && image.naturalWidth === 20, 'relative SVG image did not load');
    check(!window.readmeExecuted, 'SVG image executed host JavaScript');
    write('unrelated.s', 'ignored');
    check(host.querySelector('img') === image, 'unrelated changes rebuilt documentation');
    write('doc/README.md', '![unsupported](diagram.txt)');
    check(host.textContent.includes('unsupported type'), 'unsupported image was not contained');
    write('doc/README.md', '# Recovered');
    check(host.querySelector('h1').textContent === 'Recovered', 'README did not recover after correction');
    files.delete('doc/README.md');
    pane.handleChange(fs, {kind:'unlink', path:'doc/README.md', aliases:[], source:'guest', origin:0n});
    check(!pane.visible && host.textContent === '', 'removed README left a stale pane');
    files.set('doc/README.md', '# Recreated');
    pane.handleChange(fs, {kind:'create', path:'doc/README.md', aliases:[], source:'guest', origin:0n});
    check(pane.visible && host.querySelector('h1').textContent === 'Recreated', 'recreated README did not reopen');
    pane.update(fs, undefined);
    check(!pane.visible && host.textContent === '', 'example without documentation retained the previous pane');
}
