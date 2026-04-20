import SwiftUI

public protocol Theme {
    var primaryColor: Color { get }
    var font: Font { get }
}

public struct AppTheme: Theme {
    public var primaryColor: Color
    public var font: Font
}

public extension View {
    func themed(with theme: Theme) -> some View {
        return self
    }
}

public typealias ViewBuilder = () -> AnyView

public let defaultTheme = AppTheme(primaryColor: .blue, font: .body)
