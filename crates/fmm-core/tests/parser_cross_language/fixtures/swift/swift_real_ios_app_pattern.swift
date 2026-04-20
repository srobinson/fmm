import UIKit
import Foundation

public protocol DataSourceDelegate: AnyObject {
    func dataDidUpdate()
}

open class TableViewController: UIViewController {
    open func viewDidLoad() {
        super.viewDidLoad()
    }

    open func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
        return 0
    }

    private func setupConstraints() {}
}

public struct CellModel {
    public let title: String
    public let subtitle: String
}

public enum Section: Int {
    case header = 0
    case content
    case footer
}
