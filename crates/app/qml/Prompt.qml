pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import RimStack.Ui

PopCard {
    id: prompt
    property string title
    default property alias body: content.data
    property alias buttons: actions.data
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Theme.sized(400)
    padding: Theme.gapWide
    dim: true
    // The main action, last in the row, takes Enter. A field focused once opened takes the keys over.
    onAboutToShow: actions.children[actions.children.length - 1].forceActiveFocus()
    contentItem: Column {
        spacing: Theme.gapLarge
        Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: prompt.title }
        Column { id: content; width: parent.width; spacing: Theme.gap }
        Row { id: actions; anchors.right: parent.right; spacing: Theme.gap }
    }
}
