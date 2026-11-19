// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "ChroniclesMac",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "Chronicles", targets: ["ChroniclesApp"]),
        .executable(name: "chronicles-inspect", targets: ["ChroniclesInspect"]),
        .library(name: "ChroniclesCore", targets: ["ChroniclesCore"]),
        .executable(name: "chronicles-core-checks", targets: ["ChroniclesCoreChecks"]),
    ],
    targets: [
        .target(name: "ChroniclesCore"),
        .executableTarget(name: "ChroniclesApp", dependencies: ["ChroniclesCore"]),
        .executableTarget(name: "ChroniclesInspect", dependencies: ["ChroniclesCore"]),
        .executableTarget(name: "ChroniclesCoreChecks", dependencies: ["ChroniclesCore"], path: "Tests/ChroniclesCoreTests"),
    ]
)
