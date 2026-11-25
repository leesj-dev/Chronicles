export function formatMoney(usd) {
  if (!Number.isFinite(usd)) return "—";
  if (usd > 0 && usd < 0.01) return "<$0.01";
  return new Intl.NumberFormat("en", {style: "currency", currency: "USD", maximumFractionDigits: 2}).format(usd);
}
// A missing known list is a legacy selection: it cannot distinguish deselected models from new ones.
export function reconcileModels(available, selected, known) {
  const seen = new Set(known || []);
  const result = new Set(selected || []);
  for (const model of available) {
    if (!seen.has(model)) result.add(model);
    seen.add(model);
  }
  return {selected: result, known: seen};
}
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
  if (metric === "requests") return countFormatter.format(value);
  const abs = Math.abs(value);
  if (abs >= 999_950_000) return (value / 1e9).toFixed(2) + "B";
  if (abs >= 999_950) return (value / 1e6).toFixed(1) + "M";
  if (abs >= 1e3) return (value / 1e3).toFixed(1) + "K";
  return countFormatter.format(value);
}
export const formatAverage = (value, metric) => metric === "requests" ? exact(value) : formatMetric(value, metric);
const AXIS_STEPS = [
  [1, 0], [1.5, 7], [2, 2], [2.5, 3], [3, 5], [4, 4], [5, 1], [6, 6], [8, 8], [10, 0], [15, 7],
];
export function rankingAxis(value, fractional = false) {
  const maxIntervals = 5;
  const minStep = fractional ? 0 : 1;
  const largest = fractional && value > 0 ? value : Math.max(1, value);
  const magnitude = 10 ** Math.floor(Math.log10(largest / maxIntervals));
  let best = null;
  for (const [multiplier, rank] of AXIS_STEPS) {
    const rawStep = Math.max(minStep, multiplier * magnitude);
    if (!fractional && Math.abs(rawStep - Math.round(rawStep)) > 1e-6) continue;
    const step = fractional ? rawStep : Math.round(rawStep);
    const intervals = Math.max(1, Math.ceil(largest / step - 1e-9));
    if (intervals > maxIntervals) continue;
    const maximum = intervals * step;
    const tooFew = intervals >= 3 ? 0 : 3 - intervals;
    if (
      !best ||
      maximum < best.maximum - 1e-9 ||
      (Math.abs(maximum - best.maximum) <= 1e-9 &&
        (tooFew < best.tooFew ||
          (tooFew === best.tooFew &&
            (rank < best.rank || (rank === best.rank && intervals < best.intervals)))))
    ) {
      best = {step, intervals, maximum, tooFew, rank};
    }
  }
  const {step, intervals, maximum} = best;
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
      /(claude-(?:opus|sonnet|haiku|fable|mythos)-\d+)-(\d{1,2})(?=$|-)/,
      "$1.$2",
    )
    .replace(/^gemini[- ](3\.[5-8])[- ]flash(?=$|[- ](?!lite\b|cyber\b)).*/, "gemini-$1-flash")
    .replace(/^gemini[- ]4[- ](hydrogen|argon|neon|carbon)(?=$|[- ](?!lite\b|cyber\b)).*/, "gemini-4-$1");
}
// Names that DeepSeek repoints over time, which DeepSeek Harness logs without a version.
// When an alias moves, append a row: earlier usage keeps the version it ran on.
// `day` is the first calendar day that is past `since` in every timezone, for rows without an hour.
// Sources: https://api-docs.deepseek.com/updates/ (2026-09-10, and the 2026-10-17 revision, whose
// "-1017" suffix DeepSeek Harness itself leaves out of the name) and
// https://github.com/sst/models.dev/blob/dev/providers/deepseek/models/deepseek-flash.toml
const MODEL_ALIASES = {
  "deepseek-flash": [{since: "2026-09-10T04:00:00Z", day: "2026-09-11", model: "deepseek-v4.1-flash"}],
};
function aliasTarget(model, {date, hour}) {
  const at = Date.parse(hour);
  return MODEL_ALIASES[model]?.findLast(row =>
    Number.isNaN(at) ? date >= row.day : at >= Date.parse(row.since))?.model ?? model;
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
      .replace(/\b(flash|pro|hydrogen|argon|neon|carbon)\b/g, word => word[0].toUpperCase() + word.slice(1));
  if (/^grok[- ]/.test(n)) return n.replace(/^grok/, "Grok").replaceAll("-", " ");
  if (/^kimi[- ]/.test(n)) return n.replace(/^kimi-k/, "Kimi K").replaceAll("-", " ").replace(/\bcode\b/, "Code");
  if (/^glm-/.test(n)) return n.toUpperCase().replace(/-FLASHX?/, word => word === "-FLASHX" ? " FlashX" : " Flash");
  if (/^deepseek-/.test(n)) return n.replace(/^deepseek-/, "DeepSeek ").replaceAll("-", " ").replace(/\b(v(?=\d)|flash\b|pro\b)/g, word => word[0].toUpperCase() + word.slice(1));
  if (n === "muse-spark-open" || n === "meta-models/muse-spark-open-816b-a42b") return "Muse Spark Open";
  if (n === "meta-llama/llama-3.3-70b-instruct-turbo") return "Llama 3.3 70B Turbo";
  for (const family of ["opus", "sonnet", "haiku", "fable", "mythos"])
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
    hue = 276;
    lightness = v >= 5.5 ? 48 : v >= 5 ? 60 : v >= 4.8 ? 72 : 84;
  } else if (n.includes("sonnet")) {
    hue = 34;
    lightness = v >= 5.5 ? 48 : v >= 5 ? 60 : v >= 4.5 ? 72 : 84;
  } else if (n.includes("haiku")) {
    hue = 78;
    lightness = v >= 5.5 ? 48 : v >= 5 ? 60 : v >= 4.5 ? 72 : v >= 3.5 ? 80 : 88;
  } else if (n.includes("gpt")) {
    hue = n.includes("terra")
      ? 54
      : n.includes("luna")
        ? 148
        : n.includes("astra")
          ? 352
          : 218;
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
    if (n.includes("carbon")) {
      hue = 14;
      lightness = 52;
    } else if (n.includes("neon") || n.includes("argon") || n.includes("pro")) {
      hue = 246;
      lightness = n.includes("neon")
        ? 48
        : n.includes("argon")
          ? 61
          : v >= 3.1
            ? 74
            : 84;
    } else {
      hue = 184;
      lightness = n.includes("hydrogen")
        ? 42
        : v >= 3.8
          ? 49
          : v >= 3.7
            ? 56
            : v >= 3.6
              ? 63
              : v >= 3.5
                ? 70
                : v >= 3.1
                  ? 77
                  : 84;
    }
  } else if (n.includes("fable")) hue = 322;
  else {
    hue = [...n].reduce((s, c) => s * 31 + c.charCodeAt(0), 0) % 360;
  }
  return `hsl(${Math.abs(hue)} 70% ${lightness}%)`;
}
export function normalize(data) {
  const rows = (data.hourlyRows || data.rows).map((r) => ({
    ...r,
    sourceModel: r.model,
    model: aliasTarget(modelId(r.model), {date: r.date, hour: r.hour || r.timestamp}),
    requests: r.requests || 0,
  }));
  return rows.filter((r) => /^\d{4}-\d{2}-\d{2}$/.test(r.date)).map(r => {
    // Rows carry the cost the collector priced per message; rows without costRequests (old imports) are unpriced.
    const served = Number.isFinite(r.costRequests);
    const cost = served && r.costRequests > 0 ? r.cost : null;
    const costMissing = served ? Math.max(0, r.requests - r.costRequests) : r.requests;
    return {...r, hour: hourKey(r.hour || r.timestamp, data.timezone || "UTC"), cost, costMissing};
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
export const harnessName = (id, names) => HARNESSES[id] || names?.[id] || id;
// Known harnesses keep their fixed order; any other harness follows alphabetically.
export function historyHarnesses(rows, names) {
  const present = new Set(rows.filter(row => row.requests > 0 || row.total > 0).map(row => row.provider));
  const known = Object.keys(HARNESSES).filter(key => present.has(key));
  const others = [...present].filter(key => !(key in HARNESSES))
    .map(key => [key, harnessName(key, names)]).sort((a, b) => a[1].localeCompare(b[1]) || a[0].localeCompare(b[0]));
  return [...known.map(key => [key, HARNESSES[key]]), ...others];
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
        costMissing: 0,
        harnesses: new Set(),
      });
    const a = map.get(key);
    a.value += r[metric] || 0;
    a.requests += r.requests;
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
    costMissing: rows.reduce((s, r) => s + (r.costMissing || 0), 0),
    requests: rows.reduce((s, r) => s + r.requests, 0),
  };
}
