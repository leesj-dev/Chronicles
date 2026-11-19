import ChroniclesCore
import SwiftUI

struct DashboardView: View {
    @Bindable var store: UsageStore
    var body: some View {
        VStack(spacing: 0) {
            header
            Rectangle().fill(Design.line).frame(height: 1)
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    controls.padding(.vertical, 24)
                    Rectangle().fill(Design.line).frame(height: 1)
                    dateRange.padding(.vertical, 22)
                    notices.padding(.bottom, 20)
                    Rectangle().fill(Design.line).frame(height: 1)
                    if let report = store.report {
                        let rows = store.filteredRows
                        HStack(alignment: .top, spacing: 16) {
                            modelRail(rows).frame(width: 215)
                            Rectangle().fill(Design.line).frame(width: 1)
                            VStack(alignment: .leading, spacing: 14) {
                                DailyTimelineView(timeline: store.timeline)
                                if report.rows.isEmpty {
                                    Text("No Codex or Claude Code history found. Open a usage report or try the demo.")
                                        .font(.geist(size: 12)).foregroundStyle(Design.muted)
                                }
                            }.frame(maxWidth: .infinity, alignment: .leading)
                        }.padding(.vertical, 24)
                    }
                }
                .padding(.horizontal, 30)
                .frame(maxWidth: 1500, alignment: .leading)
                .frame(maxWidth: .infinity)
            }
        }
        .background(Design.surface)
        .foregroundStyle(Design.ink)
        .environment(\.calendar, UsageDay.calendar)
        .environment(\.timeZone, UsageDay.calendar.timeZone)
    }
    private var header: some View {
        HStack(spacing: 16) {
            Text("Chronicles").font(.geist(size: 28, weight: .semibold)).tracking(-1.3)
            Text("Your AI usage story").font(.geist(size: 11)).foregroundStyle(Design.muted)
            Spacer()
            HStack(spacing: 6) {
                Circle().fill(store.report?.demo == true ? Color.orange.opacity(0.65) : Color.green.opacity(0.7)).frame(width: 5, height: 5)
                Text(store.sourceTitle).lineLimit(1)
            }.font(.geist(size: 11)).foregroundStyle(Design.muted)
            Button { store.openSnapshot() } label: { Image(systemName: "folder").frame(width: 15) }
                .help("Open usage report").accessibilityLabel("Open usage report")
                .buttonStyle(QuietButtonStyle()).disabled(store.loading)
            Button { store.reload() } label: { Image(systemName: "arrow.clockwise").frame(width: 15) }
                .help("Refresh").accessibilityLabel("Refresh")
                .buttonStyle(QuietButtonStyle()).disabled(store.loading)
        }.padding(.leading, 88).padding(.trailing, 28).frame(height: 72)
    }
    private var controls: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack(spacing: 8) {
                Text("Harnesses").font(.geist(size: 12)).foregroundStyle(Design.muted).frame(width: 65, alignment: .leading)
                ForEach(store.availableProviders, id: \.self) { provider in
                    Button {
                        if store.providers.contains(provider) { store.providers.remove(provider) } else { store.providers.insert(provider) }
                    } label: { Text(providerName(provider)) }
                    .buttonStyle(QuietButtonStyle(prominent: store.providers.contains(provider), compact: true))
                    .accessibilityValue(store.providers.contains(provider) ? "Selected" : "Not selected")
                }
                Button(Set(store.availableProviders).isSubset(of: store.providers) ? "Deselect all" : "Select all") {
                    store.providers = Set(store.availableProviders).isSubset(of: store.providers) ? [] : Set(store.availableProviders)
                }.buttonStyle(.plain).font(.geist(size: 11)).foregroundStyle(Design.muted).padding(.leading, 5)
                Spacer()
            }
            HStack(spacing: 12) {
                ChoiceControl(title: "Metric", selection: $store.metric, options: UsageMetric.allCases, label: { $0.title })
                ChoiceControl(title: "Device", selection: $store.device, options: store.devices, label: { $0 == "all" ? "All devices" : $0 })
                Spacer()
                Toggle("Exclude auto-review", isOn: $store.excludeReview)
                    .toggleStyle(ReviewToggleStyle())
                Button("Export CSV") { store.exportCSV() }.buttonStyle(QuietButtonStyle())
            }
        }.disabled(store.loading || store.report == nil)
    }
    private var dateRange: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(alignment: .bottom, spacing: 10) {
                VStack(alignment: .leading, spacing: 6) {
                    Text("From").font(.geist(size: 11)).foregroundStyle(Design.muted)
                    DateControl(title: "From", date: $store.startDate, range: store.bounds.lowerBound...max(store.bounds.lowerBound, store.endDate))
                }
                Image(systemName: "arrow.right").font(.geist(size: 11)).foregroundStyle(Design.muted).padding(.bottom, 11)
                VStack(alignment: .leading, spacing: 6) {
                    Text("To").font(.geist(size: 11)).foregroundStyle(Design.muted)
                    DateControl(title: "To", date: $store.endDate, range: min(store.startDate, store.bounds.upperBound)...store.bounds.upperBound)
                }
                Spacer()
                ForEach([7, 30, 90], id: \.self) { days in
                    Button("\(days) days") { store.preset(days: days) }
                        .buttonStyle(.plain).font(.geist(size: 11)).foregroundStyle(Design.muted)
                        .padding(.horizontal, 7).padding(.vertical, 10)
                }
                Button("All time") { store.preset(days: nil) }
                    .buttonStyle(.plain).font(.geist(size: 11)).foregroundStyle(Design.muted)
                    .padding(.horizontal, 7).padding(.vertical, 10)
            }
            if let report = store.report, !report.rows.isEmpty {
                HistoryNavigator(values: store.historyValues, bounds: store.bounds, startDate: $store.startDate, endDate: $store.endDate)
            }
        }.disabled(store.loading || store.report == nil)
    }
    @ViewBuilder private var notices: some View {
        if store.loading {
            HStack(spacing: 9) {
                ProgressView().controlSize(.mini)
                Text("Reading usage records…").font(.geist(size: 12)).foregroundStyle(Design.muted)
                Spacer()
                Button("Cancel") { store.cancel() }.buttonStyle(QuietButtonStyle(compact: true))
            }
        } else if let error = store.error {
            Label(error, systemImage: "exclamationmark.triangle").font(.geist(size: 12)).foregroundStyle(.red).textSelection(.enabled)
        } else if let report = store.report {
            let missing = report.rows.filter { !$0.tokenRecorded }.count
            VStack(alignment: .leading, spacing: 8) {
                if missing > 0 {
                    Text("\(missing.formatted()) requests have no recorded token values. They count toward requests, not token totals.")
                }
                if report.demo { Text("This is a preview with fictional data. Refresh to read local usage records.") }
                else if !report.errors.isEmpty { Text(report.errors.joined(separator: " · ")) }
            }.font(.geist(size: 11)).foregroundStyle(Design.muted)
        }
    }
    private func modelRail(_ rows: [UsageRow]) -> some View {
        let totals = UsageAnalytics.totals(rows, metric: store.metric)
        let filter = UsageFilter(start: store.start, end: store.end, providers: store.providers, models: Set(store.availableModels), device: store.device, excludeReview: store.excludeReview)
        let all = UsageAnalytics.totals(filter.apply(to: store.report?.rows ?? []), metric: store.metric)
        let values = Dictionary(uniqueKeysWithValues: all.map { ($0.model, $0.value) })
        let order = all.map(\.model) + store.availableModels.filter { values[$0] == nil }
        return VStack(alignment: .leading, spacing: 0) {
            stat(store.metric.title, value: store.metric.formatted(Double(totals.reduce(0) { $0 + $1.value })), large: true)
            stat("Daily average", value: store.metric.formatted(UsageAnalytics.average(rows, metric: store.metric, start: store.start, end: store.end), average: true))
                .padding(.top, 14)
            stat("Models selected", value: Set(rows.map(\.model)).count.formatted()).padding(.top, 14)
            Rectangle().fill(Design.line).frame(height: 1).padding(.vertical, 22)
            HStack {
                Text("Models").font(.geist(size: 15, weight: .semibold))
                Spacer()
                Button(Set(store.availableModels).isSubset(of: store.models) ? "Deselect all" : "Select all") {
                    store.models = Set(store.availableModels).isSubset(of: store.models) ? [] : Set(store.availableModels)
                }.font(.geist(size: 10)).foregroundStyle(Design.muted).buttonStyle(.plain)
            }.padding(.bottom, 14)
            ForEach(order, id: \.self) { model in
                ModelRow(model: model, value: store.metric.formatted(Double(values[model, default: 0])), selected: store.models.contains(model)) {
                    if store.models.contains(model) { store.models.remove(model) } else { store.models.insert(model) }
                }
            }
        }.disabled(store.loading)
    }
    private func stat(_ title: String, value: String, large: Bool = false) -> some View {
        HStack(alignment: large ? .firstTextBaseline : .center) {
            Text(title).font(.geist(size: 11)).foregroundStyle(Design.muted)
            Spacer()
            Text(value).font(.geist(size: large ? 29 : 19, weight: .semibold)).tracking(large ? -0.8 : -0.4).monospacedDigit()
        }
    }
}

