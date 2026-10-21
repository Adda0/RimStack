pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

RowLayout {
    id: holder
    property string path
    property color tone: Theme.ink
    property bool copyable: false
    property string openTip
    property string openIcon: "folder-open"
    property real size: Theme.small
    spacing: Theme.gapSmall
    Layout.fillWidth: true
    Layout.minimumWidth: 0

    Text {
        id: line
        Layout.fillWidth: true
        Layout.maximumWidth: implicitWidth + 1
        Layout.minimumWidth: 0
        elide: Text.ElideMiddle
        maximumLineCount: 1
        color: holder.tone
        font.family: "monospace"
        font.pixelSize: holder.size
        text: holder.path
        Accessible.role: Accessible.StaticText
        Accessible.name: holder.path
        Tip { text: line.truncated ? holder.path : "" }
    }
    Btn { visible: holder.copyable && holder.path !== ""; compact: true; icon: "copy"; tip: Theme.say("Copy path"); onClicked: Theme.copy(holder.path) }
    Btn { visible: holder.openTip !== ""; enabled: holder.path !== ""; compact: true; icon: holder.openIcon; tip: holder.openTip; onClicked: Qt.openUrlExternally(Theme.fileUrl(holder.path)) }
    Item { Layout.fillWidth: true }
}
