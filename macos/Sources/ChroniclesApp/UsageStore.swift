import AppKit
import ChroniclesCore
import Foundation
import Observation
import UniformTypeIdentifiers

@MainActor @Observable final class UsageStore {
    private(set) var report: UsageReport?
    private(set) var loading = false
    var error: String?
    var metric: UsageMetric = .requests
    var providers = Set<String>()
    var models = Set<String>()
    var device = "all"
    var excludeReview = true
    var startDate = Date()
    var endDate = Date()
    private var source: Source = .demo
    private var loadedSource: Source = .demo
    private let collector = JSONLCollector()
    private var work: Task<Void, Never>?
    private var reportGeneration = 0
    private var reportBounds: ClosedRange<Date>?
    private(set) var availableProviders: [String] = []
    private(set) var availableModels: [String] = []
    private(set) var devices: [String] = ["all"]
    @ObservationIgnored private var filteredCache: (generation: Int, filter: UsageFilter, rows: [UsageRow])?
    @ObservationIgnored private var timelineCache: (generation: Int, filter: UsageFilter, metric: UsageMetric, value: DailyTimeline)?
    @ObservationIgnored private var historyCache: (generation: Int, metric: UsageMetric, values: [Int])?
    private enum Source { case demo, local, snapshot(URL) }

