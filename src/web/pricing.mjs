// Published standard text-token rates, USD per million tokens.
// This is an API-equivalent comparison at this snapshot's rates, not a bill
// or a reconstruction of historical pricing. Never guess a model's price.
export const PRICE_DATE = "2026-10-27";
export const PRICE_SOURCES = [
  ["OpenAI", "https://developers.openai.com/api/docs/pricing"],
  ["Claude", "https://platform.claude.com/docs/en/about-claude/pricing"],
  ["Gemini", "https://ai.google.dev/gemini-api/docs/pricing"],
  ["Gemini on Google Cloud", "https://cloud.google.com/gemini-enterprise-agent-platform/generative-ai/pricing"],
  ["Grok", "https://docs.x.ai/developers/pricing"],
  ["Kimi", "https://platform.kimi.ai/"],
  ["GLM", "https://docs.z.ai/guides/overview/pricing"],
  ["DeepSeek", "https://api-docs.deepseek.com/quick_start/pricing/"],
  ["Meta via Together AI", "https://www.together.ai/pricing"],
];
// [uncached input, output, cache read, cache write]. Claude/Kimi writes assume
// 5m retention; OpenAI writes assume 30m. All rates use the base prompt tier.
export const TOKEN_PRICES = {
  "gpt-6.1-sol": [2, 10, 0.1, 2.5],
  "gpt-6.1-terra": [1, 5, 0.05, 1.25],
  "gpt-6-sol": [2, 10, 0.2, 2.5],
  "gpt-6-astra": [10, 50, 1, 12.5],
  "gpt-6-luna": [0.1, 0.5, 0.01, 0.125],
  "gpt-5.6-sol": [4, 20, 0.4, 5],
  "gpt-5.6-terra": [2, 12, 0.2, 2.5],
  "gpt-5.6-luna": [0.2, 1.2, 0.02, 0.25],
  // Legacy model pages: https://developers.openai.com/api/docs/models/<id>
  "gpt-5.5": [5, 30, 0.5, null],
  "gpt-5.4": [2.5, 15, 0.25, null],
  "gpt-5.3-codex": [1.75, 14, 0.175, null],
  "gpt-5.2-codex": [1.75, 14, 0.175, null],
  "gpt-5.2": [1.75, 14, 0.175, null],
  "claude-opus-5.5": [4, 20, 0.2, 5],
  "claude-sonnet-5.5": [2, 10, 0.1, 2.5],
  "claude-opus-5": [5, 25, 0.5, 6.25],
  "claude-fable-5": [10, 50, 1, 12.5],
  "claude-fable-5.1": [10, 50, 0.25, 12.5],
  "claude-mythos-5": [10, 50, 1, 12.5],
  "claude-mythos-5.1": [10, 50, 0.25, 12.5],
  "claude-opus-4.8": [5, 25, 0.5, 6.25],
  "claude-opus-4.7": [5, 25, 0.5, 6.25],
  "claude-opus-4.6": [5, 25, 0.5, 6.25],
  "claude-opus-4.5": [5, 25, 0.5, 6.25],
  "claude-sonnet-5": [2, 10, 0.2, 2.5],
  "claude-sonnet-4.6": [3, 15, 0.3, 3.75],
  "claude-sonnet-4.5": [3, 15, 0.3, 3.75],
  "claude-haiku-4.5": [1, 5, 0.1, 1.25],
  "claude-haiku-5.5": [0.1, 0.5, 0.01, 0.125],
  "gemini-4-argon": [2, 10, 0.1, null],
  "gemini-3.8-flash": [0.75, 3.75, 0.075, null],
  "gemini-3.7-flash": [0.75, 3.75, 0.075, null],
  "gemini-3.6-flash": [0.75, 3.75, 0.075, null],
  "gemini-3.5-flash": [1.5, 9, 0.15, null],
  "gemini-3-flash-preview": [0.5, 3, 0.05, null],
  "gemini-3.5-flash-lite": [0.3, 2.5, 0.03, null],
  "gemini-3.1-flash-lite": [0.25, 1.5, 0.025, null],
  "gemini-3.1-pro-preview": [2, 12, 0.2, null],
  "grok-4.7": [2, 6, 0.5, null],
  "grok-4.6": [2, 6, 0.5, null],
  "grok-4.5": [2, 6, 0.3, null],
  "grok-4.3": [1.25, 2.5, 0.2, null],
  "grok-build-0.1": [1, 2, 0.2, null],
  "kimi-k3": [3, 15, 0.3, 3],
  "kimi-k3.1-pro": [2, 10, 0.2, 2],
  "kimi-k3.1-pro-highspeed": [4, 20, 0.2, 4],
  "kimi-k2.7-code": [0.95, 4, 0.19, null],
  "kimi-k2.6": [0.95, 4, 0.16, null],
  "glm-5.3": [1.4, 4.4, 0.26, null],
  "glm-5.3-flash": [0.15, 0.5, 0.03, null],
  "glm-5.3-flashx": [0.37, 1.25, 0.075, null],
  "glm-5.3-flashx-legal": [0.37, 1.25, 0.075, null],
  "glm-5.2": [1.4, 4.4, 0.26, null],
  "glm-5.1": [1.4, 4.4, 0.26, null],
  "glm-5": [1, 3.2, 0.2, null],
  "glm-4.7": [0.6, 2.2, 0.11, null],
  "glm-4.6": [0.6, 2.2, 0.11, null],
  "glm-4.7-flash": [0, 0, 0, null],
  "glm-4.5-flash": [0, 0, 0, null],
  // Peak-rate comparison, deliberately not an off-peak billing reconstruction.
  "deepseek-flash": [0.3, 1.2, 0.006, null],
  "deepseek-v4-pro": [1.32, 3.96, 0.044, null],
  // Explicit Together reference prices, not universal prices for Meta models.
  "meta-models/muse-spark-open-816b-a42b": [1.25, 4.25, 0.15, null],
  "meta-llama/llama-3.3-70b-instruct-turbo": [1.04, 1.04, null, null],
};
export const PRICE_ALIASES = {
  "muse-spark-open": "meta-models/muse-spark-open-816b-a42b",
  "gemini-3.1-pro": "gemini-3.1-pro-preview",
  "gemini-3.1-pro-preview-customtools": "gemini-3.1-pro-preview",
  // Explicit history-label mappings for API-equivalent estimates. Keep the
  // original row model intact; these labels are not separate published SKUs.
  "gemini-3.8-flash-n": "gemini-3.8-flash",
  "gemini-3-flash-a": "gemini-3-flash-preview",
  "gemini-3-flash": "gemini-3-flash-preview",
  "claude-opus-4.6-thinking": "claude-opus-4.6",
  "deepseek-v4-flash": "deepseek-flash",
  "deepseek-v4-flash-vision-exp": "deepseek-flash",
  "deepseek-v4.1-flash-1017": "deepseek-flash",
  "deepseek-v4-pro-0813": "deepseek-v4-pro",
};
// Scheduled changes verified alongside the snapshot. All history is compared
// at the chosen report price date; this is intentionally not historical billing.
export function priceFor(model, asOf = PRICE_DATE) {
  let name = model.toLowerCase().replace(/^(openai|anthropic|google|x-ai|moonshotai|z-ai|deepseek)\//, "").replace(/-(?:\d{8}|\d{4}-\d{2}-\d{2})$/, "");
  name = name.replace(/(claude-(?:opus|sonnet|haiku|fable|mythos)-\d+)-(\d{1,2})(?=$|-)/, "$1.$2");
  name = Object.hasOwn(PRICE_ALIASES, name) ? PRICE_ALIASES[name] : name;
  if (name === "gemini-4-argon" && asOf >= "2026-12-01") return [4, 20, 0.2, null];
  if (["gemini-3.8-flash", "gemini-3.7-flash", "gemini-3.6-flash"].includes(name) && asOf >= "2027-01-01") return [1.5, 7.5, 0.15, null];
  return Object.hasOwn(TOKEN_PRICES, name) ? TOKEN_PRICES[name] : null;
}
export function estimateCost(row) {
  const rates = priceFor(row.model);
  if (!rates || row.tokenRecorded === false) return null;
  let cost = 0, tokens = 0;
  for (const [index, field] of ["input", "output", "cacheRead", "cacheWrite"].entries()) {
    const value = row[field];
    if (!Number.isFinite(value) || value < 0 || (value > 0 && rates[index] === null)) return null;
    tokens += value;
    cost += value * (rates[index] || 0) / 1e6;
  }
  if (!tokens && row.total > 0) return null;
  return cost;
}
export function formatMoney(usd) {
  if (!Number.isFinite(usd)) return "—";
  if (usd > 0 && usd < 0.01) return "<$0.01";
  return new Intl.NumberFormat("en", {style: "currency", currency: "USD", maximumFractionDigits: 2}).format(usd);
}
