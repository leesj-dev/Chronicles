// Standalone checks also run with Command Line Tools, which do not ship XCTest/Testing.
import Foundation
import ChroniclesCore

struct CheckFailure: Error, CustomStringConvertible {
    var description: String
}
func expect(_ condition: @autoclosure () -> Bool, file: StaticString = #filePath, line: UInt = #line) throws {
    if !condition() { throw CheckFailure(description: "Assertion failed at \(file):\(line)") }
}
func require<T>(_ value: T?, file: StaticString = #filePath, line: UInt = #line) throws -> T {
    guard let value else { throw CheckFailure(description: "Required value missing at \(file):\(line)") }
    return value
}


func calendarDaysValidateLeapYearsAndStayIndependentOfDST() throws {
    try expect(UsageDay(rawValue: "2026-02-29") == nil)
    try expect(UsageDay(rawValue: "2024-02-29") != nil)
    try expect(UsageDay(rawValue: "2026-13-01") == nil)
    try expect(UsageDay(rawValue: "2026-3-01") == nil)
    let first = try require(UsageDay(rawValue: "2026-03-07"))
    let last = try require(UsageDay(rawValue: "2026-03-10"))
    try expect(first.distance(to: last) == 3)
    try expect(first.adding(days: 3) == last)
    let instant = try require(ISO8601DateFormatter().date(from: "2026-10-09T15:00:00Z"))
    try expect(UsageDay(date: instant, timeZone: TimeZone(identifier: "Asia/Seoul")!).rawValue == "2026-10-10")
    try expect(UsageDay(date: instant, timeZone: TimeZone(secondsFromGMT: 0)!).rawValue == "2026-10-09")
}

