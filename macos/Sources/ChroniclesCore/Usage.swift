import Foundation

/// A calendar date, independent of daylight-saving time and the chart's display timezone.
public struct UsageDay: RawRepresentable, Codable, Comparable, Hashable, Sendable {
    public let rawValue: String
    public init?(rawValue: String) {
        let parts = rawValue.split(separator: "-", omittingEmptySubsequences: false)
        guard rawValue.count == 10, parts.count == 3,
              parts[0].count == 4, parts[1].count == 2, parts[2].count == 2,
              parts.allSatisfy({ $0.utf8.allSatisfy { (48...57).contains($0) } }),
              let year = Int(parts[0]), let month = Int(parts[1]), let day = Int(parts[2]),
              year > 0, (1...12).contains(month), (1...31).contains(day) else { return nil }
        let components = DateComponents(year: year, month: month, day: day)
        guard let date = Self.calendar.date(from: components),
              Self.calendar.dateComponents([.year, .month, .day], from: date) == components else { return nil }
        self.rawValue = rawValue
    }
    public static var calendar: Calendar {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        return calendar
    }
    public var date: Date {
        let parts = rawValue.split(separator: "-").map { Int($0)! }
        return Self.calendar.date(from: DateComponents(year: parts[0], month: parts[1], day: parts[2]))!
    }
    public init(date: Date, timeZone: TimeZone) {
        var calendar = Self.calendar
        calendar.timeZone = timeZone
        let parts = calendar.dateComponents([.year, .month, .day], from: date)
        rawValue = String(format: "%04d-%02d-%02d", parts.year!, parts.month!, parts.day!)
    }
    public func adding(days: Int) -> UsageDay {
        UsageDay(date: Self.calendar.date(byAdding: .day, value: days, to: date)!, timeZone: Self.calendar.timeZone)
    }
    public func distance(to end: UsageDay) -> Int {
        Self.calendar.dateComponents([.day], from: date, to: end.date).day!
    }
    public static func < (lhs: Self, rhs: Self) -> Bool { lhs.rawValue < rhs.rawValue }
    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let value = try container.decode(String.self)
        guard let day = Self(rawValue: value) else {
            throw DecodingError.dataCorruptedError(in: container, debugDescription: "Invalid calendar date")
        }
        self = day
    }
    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }
}

public enum UsageMetric: String, CaseIterable, Codable, Identifiable, Sendable {
    case requests, total, input, output, cacheRead, cacheWrite, rounds
    public var id: String { rawValue }
    public var title: String {
        switch self {
        case .requests: "Requests / calls"
        case .total: "Total tokens"
        case .input: "Input tokens"
        case .output: "Output tokens"
        case .cacheRead: "Cache read"
        case .cacheWrite: "Cache write"
        case .rounds: "Copilot response rounds"
        }
    }
    public func formatted(_ value: Double, average: Bool = false) -> String {
        if self != .requests && self != .rounds {
            let magnitude = abs(value)
            if magnitude >= 999_950_000 {
                return String(format: "%.2fB", locale: Locale(identifier: "en_US"), value / 1e9)
            }
            if magnitude >= 999_950 {
                return String(format: "%.1fM", locale: Locale(identifier: "en_US"), value / 1e6)
            }
            if magnitude >= 1e3 {
                return String(format: "%.1fK", locale: Locale(identifier: "en_US"), value / 1e3)
            }
        }
        return value.formatted(.number.locale(Locale(identifier: "en_US")).precision(.fractionLength(0...(average && self == .requests ? 1 : 2))))
    }
}

public struct UsageRow: Codable, Hashable, Sendable {
    public var date: UsageDay
    public var provider: String
    public var model: String
    public var device: String
    public var requests: Int
    public var rounds: Int
    public var roundRecords: Int
    public var input: Int
    public var output: Int
    public var cacheRead: Int
    public var cacheWrite: Int
    public var total: Int
    public var tokenRecorded: Bool

