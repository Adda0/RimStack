pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

GridLayout {
    id: line
    property bool error: true
    property string name
    property string words
    // Shown under the words, one a line: the dlls, files or folders the problem is about.
    property var items: []
    readonly property int most: 8
    property bool open: false
    // { action, target, label }: what `Mods.applyFix` is given.
    property var fix: ({})
    property int modId: -1
    // Empty when it cannot be ignored; with the mod's package id, what `Hub.mods.ignoreProblem` is given.
    property string ignore
    property string packageId
    // The action on a line of its own, under the words.
    property bool stacked: false
    // The problem as text to paste into a report.
    readonly property string said: Mods.problemText({ name: line.name, packageId: line.packageId, words: line.words, items: line.items })
    columnSpacing: Theme.gap
    rowSpacing: Theme.gapTiny
    columns: 4 - Number(stacked)
    Icon { Layout.alignment: Qt.AlignTop; Layout.topMargin: Theme.gapHair; name: line.error ? "circle-alert" : "triangle-alert"; size: 14; color: line.error ? Theme.err : Theme.warn }
    CopyText {
        id: lineText
        readonly property bool linked: line.name !== "" && line.modId >= 0
        readonly property string named: line.name === "" ? "" : "<b>" + Theme.plain(line.name) + "</b>&nbsp; "
        readonly property var shown: line.open ? line.items : line.items.slice(0, line.most)
        readonly property string style: "<style>a { color: " + Theme.ink + "; text-decoration: none; } ul { margin-top: 2px; margin-bottom: 0px; }</style>"
        Layout.fillWidth: true
        textFormat: TextEdit.RichText
        Layout.maximumWidth: implicitWidth + 1
        whole: line.said
        text: style + (linked ? "<a href=\"mod\">" + named + "</a>" : named) + Theme.plain(line.words)
            + (shown.length === 0 ? "" : "<ul>" + shown.map(item => "<li>" + Theme.plain(item) + "</li>").join("") + "</ul>")
        onLinkActivated: Mods.showMod(line.modId)
    }
    Btn {
        visible: line.items.length > line.most
        Layout.row: 1 + Number(line.stacked)
        Layout.column: 1
        text: line.open ? Theme.fill("Show {}", line.most) : Theme.fill("{} more", (line.items.length - line.most))
        onClicked: line.open = !line.open
    }
    Row {
        Layout.alignment: Qt.AlignTop
        Layout.row: Number(line.stacked)
        Layout.column: 2 - Number(line.stacked)
        spacing: Theme.gapSmall
        visible: !Hub.mods.previewing
        Btn {
            visible: !!line.fix.action
            text: Theme.say(line.fix.label || "")
            onClicked: Mods.applyFix(line.modId, line.fix)
        }
        Btn {
            visible: !!line.ignore
            compact: true
            icon: "eye-off"
            spoken: Theme.say("Ignore this problem")
            tip: Theme.say("No warning about this problem again; the mod's details can bring it back")
            onClicked: {
                // The line goes with the problem; the undo keeps its own copy.
                const [packageId, key] = [line.packageId, line.ignore]
                Hub.mods.ignoreProblem(packageId, key, true)
                Notices.offer(Theme.say("The problem is ignored"), Theme.say("Undo"), () => Hub.mods.ignoreProblem(packageId, key, false))
            }
        }
    }
    Item { Layout.row: 0; Layout.column: 3 - Number(line.stacked); Layout.fillWidth: true }
}
