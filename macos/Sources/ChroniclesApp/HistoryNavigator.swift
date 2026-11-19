import ChroniclesCore
import SwiftUI

/// Decorative overview plus adjustable native handles; all mutations use calendar days.
struct HistoryNavigator: View {
    var values: [Int]
    var bounds: ClosedRange<Date>
    @Binding var startDate: Date
    @Binding var endDate: Date
    @State private var draggingStart: Bool?

    private var first: UsageDay { UsageDay(date: bounds.lowerBound, timeZone: UsageDay.calendar.timeZone) }
    private var last: UsageDay { UsageDay(date: bounds.upperBound, timeZone: UsageDay.calendar.timeZone) }
    private var span: Int { first.distance(to: last) }
    private func fraction(_ date: Date) -> Double {
        guard span > 0 else { return 0 }
        return min(1, max(0, Double(first.distance(to: UsageDay(date: date, timeZone: UsageDay.calendar.timeZone))) / Double(span)))
    }
    var body: some View {
        let peak = max(1, values.max() ?? 1)
        let startIndex = first.distance(to: UsageDay(date: startDate, timeZone: UsageDay.calendar.timeZone))
        let endIndex = first.distance(to: UsageDay(date: endDate, timeZone: UsageDay.calendar.timeZone))
        VStack(spacing: 7) {
            GeometryReader { geometry in
                let width = max(1, geometry.size.width - 12)
                let startX = 6 + width * fraction(startDate), endX = 6 + width * fraction(endDate)
                Canvas { context, size in
                    let count = max(1, span + 1), step = (size.width - 12) / Double(count)
                    for index in 0..<count {
                        let height = 3 + 26 * Double(values.indices.contains(index) ? values[index] : 0) / Double(peak)
                        let rect = CGRect(x: 6 + Double(index) * step, y: 30 - height, width: max(0.5, step - 1.5), height: height)
                        let selected = index >= startIndex && index <= endIndex
                        context.fill(Path(rect), with: .color(Design.muted.opacity(selected ? 0.35 : 0.12)))
                    }
                }.frame(height: 30).accessibilityHidden(true)
                Capsule().fill(Design.line).frame(height: 3).offset(y: 37)
                Capsule().fill(Design.ink).frame(width: max(0, endX - startX), height: 3).offset(x: startX, y: 37)
                handle(start: true).position(x: startX, y: 38.5)
                handle(start: false).position(x: endX, y: 38.5)
                Color.clear.contentShape(Rectangle())
                    .frame(height: 48)
                    .gesture(DragGesture(minimumDistance: 0).onChanged { event in
                        if draggingStart == nil { draggingStart = abs(event.startLocation.x - startX) <= abs(event.startLocation.x - endX) }
                        let offset = Int((min(1, max(0, (event.location.x - 6) / width)) * Double(span)).rounded())
                        let date = first.adding(days: offset).date
                        if draggingStart == true { startDate = min(date, endDate) }
                        else { endDate = max(date, startDate) }
                    }.onEnded { _ in draggingStart = nil })
                    .accessibilityHidden(true)
            }.frame(height: 48)
            HStack {
                Text(first.rawValue)
                Spacer()
                Text("\(UsageDay(date: startDate, timeZone: UsageDay.calendar.timeZone).distance(to: UsageDay(date: endDate, timeZone: UsageDay.calendar.timeZone)) + 1) days selected")
                Spacer()
                Text(last.rawValue)
            }.font(.geist(size: 10)).foregroundStyle(Design.muted)
        }
    }
    private func handle(start: Bool) -> some View {
        Button {} label: {
            Circle().fill(Design.surface).overlay(Circle().stroke(Design.muted.opacity(0.7)))
                .frame(width: 12, height: 12)
                .frame(width: 20, height: 20)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(start ? "Start date" : "End date")
        .accessibilityValue(UsageDay(date: start ? startDate : endDate, timeZone: UsageDay.calendar.timeZone).rawValue)
        .accessibilityAdjustableAction { direction in
            if direction == .increment { move(start: start, by: 1) }
            else if direction == .decrement { move(start: start, by: -1) }
        }
        .onKeyPress(.leftArrow) { move(start: start, by: -1); return .handled }
        .onKeyPress(.rightArrow) { move(start: start, by: 1); return .handled }
    }
    private func move(start: Bool, by offset: Int) {
        let current = UsageDay(date: start ? startDate : endDate, timeZone: UsageDay.calendar.timeZone)
        let date = min(max(current.adding(days: offset).date, bounds.lowerBound), bounds.upperBound)
        if start { startDate = min(date, endDate) } else { endDate = max(date, startDate) }
    }
}
