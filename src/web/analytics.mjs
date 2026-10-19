import {estimateCost, formatMoney, PRICE_DATE, PRICE_SOURCES} from "./pricing.mjs";
export {formatMoney, PRICE_DATE, PRICE_SOURCES};
export const HARNESSES = {
  codex: "Codex",
  claude: "Claude Code",
  copilot: "Copilot",
  antigravity: "Antigravity",
  cursor: "Cursor",
  devin: "Devin",
  grok: "Grok",
  ollama: "Ollama",
  opencode: "OpenCode",
  openrouter: "OpenRouter",
  zai: "Z.ai",
};
export const METRICS = {
  requests: "Requests / calls",
  total: "Total tokens",
  input: "Input tokens",
  output: "Output tokens",
  cacheRead: "Cache read tokens",
  cacheWrite: "Cache write tokens",
  rounds: "Copilot response rounds",
  cost: "Estimated cost",
};
export const addDays = (d, n) =>
  new Date(Date.parse(d + "T00:00:00Z") + n * 86400000)
    .toISOString()
    .slice(0, 10);
export function calendarDate(timeZone, date = new Date()) {
  const parts = Object.fromEntries(
    new Intl.DateTimeFormat("en", {
      timeZone,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    }).formatToParts(date).map(({ type, value }) => [type, value]),
  );
  return `${parts.year}-${parts.month}-${parts.day}`;
}
export const daysBetween = (a, b) =>
  Math.round((Date.parse(b) - Date.parse(a)) / 86400000) + 1;
export const percentage = (n, total) =>
  !total
    ? "—"
    : (n / total) * 100 < 0.1
      ? "<0.1%"
      : ((n / total) * 100).toFixed(1) + "%";
