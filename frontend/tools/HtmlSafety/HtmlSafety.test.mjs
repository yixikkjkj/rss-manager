import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));
const cli = join(dirname(fileURLToPath(import.meta.resolve('oxlint/package.json'))), 'bin/oxlint');
const fixtures = mkdtempSync(join(root, 'src', '.HtmlSafety-'));
after(() => rmSync(fixtures, { recursive: true, force: true }));

function lint(path) {
  const result = spawnSync(process.execPath, [cli, '--deny-warnings', path], {
    cwd: root,
    encoding: 'utf8',
  });
  if (result.error) throw result.error;
  return { status: result.status, output: result.stdout + result.stderr };
}
function fixture(source, name = 'Probe.ts') {
  const path = join(fixtures, name);
  writeFileSync(path, source);
  return lint(path);
}

test('Oxlint blocks HTML assignment, insertion and Lit property bindings', () => {
  for (const source of [
    "document.body.innerHTML = '<p>unsafe</p>';",
    "document.body['outerHTML'] = '<p>unsafe</p>';",
    "document.body.insertAdjacentHTML('beforeend', '<p>unsafe</p>');",
    "document.body['insertAdjacentHTML']('beforeend', '<p>unsafe</p>');",
    "import { html } from 'lit'; export const view = html`<div .innerHTML=${'unsafe'}></div>`;",
    "import { html } from 'lit'; export const view = html`<div .outerHTML=${'unsafe'}></div>`;",
  ]) {
    const result = fixture(source);
    assert.equal(result.status, 1, result.output || source);
    assert.match(result.output, /no-html-sinks/);
  }
});

test('only the actual SafeHtml module may import unsafe Lit directives', () => {
  for (const path of [
    'lit/directives/unsafe-html.js',
    'lit/directives/unsafe-svg.js',
    'lit-html/directives/unsafe-html.js',
  ]) {
    const result = fixture(`export * from '${path}';`, 'SafeHtml.ts');
    assert.equal(result.status, 1, result.output);
    assert.match(result.output, /no-restricted-imports/);
  }
  const safe = lint(join(root, 'src/components/SafeHtml/SafeHtml.ts'));
  assert.equal(safe.status, 0, safe.output);
});

test('safe rendering passes while explicit any and unused variables still fail', () => {
  const safe = fixture(`
    import { html } from 'lit';
    document.body.textContent = 'safe';
    export const snapshot = document.body.innerHTML;
    export const view = html\`<p>\${'<b>plain text</b>'}</p>\`;
  `);
  assert.equal(safe.status, 0, safe.output);
  for (const [source, rule] of [
    ['export const value: any = 1;', /no-explicit-any/],
    ['const unused = 1;', /no-unused-vars/],
  ]) {
    const result = fixture(source);
    assert.equal(result.status, 1, result.output);
    assert.match(result.output, rule);
  }
});
