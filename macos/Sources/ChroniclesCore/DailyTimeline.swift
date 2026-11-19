import Foundation

/// Prepared once per filter change. Pointer movement and scrolling never aggregate records.
public struct DailyTimeline: Equatable, Sendable {
    public struct Day: Equatable, Identifiable, Sendable {
        public var date: UsageDay
        public var values: [ModelTotal]
        public var total: Int
        public var id: UsageDay { date }
    }
    public struct Segment: Equatable, Sendable {
        public var dayIndex: Int
        public var model: String
        public var lower: Int
        public var upper: Int
    }
    public var days: [Day]
    public var segments: [Segment]
    public var maximum: Int
    public var metric: UsageMetric

    public init(rows: [UsageRow], metric: UsageMetric, start: UsageDay, end: UsageDay) {
        self.metric = metric
        days = []; segments = []; maximum = 1
        guard start <= end else { return }
        let rows = rows.filter { $0.date >= start && $0.date <= end }
        let order = UsageAnalytics.totals(rows, metric: metric).map(\.model)
        let groups = Dictionary(grouping: rows, by: \.date)
        for index in 0...start.distance(to: end) {
            let date = start.adding(days: index)
            let values = UsageAnalytics.totals(groups[date] ?? [], metric: metric).filter { $0.value > 0 }
            let counts = Dictionary(uniqueKeysWithValues: values.map { ($0.model, $0.value) })
            var cumulative = 0
            for model in order {
                let value = counts[model, default: 0]
                guard value > 0 else { continue }
                segments.append(Segment(dayIndex: index, model: model, lower: cumulative, upper: cumulative + value))
                cumulative += value
            }
            days.append(Day(date: date, values: values, total: cumulative))
            maximum = max(maximum, cumulative)
        }
    }

    /// Bars, labels, and pointer hit testing share this same equal-width day geometry.
    public func index(at fraction: Double) -> Int? {
        guard !days.isEmpty, fraction.isFinite, fraction >= 0, fraction <= 1 else { return nil }
        return min(days.count - 1, Int(floor(fraction * Double(days.count))))
    }
    public func center(of index: Int) -> Double {
        (Double(index) + 0.5) / Double(max(1, days.count))
    }
    public func tickIndices(plotWidth: Double) -> [Int] {
        guard !days.isEmpty else { return [] }
        let step = max(1, Int(ceil(Double(days.count) / (plotWidth < 600 ? 4 : 9))))
        return days.indices.filter { ($0 % step == 0 && days.count - 1 - $0 >= step / 2) || $0 == days.count - 1 }
    }
}