private struct ModelRow: View {
    var model: String
    var value: String
    var selected: Bool
    var action: () -> Void
    @State private var hovered = false
    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                RoundedRectangle(cornerRadius: 3).fill(modelColor(model)).frame(width: 9, height: 9)
                Text(ModelIdentity.displayName(model)).font(.geist(size: 11, weight: .medium)).lineLimit(1)
                Spacer(minLength: 6)
                Text(value).font(.geist(size: 11)).foregroundStyle(Design.muted)
            }.padding(.horizontal, 7).frame(height: 34)
                .background(hovered ? Design.lifted : .clear, in: RoundedRectangle(cornerRadius: 6))
                .opacity(selected ? 1 : 0.35)
        }
        .buttonStyle(.plain).onHover { hovered = $0 }
        .accessibilityLabel(ModelIdentity.displayName(model))
        .accessibilityValue("\(value), \(selected ? "selected" : "not selected")")
    }
}
private struct ReviewToggleStyle: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View {
        Button { configuration.isOn.toggle() } label: {
            HStack(spacing: 7) {
                ZStack {
                    RoundedRectangle(cornerRadius: 3).fill(configuration.isOn ? Design.ink : Design.surface)
                    RoundedRectangle(cornerRadius: 3).stroke(configuration.isOn ? Design.ink : Design.line)
                    if configuration.isOn { Image(systemName: "checkmark").font(.geist(size: 8, weight: .bold)).foregroundStyle(Design.surface) }
                }.frame(width: 13, height: 13)
                configuration.label.font(.geist(size: 11)).foregroundStyle(Design.muted)
            }
        }.buttonStyle(.plain).accessibilityValue(configuration.isOn ? "On" : "Off")
    }
}
