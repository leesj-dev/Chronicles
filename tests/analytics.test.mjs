import test from "node:test";
import assert from "node:assert/strict";
import {
  normalize,
  formatMoney,
  reconcileModels,
  historyHarnesses,
  harnessName,
  filterRows,
  aggregate,
  percentage,
  modelColor,
  modelId,
  modelName,
  periods,
  summarize,
  calendarDate,
  rankingFrames,
  rankingAxis,
  formatMetric,
  formatAverage,
} from "../src/web/analytics.mjs";
test("new models are selected across reloads while deselected models stay off", () => {
  const first = reconcileModels(["old", "disabled"], [], []);
  first.selected.delete("disabled");
  const reload = reconcileModels(["old", "disabled", "gpt-6.2-sol"], [...first.selected], [...first.known]);
  assert.deepEqual([...reload.selected].sort(), ["gpt-6.2-sol", "old"]);
  const missing = reconcileModels(["old"], [...reload.selected], [...reload.known]);
  const returned = reconcileModels(["old", "disabled"], [...missing.selected], [...missing.known]);
  assert.equal(returned.selected.has("disabled"), false);
  assert.deepEqual([...reconcileModels(["legacy", "new"], ["legacy"], undefined).selected].sort(), ["legacy", "new"]);
});
test("ranking axes use round ticks with a ceiling that contains the leader", () => {
  assert.deepEqual(rankingAxis(33000000), {maximum: 40000000, ticks: [0, 10000000, 20000000, 30000000, 40000000]});
  assert.deepEqual(rankingAxis(27000000), {maximum: 30000000, ticks: [0, 10000000, 20000000, 30000000]});
  assert.deepEqual(rankingAxis(101000000), {maximum: 120000000, ticks: [0, 40000000, 80000000, 120000000]});
  assert.deepEqual(rankingAxis(210000000), {maximum: 240000000, ticks: [0, 60000000, 120000000, 180000000, 240000000]});
  assert.equal(rankingAxis(40000000).maximum, 40000000);
  assert.equal(rankingAxis(40000001).maximum, 45000000);
  for (const value of [0, 1, 6, 12, 19.7, 51, 31000, 990000, 1250000000]) {
    const {maximum, ticks} = rankingAxis(value);
    assert.ok(maximum >= value);
    assert.equal(ticks[0], 0);
    assert.equal(ticks.at(-1), maximum);
    assert.ok(ticks.length <= 6);
    assert.ok(ticks.every(Number.isInteger));
    assert.ok(ticks.slice(1).every((tick, index) => tick - ticks[index] === ticks[1]));
  }
});
test("requests stay unabridged and abbreviated token values keep fixed decimals (2 for B, 1 for M/K)", () => {
  assert.equal(formatMetric(11680, "requests"), "11,680");
  assert.equal(formatMetric(2600000000, "total"), "2.60B");
  assert.equal(formatMetric(2000000000, "total"), "2.00B");
  assert.equal(formatMetric(392000000, "total"), "392.0M");
  assert.equal(formatMetric(130500000, "total"), "130.5M");
  assert.equal(formatMetric(1000000, "input"), "1.0M");
  assert.equal(formatMetric(11680, "output"), "11.7K");
  assert.equal(formatMetric(0, "total"), "0");
});
test("request averages use at most one decimal while token averages keep their format", () => {
  assert.equal(formatAverage(49.03, "requests"), "49");
  assert.equal(formatAverage(49.06, "requests"), "49.1");
  assert.equal(formatAverage(1234.567, "requests"), "1,234.6");
  assert.equal(formatAverage(130500000, "total"), "130.5M");
});
test("today follows the report timezone across midnight", () => {
  const instant = new Date("2026-10-09T15:00:00Z");
  assert.equal(calendarDate("Asia/Seoul", instant), "2026-10-10");
  assert.equal(calendarDate("UTC", instant), "2026-10-09");
  assert.equal(calendarDate("America/New_York", instant), "2026-10-09");
});
test("ranking race changes rolling ranks and carries totals through empty days", () => {
  const rows = [
    {date: "2026-10-01", model: "a", requests: 10},
    {date: "2026-10-01", model: "b", requests: 2},
    {date: "2026-10-02", model: "b", requests: 20},
    {date: "2026-10-02", model: "a", requests: 1},
  ];
  const weekly = rankingFrames(rows, "2026-10-01", "2026-10-03", "requests");
  assert.deepEqual(weekly.map(frame => frame.ranking.map(row => row.model)), [["a", "b"], ["b", "a"], ["b", "a"]]);
  const cumulative = rankingFrames(rows, "2026-10-01", "2026-10-03", "requests", "cumulative");
  assert.deepEqual(cumulative[2].ranking, [{model: "b", value: 22}, {model: "a", value: 11}]);
});
test("rolling windows include earlier history and expire usage at exactly seven or thirty days", () => {
  const rows = [
    {date: "2026-09-30", model: "outside", requests: 100},
    {date: "2026-10-01", model: "a", requests: 10},
    {date: "2026-10-07", model: "b", requests: 12},
    {date: "2026-10-08", model: "a", requests: 1},
  ];
  const weekly = rankingFrames(rows, "2026-10-01", "2026-10-31", "requests", "week");
  assert.deepEqual(weekly[0].ranking, [{model: "outside", value: 100}, {model: "a", value: 10}]);
  assert.deepEqual(weekly[6].ranking, [{model: "b", value: 12}, {model: "a", value: 10}]);
  assert.deepEqual(weekly[7].ranking, [{model: "b", value: 12}, {model: "a", value: 1}]);
  assert.deepEqual(weekly[14].ranking, []);
  const monthly = rankingFrames(rows, "2026-10-01", "2026-10-31", "requests", "month");
  assert.deepEqual(monthly[29].ranking, [{model: "b", value: 12}, {model: "a", value: 11}]);
  assert.deepEqual(monthly[30].ranking, [{model: "b", value: 12}, {model: "a", value: 1}]);
  const cumulative = rankingFrames(rows, "2026-10-01", "2026-10-31", "requests", "cumulative");
  assert.deepEqual(cumulative[30].ranking, [{model: "b", value: 12}, {model: "a", value: 11}]);
});
test("rolling history precedes the replay range while cumulative starts at the selected date", () => {
  const rows = [
    {date: "2026-09-10", model: "a", total: 1000},
    {date: "2026-09-11", model: "a", total: 100},
    {date: "2026-10-03", model: "a", total: 20},
    {date: "2026-10-04", model: "a", total: 10},
    {date: "2026-10-10", model: "a", total: 1},
    {date: "2026-10-11", model: "a", total: 2},
  ];
  const weekly = rankingFrames(rows, "2026-10-10", "2026-10-11", "total", "week");
  assert.deepEqual(weekly.map(frame => frame.ranking[0].value), [11, 3]);
  const monthly = rankingFrames(rows, "2026-10-10", "2026-10-11", "total", "month");
  assert.deepEqual(monthly.map(frame => frame.ranking[0].value), [131, 33]);
  const cumulative = rankingFrames(rows, "2026-10-10", "2026-10-11", "total", "cumulative");
  assert.deepEqual(cumulative.map(frame => frame.ranking[0].value), [1, 3]);
  assert.deepEqual(monthly.map(frame => frame.date), ["2026-10-10", "2026-10-11"]);
});
test("ranking race combines harnesses and includes every model with positive usage", () => {
  const rows = Array.from({length: 16}, (_, index) => ({date: "2026-10-01", model: `model-${index}`, total: index}));
  rows.push({date: "2026-10-01", model: "model-1", total: 50});
  rows.push({date: "2026-10-01", model: "unknown"});
  const [frame] = rankingFrames(rows, "2026-10-01", "2026-10-01", "total");
  assert.equal(frame.ranking.length, 15);
  assert.deepEqual(frame.ranking[0], {model: "model-1", value: 51});
  assert.deepEqual(frame.ranking.map(row => row.model), ["model-1", ...Array.from({length: 14}, (_, index) => `model-${15 - index}`)]);
  assert.ok(frame.ranking.every(row => row.value > 0));
});
const fixture = {
  rows: [
    {
      date: "2026-01-01",
      provider: "codex",
      model: "gpt-5.4",
      requests: 2,
      total: 100,
      device: "shared",
    },
    {
      date: "2026-01-01",
      provider: "copilot",
      model: "copilot/gpt-5.4",
      requests: 1,
      total: 50,
      device: "this-mac",
    },
  ],
};
test("requests and tokens sum per row and averages span the whole range", () => {
  const r = normalize(fixture);
  assert.equal(summarize(r, "requests", "2026-01-01", "2026-01-02").total, 3);
  assert.equal(summarize(r, "total", "2026-01-01", "2026-01-02").total, 150);
  assert.equal(summarize(r, "requests", "2026-01-01", "2026-01-02").average, 1.5);
});
test("cost uses the collector's per-row cost; rows without it are unpriced", () => {
  const r = normalize({rows: [
    {date: "2026-01-01", provider: "claude", model: "m", requests: 3, total: 9, cost: 1.5, costRequests: 2},
    {date: "2026-01-01", provider: "claude", model: "old", requests: 2, total: 4},
    {date: "2026-01-01", provider: "cursor", model: "auto", requests: 1, total: 1, cost: 0, costRequests: 0},
  ]});
  assert.deepEqual(r.map(row => [row.cost, row.costMissing]), [[1.5, 1], [null, 2], [null, 1]]);
  const summary = summarize(r, "cost", "2026-01-01", "2026-01-01");
  assert.equal(summary.total, 1.5);
  assert.equal(summary.costMissing, 4);
  assert.equal(summary.requests, 6);
});
test("same model merges across harnesses and colors stay stable", () => {
  const r = aggregate(normalize(fixture), "requests");
  assert.equal(r.find((r) => r.model === "gpt-5.4").value, 3);
  assert.equal(modelColor("copilot/gpt-5.4"), modelColor("gpt-5.4"));
  assert.notEqual(modelColor("gpt-5.6-sol"), modelColor("gpt-5.6-luna"));
  assert.notEqual(modelColor("gpt-5.6-sol"), modelColor("gpt-5.6-terra"));
  assert.equal(modelId("github.copilot-chat/gpt-5.4"), "gpt-5.4");
  assert.equal(modelId("claude-opus-4-6"), modelId("copilot/claude-opus-4.6"));
});
test("Haiku naming variants merge and earlier generations have distinct lightness", () => {
  for (const [legacy, canonical] of [
    ["claude-3-haiku-20240307", "claude-haiku-3-20240307"],
    ["claude-3-5-haiku-20241022", "claude-haiku-3.5-20241022"],
    ["copilot/claude-haiku-4-5", "claude-haiku-4.5"],
  ]) {
    assert.equal(modelId(legacy), canonical);
    assert.equal(modelColor(legacy), modelColor(canonical));
  }
  assert.equal(modelName("claude-haiku-5-5"), "Haiku 5.5");
  assert.equal(modelName("claude-3-5-haiku"), "Haiku 3.5");
  const levels = [3, 3.5, 4.5, 5, 5.5].map(version =>
    Number(modelColor(`claude-haiku-${version}`).match(/ ([\d.]+)%\)$/)[1]));
  assert.ok(levels.every((value, index) => index === 0 || value < levels[index - 1]));
});
test("GPT variants and Gemini tiers use capitalized display names", () => {
  for (const variant of ["Astra", "Sol", "Terra", "Luna"])
    assert.equal(modelName(`gpt-6-${variant.toLowerCase()}`), `GPT-6 ${variant}`);
  assert.equal(modelName("gemini-3.7-flash"), "Gemini 3.7 Flash");
  assert.equal(modelName("copilot/gemini-3.1-pro-preview"), "Gemini 3.1 Pro preview");
  assert.equal(modelName("Gemini Pro"), "Gemini Pro");
  for (const tier of ["Hydrogen", "Argon", "Neon", "Carbon"])
    assert.equal(modelName(`gemini-4-${tier.toLowerCase()}`), `Gemini 4 ${tier}`);
});
test("Gemini 3.x/4 variants collapse and tier colors match lineage", () => {
  assert.equal(modelId("gemini-3.8-flash-n"), "gemini-3.8-flash");
  assert.equal(modelId("Gemini 3.8 Flash (medium)"), "gemini-3.8-flash");
  assert.equal(modelId("gemini-3.8-flash-cyber"), "gemini-3.8-flash-cyber");
  assert.equal(modelId("gemini-3.5-flash-lite"), "gemini-3.5-flash-lite");
  assert.equal(modelId("Gemini 4 Hydrogen tiered high"), "gemini-4-hydrogen");
  assert.equal(modelColor("gemini-4-hydrogen"), "hsl(184 70% 42%)");
  assert.equal(modelColor("gemini-4-argon"), "hsl(246 70% 61%)");
  assert.equal(modelColor("gemini-4-neon"), "hsl(246 70% 48%)");
  assert.equal(modelColor("gemini-4-carbon"), "hsl(14 70% 52%)");
  assert.equal(modelColor("gemini-3.1-pro"), "hsl(246 70% 74%)");
  assert.equal(modelColor("gemini-2.5-pro"), "hsl(246 70% 84%)");
  assert.equal(modelColor("gemini-2.5-flash"), modelColor("gemini-3-flash"));
});
test("harness deselection and device filters change the actual aggregate", () => {
  const rows = normalize(fixture),
    options = {
      start: "2026-01-01",
      end: "2026-01-02",
      harnesses: new Set(["copilot"]),
      models: new Set(rows.map((r) => r.model)),
    };
  assert.equal(filterRows(rows, options).length, 1);
  assert.equal(
    filterRows(rows, { ...options, harnesses: new Set() }).length,
    0,
  );
  assert.equal(filterRows(rows, { ...options, device: "macmini" }).length, 0);
  assert.equal(
    filterRows(rows, {
      ...options,
      harnesses: new Set(["codex"]),
      device: "macmini",
    }).length,
    1,
  );
});
test("periods cover the range once, clip edges and use actual dates", () => {
  for (const mode of ["eras", "weeks", "months"]) {
    const p = periods(normalize(fixture), "2026-01-01", "2026-02-03", mode);
    assert.equal(p[0].start, "2026-01-01");
    assert.equal(p.at(-1).end, "2026-02-03");
    for (let i = 1; i < p.length; i++)
      assert.equal(Date.parse(p[i].start) - Date.parse(p[i - 1].end), 86400000);
  }
});
test("small shares are readable and never claim zero usage", () => {
  assert.equal(percentage(1, 10000), "<0.1%");
  assert.equal(percentage(1, 0), "—");
});

