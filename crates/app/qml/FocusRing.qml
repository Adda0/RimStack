import QtQuick
import RimStack.Ui

Rectangle {
    property Item of: parent
    property real corner: Theme.corner
    // On a selected row the focus colour is 1.0 to 2.4:1 to the accent.
    property bool onAccent: false
    visible: of.activeFocus
    // A control given the focus inside a page that scrolls is scrolled into view, just enough.
    onVisibleChanged: {
        let flick = visible ? of.parent : null
        while (flick && flick.contentY === undefined)
            flick = flick.parent
        if (flick) {
            const top = of.mapToItem(flick.contentItem, 0, -8).y, bottom = top + of.height + 16
            flick.contentY = Math.max(flick.originY, Math.min(Math.max(Math.min(flick.contentY, top), bottom - flick.height), flick.originY + flick.contentHeight - flick.height))
        }
    }
    anchors.fill: parent
    anchors.margins: -4
    radius: corner + 4
    color: "transparent"
    border.width: 2
    border.color: onAccent ? Theme.onSelected : Theme.focus
}