let fixture = #"""
{"rows":[
{"date":"2026-01-01","provider":"codex","model":"gpt-5.4","requests":2,"total":100,"device":"shared"},
{"date":"2026-01-01","provider":"copilot","model":"copilot/gpt-5.4","requests":1,"total":50,"device":"this-mac","rounds":3,"roundRecords":1}
],"missing":[{"date":"2026-01-02","provider":"copilot","model":"copilot/claude-sonnet-4.5","device":"this-mac","rounds":7}]}
"""#

func missingTokensCountAsRequestsAndRoundsWithoutInventingTokens() throws {
    let report = try JSONDecoder().decode(UsageReport.self, from: Data(fixture.utf8))
    let start = try require(UsageDay(rawValue: "2026-01-01")), end = try require(UsageDay(rawValue: "2026-01-02"))
    try expect(UsageAnalytics.totals(report.rows, metric: .requests).reduce(0) { $0 + $1.value } == 4)
    try expect(UsageAnalytics.totals(report.rows, metric: .total).reduce(0) { $0 + $1.value } == 150)
    try expect(UsageAnalytics.totals(report.rows, metric: .rounds).reduce(0) { $0 + $1.value } == 10)
    try expect(UsageAnalytics.average(report.rows, metric: .requests, start: start, end: end) == 2)
    try expect(UsageAnalytics.totals(report.rows, metric: .requests).first?.model == "gpt-5.4")
    let decoded = try JSONDecoder().decode(UsageReport.self, from: JSONEncoder().encode(report))
    try expect(decoded.rows == report.rows)
    let csv = UsageAnalytics.csv(report.rows)
    try expect(csv.contains(#""1","7","","","","","","false""#))
}

func filtersRetainSharedDevicesAndHonorEmptySelections() throws {
    let report = try JSONDecoder().decode(UsageReport.self, from: Data(fixture.utf8))
    var filter = UsageFilter(start: report.rows[0].date, end: report.rows[2].date, providers: ["codex", "copilot"], models: Set(report.rows.map(\.model)), device: "macmini")
    try expect(filter.apply(to: report.rows).count == 1)
    filter.providers = []
    try expect(filter.apply(to: report.rows).isEmpty)
    filter.providers = ["copilot"]; filter.device = "all"
    try expect(filter.apply(to: report.rows).count == 2)
    filter.models = []
    try expect(filter.apply(to: report.rows).isEmpty)
}

func dailyAggregationIncludesInactiveDaysAndCanonicalModels() throws {
    let report = try JSONDecoder().decode(UsageReport.self, from: Data(fixture.utf8))
    let start = report.rows[0].date, end = start.adding(days: 2)
    let daily = UsageAnalytics.daily(report.rows, metric: .total, start: start, end: end)
    try expect(daily.count == 6)
    try expect(daily.first { $0.day == start && $0.model == "gpt-5.4" }?.value == 150)
    try expect(daily.filter { $0.day == end }.allSatisfy { $0.value == 0 })
    try expect(UsageAnalytics.average(report.rows, metric: .total, start: start, end: end) == 50)
}

func namesColorsAndNumberFormatsMatchWebConventions() throws {
    try expect(ModelIdentity.canonical("github.copilot-chat/gpt-5.4") == "gpt-5.4")
    try expect(ModelIdentity.canonical("claude-3-5-haiku-20241022") == "claude-haiku-3.5-20241022")
    try expect(ModelIdentity.canonical("copilot/claude-opus-4-6") == "claude-opus-4.6")
    try expect(ModelIdentity.displayName("claude-haiku-5-5") == "Haiku 5.5")
    try expect(ModelIdentity.displayName("gpt-6-astra") == "GPT-6 Astra")
    try expect(ModelIdentity.displayName("copilot/gemini-3.1-pro-preview") == "Gemini 3.1 Pro preview")
    try expect(ModelIdentity.rgb("copilot/gpt-5.4").blue == ModelIdentity.rgb("gpt-5.4").blue)
    try expect(ModelIdentity.rgb("gpt-5.6-sol").blue != ModelIdentity.rgb("gpt-5.6-luna").blue)
    try expect(UsageMetric.requests.formatted(11680) == "11,680")
    try expect(UsageMetric.total.formatted(2_600_000_000) == "2.60B")
    try expect(UsageMetric.total.formatted(392_000_000) == "392.0M")
    try expect(UsageMetric.total.formatted(130500000) == "130.5M")
    try expect(UsageMetric.input.formatted(1000000) == "1.0M")
    try expect(UsageMetric.output.formatted(11680) == "11.7K")
    try expect(UsageMetric.requests.formatted(49.06, average: true) == "49.1")
}

func timelineGeometryKeepsDatesCenteredAndHoverInsideBounds() throws {
    let start = try require(UsageDay(rawValue: "2026-01-01"))
    let rows = [UsageRow(date: start, provider: "codex", model: "gpt-5.4", requests: 3),
                UsageRow(date: start, provider: "claude", model: "claude-sonnet-4.5", requests: 7),
                UsageRow(date: start.adding(days: 2), provider: "codex", model: "gpt-5.4", requests: 1)]
    let timeline = DailyTimeline(rows: rows, metric: .requests, start: start, end: start.adding(days: 2))
    try expect(timeline.days.count == 3)
    try expect(timeline.maximum == 10)
    try expect(timeline.days[0].values.map(\.value) == [7, 3])
    try expect(timeline.days[1].values.isEmpty)
    try expect(timeline.segments.count == 3) // no zero-height marks to lay out
    try expect(timeline.segments[0].lower == 0 && timeline.segments[0].upper == 7)
    try expect(timeline.segments[1].lower == 7 && timeline.segments[1].upper == 10)
    for index in timeline.days.indices { try expect(timeline.index(at: timeline.center(of: index)) == index) }
    try expect(timeline.index(at: 0) == 0 && timeline.index(at: 1) == 2)
    try expect(timeline.index(at: -0.001) == nil && timeline.index(at: 1.001) == nil)
    try expect(timeline.index(at: .nan) == nil)
    let empty = DailyTimeline(rows: [], metric: .total, start: start, end: start)
    try expect(empty.days.count == 1 && empty.maximum == 1 && empty.segments.isEmpty)
    try expect(empty.center(of: 0) == 0.5)
    let month = DailyTimeline(rows: [], metric: .requests, start: start, end: start.adding(days: 29))
    try expect(month.tickIndices(plotWidth: 900) == [0, 4, 8, 12, 16, 20, 24, 29])
    try expect(month.tickIndices(plotWidth: 400) == [0, 8, 16, 24, 29])
}

func collectorHandlesRepeatedTotalsDeltasResetsAndArchiveCopies() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let sessions = root.appendingPathComponent("sessions"), archive = root.appendingPathComponent("archive")
    try FileManager.default.createDirectory(at: sessions, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: archive, withIntermediateDirectories: true)
    let log = #"""
    {"type":"turn_context","payload":{"model":"gpt-6-sol"}}
    malformed
    {"timestamp":"2026-10-09T15:00:00Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":20,"cache_write_input_tokens":5,"output_tokens":10,"total_tokens":110}}}}
    {"timestamp":"2026-10-09T15:00:01Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":20,"cache_write_input_tokens":5,"output_tokens":10,"total_tokens":110}}}}
    {"timestamp":"2026-10-09T15:00:02Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":130,"cached_input_tokens":25,"cache_write_input_tokens":5,"output_tokens":20,"total_tokens":150},"last_token_usage":{"input_tokens":999,"output_tokens":999}}}}
    {"timestamp":"2026-10-09T15:00:03Z","payload":{"type":"token_count","info":{"model":"gpt-6-astra","total_token_usage":{"input_tokens":8,"output_tokens":2,"total_tokens":10},"last_token_usage":{"input_tokens":8,"output_tokens":2}}}}
    """#
    try log.write(to: sessions.appendingPathComponent("a.jsonl"), atomically: true, encoding: .utf8)
    try log.write(to: archive.appendingPathComponent("copy.jsonl"), atomically: true, encoding: .utf8)
    let collector = JSONLCollector()
    let report = try await collector.collect(roots: HistoryRoots(codex: [sessions, archive], claude: []), timeZone: TimeZone(identifier: "Asia/Seoul")!)
    try expect(report.files == 2)
    try expect(report.skipped == 2)
    try expect(report.rows.reduce(0) { $0 + $1.requests } == 3)
    let sol = try require(report.rows.first { $0.model == "gpt-6-sol" })
    try expect(sol.input == 100)
    try expect(sol.output == 20)
    try expect(sol.cacheRead == 25)
    try expect(sol.cacheWrite == 5)
    try expect(sol.date.rawValue == "2026-10-10")
    let cached = try await collector.collect(roots: HistoryRoots(codex: [sessions, archive], claude: []), timeZone: TimeZone(identifier: "Asia/Seoul")!)
    try expect(cached.rows == report.rows)
    try FileManager.default.removeItem(at: sessions)
    try FileManager.default.removeItem(at: archive)
    let empty = try await collector.collect(roots: HistoryRoots(codex: [sessions, archive], claude: []))
    try expect(empty.rows.isEmpty)
    try expect(empty.files == 0)
}

func claudeDeduplicatesMessagesAndInvalidatesEditedFiles() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    let path = root.appendingPathComponent("claude.jsonl")
    func line(_ output: Int) -> String {
        #"{"timestamp":"2026-01-01T12:00:00Z","requestId":"r","message":{"id":"m","model":"claude-sonnet-4-5","usage":{"input_tokens":10,"output_tokens":OUTPUT,"cache_read_input_tokens":2,"cache_creation_input_tokens":3}}}"#.replacingOccurrences(of: "OUTPUT", with: String(output))
    }
    try (line(5) + "\n" + line(20)).write(to: path, atomically: true, encoding: .utf8)
    let collector = JSONLCollector(), roots = HistoryRoots(codex: [], claude: [root])
    let first = try await collector.collect(roots: roots)
    try expect(first.rows.first?.requests == 1)
    try expect(first.rows.first?.output == 20)
    try expect(first.rows.first?.model == "claude-sonnet-4.5")
    try line(100).write(to: path, atomically: true, encoding: .utf8)
    let next = try await collector.collect(roots: roots)
    try expect(next.rows.first?.output == 100)
    try expect(next.rows.first?.total == 115)
}

@main struct CoreChecks {
    static func main() async throws {
        try calendarDaysValidateLeapYearsAndStayIndependentOfDST()
        try missingTokensCountAsRequestsAndRoundsWithoutInventingTokens()
        try filtersRetainSharedDevicesAndHonorEmptySelections()
        try dailyAggregationIncludesInactiveDaysAndCanonicalModels()
        try namesColorsAndNumberFormatsMatchWebConventions()
        try timelineGeometryKeepsDatesCenteredAndHoverInsideBounds()
        try await collectorHandlesRepeatedTotalsDeltasResetsAndArchiveCopies()
        try await claudeDeduplicatesMessagesAndInvalidatesEditedFiles()
        print("8 native core checks passed (calendar, missing tokens, filters, daily aggregation, names/colors/formatting, timeline geometry, Codex, Claude).")
    }
}
