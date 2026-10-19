// Looking for text inside the files of installed mods; the search runs on its own thread.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Popup {
    enter: PopIn {}
    exit: PopOut {}
    modal: true
    focus: true
    signal showMod(int modId)
    id: popup
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Theme.sized(760)
    padding: Theme.gapWide
    onOpened: box.focusInput()
    onClosed: Hub.find.stop()
    background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }
    Overlay.modal: Rectangle { color: Theme.scrim }

    contentItem: Column {
        spacing: Theme.gap
        Text { color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: Theme.say("Search inside the mods' files") }
        RowLayout {
            width: parent.width
            spacing: Theme.gap
            FilterField {
                id: box
                Layout.fillWidth: true
                placeholder: Theme.say("Text to look for")
                onAccepted: Hub.find.go(box.text)
            }
            Btn { primary: true; text: Theme.say("Search"); enabled: !Hub.find.busy && box.typing.length > 0; onClicked: Hub.find.go(box.text) }
            Btn { visible: Hub.find.busy; text: Theme.say("Stop"); onClicked: Hub.find.stop() }
        }
        Row {
            spacing: Theme.gap
            Choice { title: Theme.say("Search inside"); options: [Theme.say("Active mods"), Theme.say("All mods")]; current: Number(Hub.find.allMods); onPicked: index => Hub.find.allMods = index === 1 }
            Chip { pressable: true; onAccent: Hub.find.xmlOnly; text: Theme.say("XML files only"); tip: Theme.say("Otherwise every text file is read"); onClicked: Hub.find.xmlOnly = !Hub.find.xmlOnly }
            Chip { pressable: true; onAccent: Hub.find.matchCase; text: Theme.say("Match case"); onClicked: Hub.find.matchCase = !Hub.find.matchCase }
            Chip { pressable: true; onAccent: Hub.find.regexMode; text: Theme.say("Regular expression"); onClicked: Hub.find.regexMode = !Hub.find.regexMode }
        }
        Text {
            width: parent.width
            visible: Hub.find.progress.length > 0 || Hub.find.summary.length > 0
            color: Theme.ink2
            font.pixelSize: Theme.small
            text: Hub.find.busy ? Hub.find.progress : Hub.find.summary
        }
        ListView {
            id: hits
            width: parent.width
            height: Math.min(contentHeight, 500)
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            Wheel {}
            model: Hub.find.hits
            delegate: Item {
                id: hit
                required property var modelData
                width: hits.width
                height: lines.implicitHeight + 6
                Rectangle { anchors.fill: parent; radius: Theme.corner; color: Theme.hover; opacity: hover.hovered ? 0.05 : 0 }
                HoverHandler { id: hover }
                Column {
                    id: lines
                    width: parent.width
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 2
                    RowLayout {
                        width: parent.width
                        spacing: Theme.gap
                        Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: hit.modelData.modName + " — " + hit.modelData.file + ":" + hit.modelData.line }
                        Btn { compact: true; icon: "folder-open"; tip: Theme.say("Open folder"); onClicked: Qt.openUrlExternally(Theme.fileUrl(hit.modelData.folder)) }
                        Btn { compact: true; icon: "search"; tip: Theme.say("Show the mod"); onClicked: { popup.close(); popup.showMod(hit.modelData.modId) } }
                    }
                    Text { width: parent.width; elide: Text.ElideRight; color: Theme.ink3; font.pixelSize: Theme.small; font.family: "monospace"; text: hit.modelData.text }
                }
                TapHandler { gesturePolicy: TapHandler.ReleaseWithinBounds; onTapped: { popup.close(); popup.showMod(hit.modelData.modId) } }
            }
        }
    }
}