    public init(date: UsageDay, provider: String, model: String, device: String = "this-mac",
                requests: Int = 0, rounds: Int = 0, roundRecords: Int = 0,
                input: Int = 0, output: Int = 0, cacheRead: Int = 0, cacheWrite: Int = 0,
                total: Int? = nil, tokenRecorded: Bool = true) {
        self.date = date; self.provider = provider; self.model = ModelIdentity.canonical(model)
        self.device = device; self.requests = requests; self.rounds = rounds; self.roundRecords = roundRecords
        self.input = input; self.output = output; self.cacheRead = cacheRead; self.cacheWrite = cacheWrite
        self.total = total ?? input + output + cacheRead + cacheWrite
        self.tokenRecorded = tokenRecorded
    }
    enum CodingKeys: String, CodingKey {
        case date, provider, model, device, requests, rounds, roundRecords, input, output, cacheRead, cacheWrite, total, tokenRecorded
    }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        self.init(date: try c.decode(UsageDay.self, forKey: .date),
                  provider: try c.decode(String.self, forKey: .provider), model: try c.decode(String.self, forKey: .model),
                  device: try c.decodeIfPresent(String.self, forKey: .device) ?? "this-mac",
                  requests: try c.decodeIfPresent(Int.self, forKey: .requests) ?? 0,
                  rounds: try c.decodeIfPresent(Int.self, forKey: .rounds) ?? 0,
                  roundRecords: try c.decodeIfPresent(Int.self, forKey: .roundRecords) ?? 0,
                  input: try c.decodeIfPresent(Int.self, forKey: .input) ?? 0,
                  output: try c.decodeIfPresent(Int.self, forKey: .output) ?? 0,
                  cacheRead: try c.decodeIfPresent(Int.self, forKey: .cacheRead) ?? 0,
                  cacheWrite: try c.decodeIfPresent(Int.self, forKey: .cacheWrite) ?? 0,
                  total: try c.decodeIfPresent(Int.self, forKey: .total) ?? 0,
                  tokenRecorded: try c.decodeIfPresent(Bool.self, forKey: .tokenRecorded) ?? true)
    }
    public func value(for metric: UsageMetric) -> Int {
        switch metric {
        case .requests: requests
        case .total: total
        case .input: input
        case .output: output
        case .cacheRead: cacheRead
        case .cacheWrite: cacheWrite
        case .rounds: rounds
        }
    }
}

public struct UsageReport: Codable, Sendable {
    public var rows: [UsageRow]
    public var timezone: String
    public var demo: Bool
    public var files: Int
    public var skipped: Int
    public var errors: [String]
    public var updatedAt: String?

    public init(rows: [UsageRow], timezone: String, demo: Bool = false, files: Int = 0, skipped: Int = 0, errors: [String] = [], updatedAt: String? = nil) {
        self.rows = rows; self.timezone = timezone; self.demo = demo; self.files = files
        self.skipped = skipped; self.errors = errors; self.updatedAt = updatedAt
    }
    enum CodingKeys: String, CodingKey { case rows, missing, timezone, demo, files, skipped, errors, updatedAt }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        var rows = try c.decode([UsageRow].self, forKey: .rows)
        // A missing-token event still contributes one request and any recorded rounds.
        for var row in try c.decodeIfPresent([UsageRow].self, forKey: .missing) ?? [] {
            row.requests = 1; row.tokenRecorded = false
            row.input = 0; row.output = 0; row.cacheRead = 0; row.cacheWrite = 0; row.total = 0
            rows.append(row)
        }
        self.init(rows: rows, timezone: try c.decodeIfPresent(String.self, forKey: .timezone) ?? "UTC",
                  demo: try c.decodeIfPresent(Bool.self, forKey: .demo) ?? false,
                  files: try c.decodeIfPresent(Int.self, forKey: .files) ?? 0,
                  skipped: try c.decodeIfPresent(Int.self, forKey: .skipped) ?? 0,
                  errors: try c.decodeIfPresent([String].self, forKey: .errors) ?? [],
                  updatedAt: try c.decodeIfPresent(String.self, forKey: .updatedAt))
    }
    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(rows.filter(\.tokenRecorded), forKey: .rows)
        try c.encode(rows.filter { !$0.tokenRecorded }, forKey: .missing)
        try c.encode(timezone, forKey: .timezone); try c.encode(demo, forKey: .demo)
        try c.encode(files, forKey: .files); try c.encode(skipped, forKey: .skipped)
        try c.encode(errors, forKey: .errors); try c.encodeIfPresent(updatedAt, forKey: .updatedAt)
    }
}

