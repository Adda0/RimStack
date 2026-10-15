pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

GridLayout {
    id: line
    property bool error: true
    property string name
    property string words
    // { action, target, label }: what `Mods.applyFix` is given.
    property var fix: ({})
    property int modId: -1
    // The action on a line of its own, under the words.
    property bool stacked: false
    columnSpacing: Theme.gap
    rowSpacing: Theme.gapTiny
    columns: 3 - Number(stacked)
    Icon { Layout.alignment: Qt.AlignTop; Layout.topMargin: Theme.gapHair; name: line.error ? "circle-alert" : "triangle-alert"; size: 14; color: line.error ? Theme.err : Theme.warn }
    Text {
        Layout.fillWidth: true
        wrapMode: Text.WordWrap
        textFormat: Text.StyledText
        color: Theme.ink
        font.pixelSize: Theme.body
        lineHeight: 1.25
        text: (line.name !== "" ? "<b>" + Theme.plain(line.name) + "</b>  " : "") + Theme.plain(line.words)
        HoverHandler { enabled: line.name !== "" && line.modId >= 0; cursorShape: Qt.PointingHandCursor }
        TapHandler { enabled: line.name !== "" && line.modId >= 0; onTapped: Mods.showMod(line.modId) }
    }
    Btn {
        Layout.alignment: Qt.AlignTop
        Layout.row: Number(line.stacked)
        Layout.column: 2 - Number(line.stacked)
        visible: !!line.fix.action && !Hub.mods.previewing
        text: Theme.say(line.fix.label || "")
        onClicked: Mods.applyFix(line.modId, line.fix)
    }
}
