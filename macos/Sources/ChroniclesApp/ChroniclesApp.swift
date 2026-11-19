import AppKit
import ChroniclesCore
import SwiftUI

@main struct ChroniclesApp: App {
    @State private var store = UsageStore()
    var body: some Scene {
        WindowGroup("Chronicles") {
            DashboardView(store: store)
                .frame(minWidth: 900, minHeight: 620)
                .task { if store.report == nil { store.scanLocal() } }
        }
        .defaultSize(width: 1280, height: 840)
        .windowStyle(.hiddenTitleBar)
        .commands {
            CommandGroup(after: .newItem) {
                Button("Open Usage Report…") { store.openSnapshot() }.keyboardShortcut("o")
                    .disabled(store.loading)
                Button("Export Selection as CSV…") { store.exportCSV() }.keyboardShortcut("e", modifiers: [.command, .shift])
                    .disabled(store.report == nil || store.loading)
            }
            CommandMenu("History") {
                Button("Scan Local History") { store.scanLocal() }.disabled(store.loading)
                Button("Refresh") { store.reload() }.keyboardShortcut("r").disabled(store.loading)
                Divider()
                Button("Show Fictional Demo") { store.startDemo() }.disabled(store.loading)
            }
        }
    }
}

func modelColor(_ model: String) -> Color {
    let rgb = ModelIdentity.rgb(model)
    return Color(red: rgb.red, green: rgb.green, blue: rgb.blue)
}
func providerName(_ key: String) -> String {
    ["codex": "Codex", "claude": "Claude Code", "copilot": "Copilot", "antigravity": "Antigravity", "cursor": "Cursor", "grok": "Grok", "opencode": "OpenCode"][key] ?? key.capitalized
}
