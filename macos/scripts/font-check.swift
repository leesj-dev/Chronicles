import CoreText
import Foundation

let path = CommandLine.arguments.dropFirst().first ?? "macos/build/Chronicles.app/Contents/Resources/Geist.ttf"
let url = URL(fileURLWithPath: path)
precondition(CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil), "Bundled Geist registration failed")
for weight in [400, 500, 600, 700] {
    let attributes = [kCTFontNameAttribute: "Geist-Regular",
                      kCTFontVariationAttribute: [NSNumber(value: 0x77676874): NSNumber(value: weight)]] as CFDictionary
    let font = CTFontCreateWithFontDescriptor(CTFontDescriptorCreateWithAttributes(attributes), 12, nil)
    precondition(CTFontCopyFamilyName(font) as String == "Geist", "Unexpected fallback font")
    let variation = CTFontCopyVariation(font) as? [NSNumber: NSNumber]
    let axes = CTFontCopyVariationAxes(font) as? [[CFString: Any]]
    let defaultWeight = axes?.first?[kCTFontVariationAxisDefaultValueKey] as? NSNumber
    // CoreText omits an axis from the variation dictionary when it equals its default.
    let actualWeight = variation?[NSNumber(value: 0x77676874)] ?? defaultWeight
    precondition(actualWeight?.intValue == weight, "Variable weight not applied")
}
print("Bundled Geist verified: native CoreText registration and weights 400/500/600/700.")
