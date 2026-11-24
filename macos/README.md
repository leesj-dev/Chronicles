# Native macOS port

Chronicles is being ported to a native macOS app: Swift for collection and analytics, SwiftUI/AppKit for the app, and native Canvas for the daily timeline. The app contains no Python runtime, WebKit view, local HTTP server, or third-party dependencies.

Requires macOS 14 or newer and a Swift 6 toolchain (Xcode or Command Line Tools).

```sh
macos/scripts/build-app.sh
open macos/build/Chronicles.app
```

The app bundle uses an optimized release build by default. Use `macos/scripts/build-app.sh debug` when debugging. These bundles are signed ad hoc for development; Developer ID signing and notarization are not implemented yet.

The app reads local Codex and Claude Code histories on launch. If no records are found, it falls back to clearly labeled fictional demo records; Refresh checks local histories again. The scan runs away from the UI thread, streams JSONL, caches per-file normalized counters in memory, and never writes to provider histories. `CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `CHRONICLES_TIMEZONE`, and `TZ` are honored when present in the app's launch environment. Finder launches normally use the system timezone and standard home directories.

**Open Usage Report…** reads the existing collector's JSON report format, including missing-token events and shared devices. It replaces the displayed report; it does not merge or persist imported records. Existing standalone HTML exports are not JSON reports. Refresh rereads the selected source. CSV saves the current filters with a native save panel; unrecorded token values stay blank.

## First working slice

- Native window with a custom neutral design matching the web dashboard, compact filters, calendar popovers, a draggable history navigator, model totals, and system appearance.
- Native Canvas daily stacked bars with stable model colors, a cursor-adjacent overlay tooltip, and keyboard/accessibility date navigation. Bars, labels and hover share centered day coordinates; fixed plot insets keep range dragging from moving the axes.
- Selected total and average, including inactive days; requests stay unabridged.
- Native CSV export and JSON report opening.
- Swift Codex and Claude Code collectors, preserving cumulative token deltas, duplicate detection, cache buckets, timestamps, and canonical model naming.
- Native demo resource packaged from `examples/demo.json`.
- The web app's existing Geist variable font, bundled as a native TTF with its SIL OFL license. CoreText applies the original weight axis; no font conversion runs during builds or app launch.

Still to port: Model chapters, Ranking race, additional history sources, OpenUsage account metrics, SSH import, persisted preferences, and standalone report export. Account metrics in imported JSON are currently ignored. Local scanning is explicitly limited to Codex and Claude Code. The web dashboard and Rust app remain the behavioral reference during migration.

## Architecture

```text
macos/
  Package.swift
  Sources/
    ChroniclesCore/       Report contract, analytics, model identities, JSONL collectors
    ChroniclesApp/        SwiftUI app, Canvas chart, AppKit file panels
    ChroniclesInspect/    Read-only CLI for differential verification
  Tests/ChroniclesCoreTests/
  scripts/
    build-app.sh
    analytics-parity.mjs
    collector-parity.py
```

`ChroniclesCore` is independent of the UI. Calendar-date chart coordinates use UTC so daylight-saving changes do not alter day counts; collection converts timestamps to the report timezone before aggregation. Missing-token events contribute requests and recorded rounds without fabricated token totals. Account balances and quotas will remain separate from dated usage.

The daily timeline uses a native Canvas drawing surface with fixed plot geometry. The initial Swift Charts version used date-boundary ticks and automatic axis layout, and inserted hover details into the document flow. Canvas lets this simple bar chart match the web's centered dates and pointer tooltip while keeping geometry stable during range changes. Prepared aggregation is cached by report/filter/metric; hover updates only a separate overlay, and zero-height segments are omitted. Model chapters will use native layouts and shapes, and Ranking race will use a custom SwiftUI view with time-based interpolation.

This first build is unsandboxed so it can discover the existing history locations. macOS privacy restrictions still apply. Before distribution, decide the supported file-access workflow, complete signing/notarization, and verify provider access under that distribution setup. Native functionality is not yet at feature parity with the web version.

## Verification

```sh
swift build --package-path macos
macos/.build/debug/chronicles-core-checks
node macos/scripts/analytics-parity.mjs
macos/scripts/build-app.sh
swift macos/scripts/font-check.swift
```

The standalone core checks run with Command Line Tools, without installing XCTest or Swift Testing. They cover dates/DST, missing tokens, filters/shared devices, inactive days, names/colors/formatting, centered timeline hit testing and boundaries, Codex cumulative deltas/resets/archive copies, and Claude deduplication/cache invalidation. The JavaScript differential check compares all seven metrics' model totals, daily values, and averages against the web implementation, using the canonical demo and a missing-token fixture (42 comparisons). The font check registers the packaged font through CoreText and checks regular, medium, semibold and bold weights. Node is used for the development checks. The legacy `collector-parity.py` script targeted the removed Python collector and is no longer part of verification.

Local verification of this slice: the app was built and launched; metric/harness/date changes and the calendar popover were checked in the native UI. The start-date handle was incremented through its accessibility action. A CSV saved through the native panel was read back and matched the web selection exactly (12 rows, 990,536 tokens, April 21–26 demo selection excluding Claude Code). A real local scan completed across 1,321 files; this proves native collection/read-back, not full parity across all local history. The eight core checks, 42 analytics comparisons, fictional-history collector comparison, and existing 16 JavaScript/24 Python tests passed. CI configuration is added; hosted CI has not been run for these uncommitted changes.

The tooltip uses intrinsic single-line content width and measures/clamps its current content in the same layout pass, including at the final date. Native UI verification covered the final day at the right/bottom edge in a 900-point-wide window, with both tooltip rows visible, and scrolling with the tooltip open. The dashboard History popup, chart hint and added report footer have been removed. Hover only updates the overlay, with cached aggregation and a batched Canvas drawing; actual frame-rate comparison against the web remains unmeasured.

Next: profile optimized collection and chart responsiveness on large archives, verify dark appearance and keyboard interactions more broadly, and port the remaining visualizations and sources in independently reviewable slices.
