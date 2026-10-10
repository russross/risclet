const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const textmate = require('vscode-textmate');
const oniguruma = require('vscode-oniguruma');

// Use VS Code's tokenizer and regex engine, carrying state between source lines.
async function main() {
  const manifest = JSON.parse(fs.readFileSync(path.join(__dirname, 'package.json'), 'utf8'));
  assert.ok(manifest.contributes.languages[0].extensions.includes('.s'));
  const wasm = fs.readFileSync(require.resolve('vscode-oniguruma/release/onig.wasm'));
  await oniguruma.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength));
  const registry = new textmate.Registry({
    onigLib: Promise.resolve({
      createOnigScanner: patterns => new oniguruma.OnigScanner(patterns),
      createOnigString: source => new oniguruma.OnigString(source),
    }),
    loadGrammar: async () => textmate.parseRawGrammar(
      fs.readFileSync(path.join(__dirname, 'syntaxes/riscv.tmLanguage.json'), 'utf8'),
      'riscv.json',
    ),
  });
  const grammar = await registry.loadGrammar('source.riscv');
  assert.ok(grammar);
  const cases = JSON.parse(fs.readFileSync(path.join(__dirname, '../tests/highlighting.json'), 'utf8'));

  // Fixtures distinguish lexical context, unsupported dialects and literal errors.
  let state = textmate.INITIAL;
  for (const {line, checks} of cases) {
    const result = grammar.tokenizeLine(line, state);
    state = result.ruleStack;
    for (const [column, , scope] of checks) {
      const token = result.tokens.find(token => token.startIndex <= column && token.endIndex > column);
      assert.ok(token?.scopes.includes(`${scope}.riscv`), `${line}:${column}: expected ${scope}, got ${token?.scopes}`);
    }
  }
  registry.dispose();
  console.log('TextMate highlighting checks passed.');
}
main().catch(error => {
  console.error(error.message);
  process.exitCode = 1;
});
