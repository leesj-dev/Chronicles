import ChroniclesCore
import SwiftUI

struct DailyTimelineView: View {
    var timeline: DailyTimeline
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack {
                HStack(spacing: 7) {
                    Image(systemName: "chart.bar.fill").font(.geist(size: 11))
                    Text("Daily timeline").font(.geist(size: 12, weight: .semibold))
                }.padding(.horizontal, 11).padding(.vertical, 8)
                    .background(Design.lifted, in: RoundedRectangle(cornerRadius: 7))
                Spacer()
                Text(timeline.metric.title).font(.geist(size: 11)).foregroundStyle(Design.muted)
            }
            TimelineSurface(timeline: timeline).frame(height: 320)
        }
    }
}

/// Fixed plot insets avoid automatic axis relayout as values change during a range drag.
private struct TimelineGeometry {
    var size: CGSize
    var plot: CGRect { CGRect(x: 52, y: 16, width: max(1, size.width - 68), height: max(1, size.height - 68)) }
}

private struct TimelineSurface: View {
    var timeline: DailyTimeline
    var body: some View {
        GeometryReader { geometry in
            let layout = TimelineGeometry(size: geometry.size)
            ZStack(alignment: .topLeading) {
                TimelineDrawing(timeline: timeline, layout: layout).equatable()
                    .allowsHitTesting(false).accessibilityHidden(true)
                TimelineInteraction(timeline: timeline, layout: layout)
            }
        }
    }
}

/// This drawing has no hover state. Native Canvas batches bars into one drawing surface.
private struct TimelineDrawing: View, Equatable {
    var timeline: DailyTimeline
    var layout: TimelineGeometry
    nonisolated static func == (lhs: Self, rhs: Self) -> Bool { lhs.timeline == rhs.timeline && lhs.layout.size == rhs.layout.size }
    var body: some View {
        Canvas { context, _ in
            let plot = layout.plot
            for index in 0...4 {
                let fraction = Double(index) / 4
                let y = plot.maxY - plot.height * fraction
                var line = Path(); line.move(to: CGPoint(x: plot.minX, y: y)); line.addLine(to: CGPoint(x: plot.maxX, y: y))
                context.stroke(line, with: .color(Design.line), lineWidth: 0.5)
                let label = Text(timeline.metric.formatted(Double(timeline.maximum) * fraction))
                    .font(.geist(size: 10)).foregroundStyle(Design.muted)
                context.draw(label, at: CGPoint(x: plot.minX - 10, y: y), anchor: .trailing)
            }
            let step = plot.width / Double(max(1, timeline.days.count))
            for segment in timeline.segments {
                let bottom = plot.maxY - plot.height * Double(segment.lower) / Double(timeline.maximum)
                let top = plot.maxY - plot.height * Double(segment.upper) / Double(timeline.maximum)
                let bar = CGRect(x: plot.minX + (Double(segment.dayIndex) + 0.1) * step, y: top, width: step * 0.8, height: bottom - top)
                context.fill(Path(bar), with: .color(modelColor(segment.model)))
            }
            for index in timeline.tickIndices(plotWidth: plot.width) {
                let x = plot.minX + plot.width * timeline.center(of: index)
                context.draw(Text(String(timeline.days[index].date.rawValue.suffix(5)))
                    .font(.geist(size: 10)).foregroundStyle(Design.muted), at: CGPoint(x: x, y: plot.maxY + 23))
            }
            if timeline.segments.isEmpty {
                context.draw(Text("No recorded usage for this selection").font(.geist(size: 12)).foregroundStyle(Design.muted),
                             at: CGPoint(x: plot.midX, y: plot.midY))
            }
        }
    }
}

