pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Column {
    id: side
    width: parent.width
    visible: Mods.tab === 3
    readonly property var needing: Mods.info.neededBy || []
    Flow {
        visible: side.needing.length > 0
        width: parent.width
        topPadding: Theme.gapLarge
        spacing: Theme.gapTiny
        Repeater {
            model: side.needing
            Chip {
                required property var modelData
                pressable: true
                maxWidth: side.width
                text: modelData.name
                tip: Theme.say("Select this mod")
                onClicked: Mods.select(modelData.modId, true)
            }
        }
    }
    Text {
        visible: side.needing.length === 0
        width: parent.width
        topPadding: Theme.gapLarge
        wrapMode: Text.WordWrap
        color: Theme.ink2
        font.pixelSize: Theme.body
        text: Theme.say("Nothing needs it.")
    }
}
