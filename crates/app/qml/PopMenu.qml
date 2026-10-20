// Entries: { text, icon, off, danger, run, font }, { line: true } for a divider,
// or { title } for a caption. A menu taller than the window scrolls.
// Entries marked { pick: true } bring a search field that narrows them as one types.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import RimStack.Ui

PopCard {
    // It leaves at once: while a menu fades out the window's shortcuts do not answer.
    exit: null
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
                show()
                return
            }
        }
    }
    // Scrolled just enough to show the entry the keys landed on.
    function show() {
        const shown = repeater.itemAt(at)
        scroll.contentY = Math.max(Math.min(scroll.contentY, shown.y), shown.y + shown.height - scroll.height)
    }
    // Up to `by` entries on, stopping at either end: Home, End and the page keys.
    function move(by) {
        for (let left = Math.abs(by); left > 0; left--) {
            const was = at
            step(Math.sign(by))
            if (was >= 0 && (at - was) * by <= 0) {
                at = was
                break
            }
        }
        show()
    }
    // The next entry that starts with the letter typed.
    function jump(letter) {
        const was = at
        for (let tries = 0; tries < entries.length; tries++) {
            step(1)
            if (at >= 0 && Theme.say(entries[at].text).toLowerCase().startsWith(letter))
                return
        }
        at = was
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
    // Not the base's first control: the search field, or else the entries.
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
    x: Math.max(Theme.gapTiny, Math.min(wanted.x, parent.width - width - Theme.gapTiny))
    y: Math.max(Theme.gapTiny, Math.min(wanted.y, parent.height - height - Theme.gapTiny))
    height: Math.min(implicitHeight, parent.height - 8)
    function openAt(item, x, y, entries) {
        menu.entries = entries
        wanted = item.mapToItem(parent, x, y)
        open()
    }

    parent: Overlay.overlay
    padding: Theme.gapTiny
    contentItem: Item {
        id: body
        readonly property real above: search.visible ? search.height + Theme.gapTiny : 0
        implicitWidth: Math.max(column.implicitWidth, search.visible ? Theme.sized(220) : 0)
        implicitHeight: above + column.implicitHeight
        Keys.onDownPressed: menu.step(1)
        Keys.onUpPressed: menu.step(-1)
        Keys.onReturnPressed: menu.choose()
        Keys.onEnterPressed: menu.choose()
        Keys.onSpacePressed: menu.choose()
        // A letter jumps only without a search field, which takes the letters.
        Keys.onPressed: event => {
            const page = Math.max(1, Math.floor(scroll.height / 26))
            const by = ({ [Qt.Key_Home]: -menu.entries.length, [Qt.Key_End]: menu.entries.length, [Qt.Key_PageUp]: -page, [Qt.Key_PageDown]: page })[event.key] || 0
            const letter = !menu.searching && event.text.length === 1 && event.text > " " ? event.text.toLowerCase() : ""
            event.accepted = by !== 0 || letter !== ""
            if (by !== 0)
                menu.move(by)
            else if (letter !== "")
                menu.jump(letter)
        }
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
                    implicitWidth: line ? 1 : caption ? heading.implicitWidth + 2 * Theme.gapMedium : words.x + words.implicitWidth + Theme.gapWide
                    width: Math.max(implicitWidth, parent.width)
                    height: line ? Theme.gap + Theme.gapLine : caption ? Theme.tag + Theme.gapTiny : Theme.strip
                    Text { id: heading; visible: entry.caption; x: Theme.gapMedium; anchors.verticalCenter: parent.verticalCenter; color: Theme.ink3; font.pixelSize: Theme.caption; font.weight: Font.Medium; text: Theme.say(entry.modelData.title || "") }
                    Rectangle { visible: entry.line; y: Theme.gapTiny; x: Theme.gapTiny; width: parent.width - 2 * Theme.gapTiny; height: 1; color: Theme.line }
                    Rectangle {
                        visible: !entry.line && !entry.caption
                        anchors.fill: parent
                        radius: Theme.corner
                        color: (over.hovered || menu.at === entry.index) && !entry.off ? Theme.accent : "transparent"
                        opacity: entry.off ? 0.4 : 1
                        Accessible.role: Accessible.MenuItem
                        Accessible.name: entry.modelData.text || ""
                        Rectangle { x: Theme.gapMedium; anchors.verticalCenter: parent.verticalCenter; visible: !!entry.modelData.colour; width: Theme.gapMedium; height: width; radius: width / 2; color: entry.modelData.colour || "transparent" }
                        Icon { x: Theme.gap; anchors.verticalCenter: parent.verticalCenter; visible: !!entry.modelData.icon; name: entry.modelData.icon || ""; color: words.color }
                        Text {
                            id: words
                            x: 2 * Theme.gap + Theme.icon
                            anchors.verticalCenter: parent.verticalCenter
                            color: (over.hovered || menu.at === entry.index) && !entry.off ? Theme.accentInk : entry.modelData.danger ? Theme.err : Theme.ink
                            font.pixelSize: Theme.body
                            font.family: entry.modelData.font || Qt.application.font.family
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