/// Only this lightweight overlay updates when the pointer moves. It never changes layout.
private struct TimelineInteraction: View {
    var timeline: DailyTimeline
    var layout: TimelineGeometry
    @State private var hoverIndex: Int?
    @State private var pointer: CGPoint?
    @State private var keyboardIndex: Int?
    @FocusState private var focused: Bool
    private var selection: Int? { hoverIndex ?? keyboardIndex }
    var body: some View {
        ZStack(alignment: .topLeading) {
            Color.clear.contentShape(Rectangle())
                .frame(width: layout.size.width, height: layout.size.height)
                .onContinuousHover { phase in
                    switch phase {
                    case .active(let point):
                        updatePointer(point)
                    case .ended: hoverIndex = nil; pointer = nil
                    }
                }
                .gesture(DragGesture(minimumDistance: 0).onChanged { event in
                    focused = true
                    updatePointer(event.location)
                }.onEnded { event in
                    updatePointer(event.location)
                })
                .focusable().focusEffectDisabled().focused($focused)
                .onKeyPress(.leftArrow) { move(-1); return .handled }
                .onKeyPress(.rightArrow) { move(1); return .handled }
                .onKeyPress(.escape) { clear(); return .handled }
                .accessibilityLabel("Daily timeline, \(timeline.metric.title)")
                .accessibilityValue(accessibleSelection)
                .accessibilityAdjustableAction { direction in move(direction == .increment ? 1 : -1) }
            if let index = selection, timeline.days.indices.contains(index) {
                let anchor = pointer ?? CGPoint(x: layout.plot.minX + layout.plot.width * timeline.center(of: index), y: layout.plot.minY)
                TooltipPlacement(anchor: anchor, availableSize: layout.size) {
                    DailyTooltip(day: timeline.days[index], metric: timeline.metric, maximumWidth: layout.size.width - 16)
                }.allowsHitTesting(false).accessibilityHidden(true)
            }
        }
        .frame(width: layout.size.width, height: layout.size.height, alignment: .topLeading)
        .onChange(of: timeline) { _, _ in clear() }
        .onChange(of: focused) { _, value in if !value { keyboardIndex = nil } }
        .transaction { $0.animation = nil }
    }
    private var accessibleSelection: String {
        guard let index = selection, timeline.days.indices.contains(index) else { return "Use arrow keys or adjust to explore days" }
        let day = timeline.days[index]
        return ([day.date.rawValue] + day.values.map { "\(ModelIdentity.displayName($0.model)): \(timeline.metric.formatted(Double($0.value)))" }).joined(separator: ", ")
    }
    private func move(_ offset: Int) {
        guard !timeline.days.isEmpty else { return }
        keyboardIndex = min(timeline.days.count - 1, max(0, (selection ?? (offset > 0 ? -1 : 1)) + offset))
        hoverIndex = nil; pointer = nil
    }
    private func updatePointer(_ point: CGPoint) {
        keyboardIndex = nil
        guard point.y >= layout.plot.minY, point.y <= layout.plot.maxY else { hoverIndex = nil; pointer = nil; return }
        hoverIndex = timeline.index(at: (point.x - layout.plot.minX) / layout.plot.width)
        pointer = hoverIndex == nil ? nil : point
    }
    private func clear() { hoverIndex = nil; keyboardIndex = nil; pointer = nil }
}

private struct DailyTooltip: View {
    var day: DailyTimeline.Day
    var metric: UsageMetric
    var maximumWidth: CGFloat
    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text(day.date.rawValue).font(.geist(size: 12, weight: .semibold))
            ForEach(day.values) { value in
                HStack(spacing: 24) {
                    HStack(spacing: 7) {
                        RoundedRectangle(cornerRadius: 2).fill(modelColor(value.model)).frame(width: 8, height: 8)
                        Text(ModelIdentity.displayName(value.model)).lineLimit(1)
                    }
                    Spacer(minLength: 0)
                    Text(metric.formatted(Double(value.value))).monospacedDigit().fixedSize()
                }.font(.geist(size: 12))
            }
            if day.values.isEmpty { Text("No recorded usage").font(.geist(size: 12)).foregroundStyle(Design.muted) }
        }
        .padding(12).frame(maxWidth: maximumWidth, alignment: .leading)
        .fixedSize(horizontal: true, vertical: true)
        .foregroundStyle(Design.ink)
        .background(Design.lifted, in: RoundedRectangle(cornerRadius: 8))
        .overlay(RoundedRectangle(cornerRadius: 8).stroke(Design.line))
        .shadow(color: .black.opacity(0.16), radius: 16, y: 5)
    }
}
/// Measure and clamp in the same layout pass, including when the final day's content changes.
/// A preference/state measurement would briefly position a new tooltip using the previous size.
private struct TooltipPlacement: Layout {
    var anchor: CGPoint
    var availableSize: CGSize
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout Void) -> CGSize { availableSize }
    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout Void) {
        guard let tip = subviews.first else { return }
        let size = tip.sizeThatFits(.unspecified)
        let x = max(8, min(availableSize.width - 8 - size.width, anchor.x + 12))
        let y = max(8, min(availableSize.height - 8 - size.height, anchor.y))
        tip.place(at: CGPoint(x: bounds.minX + x, y: bounds.minY + y), anchor: .topLeading, proposal: ProposedViewSize(size))
    }
}
