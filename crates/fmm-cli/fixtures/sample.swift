// Sample Swift file for fmm parser validation
// Covers: classes, structs, enums, protocols, functions, properties,
// typealiases, extensions, imports, visibility modifiers

import Foundation
import UIKit
@testable import MyTestModule

// MARK: - Public classes

public class NetworkManager {
    public func fetchData(from url: String) -> Data? {
        return nil
    }

    public func cancelAll() {
        // cancel all requests
    }

    private func internalRetry() {
        // private helper
    }

    func defaultAccessMethod() {
        // internal by default
    }
}

open class BaseViewController: UIViewController {
    open func viewDidLoad() {
        super.viewDidLoad()
    }

    open func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
    }
}

// MARK: - Public structs

public struct Point {
    public var x: Double
    public var y: Double

    public init(x: Double, y: Double) {
        self.x = x
        self.y = y
    }
}

public struct APIConfig {
    public let baseURL: String
    public let timeout: TimeInterval
}

// MARK: - Public enums

public enum Direction {
    case north, south, east, west

    public func opposite() -> Direction {
        switch self {
        case .north: return .south
        case .south: return .north
        case .east: return .west
        case .west: return .east
        }
    }
}

public enum NetworkError: Error {
    case timeout
    case unauthorized
    case serverError(code: Int)
}

// MARK: - Protocols

public protocol Drawable {
    func draw(in rect: CGRect)
    var bounds: CGRect { get }
}

public protocol Cacheable {
    associatedtype Key
    func cache(for key: Key) -> Data?
}

// MARK: - Internal/Private items (should NOT be exported)

internal struct InternalConfig {
    let debug: Bool
}

fileprivate func helperFunction() -> Bool {
    return true
}

private func secretFunction() -> String {
    return "secret"
}

struct DefaultAccessStruct {
    var name: String
}

func defaultAccessFunc() -> Int {
    return 42
}

// MARK: - Top-level public declarations

public func createManager(with config: APIConfig) -> NetworkManager {
    return NetworkManager()
}

public let MAX_RETRIES = 3
public var isDebugMode = false

public typealias JSONDictionary = [String: Any]
public typealias CompletionHandler = (Result<Data, Error>) -> Void

// MARK: - Extensions

public extension String {
    func trimmed() -> String {
        return trimmingCharacters(in: .whitespaces)
    }

    var isBlank: Bool {
        return trimmed().isEmpty
    }
}

extension Int {
    func doubled() -> Int {
        return self * 2
    }
}

public extension Array where Element: Equatable {
    func uniqueElements() -> [Element] {
        var result: [Element] = []
        for element in self {
            if !result.contains(element) {
                result.append(element)
            }
        }
        return result
    }
}
