import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const script = fileURLToPath(new URL('../../.github/scripts/demo_version.py', import.meta.url));
function select(metadata, version = '0.4.12') {
    return spawnSync('python3', [script, version], {input:JSON.stringify(metadata), encoding:'utf8'});
}
const release = (version, extra = {}) => ({
    tag_name:'v' + version, draft:false, published_at:'2026-10-05T00:00:00Z',
    assets:[{name:'risclet-riscv64gc-unknown-linux-musl'}], ...extra,
});

// The requested Cargo version determines the binary even when other releases exist.
test('deployment accepts exactly the requested published version', () => {
    const result = select(release('0.4.12'));
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout.trim(), '0.4.12');
    for (const version of ['0.5.0-rc.10', '0.5.0+build.1']) {
        assert.equal(select(release(version), version).stdout.trim(), version);
    }
});

test('deployment rejects another version, unpublished releases, and missing binaries', () => {
    for (const [metadata, message] of [
        [release('0.4.13'), /does not match Cargo version/],
        [release('0.4.12', {draft:true}), /is not published/],
        [release('0.4.12', {published_at:null}), /is not published/],
        [release('0.4.12', {assets:[]}), /no RISC-V Linux binary/],
        [null, /expected release metadata/],
    ]) {
        const result = select(metadata);
        assert.equal(result.status, 1);
        assert.match(result.stderr, message);
        assert.doesNotMatch(result.stderr, /Traceback/);
    }
});
