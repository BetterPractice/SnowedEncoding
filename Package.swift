// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "Snowed",
    products: [
        .library(name: "Snowed", targets: ["Snowed"]),
    ],
    targets: [
        .target(
            name: "Snowed",
            path: "implementations/swift/Sources/Snowed"
        ),
        .testTarget(
            name: "SnowedTests",
            dependencies: ["Snowed"],
            path: "implementations/swift/Tests/SnowedTests",
            resources: []
        ),
    ]
)
