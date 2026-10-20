pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

PopCard {
    id: prompt
    property string title
    default property alias body: content.body
    property alias buttons: actions.data
    property real wanted: Theme.sized(400)
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(parent.width - 2 * Theme.gapPage, prompt.wanted)
    height: Math.min(parent.height - 2 * Theme.gapPage, implicitHeight)
    padding: Theme.gapWide
    dim: true
    // The main action, last in the row, takes Enter. A field focused once opened takes the keys over.
    onAboutToShow: actions.children[actions.children.length - 1].forceActiveFocus()
    contentItem: ColumnLayout {
        spacing: Theme.gapLarge
        Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: prompt.title }
        Scrolled {
            id: content
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredHeight: contentHeight
            spacing: Theme.gap
        }
        Row { id: actions; Layout.alignment: Qt.AlignRight; spacing: Theme.gap }
    }
}
