import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const script = fileURLToPath(new URL('../../.github/scripts/demo_version.py', import.meta.url));
function select(pages) {
    return spawnSync('python3', [script], {input:JSON.stringify(pages), encoding:'utf8'});
}
const release = (version, extra = {}) => ({
    tag_name:'v' + version, draft:false,
    assets:[{name:'risclet-riscv64gc-unknown-linux-musl'}], ...extra,
});

// Publish order, pagination, drafts, and prereleases cannot silently select older binaries.
test('manual deployment selects the highest usable published semantic version', () => {
    const result = select([[release('0.4.9'), release('0.4.12'), release('0.5.0-rc.2')],
        [release('0.5.0-rc.10'), release('0.5.0', {draft:true}), release('1.0.0', {assets:[]}), release('0.4.10')]]);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout.trim(), '0.5.0-rc.10');
    assert.equal(select([[release('0.5.0-rc.10'), release('0.5.0'), release('0.4.99')]]).stdout.trim(), '0.5.0');
});

test('manual deployment fails clearly when no published binary can be selected', () => {
    const result = select([[release('0.5.0', {draft:true}), release('not-a-version')]]);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /no published Risclet release/);
    assert.doesNotMatch(result.stderr, /Traceback/);
});
