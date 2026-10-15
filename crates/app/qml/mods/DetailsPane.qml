pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import RimStack.Ui

Rectangle {
    color: Theme.panel

    Flickable {
        anchors.fill: parent
        contentHeight: side.implicitHeight + 28
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        Wheel {}
        ScrollBar.vertical: ScrollBar {}

        Column {
            id: side
            x: 14
            y: 14
            width: parent.width - 28

            CompareSummary {}
            SortMoves {}
            ProblemsPanel { id: problems }
            SelectionSummary { visible: Mods.selectedCount > 1 && Mods.comparing === null && !problems.visible }
            Column {
                width: parent.width
                visible: Mods.selectedCount === 1 && Mods.comparing === null && !problems.visible && !Hub.mods.previewing
                ModHeader {}
                AboutTab {}
                LoadOrderTab {}
                FilesTab {}
            }
            ListSummary { visible: Mods.selectedId < 0 && Mods.comparing === null && !problems.visible && !Hub.mods.previewing }
        }
    }
}
