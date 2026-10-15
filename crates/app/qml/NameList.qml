// `lines` is the model's `details` for these: { name, note, trouble, modId }.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Column {
    id: names
    property string title
    property var lines: []
    // Shown as a button on each line when set; `acted` then gives the
    // line's mod, by library id, or null.
    property string action
    signal acted(var modId)
    property string lead
    visible: lines.length > 0 || lead !== ""
    GroupTitle { text: names.title }
    Text { visible: names.lead !== ""; width: names.width; height: Theme.sized(22); verticalAlignment: Text.AlignVCenter; elide: Text.ElideRight; color: Theme.ink3; font.pixelSize: Theme.body; text: names.lead }
    Repeater {
        model: names.lines
        RowLayout {
            id: entry
            required property var modelData
            readonly property bool trouble: modelData.trouble
            width: names.width
            height: Theme.sized(names.action !== "" ? 28 : 22)
            spacing: Theme.gap
            Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: entry.modelData.name }
            Text { color: entry.trouble ? Theme.err : Theme.ink3; font.pixelSize: Theme.small; text: (entry.trouble ? "! " : "") + entry.modelData.note }
            Btn { visible: names.action !== ""; text: names.action; onClicked: names.acted(entry.modelData.modId) }
        }
    }
}
