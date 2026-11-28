# Chronicles

**Your AI usage story across models, harnesses, and time.**

Chronicles is an open-source dashboard for exploring your AI usage history. Compare daily model usage, see when your model mix changed, and bring together records from multiple devices. The `chronicles` Rust binary serves the embedded vanilla JavaScript dashboard and collects local history. No account or Python installation is required.

### Daily timeline

Compare daily usage by model with stacked bars and a shared color palette.

![Daily timeline with fictional data](docs/daily.png)

### Model chapters

See how your model mix changes across chapters. Circle area shows total usage; bars compare daily averages.

![Model chapters with fictional data](docs/chapters.png)

### Ranking race

Replay model rankings over time using a 7-day window, a 30-day window, or cumulative usage. Pause, scrub to a date, and choose 1x, 2x, or 4x playback.

![Ranking race with fictional data](docs/race.png)

All screenshots use fictional demo records.

## Native macOS app (in development)

A fully native macOS port is underway in [`macos/`](macos/README.md), using Swift and SwiftUI/AppKit, with a native Canvas daily timeline. The first slice includes the daily timeline, filters, CSV export, JSON report opening, and Codex/Claude Code collection. The app starts with fictional demo records and runs without Python, WebKit, or a local server. macOS 14+ and Swift 6 are required to build it.

```sh
macos/scripts/build-app.sh
open macos/build/Chronicles.app
```

The native app is not yet at feature parity with the web version. See the [port status and verification instructions](macos/README.md).

## Quick start