test("harness availability comes from history, never from account snapshots", () => {
  const rows = normalize({
    rows: [{provider: "codex", model: "gpt-5.4", date: "2026-01-01", requests: 1, total: 10}],
    accountSnapshots: [{provider: "cursor", lines: [{type: "progress", used: 20}]}],
  });
  assert.deepEqual(historyHarnesses(rows).map(([key]) => key), ["codex"]);
  assert.deepEqual(historyHarnesses([]), []);
  assert.deepEqual(historyHarnesses([{provider: "cursor", requests: 0, total: 0}]), []);
});
test("harnesses the page does not know use the server's display names, then their id", () => {
  const rows = [
    {provider: "gemini", requests: 1, total: 1}, {provider: "aside", requests: 1, total: 1},
    {provider: "claude", requests: 1, total: 1}, {provider: "mystery", requests: 1, total: 1},
  ];
  assert.deepEqual(historyHarnesses(rows, {aside: "Aside", gemini: "Gemini CLI", claude: "ignored"}),
    [["claude", "Claude Code"], ["aside", "Aside"], ["gemini", "Gemini CLI"], ["mystery", "mystery"]]);
  assert.equal(harnessName("codex"), "Codex");
  assert.equal(harnessName("amp", {amp: "Amp"}), "Amp");
});
test("USD costs keep small amounts visible and unknown values unavailable", () => {
  assert.equal(formatMoney(2), "$2.00");
  assert.equal(formatMoney(null), "—");
  assert.equal(formatMoney(Infinity), "—");
  assert.equal(formatMoney(0.0001), "<$0.01");
  assert.equal(formatMoney(0), "$0.00");
});

