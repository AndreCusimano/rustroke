import CoreGraphics
import Foundation
// Prints the ID of the largest titled, normal-layer window owned by a PID.
// winit also creates untitled helper windows, which are skipped.
let pid = Int32(CommandLine.arguments[1])!
let list = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as! [[String: Any]]
var best: (Int, Double)? = nil
for w in list where (w[kCGWindowOwnerPID as String] as? Int32) == pid && (w[kCGWindowLayer as String] as? Int) == 0 {
    let b = w[kCGWindowBounds as String] as! [String: Double]
    let area = b["Width"]! * b["Height"]!
    let name = w[kCGWindowName as String] as? String ?? ""
    if !name.isEmpty, best == nil || area > best!.1 { best = (w[kCGWindowNumber as String] as! Int, area) }
}
if let b = best { print(b.0) }
