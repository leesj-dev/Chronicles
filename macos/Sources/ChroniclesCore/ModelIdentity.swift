import Foundation

public enum ModelIdentity {
    private static func replacing(_ value: String, pattern: String, transform: ([String]) -> String) -> String {
        let regex = try! NSRegularExpression(pattern: pattern)
        var result = value
        let original = value as NSString
        for match in regex.matches(in: value, range: NSRange(location: 0, length: original.length)).reversed() {
            let groups = (0..<match.numberOfRanges).map { match.range(at: $0).location == NSNotFound ? "" : original.substring(with: match.range(at: $0)) }
            let range = Range(match.range, in: result)!
            result.replaceSubrange(range, with: transform(groups))
        }
        return result
    }
    public static func canonical(_ name: String) -> String {
        var result = name
        for prefix in ["copilot/", "github.copilot-chat/"] where result.hasPrefix(prefix) {
            result.removeFirst(prefix.count)
        }
        result = result.lowercased()
        result = replacing(result, pattern: #"^claude-(\d+)(?:[.-](\d+))?-haiku(?=$|-)"#) {
            "claude-haiku-" + $0[1] + ($0[2].isEmpty ? "" : "." + $0[2])
        }
        return replacing(result, pattern: #"(claude-(?:opus|sonnet|haiku|fable)-\d+)-(\d{1,2})(?=$|-)"#) { $0[1] + "." + $0[2] }
    }
    public static func displayName(_ name: String) -> String {
        var value = canonical(name)
        if value.hasPrefix("claude-") { value.removeFirst(7) }
        if value.hasPrefix("gpt-") {
            value = "GPT-" + value.dropFirst(4).replacingOccurrences(of: "-", with: " ")
            for word in ["astra", "sol", "terra", "luna"] {
                value = replacing(value, pattern: "\\b" + word + "\\b") { _ in word.capitalized }
            }
            return value
        }
        if value.hasPrefix("gemini-") || value.hasPrefix("gemini ") {
            value = "Gemini " + value.dropFirst(7).replacingOccurrences(of: "-", with: " ")
            for word in ["flash", "pro"] {
                value = replacing(value, pattern: "\\b" + word + "\\b") { _ in word.capitalized }
            }
            return value
        }
        for family in ["opus", "sonnet", "haiku", "fable"] where value.hasPrefix(family + "-") {
            value = family.capitalized + " " + value.dropFirst(family.count + 1)
            return replacing(value, pattern: #"(\d+)-(\d+)(?=$|-)"#) { $0[1] + "." + $0[2] }.replacingOccurrences(of: "-", with: " ")
        }
        return name
    }
    /// Preserve the web dashboard's family hues and generation lightness.
    public static func rgb(_ name: String) -> (red: Double, green: Double, blue: Double) {
        let name = canonical(name)
        let regex = try! NSRegularExpression(pattern: #"(?:gpt|claude-opus|claude-sonnet|claude-haiku|opus|sonnet|haiku|gemini)[- ](\d+(?:[.-]\d+)?)"#)
        let match = regex.firstMatch(in: name, range: NSRange(location: 0, length: (name as NSString).length))
        let version = match.map { Double((name as NSString).substring(with: $0.range(at: 1)).replacingOccurrences(of: "-", with: ".")) ?? 0 } ?? 0
        var hue = 220.0, lightness = 55.0
        if name.contains("opus") {
            hue = 275; lightness = version >= 5.5 ? 48 : version >= 5 ? 60 : version >= 4.8 ? 72 : 84
        } else if name.contains("sonnet") {
            hue = 26; lightness = version >= 5.5 ? 48 : version >= 5 ? 60 : version >= 4.5 ? 72 : 84
        } else if name.contains("haiku") {
            hue = 95; lightness = version >= 5.5 ? 48 : version >= 5 ? 60 : version >= 4.5 ? 72 : version >= 3.5 ? 80 : 88
        } else if name.contains("gpt") {
            hue = name.contains("terra") ? 45 : name.contains("luna") ? 145 : name.contains("astra") ? 355 : 215
            lightness = version >= 6.1 ? 43 : version >= 6 ? 51 : version >= 5.6 ? 59 : version >= 5.5 ? 67 : version >= 5.4 ? 75 : 83
        } else if name.contains("gemini") {
            hue = 180; lightness = version >= 3.8 ? 48 : version >= 3.7 ? 60 : version >= 3.1 ? 72 : 84
        } else if name.contains("fable") {
            hue = 325
        } else {
            hue = abs(name.utf16.reduce(0.0) { $0 * 31 + Double($1) }.truncatingRemainder(dividingBy: 360))
        }
        let l = lightness / 100, c = (1 - abs(2 * l - 1)) * 0.7
        let h = hue / 60, x = c * (1 - abs(h.truncatingRemainder(dividingBy: 2) - 1)), m = l - c / 2
        let rgb: (Double, Double, Double)
        switch h {
        case ..<1: rgb = (c, x, 0)
        case ..<2: rgb = (x, c, 0)
        case ..<3: rgb = (0, c, x)
        case ..<4: rgb = (0, x, c)
        case ..<5: rgb = (x, 0, c)
        default: rgb = (c, 0, x)
        }
        return (rgb.0 + m, rgb.1 + m, rgb.2 + m)
    }
}
