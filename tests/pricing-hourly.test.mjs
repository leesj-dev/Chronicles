import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {normalize, summarize, timelineBuckets, hourlyBuckets, hourKey, modelName} from "../src/web/analytics.mjs";
import {estimateCost, priceFor, formatMoney} from "../src/web/pricing.mjs";

const usage = {date: "2026-10-19", model: "claude-haiku-5-5", provider: "claude", device: "this-mac", requests: 1, input: 1e6, output: 1e6, cacheRead: 1e6, cacheWrite: 1e6, total: 4e6};
test("cost keeps cache buckets disjoint, merges Haiku IDs and preserves unknown prices", () => {
  const rows = normalize({rows: [usage, {...usage, model: "unreleased-model"}], missing: [{date: usage.date, model: usage.model, provider: "copilot"}]});
  assert.equal(rows[0].model, "claude-haiku-5.5");
  assert.ok(Math.abs(rows[0].cost - 0.735) < 1e-12);
  assert.equal(rows[1].cost, null);
  assert.equal(rows[2].cost, null);
  const summary = summarize(rows, "cost", usage.date, usage.date);
  assert.equal(summary.costMissing, 2);
  assert.equal(summary.requests, 3);
  assert.equal(estimateCost({...usage, model: "gpt-5.4", cacheWrite: 1}), null);
  assert.equal(estimateCost({...usage, model: "gpt-5.4", output: undefined, cacheWrite: 0}), null);
  assert.equal(priceFor("unknown/gpt-5.4"), null);
  assert.deepEqual(priceFor("anthropic/claude-haiku-5-5"), priceFor("claude-haiku-5.5"));
});
test("requested families have explicit rates and stable display labels", () => {
  for (const model of ["claude-haiku-5.5", "gemini-4-argon", "grok-4.7", "kimi-k3", "glm-5.3", "deepseek-v4-pro", "meta-models/muse-spark-open-816b-a42b"])
    assert.equal(priceFor(model).length, 4, model);
  assert.equal(modelName("gemini-4-argon"), "Gemini 4 Argon");
  assert.equal(modelName("kimi-k3"), "Kimi K3");
  assert.equal(modelName("glm-5.3"), "GLM-5.3");
  assert.deepEqual(priceFor("gemini-4-argon", "2026-11-30"), [2,10,0.1,null]);
  assert.deepEqual(priceFor("gemini-4-argon", "2026-12-01"), [4,20,0.2,null]);
  assert.deepEqual(priceFor("gemini-3.8-flash", "2027-01-01"), [1.5,7.5,0.15,null]);
});
test("USD costs keep small amounts visible and unknown values unavailable", () => {
  assert.equal(formatMoney(2), "$2.00");
  assert.equal(formatMoney(null), "—");
  assert.equal(formatMoney(Infinity), "—");
  assert.equal(formatMoney(0.0001), "<$0.01");
  assert.equal(formatMoney(0), "$0.00");
});
test("Pro Preview, Codex 5.2 and Fable versions use their published rates", () => {
  assert.deepEqual(priceFor("gemini-3.1-pro-preview"), [2,12,0.2,null]);
  assert.deepEqual(priceFor("gemini-3.1-pro-preview-customtools"), priceFor("gemini-3.1-pro-preview"));
  assert.deepEqual(priceFor("gpt-5.2-codex"), [1.75,14,0.175,null]);
  assert.deepEqual(priceFor("claude-fable-5"), [10,50,1,12.5]);
  assert.deepEqual(priceFor("claude-fable-5-1"), [10,50,0.25,12.5]);
  for (const model of ["gemini-3.1-pro-preview", "gpt-5.2-codex", "claude-fable-5", "claude-fable-5-1", "claude-opus-4-6-thinking"]) {
    const row = normalize({rows:[{...usage,model,cacheWrite:0}]})[0];
    assert.notEqual(row.cost, null, model);
    assert.equal(row.costMissing, 0, model);
  }
  const values = {...usage,model:"claude-opus-4-6-thinking"};
  assert.equal(normalize({rows:[values]})[0].cost, normalize({rows:[{...values,model:"claude-opus-4.6"}]})[0].cost);
  assert.equal(estimateCost({...usage,model:"claude-fable-5.1"}), 72.75);
  assert.equal(estimateCost({...usage,model:"claude-fable-5"}), 73.5);
});
test("history variants use explicit base-model estimates without changing model identity", () => {
  for (const [variant, base] of [
    ["gemini-3.8-flash-n", "gemini-3.8-flash"],
    ["gemini-3-flash-a", "gemini-3-flash-preview"],
    ["claude-opus-4.6-thinking", "claude-opus-4.6"],
  ]) {
    const row = normalize({rows:[{...usage, model:variant}]})[0];
    assert.equal(row.model, variant);
    // Gemini has no published cache-write token category.
    const recorded = {...row, cacheWrite:0};
    assert.equal(estimateCost(recorded), estimateCost({...recorded,model:base}));
    assert.notEqual(estimateCost(recorded), null);
    assert.deepEqual(priceFor(variant), priceFor(base));
  }
  assert.deepEqual(priceFor("gemini-3.7-flash"), [0.75,3.75,0.075,null]);
  assert.deepEqual(priceFor("gemini-3.8-flash-n", "2027-01-01"), [1.5,7.5,0.15,null]);
  assert.deepEqual(priceFor("gemini-3.7-flash", "2027-01-01"), [1.5,7.5,0.15,null]);
  assert.notEqual(priceFor("claude-fable-5"), null);
  assert.equal(priceFor("gemini-3.8-flash-made-up"), null);
});
test("hourly aggregation preserves daily totals, empty hours and untimed legacy records", () => {
  const rows = normalize({timezone: "Asia/Seoul", rows: [], hourlyRows: [
    {...usage, hour: "2026-10-19T00:00:00+09:00"},
    {...usage, hour: "2026-10-19T11:00:00+09:00"},
    {...usage, hour: null},
  ]});
  const daily = timelineBuckets(rows, "cost", usage.date, usage.date, "Asia/Seoul");
  const hourly = timelineBuckets(rows, "cost", usage.date, usage.date, "Asia/Seoul", true);
  assert.equal(hourly.size, 25);
  assert.equal(hourly.get("unknown").get("claude-haiku-5.5"), rows[2].cost);
  const total = buckets => [...buckets.values()].reduce((sum, counts) => sum + [...counts.values()].reduce((a,b) => a+b, 0), 0);
  assert.ok(Math.abs(total(daily) - total(hourly)) < 1e-12);
  assert.equal(normalize({rows:[usage]})[0].hour, null);
  assert.equal(hourKey("2026-10-18T15:00:00Z", "Asia/Seoul"), "2026-10-19T00:00:00+09:00");
  assert.equal(hourKey("2026-10-19", "Asia/Seoul"), null);
});
test("daylight saving creates 23 or 25 buckets and repeated hours stay separate", () => {
  assert.equal(hourlyBuckets("2026-03-08", "America/New_York").length, 23);
  const fall = hourlyBuckets("2026-11-01", "America/New_York");
  assert.equal(fall.length, 25);
  assert.equal(fall.filter(key => key.includes("T01:00")).length, 2);
  assert.notEqual(hourKey("2026-11-01T05:30:00Z", "America/New_York"), hourKey("2026-11-01T06:30:00Z", "America/New_York"));
  assert.equal(hourlyBuckets("2026-10-19", "Asia/Kathmandu")[0], "2026-10-19T00:00:00+05:45");
});
test("old demo reports remain readable and cost aggregation is additive", () => {
  const report = JSON.parse(readFileSync(new URL("../examples/demo.json", import.meta.url)));
  const rows = normalize(report);
  assert.ok(rows.length > 0);
  assert.ok(rows.every(row => row.hour === null));
  const summary = summarize(rows, "cost", "2026-01-05", "2026-04-26");
  assert.ok(summary.total > 0);
  assert.ok(summary.costMissing > 0);
});
