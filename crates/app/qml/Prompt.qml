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
    readonly property alias scroll: content
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
        // A body taller than the window scrolls between the title and the buttons, by the keys too.
        Keys.onPressed: event => {
            const line = Theme.sized(40), page = Math.max(line, content.height - line)
            const by = ({ [Qt.Key_Up]: -line, [Qt.Key_Down]: line, [Qt.Key_PageUp]: -page, [Qt.Key_PageDown]: page, [Qt.Key_Home]: -content.contentHeight, [Qt.Key_End]: content.contentHeight })[event.key] || 0
            event.accepted = by !== 0 && content.contentHeight > content.height
            if (event.accepted)
                content.contentY = Math.max(0, Math.min(content.contentHeight - content.height, content.contentY + by))
        }
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
