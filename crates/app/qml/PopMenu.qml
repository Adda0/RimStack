// Entries: { text, icon, off, danger, run }, { line: true } for a divider,
// or { title } for a caption. A menu taller than the window scrolls.
// Entries marked { pick: true } bring a search field that narrows them as one types.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import RimStack.Ui

Popup {
    // It leaves at once: while a menu fades out the window's shortcuts do not answer.
    enter: PopIn {}
    id: menu
    property var entries: []
    // The entry the arrow keys are on; -1 for none.
    property int at: -1
    // The entries' column, for the checks to find an entry in.
    readonly property alias column: column
    readonly property alias search: search
    readonly property alias scroller: scroll
    readonly property bool searching: entries.some(entry => !!entry.pick)
    readonly property string query: search.typing.trim().toLowerCase()
    function hidden(entry) {
        return !!entry.pick && query !== "" && !entry.text.toLowerCase().includes(query)
    }
    function step(by) {
        let next = at
        for (let tries = 0; tries < entries.length; tries++) {
            next = (next + by + entries.length) % entries.length
            const entry = entries[next]
            if (!entry.line && !entry.title && !entry.off && !hidden(entry)) {
                at = next
                // Scrolled just enough to show the entry the keys landed on.
                const shown = repeater.itemAt(next)
                scroll.contentY = Math.max(Math.min(scroll.contentY, shown.y), shown.y + shown.height - scroll.height)
                return
            }
        }
    }
    // Enter takes the entry the keys are on, or else the first one the search left.
    function choose() {
        if (at < 0 || hidden(entries[at]))
            at = entries.findIndex(entry => entry.pick && !hidden(entry))
        if (at < 0)
            return
        const chosen = entries[at]
        close()
        chosen.run()
    }
    onQueryChanged: at = -1
    // On show, not once faded in: keys typed straight after opening reach the menu.
    onAboutToShow: {
        at = -1
        scroll.contentY = 0
        search.text = ""
        if (searching)
            search.focusInput()
        else
            contentItem.forceActiveFocus()
    }

    // Clamped to the window here: the entries are not laid out yet when openAt runs.
    property point wanted
    x: Math.max(4, Math.min(wanted.x, parent.width - width - 4))
    y: Math.max(4, Math.min(wanted.y, parent.height - height - 4))
    height: Math.min(implicitHeight, parent.height - 8)
    function openAt(item, x, y, entries) {
        menu.entries = entries
        wanted = item.mapToItem(parent, x, y)
        open()
    }

    parent: Overlay.overlay
    // Modal without the dimming: a click on an entry or beside the menu reaches nothing under it.
    modal: true
    dim: false
    padding: Theme.gapTiny
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }
    contentItem: Item {
        id: body
        readonly property real above: search.visible ? search.height + Theme.gapTiny : 0
        implicitWidth: Math.max(column.implicitWidth, search.visible ? Theme.sized(220) : 0)
        implicitHeight: above + column.implicitHeight
        Keys.onDownPressed: menu.step(1)
        Keys.onUpPressed: menu.step(-1)
        Keys.onReturnPressed: menu.choose()
        FilterField {
            id: search
            visible: menu.searching
            width: body.width
            icon: "search"
            placeholder: Theme.say("Search")
            onAccepted: menu.choose()
            onEscaped: menu.close()
        }
        Flickable {
            id: scroll
            y: body.above
            width: body.width
            height: body.height - body.above
            contentHeight: column.height
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: Scroller {}
        Column {
            id: column
            width: scroll.width
            Repeater {
                id: repeater
                model: menu.entries
                Item {
                    id: entry
                    required property var modelData
                    required property int index
                    readonly property bool line: !!modelData.line
                    visible: !menu.hidden(modelData)
                    readonly property bool off: !!modelData.off
                    readonly property bool caption: !!modelData.title
                    implicitWidth: line ? 1 : caption ? heading.implicitWidth + 20 : words.implicitWidth + 46
                    width: Math.max(implicitWidth, parent.width)
                    height: line ? 9 : caption ? 22 : 26
                    Text { id: heading; visible: entry.caption; x: 9; anchors.verticalCenter: parent.verticalCenter; color: Theme.ink3; font.pixelSize: Theme.caption; font.weight: Font.Medium; text: Theme.say(entry.modelData.title || "") }
                    Rectangle { visible: entry.line; y: 4; x: 4; width: parent.width - 8; height: 1; color: Theme.line }
                    Rectangle {
                        visible: !entry.line && !entry.caption
                        anchors.fill: parent
                        radius: Theme.corner
                        color: (over.hovered || menu.at === entry.index) && !entry.off ? Theme.accent : "transparent"
                        opacity: entry.off ? 0.4 : 1
                        Accessible.role: Accessible.MenuItem
                        Accessible.name: entry.modelData.text || ""
                        Rectangle { x: 10; anchors.verticalCenter: parent.verticalCenter; visible: !!entry.modelData.colour; width: 10; height: 10; radius: 5; color: entry.modelData.colour || "transparent" }
                        Icon { x: 8; anchors.verticalCenter: parent.verticalCenter; visible: !!entry.modelData.icon; name: entry.modelData.icon || ""; size: 14; color: words.color }
                        Text {
                            id: words
                            x: 30
                            anchors.verticalCenter: parent.verticalCenter
                            color: (over.hovered || menu.at === entry.index) && !entry.off ? Theme.accentInk : entry.modelData.danger ? Theme.err : Theme.ink
                            font.pixelSize: Theme.body
                            text: Theme.say(entry.modelData.text || "")
                        }
                        HoverHandler { id: over }
                        TapHandler {
                            enabled: !entry.off
                            // Grabbed at the press: a passive grab lets the press reach a row under the menu too.
                            gesturePolicy: TapHandler.ReleaseWithinBounds
                            onTapped: {
                                menu.close()
                                entry.modelData.run()
                            }
                        }
                    }
                }
            }
        }
        }
    }
}
