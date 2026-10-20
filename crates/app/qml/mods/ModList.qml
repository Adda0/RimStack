pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

ListView {
    id: list
    property bool columns: false
    readonly property real barRoom: bar.room
    readonly property bool wide: columns && width >= Theme.sized(620)
    readonly property bool roomy: columns && width >= Theme.sized(440)
    function roomBeside(shown, author) {
        const taken = shown.reduce((sum, column) => sum + (column === "author" ? author : Mods.columns.widths[column]) + 12, 0)
        return width - barRoom - Theme.sized(wide ? 124 : 44) - 90 - taken
    }
    readonly property int nameRoom: Theme.sized(180)
    readonly property int authorWidth: roomBeside(roomy ? Mods.columns.shown : [], Mods.columns.widths.author) >= nameRoom ? Mods.columns.widths.author : Theme.sized(86)
    readonly property var fitting: {
        let shown = roomy ? Mods.columns.shown.slice() : []
        for (const column of ["load", "author", "version", "game"]) {
            if (roomBeside(shown, authorWidth) >= nameRoom)
                break
            shown = shown.filter(other => other !== column)
        }
        return shown
    }
    function columnWidth(column) {
        return column === "author" ? authorWidth : Mods.columns.widths[column]
    }

    clip: true
    reuseItems: true
    acceptedButtons: Qt.NoButton
    boundsBehavior: Flickable.StopAtBounds
    Wheel {}
    ScrollBar.vertical: ListBar { id: bar; rightPadding: Theme.gapLine + ring.border.width + Theme.gapHair }
    readonly property alias bar: bar
    delegate: ModRow {}
    footer: Item {
        id: end
        readonly property var listView: ListView.view
        width: listView ? listView.width : 0
        height: Theme.sized(30)
        DropArea {
            anchors.fill: parent
            onEntered: Mods.drag.target = { model: end.listView.model, modIndex: end.listView.model.endPlace, item: end }
            onExited: if (Mods.drag.target && Mods.drag.target.item === end) Mods.drag.target = null
            Rectangle { visible: parent.containsDrag; x: 6; width: parent.width - 12; height: 2; radius: Math.min(width, height) / 2; color: Theme.focus }
        }
    }
    activeFocusOnTab: true
    Accessible.role: Accessible.List
    Keys.onPressed: event => event.accepted = Mods.listKey(list, event.key, event.modifiers)
    Rectangle { id: ring; parent: list; anchors.fill: parent; anchors.margins: Theme.gapLine; z: 4; radius: Theme.corner; color: "transparent"; border.width: 2; border.color: Theme.focus; visible: list.activeFocus }

    property int tick: 0
    // The selected mod's row, or the folded heading over it.
    readonly property int place: tick >= 0 && Mods.selectedActive === model.active ? Hub.mods.placeOf(model.active, Mods.selectedId) : -1
    readonly property int topCategory: {
        const drawn = tick
        if (drawn < 0 || count === 0 || !model.at(0).isHeader)
            return -1
        const top = indexAt(10, contentY + 1)
        // Right after a filter shrinks the rows, a delegate past the end can still be found here.
        if (top < 0 || top >= model.rows())
            return -1
        const row = model.at(top)
        return row.isHeader && !row.isSub ? -1 : row.category
    }
    Rectangle {
        parent: list
        z: 3
        visible: list.topCategory >= 0
        y: {
            const drawn = list.tick
            const next = list.itemAt(10, list.contentY + height)
            return next && next.isHeader && !next.isSub && !next.again ? Math.min(0, next.y - list.contentY - height) : 0
        }
        width: list.width - list.barRoom
        height: Theme.control
        color: list.columns ? Theme.win : Theme.panel
        Rectangle { x: 2; width: 3; height: parent.height; color: Theme.categoryColours[list.topCategory] || Theme.ink3; opacity: 0.6 }
        Row {
            x: 32
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.gapSmall
            Icon { name: Theme.categoryIcons[list.topCategory] || ""; color: Theme.categoryColours[list.topCategory] || Theme.ink2; size: 14; anchors.verticalCenter: parent.verticalCenter }
            Text { color: Theme.ink; font.pixelSize: Theme.small; font.weight: Font.DemiBold; text: Theme.say(Theme.categoryNames[list.topCategory] || "") }
        }
        Rectangle { anchors.bottom: parent.bottom; x: 6; width: parent.width - 12; height: 1; color: Theme.line }
        TapHandler { acceptedButtons: Qt.RightButton; onTapped: point => Mods.categoryMenu(parent, point.position.x, point.position.y, list.topCategory) }
    }

    property real kept: 0
    // Rust lays the model out anew after a change; the list keeps its place over that reset.
    property real keptOverReset: 0
    Connections {
        target: list.model
        function onModelAboutToBeReset() { list.keptOverReset = list.contentY }
        function onModelReset() {
            const at = list.keptOverReset
            Qt.callLater(() => {
                list.contentY = at
                list.returnToBounds()
            })
        }
    }
    Connections {
        target: list.model
        function onRowsInserted() { list.tick++ }
        function onRowsRemoved() { list.tick++ }
        function onRowsMoved() { list.tick++ }
        function onDataChanged() { list.tick++ }
        function onModelReset() { list.tick++ }
    }
    Connections {
        target: Mods
        function onReveal(model, row, centre) {
            if (model !== list.model)
                return
            list.positionViewAtIndex(row, centre ? ListView.Center : ListView.Contain)
            const shown = list.itemAtIndex(row)
            if (shown && list.topCategory >= 0 && shown.y - list.contentY < Theme.control)
                list.contentY = Math.max(-list.topMargin, shown.y - Theme.control)
        }
        function onFocusList(model) {
            if (model === list.model)
                list.forceActiveFocus()
        }
        function onRowWanted(run) {
            const row = list.visible ? list.place : -1
            Mods.reveal(list.model, row, false)
            const shown = list.itemAtIndex(row)
            if (shown)
                run(shown)
        }
    }

    Timer {
        interval: 16
        repeat: true
        running: Mods.drag.held !== null
        onTriggered: {
            const at = list.mapFromItem(null, Mods.drag.point.x, Mods.drag.point.y)
            if (at.x < 0 || at.x > list.width || at.y < -40 || at.y > list.height + 40)
                return
            const edge = 40
            let pull = 0
            if (at.y < edge)
                pull = -(edge - Math.max(at.y, 0)) / edge
            else if (at.y > list.height - edge)
                pull = (Math.min(at.y, list.height) - (list.height - edge)) / edge
            if (pull === 0)
                return
            const top = -list.topMargin
            const bottom = Math.max(top, list.contentHeight - list.height + list.bottomMargin)
            list.contentY = Math.max(top, Math.min(bottom, list.contentY + pull * 22))
        }
    }
}
