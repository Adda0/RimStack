pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import RimStack.Ui

PopCard {
    id: prompt
    property string title
    default property alias body: content.data
    property alias buttons: actions.data
    readonly property alias scroll: scroll
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Theme.sized(400)
    padding: Theme.gapWide
    dim: true
    // The main action, last in the row, takes Enter. A field focused once opened takes the keys over.
    onAboutToShow: actions.children[actions.children.length - 1].forceActiveFocus()
    contentItem: Column {
        id: column
        // A popup takes no Accessible of its own: its content stands for it.
        Accessible.role: Accessible.Dialog
        Accessible.name: prompt.title
        spacing: Theme.gapLarge
        // A body taller than the window scrolls between the title and the buttons, by the keys too.
        Keys.onPressed: event => {
            const line = Theme.sized(40), page = Math.max(line, scroll.height - line)
            const by = ({ [Qt.Key_Up]: -line, [Qt.Key_Down]: line, [Qt.Key_PageUp]: -page, [Qt.Key_PageDown]: page, [Qt.Key_Home]: -scroll.contentHeight, [Qt.Key_End]: scroll.contentHeight })[event.key] || 0
            event.accepted = by !== 0 && scroll.contentHeight > scroll.height
            if (event.accepted)
                scroll.contentY = Math.max(0, Math.min(scroll.contentHeight - scroll.height, scroll.contentY + by))
        }
        Text { id: heading; width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: prompt.title }
        Flickable {
            id: scroll
            // Into the right padding, where the scrollbar goes.
            width: parent.width + prompt.rightPadding
            height: Math.min(content.height, Math.max(0, (prompt.parent?.height ?? 0) - 2 * Theme.gapWide - prompt.topPadding - prompt.bottomPadding - heading.height - actions.height - 2 * column.spacing))
            contentHeight: content.height
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            Wheel {}
            ScrollBar.vertical: Scroller {}
            Column { id: content; width: column.width; spacing: Theme.gap }
        }
        Row { id: actions; anchors.right: parent.right; spacing: Theme.gap }
    }
}
