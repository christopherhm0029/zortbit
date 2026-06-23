// Zortbit OCR sidecar — on-device text extraction via Apple Vision.
// Compile: swiftc -O zb_ocr.swift -o ../bin/zb_ocr
// Usage:   zb_ocr <image-path>   → prints recognized text to stdout (exit 0).
// Links only system frameworks (Foundation + Vision); no bundled runtime.

import Foundation
import Vision

guard CommandLine.arguments.count == 2 else { exit(2) }
let url = URL(fileURLWithPath: CommandLine.arguments[1])
guard let data = try? Data(contentsOf: url) else { exit(3) }

let req = VNRecognizeTextRequest()
req.recognitionLevel = .accurate
req.usesLanguageCorrection = true

let handler = VNImageRequestHandler(data: data, options: [:])
do {
    try handler.perform([req])
    let lines = (req.results ?? []).compactMap { $0.topCandidates(1).first?.string }
    print(lines.joined(separator: "\n"))
} catch {
    FileHandle.standardError.write(Data("ocr failed\n".utf8))
    exit(4)
}
