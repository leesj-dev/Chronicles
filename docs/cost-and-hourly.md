# Cost and hourly timeline

Choose **Estimated cost** from the existing metric menu. Totals, model values,
daily/hourly bars, chapters, rankings and CSV export all use the same estimate.
The pricing basis and source links appear under **How these records are counted**.

Costs compare recorded tokens at the checked-in standard, short-context API
rates in `src/web/pricing.mjs`, checked on 2026-10-27. This is a comparison of API
equivalents, not subscription spending, a bill, or historical invoicing.
Input, output, cache reads and cache writes are separate token buckets.
Missing token fields, unknown models and unsupported cache categories stay
unpriced; they do not become free usage. Coverage counts requests/records,
not tokens. Partial totals retain a visible coverage note.

The catalog covers OpenAI, Claude (including Haiku 5.5), Gemini (including
4 Argon), Grok, Kimi, GLM, DeepSeek and selected Meta models. Explicit history
label mappings price Gemini 3.8 Flash N, Gemini 3 Flash A and Opus 4.6 Thinking
using the corresponding base-model rates, without renaming the records. Meta reference
rates come from Together AI and do not represent every hosting provider.
DeepSeek comparisons use peak rates. Haiku 5.5 uses its base prompt-length
tier. Fast/batch modes, long-context premiums, tax, tool calls, media and cache
storage are excluded. Argon and Gemini 3.8 Flash promotional end dates and
their published successor rates are recorded in `priceFor`; the comparison
date stays pinned to `PRICE_DATE` for reproducible reports. Update that date
and recheck the source links when refreshing the catalog.

The October 27 refresh adds GPT-6.1 Terra (there is no GPT-6 Terra mapping),
Claude Mythos 5/5.1, Kimi K3.1 Pro/Highspeed, Gemini 3.6 Flash/3.5 Flash and
GLM-5.3-FlashX-Legal. Haiku 5.5, Opus/Sonnet 5.5, Fable 5.1, Gemini 4 Argon,
Grok 4.7, GLM 5.3 and DeepSeek Flash/Pro already matched their published base
rates. OpenAI cache writes assume 30m retention; Claude and Kimi assume 5m.
Haiku 5.5's higher tier starts above 100,000 prompt tokens and is excluded
from this base-rate comparison. Daily/hourly token sums cannot establish
individual request prompt lengths or cache TTLs.

Refresh references: [OpenAI](https://developers.openai.com/api/docs/pricing),
[Claude](https://platform.claude.com/docs/en/about-claude/pricing),
[Gemini](https://ai.google.dev/gemini-api/docs/pricing),
[Gemini on Google Cloud](https://cloud.google.com/gemini-enterprise-agent-platform/generative-ai/pricing),
[Kimi](https://platform.kimi.ai/),
[GLM](https://docs.z.ai/guides/overview/pricing),
[Grok](https://docs.x.ai/developers/pricing),
[DeepSeek](https://api-docs.deepseek.com/quick_start/pricing/), and
[Together AI](https://www.together.ai/pricing).

All costs are displayed in USD. There is no currency selector or exchange-rate
conversion. CSV includes the USD estimate, currency, price snapshot date and
unpriced-record count.

Click a daily bar, or select **Hourly**, to inspect one day. Previous/next
controls stay inside the selected date range. The **Hourly** button opens today
in the report timezone, clamped to the selected date range; clicking a daily
bar opens that specific day. Changing the ranking race metric keeps the
current slider position. **Daily** restores the
daily timeline with the same filters and metric. The sidebar summarizes the
displayed day while inspecting hours. Chapters and rankings continue to use
the selected date range.

The collector preserves source timestamps in SSH event exports while keeping
the existing `usage-atlas.events.v1` schema backward compatible. `/api/usage`
retains its daily `rows` and additionally exposes `hourlyRows`, grouped by
provider, date, model, device and offset-bearing `hour`. Clients use one set
of hourly rows for both day and hour aggregation; they must not concatenate
the two arrays. Missing-token records remain in `missing`.

Hourly grouping follows the report's system timezone. Repeated daylight-saving
hours retain their UTC offsets; 23- and 25-hour days have the corresponding
number of slots. Empty hours are shown. Old imports and snapshots without
timestamps appear in an **Unknown time** bar and still contribute to the day
total. The hourly average is unavailable when time is missing. No real usage
is distributed across guessed hours. The preview demo alone adds fictional
hourly detail while preserving its original daily totals.

Standalone HTML snapshots embed the price catalog and hourly data and require
no external network requests. The native macOS implementation can follow this
data and calculation contract; its source is unchanged by this feature.
