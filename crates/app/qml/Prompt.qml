pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import RimStack.Ui

Popup {
    id: prompt
    property string title
    default property alias body: content.data
    property alias buttons: actions.data
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Theme.sized(400)
    padding: Theme.gapWide
    modal: true
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }
    enter: PopIn {}
    exit: PopOut {}
    contentItem: Column {
        spacing: Theme.gapLarge
        Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: prompt.title }
        Column { id: content; width: parent.width; spacing: Theme.gap }
        Row { id: actions; anchors.right: parent.right; spacing: Theme.gap }
    }
}
