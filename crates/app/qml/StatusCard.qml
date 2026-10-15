import QtQuick
import RimStack.Ui

// The cards of a row share one height (`minHeight`), so their state chips sit on the
// title line and their actions on the bottom edge, level across the row.
Rectangle {
    id: card
    property string title
    property string note
    property real minHeight: 0
    default property alias body: content.data
    property alias status: statusRow.data
    property alias actions: actionRow.data
    readonly property real natural: column.y + column.implicitHeight + Theme.gap + actionRow.implicitHeight + Theme.gapCard
    implicitHeight: Math.max(natural, minHeight)
    radius: Theme.cornerLarge
    color: Theme.panel
    border.width: 1
    border.color: Theme.line

    Column {
        id: column
        x: Theme.gapCard
        y: Theme.gapCard
        width: parent.width - 2 * Theme.gapCard
        spacing: Theme.gap
        Item {
            id: head
            width: parent.width
            height: Math.max(name.implicitHeight, statusRow.height)
            Text { id: name; anchors.verticalCenter: parent.verticalCenter; width: Math.min(implicitWidth, head.width - statusRow.width - Theme.gap); elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; font.weight: Font.DemiBold; text: card.title; Accessible.role: Accessible.Heading }
            Row { id: statusRow; anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; spacing: Theme.gapSmall }
        }
        Text { visible: card.note !== ""; width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink3; font.pixelSize: Theme.small; text: card.note }
        Column { id: content; width: parent.width; spacing: Theme.gap }
    }
    Row {
        id: actionRow
        x: Theme.gapCard
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.gapCard
        spacing: Theme.gap
    }
}