test("the floating deepseek-flash alias resolves only for usage dated after its release", () => {
  const model = fields => normalize({timezone: "Asia/Seoul", rows: [{model: "deepseek-flash", ...fields}]})[0].model;
  // DeepSeek pointed the alias at V4.1 Flash on 2026-09-10 at 04:00 UTC (13:00 in Seoul).
  assert.equal(model({date: "2026-09-10", hour: "2026-09-10T13:00:00+09:00"}), "deepseek-v4.1-flash");
  assert.equal(model({date: "2026-09-10", hour: "2026-09-10T12:00:00+09:00"}), "deepseek-flash");
  assert.equal(model({date: "2026-11-24"}), "deepseek-v4.1-flash");
  // Without an hour the release day itself cannot be placed on either side.
  assert.equal(model({date: "2026-09-11"}), "deepseek-v4.1-flash");
  assert.equal(model({date: "2026-09-10"}), "deepseek-flash");
});
test("only the floating alias is resolved, and the recorded name is kept", () => {
  const rows = normalize({rows: ["deepseek-v4-flash", "deepseek-v4-pro", "deepseek-flash"]
    .map(model => ({date: "2026-11-24", model}))});
  assert.deepEqual(rows.map(row => row.model), ["deepseek-v4-flash", "deepseek-v4-pro", "deepseek-v4.1-flash"]);
  assert.equal(rows[2].sourceModel, "deepseek-flash");
});
test("DeepSeek names show a capitalized version only when one is known", () => {
  assert.equal(modelName("deepseek-v4.1-flash"), "DeepSeek V4.1 Flash");
  assert.equal(modelName("deepseek-v4-pro"), "DeepSeek V4 Pro");
  assert.equal(modelName("deepseek-flash"), "DeepSeek Flash");
});
