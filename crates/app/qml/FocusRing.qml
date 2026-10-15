import QtQuick
import RimStack.Ui

Rectangle {
    property Item of: parent
    property real corner: Theme.corner
    // On a selected row the focus colour is 1.0 to 2.4:1 to the accent.
    property bool onAccent: false
    visible: of.activeFocus
    anchors.fill: parent
    anchors.margins: -4
    radius: corner + 4
    color: "transparent"
    border.width: 2
    border.color: onAccent ? Theme.onSelected : Theme.focus
}
