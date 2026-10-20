import QtQuick
import RimStack.Ui

Item {
    id: keys
    property int count: 0
    property int current: 0
    property ListView view: null
    property string title
    signal chosen(int index)
    signal entered()

    function go(to) {
        if (count === 0)
            return
        const next = Math.max(0, Math.min(count - 1, to))
        chosen(next)
        if (view)
            view.positionViewAtIndex(next, ListView.Contain)
    }

    anchors.fill: parent
    activeFocusOnTab: count > 0
    Accessible.role: Accessible.List
    Accessible.name: title
    Keys.onUpPressed: go(current - 1)
    Keys.onDownPressed: go(current + 1)
    Keys.onPressed: event => {
        const page = view ? Math.max(1, Math.floor(view.height / Theme.row) - 1) : 10
        const to = new Map([[Qt.Key_Home, 0], [Qt.Key_End, count - 1], [Qt.Key_PageUp, current - page], [Qt.Key_PageDown, current + page]]).get(event.key)
        if (to === undefined)
            return
        go(to)
        event.accepted = true
    }
    Keys.onReturnPressed: entered()
    Keys.onEnterPressed: entered()
    TapHandler { onTapped: keys.forceActiveFocus() }
    Rectangle { visible: keys.activeFocus; anchors.fill: parent; anchors.margins: Theme.gapLine; z: 2; radius: Theme.corner; color: "transparent"; border.width: 2; border.color: Theme.focus }
}
