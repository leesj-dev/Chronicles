import SwiftUI

/// Quiet neutral surfaces let the model-family palette carry the information.
enum Design {
    static let ink = adaptive(light: 0x17191D, dark: 0xF1F3F5)
    static let muted = adaptive(light: 0x707680, dark: 0x9DA3AD)
    static let surface = adaptive(light: 0xFFFFFF, dark: 0x17191D)
    static let lifted = adaptive(light: 0xF5F6F8, dark: 0x22252B)
    static let line = adaptive(light: 0xE5E7EB, dark: 0x33373E)
    private static func adaptive(light: Int, dark: Int) -> Color {
        Color(nsColor: NSColor(name: nil) { appearance in
            let hex = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? dark : light
            return NSColor(srgbRed: Double((hex >> 16) & 255) / 255, green: Double((hex >> 8) & 255) / 255, blue: Double(hex & 255) / 255, alpha: 1)
        })
    }
}

struct QuietButtonStyle: ButtonStyle {
    var prominent = false
    var compact = false
    @Environment(\.isEnabled) private var enabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.geist(size: 12, weight: .medium))
            .padding(.horizontal, compact ? 9 : 12).frame(height: compact ? 30 : 34)
            .foregroundStyle(prominent ? Design.surface : Design.ink)
            .background(prominent ? Design.ink : configuration.isPressed ? Design.lifted : Design.surface, in: RoundedRectangle(cornerRadius: 7))
            .overlay(RoundedRectangle(cornerRadius: 7).stroke(prominent ? Design.ink : Design.line, lineWidth: 1))
            .opacity(enabled ? 1 : 0.45)
    }
}

struct ControlLabel: View {
    var title: String
    var value: String
    var body: some View {
        HStack(spacing: 10) {
            Text(title).foregroundStyle(Design.muted)
            Text(value).foregroundStyle(Design.ink).lineLimit(1)
            Image(systemName: "chevron.down").font(.geist(size: 8, weight: .semibold)).foregroundStyle(Design.muted)
        }
        .font(.geist(size: 12)).padding(.horizontal, 11).frame(height: 34)
        .background(Design.surface, in: RoundedRectangle(cornerRadius: 7))
        .overlay(RoundedRectangle(cornerRadius: 7).stroke(Design.line))
    }
}

struct DateControl: View {
    var title: String
    @Binding var date: Date
    var range: ClosedRange<Date>
    @State private var open = false
    var body: some View {
        Button { open.toggle() } label: {
            HStack(spacing: 10) {
                Text(UsageDay(date: date, timeZone: UsageDay.calendar.timeZone).rawValue)
                    .font(.geist(size: 12))
                Image(systemName: "calendar").font(.geist(size: 12)).foregroundStyle(Design.muted)
            }
        }
        .buttonStyle(QuietButtonStyle())
        .accessibilityLabel(title + " date")
        .accessibilityValue(UsageDay(date: date, timeZone: UsageDay.calendar.timeZone).rawValue)
        .popover(isPresented: $open, arrowEdge: .bottom) {
            VStack(alignment: .leading, spacing: 14) {
                Text(title).font(.geist(size: 13, weight: .semibold))
                DatePicker(title, selection: Binding(get: { date }, set: {
                    date = UsageDay(date: $0, timeZone: UsageDay.calendar.timeZone).date
                }), in: range, displayedComponents: .date)
                    .datePickerStyle(.graphical).labelsHidden()
                HStack { Spacer(); Button("Done") { open = false }.buttonStyle(QuietButtonStyle(prominent: true, compact: true)) }
            }
            .padding(16).frame(width: 270)
            .environment(\.calendar, UsageDay.calendar)
            .environment(\.timeZone, UsageDay.calendar.timeZone)
        }
    }
}

import AppKit
import ChroniclesCore

struct ChoiceControl<Value: Hashable>: View {
    var title: String
    @Binding var selection: Value
    var options: [Value]
    var label: (Value) -> String
    @State private var open = false
    var body: some View {
        Button { open.toggle() } label: { ControlLabel(title: title, value: label(selection)) }
            .buttonStyle(.plain)
            .accessibilityLabel(title).accessibilityValue(label(selection))
            .popover(isPresented: $open, arrowEdge: .bottom) {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(Array(options.enumerated()), id: \.offset) { _, option in
                        Button { selection = option; open = false } label: {
                            HStack(spacing: 12) {
                                Text(label(option)).font(.geist(size: 12))
                                Spacer()
                                if option == selection { Image(systemName: "checkmark").font(.geist(size: 10, weight: .semibold)) }
                            }
                            .foregroundStyle(Design.ink)
                            .padding(.horizontal, 10).padding(.vertical, 8)
                            .frame(maxWidth: .infinity)
                            .background(option == selection ? Design.lifted : .clear, in: RoundedRectangle(cornerRadius: 5))
                        }.buttonStyle(.plain).accessibilityValue(option == selection ? "Selected" : "Not selected")
                    }
                }.padding(6).frame(width: 230)
            }
    }
}
