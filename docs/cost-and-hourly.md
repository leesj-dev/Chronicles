# Cost and hourly timeline

Choose **Estimated cost** from the existing metric menu. Totals, model values,
daily/hourly bars, chapters, rankings and CSV export all use the same cost.
The basis is described under **How these records are counted**.

Each message is priced individually by the [engine](../engine/README.md), which
uses tokscale-core: long-context tiers, 1-hour cache writes, reasoning tokens and
service tiers, at current rates. tokscale-core resolves rates from LiteLLM,
OpenRouter and models.dev and caches them in `TOKSCALE_CONFIG_DIR`
(`.local/engine`); `chronicles serve --offline` prices from that cache
without fetching. When a source records a cost, tokscale-core uses that amount
and marks it as reported (`CostSource::ProviderReported`).
The engine then applies the published rates tokscale-core does not know, listed with
their sources in [`engine/src/rules.rs`](../engine/src/rules.rs).
The comparison shows API-equivalent costs, and estimated costs for older
records use current rates.

The server sums the costs per row and exposes `cost` and `costRequests` (the
number of priced requests) on every row and hourly row; messages without a price
count as unpriced. The page uses them as they are and never estimates: a row
without `costRequests` is entirely unpriced. That is the case for
`.local/imports/macmini.json` files written before sync recorded costs; they
still load, and running `chronicles sync` again fills in their costs.
Coverage counts requests/records, not tokens, and partial totals keep a visible
coverage note. The fictional demo carries invented costs so the metric works there.

All costs are displayed in USD. There is no currency selector or exchange-rate
conversion. CSV includes the USD estimate, currency and unpriced-record count.

Click a daily bar, or select **Hourly**, to inspect one day. Previous/next
controls stay inside the selected date range. The **Hourly** button opens today
in the report timezone, clamped to the selected date range; clicking a daily
bar opens that specific day. Changing the ranking race metric keeps the
current slider position. **Daily** restores the
daily timeline with the same filters and metric. The sidebar summarizes the
displayed day while inspecting hours. Chapters and rankings continue to use
the selected date range.

The collector preserves source timestamps in SSH event exports. They keep the
`usage-atlas.events.v1` schema, with `cost`, `priced`, `cacheWrite1h` and `reasoning` added to events; older files without them stay readable. `/api/usage`
retains its daily `rows` and additionally exposes `hourlyRows`, grouped by
provider, date, model, device and offset-bearing `hour`. Clients use one set
of hourly rows for both day and hour aggregation; they must not concatenate
the two arrays.

Hourly grouping follows the report's system timezone. Repeated daylight-saving
hours retain their UTC offsets; 23- and 25-hour days have the corresponding
number of slots. Empty hours are shown. Old imports and snapshots without
timestamps appear in an **Unknown time** bar and still contribute to the day
total. The hourly average is unavailable when time is missing. No real usage
is distributed across guessed hours. The preview demo alone adds fictional
hourly detail while preserving its original daily totals.

Standalone HTML snapshots embed the costs and hourly data and require
no external network requests. The native macOS implementation can follow this
data and calculation contract; its source is unchanged by this feature.