const compactFormatter = new Intl.NumberFormat("en", {notation: "compact", maximumFractionDigits: 2});
const exactFormatter = new Intl.NumberFormat("en", {maximumFractionDigits: 1});
export const compact = (n) => compactFormatter.format(n);
export const exact = (n) => exactFormatter.format(n);
const countFormatter = new Intl.NumberFormat("en", {maximumFractionDigits: 2});
export function formatMetric(value, metric) {
  if (["requests", "rounds"].includes(metric)) return countFormatter.format(value);
  for (const [size, unit] of [[1e9, "B"], [1e6, "M"], [1e3, "K"]])
    if (Math.abs(value) >= size) return (value / size).toFixed(2) + unit;
  return countFormatter.format(value);
}
export const formatAverage = (value, metric) => metric === "requests" ? exact(value) : formatMetric(value, metric);
export function rankingAxis(value, fractional = false) {
  if (fractional && value > 0 && value < 5) {
    const unit = 10 ** Math.floor(Math.log10(value / 5));
    const step = [1, 2, 5, 10].find(size => size * unit >= value / 5) * unit;
    const intervals = Math.ceil(value / step);
    return {maximum: intervals * step, ticks: Array.from({length: intervals + 1}, (_, i) => i * step)};
  }
  const largest = Math.max(1, value),
    unit = 10 ** Math.floor(Math.log10(largest / 5)),
    step = Math.max(1, [1, 2, 5, 10].find(size => size * unit >= largest / 5) * unit),
    intervals = Math.ceil(largest / step),
    maximum = intervals * step;
  return {maximum, ticks: Array.from({length: intervals + 1}, (_, index) => index * step)};
}
export function modelId(name) {
  return name
    .replace(/^copilot\//, "")
    .replace(/^github\.copilot-chat\//, "")
    .toLowerCase()
    .replace(/^claude-(\d+)(?:[.-](\d+))?-haiku(?=$|-)/,
      (_, major, minor) => `claude-haiku-${major}${minor ? "." + minor : ""}`)
    .replace(
      /(claude-(?:opus|sonnet|haiku|fable)-\d+)-(\d{1,2})(?=$|-)/,
      "$1.$2",
    );
}
export function modelName(name) {
  let n = modelId(name).replace(/^claude-/, "");
  if (n.startsWith("gpt-"))
    return n
      .replace("gpt-", "GPT-")
      .replaceAll("-", " ")
      .replace("GPT ", "GPT-")
      .replace(/\b(astra|sol|terra|luna)\b/g, word => word[0].toUpperCase() + word.slice(1));
  if (/^gemini[- ]/.test(n))
    return n.replace(/^gemini[- ]/, "Gemini ").replaceAll("-", " ")
      .replace(/\b(flash|pro|argon)\b/g, word => word[0].toUpperCase() + word.slice(1));
  if (/^grok[- ]/.test(n)) return n.replace(/^grok/, "Grok").replaceAll("-", " ");
  if (/^kimi[- ]/.test(n)) return n.replace(/^kimi-k/, "Kimi K").replaceAll("-", " ").replace(/\bcode\b/, "Code");
  if (/^glm-/.test(n)) return n.toUpperCase().replace(/-FLASHX?/, word => word === "-FLASHX" ? " FlashX" : " Flash");
  if (/^deepseek-/.test(n)) return n.replace(/^deepseek-/, "DeepSeek ").replaceAll("-", " ").replace(/\b(flash|pro)\b/g, word => word[0].toUpperCase() + word.slice(1));
  if (n === "muse-spark-open" || n === "meta-models/muse-spark-open-816b-a42b") return "Muse Spark Open";
  if (n === "meta-llama/llama-3.3-70b-instruct-turbo") return "Llama 3.3 70B Turbo";
  for (const family of ["opus", "sonnet", "haiku", "fable"])
    if (n.startsWith(family + "-"))
      return n
        .replace(family + "-", family[0].toUpperCase() + family.slice(1) + " ")
        .replace(/(\d+)-(\d+)(?=$|-)/, "$1.$2")
        .replaceAll("-", " ");
  return name;
}
export function modelColor(name) {
  const n = modelId(name),
    v = Number(
      (
        n.match(
          /(?:gpt|claude-opus|claude-sonnet|claude-haiku|opus|sonnet|haiku|gemini)[- ](\d+(?:[.-]\d+)?)/,
        )?.[1] || "0"
      ).replace("-", "."),
    );
  let hue = 220,
    lightness = 55;
  if (n.includes("opus")) {
    hue = 275;
    lightness = v >= 5.5 ? 48 : v >= 5 ? 60 : v >= 4.8 ? 72 : 84;
  } else if (n.includes("sonnet")) {
    hue = 26;
    lightness = v >= 5.5 ? 48 : v >= 5 ? 60 : v >= 4.5 ? 72 : 84;
  } else if (n.includes("haiku")) {
    hue = 95;
    lightness = v >= 5.5 ? 48 : v >= 5 ? 60 : v >= 4.5 ? 72 : v >= 3.5 ? 80 : 88;
  } else if (n.includes("gpt")) {
    hue = n.includes("terra")
      ? 45
      : n.includes("luna")
        ? 145
        : n.includes("astra")
          ? 355
          : 215;
    lightness =
      v >= 6.1
        ? 43
        : v >= 6
          ? 51
          : v >= 5.6
            ? 59
            : v >= 5.5
              ? 67
              : v >= 5.4
                ? 75
                : 83;
  } else if (n.includes("gemini")) {
    hue = 180;
    lightness = v >= 3.8 ? 48 : v >= 3.7 ? 60 : v >= 3.1 ? 72 : 84;
  } else if (n.includes("fable")) hue = 325;
  else {
    hue = [...n].reduce((s, c) => s * 31 + c.charCodeAt(0), 0) % 360;
  }
  return `hsl(${Math.abs(hue)} 70% ${lightness}%)`;
}
export function normalize(data) {
  const rows = (data.hourlyRows || data.rows).map((r) => ({
    ...r,
    model: modelId(r.model),
    requests: r.requests || 0,
    rounds: r.rounds || 0,
    tokenRecorded: true,
    missing: 0,
    roundMissing:
      r.provider === "copilot"
        ? Math.max(0, (r.requests || 0) - (r.roundRecords || 0))
        : 0,
  }));
  for (const r of data.missing || [])
    rows.push({
      ...r,
      model: modelId(r.model),
      requests: 1,
      rounds: r.rounds || 0,
      total: 0,
      input: 0,
      output: 0,
      cacheRead: 0,
      cacheWrite: 0,
      tokenRecorded: false,
      missing: 1,
      roundMissing: r.rounds ? 0 : 1,
    });
  return rows.filter((r) => /^\d{4}-\d{2}-\d{2}$/.test(r.date)).map(r => {
    const cost = estimateCost(r);
    return {...r, hour: hourKey(r.hour || r.timestamp, data.timezone || "UTC"), cost, costMissing: cost === null ? r.requests : 0};
  });
}
const hourFormatters = new Map();
export function hourKey(instant, timeZone) {
  if (!instant || !/(?:Z|[+-]\d\d:\d\d)$/.test(instant)) return null;
  const date = new Date(instant);
  if (!Number.isFinite(date.getTime())) return null;
  if (!hourFormatters.has(timeZone)) hourFormatters.set(timeZone, new Intl.DateTimeFormat("en", {
    timeZone, year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", hourCycle: "h23", timeZoneName: "longOffset",
  }));
  const p = Object.fromEntries(hourFormatters.get(timeZone).formatToParts(date).map(p => [p.type, p.value]));
  const offset = p.timeZoneName === "GMT" ? "+00:00" : p.timeZoneName.replace("GMT", "");
  return `${p.year}-${p.month}-${p.day}T${p.hour}:00:00${offset}`;
}
const bucketCache = new Map();
export function hourlyBuckets(date, timeZone) {
  const cacheKey = `${date}/${timeZone}`;
  if (!bucketCache.has(cacheKey)) {
    const start = Date.parse(date + "T00:00:00Z"), keys = new Set();
    // Includes half/quarter-hour offsets and repeated daylight-saving hours.
    for (let t = start - 18 * 3600000; t < start + 42 * 3600000; t += 15 * 60000) {
      const key = hourKey(new Date(t).toISOString(), timeZone);
      if (key.slice(0, 10) === date) keys.add(key);
    }
    bucketCache.set(cacheKey, [...keys].sort((a,b) => Date.parse(a) - Date.parse(b)));
  }
  return bucketCache.get(cacheKey);
}
export function timelineBuckets(rows, metric, start, end, timeZone, hourly = false) {
  const map = new Map();
  if (hourly) for (const key of hourlyBuckets(start, timeZone)) map.set(key, new Map());
  else for (let d = start; d <= end; d = addDays(d, 1)) map.set(d, new Map());
  for (const row of rows) {
    if (row.date < start || row.date > end) continue;
    const key = hourly ? row.hour || "unknown" : row.date;
    if (!map.has(key)) map.set(key, new Map());
    const counts = map.get(key);
    counts.set(row.model, (counts.get(row.model) || 0) + (row[metric] || 0));
  }
  return map;
}
// Availability reflects loaded history, independent of chart filters and quotas.
export function historyHarnesses(rows) {
  const present = new Set(rows.filter(row => row.requests > 0 || row.total > 0).map(row => row.provider));
  return Object.entries(HARNESSES).filter(([key]) => present.has(key));
}
export function filterRows(
  rows,
  { start, end, harnesses, models, device = "all", excludeReview = true },
) {
  return rows.filter(
    (r) =>
      r.date >= start &&
      r.date <= end &&
      harnesses.has(r.provider) &&
      models.has(r.model) &&
      (device === "all" || r.device === device || r.device === "shared") &&
      (!excludeReview || r.model !== "codex-auto-review"),
  );
}
export function aggregate(rows, metric) {
  const map = new Map();
  for (const r of rows) {
    const key = r.model;
    if (!map.has(key))
      map.set(key, {
        model: key,
        value: 0,
        requests: 0,
        missing: 0,
        costMissing: 0,
        harnesses: new Set(),
      });
    const a = map.get(key);
    a.value += r[metric] || 0;
    a.requests += r.requests;
    a.missing += r.missing;
    a.costMissing += r.costMissing || 0;
    a.harnesses.add(r.provider);
  }
  return [...map.values()].sort(
    (a, b) => b.value - a.value || a.model.localeCompare(b.model),
  );
}
export function rankingFrames(rows, start, end, metric, mode = "week") {
  const windowDays = mode === "month" ? 30 : 7;
  const historyStart = mode === "cumulative" ? start : addDays(start, 1 - windowDays);
  const byDay = new Map();
  for (const row of rows) {
    if (row.date < historyStart || row.date > end) continue;
    if (!byDay.has(row.date)) byDay.set(row.date, new Map());
    const values = byDay.get(row.date);
    values.set(row.model, (values.get(row.model) || 0) + (row[metric] || 0));
  }
  const frames = [];
  const values = new Map();
  for (const [date, history] of byDay)
    if (date < start)
      for (const [model, value] of history)
        values.set(model, (values.get(model) || 0) + value);
  for (let date = start; date <= end; date = addDays(date, 1)) {
    for (const [model, value] of byDay.get(date) || [])
      values.set(model, (values.get(model) || 0) + value);
    if (mode !== "cumulative")
      for (const [model, value] of byDay.get(addDays(date, -windowDays)) || []) {
        const remaining = (values.get(model) || 0) - value;
        if (remaining > 0) values.set(model, remaining);
        else values.delete(model);
      }
    const ranking = [...values].filter(([, value]) => value > 0)
      .map(([model, value]) => ({model, value}))
      .sort((a, b) => b.value - a.value || a.model.localeCompare(b.model));
    frames.push({date, ranking});
  }
  return frames;
}
export function periods(rows, start, end, mode = "eras") {
  let cuts = [start];
  if (mode === "eras") {
    const first = new Map(),
      totals = new Map();
    for (const r of rows) {
      first.set(
        r.model,
        first.has(r.model)
          ? first.get(r.model) < r.date
            ? first.get(r.model)
            : r.date
          : r.date,
      );
      totals.set(r.model, (totals.get(r.model) || 0) + r.requests);
    }
    const overall = rows.reduce((s, r) => s + r.requests, 0);
    const candidates = [...first]
      .filter(([m]) => totals.get(m) >= Math.max(3, overall * 0.005))
      .map(([, d]) => d)
      .sort();
    for (const d of candidates)
      if (d > cuts.at(-1) && daysBetween(cuts.at(-1), d) >= 8) cuts.push(d);
  } else if (mode === "weeks") {
    for (let d = addDays(start, 7); d <= end; d = addDays(d, 7)) cuts.push(d);
  } else if (mode === "months") {
    let d = start.slice(0, 7) + "-01";
    for (
      let next = addDays(d, 32).slice(0, 7) + "-01";
      next <= end;
      next = addDays(next, 32).slice(0, 7) + "-01"
    )
      cuts.push(next);
  }
  return cuts.map((d, i) => ({
    start: d,
    end: i + 1 < cuts.length ? addDays(cuts[i + 1], -1) : end,
  }));
}
export function summarize(rows, metric, start, end) {
  const models = aggregate(rows, metric),
    total = models.reduce((s, r) => s + r.value, 0);
  return {
    models,
    total,
    average: total / daysBetween(start, end),
    missing: rows.reduce((s, r) => s + r.missing, 0),
    roundMissing: rows.reduce((s, r) => s + r.roundMissing, 0),
    costMissing: rows.reduce((s, r) => s + (r.costMissing || 0), 0),
    requests: rows.reduce((s, r) => s + r.requests, 0),
  };
}