    var start: UsageDay { UsageDay(date: startDate, timeZone: UsageDay.calendar.timeZone) }
    var end: UsageDay { UsageDay(date: endDate, timeZone: UsageDay.calendar.timeZone) }
    var bounds: ClosedRange<Date> {
        reportBounds ?? startDate...max(startDate, endDate)
    }
    private var filter: UsageFilter {
        UsageFilter(start: start, end: end, providers: providers, models: models, device: device, excludeReview: excludeReview)
    }
    var filteredRows: [UsageRow] {
        let current = filter
        if let cached = filteredCache, cached.generation == reportGeneration, cached.filter == current { return cached.rows }
        let rows = current.apply(to: report?.rows ?? [])
        filteredCache = (reportGeneration, current, rows)
        return rows
    }
    var timeline: DailyTimeline {
        let current = filter
        if let cached = timelineCache, cached.generation == reportGeneration, cached.filter == current, cached.metric == metric { return cached.value }
        let value = DailyTimeline(rows: filteredRows, metric: metric, start: start, end: end)
        timelineCache = (reportGeneration, current, metric, value)
        return value
    }
    var historyValues: [Int] {
        if let cached = historyCache, cached.generation == reportGeneration, cached.metric == metric { return cached.values }
        let first = UsageDay(date: bounds.lowerBound, timeZone: UsageDay.calendar.timeZone)
        let last = UsageDay(date: bounds.upperBound, timeZone: UsageDay.calendar.timeZone)
        var values = Array(repeating: 0, count: first.distance(to: last) + 1)
        for row in report?.rows ?? [] { values[first.distance(to: row.date)] += row.value(for: metric) }
        historyCache = (reportGeneration, metric, values)
        return values
    }
    var sourceTitle: String {
        switch loadedSource { case .demo: "Fictional demo"; case .local: "Local history · Codex & Claude Code"; case .snapshot(let url): url.lastPathComponent }
    }
    func startDemo() { guard !loading else { return }; source = .demo; reload(reset: true) }
    func scanLocal() { guard !loading else { return }; source = .local; reload(reset: true) }
    func reload(reset: Bool = false) {
        guard !loading else { return }
        if !reset, report?.demo == true { source = .local }
        let requestedSource = source
        loading = true; error = nil
        work = Task {
            defer { loading = false; work = nil }
            do {
                var next: UsageReport
                var resolvedSource = requestedSource
                switch requestedSource {
                case .local:
                    let env = ProcessInfo.processInfo.environment
                    let name = env["CHRONICLES_TIMEZONE"] ?? env["TZ"]
                    let timezone: TimeZone
                    if let name {
                        guard let selected = TimeZone(identifier: name) else {
                            throw NSError(domain: "Chronicles", code: 1, userInfo: [NSLocalizedDescriptionKey: "Invalid timezone: \(name)"])
                        }
                        timezone = selected
                    } else { timezone = .current }
                    next = try await collector.collect(timeZone: timezone)
                    if next.rows.isEmpty && next.errors.isEmpty {
                        next = try await demoReport()
                        resolvedSource = .demo
                    }
                case .demo:
                    next = try await demoReport()
                case .snapshot(let url): next = try await read(url)
                }
                try Task.checkCancellation()
                install(next, reset: reset || report?.demo != next.demo, loadedSource: resolvedSource)
            } catch is CancellationError {} catch { self.error = error.localizedDescription }
        }
    }
    func cancel() { work?.cancel() }
    private func demoReport() async throws -> UsageReport {
        guard let url = Bundle.main.url(forResource: "demo", withExtension: "json") else {
            throw NSError(domain: "Chronicles", code: 2, userInfo: [NSLocalizedDescriptionKey: "Demo resource missing. Build the app with macos/scripts/build-app.sh."])
        }
        return try await read(url)
    }
    private func read(_ url: URL) async throws -> UsageReport {
        try await Task.detached {
            let access = url.startAccessingSecurityScopedResource()
            defer { if access { url.stopAccessingSecurityScopedResource() } }
            return try JSONDecoder().decode(UsageReport.self, from: Data(contentsOf: url))
        }.value
    }
    private func install(_ next: UsageReport, reset: Bool, loadedSource: Source) {
        let oldBounds = bounds
        let oldModels = Set(availableModels)
        let oldProviders = Set(availableProviders)
        report = next
        reportGeneration += 1
        filteredCache = nil; timelineCache = nil; historyCache = nil
        if let first = next.rows.map(\.date).min(), let last = next.rows.map(\.date).max() { reportBounds = first.date...last.date }
        else { reportBounds = nil }
        availableProviders = Array(Set(next.rows.filter { $0.requests > 0 || $0.total > 0 }.map(\.provider))).sorted()
        availableModels = Array(Set(next.rows.map(\.model))).sorted()
        devices = ["all"] + Array(Set(next.rows.map(\.device).filter { $0 != "shared" && $0 != "all" })).sorted()
        self.loadedSource = loadedSource
        if reset {
            providers = Set(availableProviders); models = Set(availableModels); device = "all"
            startDate = bounds.lowerBound; endDate = bounds.upperBound
        } else {
            models.formUnion(Set(availableModels).subtracting(oldModels))
            providers.formUnion(Set(availableProviders).subtracting(oldProviders))
            if startDate == oldBounds.lowerBound { startDate = bounds.lowerBound }
            if endDate == oldBounds.upperBound { endDate = bounds.upperBound }
            startDate = min(max(startDate, bounds.lowerBound), bounds.upperBound)
            endDate = max(startDate, min(endDate, bounds.upperBound))
        }
    }
    func openSnapshot() {
        guard !loading else { return }
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.json]; panel.allowsMultipleSelection = false
        panel.message = "Open a Chronicles usage report."
        guard panel.runModal() == .OK, let url = panel.url else { return }
        source = .snapshot(url); reload(reset: true)
    }
    func exportCSV() {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.commaSeparatedText]
        panel.nameFieldStringValue = "Chronicles-\(start.rawValue)-\(end.rawValue).csv"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        do { try UsageAnalytics.csv(filteredRows).write(to: url, atomically: true, encoding: .utf8) }
        catch { self.error = error.localizedDescription }
    }
    func preset(days: Int?) {
        endDate = bounds.upperBound
        startDate = days.map { max(bounds.lowerBound, UsageDay(date: endDate, timeZone: UsageDay.calendar.timeZone).adding(days: 1 - $0).date) } ?? bounds.lowerBound
    }
}
