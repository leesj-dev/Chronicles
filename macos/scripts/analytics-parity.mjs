import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFileSync, mkdtempSync, writeFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {fileURLToPath} from 'node:url';
import {resolve} from 'node:path';
import {normalize, aggregate, summarize, addDays} from '../../src/web/analytics.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const binary = resolve(root, 'macos/.build/debug/chronicles-inspect');
const directory = mkdtempSync(resolve(tmpdir(), 'chronicles-parity-'));
const reports = [
  JSON.parse(readFileSync(resolve(root, 'examples/demo.json'), 'utf8')),
  {rows: [
    {date: '2026-01-01', provider: 'codex', model: 'gpt-5.4', requests: 2, total: 100, device: 'shared'},
    {date: '2026-01-01', provider: 'copilot', model: 'copilot/gpt-5.4', requests: 1, total: 50, rounds: 3, device: 'this-mac'},
  ]},
];
let checks = 0;
try {
  for (let i = 0; i < reports.length; i++) {
    const path = resolve(directory, `report-${i}.json`);
    writeFileSync(path, JSON.stringify(reports[i]));
    const actual = JSON.parse(execFileSync(binary, ['--report', path, '--analytics'], {encoding: 'utf8'}));
    const rows = normalize(reports[i]);
    const dates = rows.map(row => row.date).sort(), start = dates[0], end = dates.at(-1);
    for (const metric of ['requests', 'total', 'input', 'output', 'cacheRead', 'cacheWrite']) {
      const totals = aggregate(rows, metric).map(({model, value}) => ({model, value}));
      const daily = [];
      for (let day = start; day <= end; day = addDays(day, 1)) {
        const values = new Map(aggregate(rows.filter(row => row.date === day), metric).map(row => [row.model, row.value]));
        for (const {model} of totals) daily.push({day, model, value: values.get(model) || 0});
      }
      assert.deepEqual(actual[metric].totals, totals, `${i}: ${metric} totals`);
      assert.deepEqual(actual[metric].daily, daily, `${i}: ${metric} daily`);
      assert.equal(actual[metric].average, summarize(rows, metric, start, end).average, `${i}: ${metric} average`);
      checks += 3;
    }
  }
} finally { rmSync(directory, {recursive: true, force: true}); }
console.log(`Native/web analytics parity: ${checks} comparisons passed (demo + shared fixture).`);
