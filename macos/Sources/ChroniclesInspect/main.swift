import ChroniclesCore
import Foundation

// Read-only developer utility for parity checks; no server, credentials, or conversation output.
@main struct Inspect {
    static func main() async throws {
        let args = Array(CommandLine.arguments.dropFirst())
        let report: UsageReport
        if let index = args.firstIndex(of: "--report"), args.indices.contains(index + 1) {
            report = try JSONDecoder().decode(UsageReport.self, from: Data(contentsOf: URL(fileURLWithPath: args[index + 1])))
        } else {
            var roots = HistoryRoots.standard()
            if let index = args.firstIndex(of: "--codex-home"), args.indices.contains(index + 1) {
                let home = URL(fileURLWithPath: args[index + 1])
                roots.codex = [home.appendingPathComponent("sessions"), home.appendingPathComponent("archived_sessions")]
            }
            if let index = args.firstIndex(of: "--claude-home"), args.indices.contains(index + 1) {
                roots.claude = [URL(fileURLWithPath: args[index + 1]).appendingPathComponent("projects")]
            }
            let timezone = args.firstIndex(of: "--timezone").flatMap { args.indices.contains($0 + 1) ? TimeZone(identifier: args[$0 + 1]) : nil } ?? .current
            report = try await JSONLCollector().collect(roots: roots, timeZone: timezone)
        }
        if args.contains("--analytics"), let start = report.rows.map(\.date).min(), let end = report.rows.map(\.date).max() {
            struct Result: Encodable { var totals: [ModelTotal]; var daily: [DailyValue]; var average: Double }
            var results: [String: Result] = [:]
            for metric in UsageMetric.allCases {
                results[metric.rawValue] = Result(totals: UsageAnalytics.totals(report.rows, metric: metric),
                                                 daily: UsageAnalytics.daily(report.rows, metric: metric, start: start, end: end),
                                                 average: UsageAnalytics.average(report.rows, metric: metric, start: start, end: end))
            }
            print(String(decoding: try JSONEncoder().encode(results), as: UTF8.self))
        } else { print(String(decoding: try JSONEncoder().encode(report), as: UTF8.self)) }
    }
}
