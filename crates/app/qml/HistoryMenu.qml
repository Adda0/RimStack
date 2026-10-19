pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Popup {
    enter: PopIn {}
    exit: PopOut {}
    id: historyMenu
    modal: true
    dim: false
    signal useList(var ids)
    readonly property var entries: Hub.lists.history || []
    function revert(entry) {
        if (entry.restore !== "") {
            const ids = Hub.lists.snapshotNamed(entry.restore)
            if (ids)
                historyMenu.useList(ids)
        } else if (entry.bringBack !== "") {
            const problem = Hub.textures.unhideFolder(entry.bringBack)
            Notices.say(problem !== "" ? problem : Theme.say("Textures brought back"), problem !== "" ? "err" : "ok")
        }
        close()
    }

    width: Theme.sized(440)
    height: Math.min(Theme.sized(520), implicitHeight)
    padding: Theme.gapLarge
    background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }
    contentItem: Flickable {
        implicitHeight: lines.implicitHeight
        contentHeight: lines.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        Wheel {}
        Column {
            id: lines
            width: parent.width
            spacing: Theme.gapSmall
            Text { color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: Theme.say("What RimStack changed") }
            Text { visible: historyMenu.entries.length === 0; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("Nothing yet") }
            Repeater {
                model: historyMenu.entries
                RowLayout {
                    id: entry
                    required property var modelData
                    width: lines.width
                    spacing: Theme.gap
                    Text { Layout.preferredWidth: Theme.sized(112); color: Theme.ink3; font.pixelSize: Theme.small; text: Theme.moment(entry.modelData.time) }
                    Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; text: entry.modelData.said }
                    Btn {
                        visible: entry.modelData.restore !== "" || entry.modelData.bringBack !== ""
                        text: entry.modelData.restore !== "" ? Theme.say("Restore") : Theme.say("Bring back")
                        spoken: Theme.fill("Restore the list from before: {}", entry.modelData.said)
                        tip: entry.modelData.restore !== "" ? Theme.say("The load order as it was just before. Nothing is written before Save.") : Theme.say("Renames the files back inside the mod")
                        onClicked: historyMenu.revert(entry.modelData)
                    }
                }
            }
            Text { visible: Notices.earlier.length > 0; topPadding: Theme.gap; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: Theme.say("Said since the start") }
            Repeater {
                model: Notices.earlier
                RowLayout {
                    id: said
                    required property var modelData
                    width: lines.width
                    spacing: Theme.gap
                    Text { Layout.preferredWidth: Theme.sized(92); color: Theme.ink3; font.pixelSize: Theme.small; text: Theme.clock(said.modelData.time / 1000) }
                    Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: said.modelData.tone === "err" ? Theme.err : Theme.ink2; font.pixelSize: Theme.body; text: said.modelData.text }
                }
            }
        }
    }
}