Requires a modern browser and **Rust 1.98** (installed through [rustup](https://rustup.rs); `engine/rust-toolchain.toml` selects it). Node.js 20+ is optional for development checks. The first build downloads tokscale-core and takes a few minutes.

```sh
git clone https://github.com/leesj-dev/Chronicles.git
cd Chronicles
(cd engine && cargo build --release --locked)
engine/target/release/chronicles serve
```

Open [localhost:8768](http://127.0.0.1:8768/). The first scan can take a while for large histories.

To explore fictional sample data:

```sh
engine/target/release/chronicles serve --demo
```

`--offline` makes the engine price from cached rates only. The same flag is available on `export`, `report`, and `sync`. If no local usage records are found, the app automatically shows a preview demo with a clear notice. Refresh checks for local records again. Demo screenshots and `examples/demo.json` contain fictional records only.

## Explore your history

- **Daily timeline:** stacked daily bars with model tooltips and shared model colors.
- **Model chapters:** ranked circles with model names directly above them. Circle area represents total usage; bars use a common scale to compare daily averages across chapters.
- **Ranking race:** animate model rankings for the selected metric and filters. Rolling windows include available history before the selected start date; cumulative usage starts at that date. At 1x, playback advances by two dates per second.
- **Filters:** select models and harnesses individually or select/deselect all. Newly found models are selected automatically; models you deselect stay off. When upgrading from an older saved selection, current models are selected once because the old settings did not track which models had been seen. Only harnesses with loaded usage records appear; changing chart filters keeps that list stable.
- **Summary and models:** see the selected-period total, daily average, and model count beside the chart. Toggle models from the list; on small screens, the panels stack vertically.
- **Numbers:** request counts stay unabridged (for example, `11,680`), and request averages use at most one decimal place. Abbreviated token values use two decimal places (for example, `130.50M`).
- **Metrics:** requests/calls, total/input/output tokens, cache read/write, and estimated cost. All visualizations use the same metrics and colors.
- **Date range:** drag either endpoint, choose dates in the calendar popovers, or choose a preset. Calendars support month navigation and keyboard selection and disable dates outside the available or selected range.
- **Chapter boundaries:** model transitions, 7-day intervals, or calendar months.
- **Export CSV:** exports the current selection, including estimated cost and the number of unpriced records.
- **Appearance:** automatically follows system light/dark mode.

Transition chapters begin when a new model appears with at least 3 records and 0.5% of selected requests. Boundaries stay at least 7 days apart. Filtering recalculates chapters, and daily averages include inactive days.

## Supported sources

Chronicles collects through the Rust engine in [`engine/`](engine/README.md), which reads local history with [tokscale-core](https://github.com/junhoyeo/tokscale)'s parsers and prices every message. One requests/calls unit is one message the engine reads. Main harnesses:

| Harness | Notes |
| --- | --- |
| Codex | Sessions and archived sessions, including compressed archives |
| Claude Code | Response messages carrying usage |
| Copilot | Local history, including VS Code Chat requests stored without token counts. Those requests contribute to request totals only. |
| Antigravity (IDE extension and CLI, shown as one harness), Grok, OpenCode | Read by the engine |
| Aside, Gemini CLI, Amp and others | Local history from tokscale-core parsers. The page uses the display names supplied by the engine. |

See the [engine README](engine/README.md) for collection options. Duplicate records across copies, forks and imported devices are merged by the engine's dedup keys. Deleted or unpreserved records cannot be recovered.

Cursor usage is available only through its private dashboard API, so Chronicles omits it from local scans and SSH sync. Chronicles no longer reads old `.local/cursor-history.json` or `.local/imports/cursor.csv` files.

On macOS and Linux, parser diagnostics beginning with `Warning:` appear in the page’s error list.

### History & account limits

An optional integration with [OpenUsage](https://github.com/robinebers/openusage), the macOS app, displays account metrics for **Claude, Codex, Cursor, Antigravity, Copilot, Devin, Grok, Ollama, OpenCode, OpenRouter, and Z.ai**. Start OpenUsage and enable the providers you use. Chronicles reads its loopback API at `127.0.0.1:6736`; OpenUsage manages account authentication and polling.

Only available metrics are shown, and this panel is hidden in demo mode. Snapshots are cached for up to 30 seconds and display the upstream update time. Multi-account responses are supported. OpenUsage is optional for native history and demo mode.

Quotas, balances, and spend windows do not become dated model records. The bridge supplies no historical model ledger for Devin, Ollama, OpenRouter, or Z.ai. Ollama here refers to Ollama Cloud limits. Currency values stay separate from token and request totals.

## Timezone and other devices

Dates use the system timezone automatically (for example, `Asia/Seoul` on a Mac configured for Korean time). Chronicles checks `TZ`, `/etc/localtime`, and `/etc/timezone`, falling back to UTC if no IANA timezone is available. Restart the server after changing the system timezone. To override it, set an IANA timezone before launching:

```sh
CHRONICLES_TIMEZONE=Asia/Seoul engine/target/release/chronicles serve
```

To import another Mac, configure an SSH host with batch authentication that runs macOS on the same architecture as this Mac, then run:

```sh
engine/target/release/chronicles sync --host YOUR_SSH_HOST
```

Sync defaults to the SSH host `macmini`. It copies the local binary to `~/.cache/chronicles/chronicles` on the remote when its SHA-256 differs, runs `chronicles report` there, and converts its output with the same code the local scan uses, so shared events get identical ids and merge. A remote of another OS or architecture is refused. Only normalized usage counters and costs are saved in `.local/imports/macmini.json`; conversation text and credentials are not copied. Dates use each report's timezone setting on this computer. One remote snapshot is supported, and syncing another host replaces it. Imports written before costs were recorded load as unpriced rows; run sync again to fill in costs.

## Standalone HTML

```sh
engine/target/release/chronicles export exports/Chronicles.html
engine/target/release/chronicles export exports/demo.html --demo
```

Open the resulting file directly in a browser. All three visualizations and their filters work offline. HTML snapshots contain the complete exported record set and available account metrics; CSV exports follow the current selection. Keep personal snapshots private unless you intend to share your usage history.

## Counting and coverage

Requests/calls treats each message alike for approximate usage-pattern comparisons. It does not imply equal work, compute, or billing. Harnesses that save one record per turn and ones that save one per model call are not directly comparable.

tokscale-core uses the cost the source reports when available. Otherwise, it estimates the cost per message at current rates from LiteLLM, OpenRouter and models.dev. The totals represent API-equivalent usage costs. Records without a reported cost or an applicable rate stay unpriced and are counted in a coverage note. See [docs/cost-and-hourly.md](docs/cost-and-hourly.md).

Cached input stays separate from uncached input; output includes reasoning tokens.

## Project structure

```text
engine/              Chronicles Rust crate
  src/               Collector, HTTP server, export, and SSH sync
  tests/fixtures/    Fictional JSON captured from the previous implementation
src/web/             Embedded HTML, CSS, vanilla JavaScript, and analytics
examples/            Fictional demo records
tests/               JavaScript tests
docs/                Demo screenshots and accounting documentation
.local/              Private caches and imports when run from a checkout
```

Chronicles looks for a checkout above the working directory, then above the binary. When installed elsewhere, it stores data under `~/Library/Application Support/Chronicles` on macOS or `${XDG_DATA_HOME:-$HOME/.local/share}/Chronicles` on Linux. Within that root it uses `.local/engine` for tokscale caches and `.local/imports/macmini.json` for the SSH snapshot. Set `TOKSCALE_CONFIG_DIR` to override the cache directory.

## Development

Use `serve --web-dir src/web` to serve browser assets from disk during development. Rebuild and restart the binary to pick up changes to embedded assets and demo data.

```sh
npm run check
node --test tests/
(cd engine && cargo test --locked)
```

Rust tests use fictional reports, an isolated home, and fake SSH commands. See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines.

## Privacy and license

The server binds only to loopback, validates Host, serves an explicit asset allowlist, and uses a same-origin content policy. No analytics, external fonts, remote scripts, or conversation uploads. Private imports, exports, and caches are gitignored. Account-limit metrics come from OpenUsage over loopback.

[MIT](LICENSE). Independent project; not affiliated with OpenAI, Anthropic, GitHub, Google, or the upstream projects. Controls derive from [shadcn-html](https://github.com/codylindley/shadcn-html) under MIT; the full notice is retained in `src/web/shadcn.css`. Nova styling follows [shadcn/ui](https://github.com/shadcn-ui/ui/blob/main/apps/v4/registry/styles/style-nova.css). The refresh icon is from [Lucide](https://github.com/lucide-icons/lucide) under ISC; its notice is retained in `src/web/index.html`.
