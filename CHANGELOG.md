# Changelog

## Unreleased

- Apply published rates tokscale-core leaves out: Claude fast mode, Codex Ultrafast, GPT-5.5 Fast at 2.5x, and free Codex auto-review from October 6, 2026. The shared files `events.rs`, `report.rs` and `rules.rs` are identical in CrossBar.
- Count reasoning tokens in output and totals again; tokscale-core reports them apart from output.
- Replace the Python server with the Rust `chronicles` binary. Use `serve` for `server.py`, `export PATH` for `server.py --export PATH`, `sync` for `sync.py`, and `report` for the former standalone engine command.
- Remove Cursor usage collection, including automatic dashboard history and CSV import. Cursor usage is available only through its private dashboard API, which tokscale-core does not fetch.
- SSH sync runs the same collector on the other Mac and imports costs. Event IDs deduplicate usage across devices. Run sync again to price old imports.
- Remove the page-side price estimate and pricing feeds/snapshots. The engine provides per-message costs and replaces `--offline-pricing` with `--offline` for cached rates. The demo now carries fictional costs.
- Remove the Copilot response-rounds metric and the missing-token and legacy Antigravity notices, which only the deleted adapters produced.
- Show harnesses by display name from the server, including ones the page does not know; new harnesses start selected.
- Keep browser assets in `src/web/` and embed them in the Rust binary. Private data stays in the checkout when running from it.
- Revise setup, source coverage, architecture and development documentation.

- Unify seven history providers behind provider-specific adapters and a common ScanResult contract.
- Show only harnesses with loaded usage history; keep availability independent of chart filters.

- Match the official Nova slider and checkbox dimensions, icons and focus rings.
- Remove dropdown search while retaining listbox keyboard selection.
- Fix date-range pointer routing, coincident handles and selected-track alignment.
- Preserve model button nodes on toggle to avoid restarting hover transitions.
- Keep current filters intact if switching data sources fails.

- Use the original shadcn-html combobox markup and CSS; remove conflicting control padding and height overrides.
- Adapt the upstream shadcn-html combobox JavaScript and fix trigger re-click reopening.

- Apply Nova control density and content-measured chapter headers; hide empty source cards and all demo sources.
- Replace native dropdown menus with keyboard-accessible shadcn popover/listbox controls.
- Replace UI controls with vendored shadcn-html semantic tokens and components.
- Separate selected fills from keyboard focus rings; simplify all/none labels.
- Add all 11 OpenUsage app providers, with separate account-metric cards via its local API.
- Collect Grok CLI and OpenCode v1/v2 history.
- Preserve migration/copy deduplication, token bucket accounting and standalone offline exports.

## 0.1.0

- Local usage collection for Codex, Claude Code, macOS Copilot, and Antigravity.
- English daily timeline and ranked model chapters with shared colors and metrics.
- Harness/model multi-selection, all/none selection, and date sliders.
- Missing historical token coverage, request and response-round counts.
- SSH metadata imports, CSV, and self-contained HTML snapshots.
- System appearance, fictional demo, and collector/analytics checks.