public struct UsageFilter: Equatable, Sendable {
    public var start: UsageDay
    public var end: UsageDay
    public var providers: Set<String>
    public var models: Set<String>
    public var device: String
    public var excludeReview: Bool
    public init(start: UsageDay, end: UsageDay, providers: Set<String>, models: Set<String>, device: String = "all", excludeReview: Bool = true) {
        self.start = start; self.end = end; self.providers = providers; self.models = models
        self.device = device; self.excludeReview = excludeReview
    }
    public func apply(to rows: [UsageRow]) -> [UsageRow] {
        rows.filter {
            $0.date >= start && $0.date <= end && providers.contains($0.provider) && models.contains($0.model)
            && (device == "all" || $0.device == device || $0.device == "shared")
            && (!excludeReview || $0.model != "codex-auto-review")
        }
    }
}

public struct ModelTotal: Codable, Equatable, Identifiable, Sendable {
    public var model: String
    public var value: Int
    public var id: String { model }
}
public struct DailyValue: Codable, Identifiable, Sendable {
    public var day: UsageDay
    public var model: String
    public var value: Int
    public var id: String { day.rawValue + ":" + model }
}
public enum UsageAnalytics {
    public static func totals(_ rows: [UsageRow], metric: UsageMetric) -> [ModelTotal] {
        let values = rows.reduce(into: [String: Int]()) { $0[$1.model, default: 0] += $1.value(for: metric) }
        return values.map { ModelTotal(model: $0.key, value: $0.value) }.sorted {
            $0.value == $1.value ? $0.model < $1.model : $0.value > $1.value
        }
    }
    public static func daily(_ rows: [UsageRow], metric: UsageMetric, start: UsageDay, end: UsageDay) -> [DailyValue] {
        guard start <= end else { return [] }
        let models = totals(rows, metric: metric).map(\.model)
        let grouped = Dictionary(grouping: rows, by: \.date)
        var values: [DailyValue] = []
        var day = start
        while day <= end {
            let counts = Dictionary(uniqueKeysWithValues: totals(grouped[day] ?? [], metric: metric).map { ($0.model, $0.value) })
            values += models.map { DailyValue(day: day, model: $0, value: counts[$0, default: 0]) }
            day = day.adding(days: 1)
        }
        return values
    }
    public static func average(_ rows: [UsageRow], metric: UsageMetric, start: UsageDay, end: UsageDay) -> Double {
        guard start <= end else { return 0 }
        return Double(rows.reduce(0) { $0 + $1.value(for: metric) }) / Double(start.distance(to: end) + 1)
    }
    public static func csv(_ rows: [UsageRow]) -> String {
        let fields = ["date", "provider", "device", "model", "requests", "rounds", "input", "output", "cacheRead", "cacheWrite", "total", "tokenRecorded"]
        func quote(_ value: String) -> String { "\"" + value.replacingOccurrences(of: "\"", with: "\"\"") + "\"" }
        let records: [String] = rows.map { row in
            var values = [row.date.rawValue, row.provider, row.device, row.model, String(row.requests), String(row.rounds)]
            let tokens = [row.input, row.output, row.cacheRead, row.cacheWrite, row.total]
            values.append(contentsOf: tokens.map { row.tokenRecorded ? String($0) : "" })
            values.append(row.tokenRecorded ? "true" : "false")
            return values.map(quote).joined(separator: ",")
        }
        return ([fields.joined(separator: ",")] + records).joined(separator: "\n")
    }
}
