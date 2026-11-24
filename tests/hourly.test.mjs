import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {normalize, summarize, timelineBuckets, hourlyBuckets, hourKey} from "../src/web/analytics.mjs";

const usage = {date: "2026-10-19", model: "claude-haiku-5-5", provider: "claude", device: "this-mac", requests: 1, input: 1e6, output: 1e6, cacheRead: 1e6, cacheWrite: 1e6, total: 4e6};
const priced = {...usage, cost: 1, costRequests: 1};
test("hourly aggregation preserves daily totals, empty hours and untimed legacy records", () => {
  const rows = normalize({timezone: "Asia/Seoul", rows: [], hourlyRows: [
    {...priced, hour: "2026-10-19T00:00:00+09:00"},
    {...priced, hour: "2026-10-19T11:00:00+09:00"},
    {...priced, hour: null},
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
test("demo reports are fully priced and cost aggregation is additive", () => {
  const report = JSON.parse(readFileSync(new URL("../examples/demo.json", import.meta.url)));
  const rows = normalize(report);
  assert.ok(rows.length > 0);
  assert.ok(rows.every(row => row.hour === null));
  const summary = summarize(rows, "cost", "2026-01-05", "2026-04-26");
  assert.ok(summary.total > 0);
  assert.equal(summary.costMissing, 0);
});
test("rows keep the collector cost and coverage; rows without costRequests are unpriced", () => {
  const rows = normalize({rows: [
    {...usage, requests: 4, cost: 12.5, costRequests: 3},
    {...usage, model: "gpt-6-terra", requests: 2, cost: 0, costRequests: 0},
    {...usage, model: "unreleased-model", requests: 1},
  ]});
  assert.equal(rows[0].cost, 12.5);
  assert.equal(rows[0].costMissing, 1);
  assert.equal(rows[1].cost, null);
  assert.equal(rows[1].costMissing, 2);
  assert.equal(rows[2].cost, null, "old imports carry no cost");
  assert.equal(rows[2].costMissing, 1);
  const summary = summarize(rows.slice(0, 2), "cost", usage.date, usage.date);
  assert.equal(summary.total, 12.5);
  assert.equal(summary.costMissing, 3);
  assert.equal(summary.requests, 6);
});
