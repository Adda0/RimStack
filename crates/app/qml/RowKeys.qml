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
        if (event.key !== Qt.Key_Home && event.key !== Qt.Key_End)
            return
        go(event.key === Qt.Key_Home ? 0 : count - 1)
        event.accepted = true
    }
    Keys.onReturnPressed: entered()
    Keys.onEnterPressed: entered()
    TapHandler { onTapped: keys.forceActiveFocus() }
    Rectangle { visible: keys.activeFocus; anchors.fill: parent; anchors.margins: Theme.gapLine; z: 2; radius: Theme.corner; color: "transparent"; border.width: 2; border.color: Theme.focus }
}
