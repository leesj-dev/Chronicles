import CoreText
import SwiftUI

extension Font {
    /// The exact variable Geist font vendored by the web app, with explicit weight values.
    static func geist(size: CGFloat, weight: Font.Weight = .regular) -> Font {
        guard GeistRegistration.ready else { return .system(size: size, weight: weight) }
        let numericWeight: Double
        switch weight {
        case .ultraLight: numericWeight = 100
        case .thin: numericWeight = 200
        case .light: numericWeight = 300
        case .medium: numericWeight = 500
        case .semibold: numericWeight = 600
        case .bold: numericWeight = 700
        case .heavy: numericWeight = 800
        case .black: numericWeight = 900
        default: numericWeight = 400
        }
        let attributes = [kCTFontNameAttribute: "Geist-Regular",
                          kCTFontVariationAttribute: [NSNumber(value: 0x77676874): NSNumber(value: numericWeight)]] as CFDictionary
        let descriptor = CTFontDescriptorCreateWithAttributes(attributes)
        return Font(CTFontCreateWithFontDescriptor(descriptor, size, nil))
    }
}
private enum GeistRegistration {
    static let ready: Bool = {
        guard let url = Bundle.main.url(forResource: "Geist", withExtension: "ttf") else { return false }
        return CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
    }()
}
