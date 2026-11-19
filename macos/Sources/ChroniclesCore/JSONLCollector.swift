import Foundation

public struct HistoryRoots: Sendable {
    public var codex: [URL]
    public var claude: [URL]
    public init(codex: [URL], claude: [URL]) { self.codex = codex; self.claude = claude }
    public static func standard(environment: [String: String] = ProcessInfo.processInfo.environment) -> HistoryRoots {
        let home = FileManager.default.homeDirectoryForCurrentUser
        let codex = environment["CODEX_HOME"].map { URL(fileURLWithPath: $0) } ?? home.appendingPathComponent(".codex")
        let claude = environment["CLAUDE_CONFIG_DIR"].map { URL(fileURLWithPath: $0) } ?? home.appendingPathComponent(".claude")
        return HistoryRoots(codex: [codex.appendingPathComponent("sessions"), codex.appendingPathComponent("archived_sessions")],
                            claude: [claude.appendingPathComponent("projects")])
    }
}

/// No helper process: reads JSONL directly and caches normalized counters in memory.
public actor JSONLCollector {
    private struct Event: Sendable {
        var key: String
        var provider: String
        var day: UsageDay
        var model: String
        var input: Int, output: Int, cacheRead: Int, cacheWrite: Int
    }
    private struct Cached: Sendable {
        var size: Int, modified: Date, events: [Event], skipped: Int
    }
    private struct Bucket: Hashable { var day: UsageDay; var provider: String; var model: String }
    private var cache: [String: Cached] = [:]
    public init() {}

    public func collect(roots: HistoryRoots = .standard(), timeZone: TimeZone = .current) throws -> UsageReport {
        var unique: [String: Event] = [:]
        var active = Set<String>(), errors = Set<String>(), skipped = 0
        for (provider, roots) in [("codex", roots.codex), ("claude", roots.claude)] {
            for root in roots where FileManager.default.fileExists(atPath: root.path) {
                guard let enumerator = FileManager.default.enumerator(at: root, includingPropertiesForKeys: [.isRegularFileKey, .fileSizeKey, .contentModificationDateKey], errorHandler: { _, _ in
                    errors.insert(provider + ": a history folder could not be read")
                    return true
                }) else {
                    errors.insert(provider + ": history could not be read"); continue
                }
                for case let url as URL in enumerator where url.pathExtension == "jsonl" {
                    try Task.checkCancellation()
                    let key = provider + ":" + timeZone.identifier + ":" + url.resolvingSymlinksInPath().path
                    guard active.insert(key).inserted else { continue }
                    do {
                        let attributes = try url.resourceValues(forKeys: [.isRegularFileKey, .fileSizeKey, .contentModificationDateKey])
                        guard attributes.isRegularFile == true else { continue }
                        let size = attributes.fileSize ?? 0, modified = attributes.contentModificationDate ?? .distantPast
                        if cache[key]?.size != size || cache[key]?.modified != modified {
                            let parsed = try parse(url, provider: provider, timeZone: timeZone)
                            cache[key] = Cached(size: size, modified: modified, events: parsed.events, skipped: parsed.skipped)
                        }
                        guard let cached = cache[key] else { continue }
                        skipped += cached.skipped
                        for event in cached.events {
                            if var existing = unique[event.key] {
                                existing.input = max(existing.input, event.input); existing.output = max(existing.output, event.output)
                                existing.cacheRead = max(existing.cacheRead, event.cacheRead); existing.cacheWrite = max(existing.cacheWrite, event.cacheWrite)
                                unique[event.key] = existing
                            } else { unique[event.key] = event }
                        }
                    } catch is CancellationError { throw CancellationError() }
                    catch { errors.insert(provider + ": history could not be read") }
                }
            }
        }
        cache = cache.filter { active.contains($0.key) }
        var buckets: [Bucket: UsageRow] = [:]
        for event in unique.values {
            let key = Bucket(day: event.day, provider: event.provider, model: event.model)
            var row = buckets[key] ?? UsageRow(date: event.day, provider: event.provider, model: event.model)
            row.requests += 1; row.input += event.input; row.output += event.output
            row.cacheRead += event.cacheRead; row.cacheWrite += event.cacheWrite
            row.total = row.input + row.output + row.cacheRead + row.cacheWrite
            buckets[key] = row
        }
        let rows = buckets.values.sorted {
            if $0.date != $1.date { return $0.date < $1.date }
            if $0.provider != $1.provider { return $0.provider < $1.provider }
            return $0.model < $1.model
        }
        return UsageReport(rows: rows, timezone: timeZone.identifier, files: active.count, skipped: skipped,
                           errors: errors.sorted(), updatedAt: Date().ISO8601Format())
    }

    private func parse(_ url: URL, provider: String, timeZone: TimeZone) throws -> (events: [Event], skipped: Int) {
        let stream = try FileHandle(forReadingFrom: url)
        defer { try? stream.close() }
        var events: [Event] = [], skipped = 0, model = "unknown", previous: [String: Int]?
        let tokenKeys = ["input_tokens", "cached_input_tokens", "cache_write_input_tokens", "output_tokens", "total_tokens"]
        func number(_ value: Any?) -> Int {
            guard let value = value as? NSNumber, CFGetTypeID(value) != CFBooleanGetTypeID(),
                  String(cString: value.objCType) != "d", String(cString: value.objCType) != "f" else { return 0 }
            return max(0, value.intValue)
        }
        func identity(_ parts: [Any]) throws -> String {
            String(decoding: try JSONSerialization.data(withJSONObject: parts, options: [.sortedKeys, .withoutEscapingSlashes]), as: UTF8.self)
        }
        let fractional = ISO8601DateFormatter(); fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let plain = ISO8601DateFormatter()
        func process(_ line: Data) throws {
            try Task.checkCancellation()
            guard !line.isEmpty else { skipped += 1; return }
            guard let record = try? JSONSerialization.jsonObject(with: line) as? [String: Any] else { skipped += 1; return }
            let timestamp = record["timestamp"] as? String ?? ""
            var instant: Date?
            var key: String, name: String, input: Int, output: Int, read: Int, write: Int
            if provider == "claude" {
                guard let message = record["message"] as? [String: Any], let usage = message["usage"] as? [String: Any] else { return }
                instant = fractional.date(from: timestamp) ?? plain.date(from: timestamp)
                guard instant != nil else { return }
                name = message["model"] as? String ?? "unknown"
                guard name != "<synthetic>" else { return }
                if let id = message["id"] as? String, !id.isEmpty {
                    key = try identity(["claude", id, record["requestId"] ?? NSNull()])
                } else { key = try identity(["claude", url.path, record["uuid"] ?? timestamp]) }
                input = number(usage["input_tokens"]); output = number(usage["output_tokens"])
                read = number(usage["cache_read_input_tokens"]); write = number(usage["cache_creation_input_tokens"])
            } else {
                guard let payload = record["payload"] as? [String: Any] else { return }
                if record["type"] as? String == "turn_context" { model = payload["model"] as? String ?? model }
                guard payload["type"] as? String == "token_count", let info = payload["info"] as? [String: Any],
                      let total = info["total_token_usage"] as? [String: Any] else { return }
                instant = fractional.date(from: timestamp) ?? plain.date(from: timestamp)
                guard instant != nil else { return }
                let current = Dictionary(uniqueKeysWithValues: tokenKeys.map { ($0, number(total[$0])) })
                if previous == current { return }
                var usage = (info["last_token_usage"] as? [String: Any]).map { values in
                    Dictionary(uniqueKeysWithValues: tokenKeys.map { ($0, number(values[$0])) })
                } ?? current
                if let previous, current["total_tokens"]! >= previous["total_tokens"]! {
                    usage = Dictionary(uniqueKeysWithValues: tokenKeys.map { ($0, max(0, current[$0]! - previous[$0]!)) })
                }
                previous = current
                name = info["model"] as? String ?? model
                key = try identity(["codex", timestamp, tokenKeys.sorted().map { [$0, current[$0]!] as [Any] }])
                read = usage["cached_input_tokens"]!; write = usage["cache_write_input_tokens"]!
                input = max(0, usage["input_tokens"]! - read - write); output = usage["output_tokens"]!
            }
            if input + output + read + write > 0, let instant {
                events.append(Event(key: key, provider: provider, day: UsageDay(date: instant, timeZone: timeZone),
                                    model: name, input: input, output: output, cacheRead: read, cacheWrite: write))
            }
        }
        // Stream large histories; never retain conversation objects after each line.
        var pending = Data()
        while let chunk = try stream.read(upToCount: 65_536), !chunk.isEmpty {
            pending.append(chunk)
            while let newline = pending.firstIndex(of: 10) {
                try autoreleasepool { try process(Data(pending[..<newline])) }
                pending.removeSubrange(...newline)
            }
        }
        if !pending.isEmpty { try autoreleasepool { try process(pending) } }
        return (events, skipped)
    }
}

import CoreFoundation
