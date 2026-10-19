pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Popup {
    id: comparePopup
    parent: Overlay.overlay
    anchors.centerIn: Overlay.overlay
    enter: PopIn {}
    exit: PopOut {}
    modal: true
    dim: false
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    property string listName: ""
    // Read fresh on each open: the load order changes while the menu is up.
    property var comparison: ({ "onlySaved": [], "onlyCurrent": [], "moved": [], "addable": [] })

    onAboutToShow: comparison = Hub.lists.compareList(listName)

    width: Theme.sized(500)
    height: Math.min(implicitHeight, parent.height - Theme.gapLarge)
    padding: Theme.gapLarge
    background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }

    contentItem: ColumnLayout {
        spacing: Theme.gapLarge

        RowLayout {
            Layout.fillWidth: true
            height: Theme.header
            spacing: Theme.gap

            Text {
                Layout.fillWidth: true
                color: Theme.ink
                font.pixelSize: Theme.title
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                text: Theme.fill("Compare {} with the load order", comparePopup.listName)
            }

            Btn { text: Theme.say("Close"); onClicked: comparePopup.close() }
        }

        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true

            ColumnLayout {
                width: comparePopup.availableWidth
                spacing: Theme.gapSmall

                Text {
                    Layout.fillWidth: true
                    visible: comparePopup.comparison.onlySaved.length === 0 && comparePopup.comparison.onlyCurrent.length === 0 && comparePopup.comparison.moved.length === 0
                    wrapMode: Text.WordWrap
                    color: Theme.ink2
                    font.pixelSize: Theme.body
                    text: Theme.say("The saved list and the load order hold the same mods in the same order")
                }

                Repeater {
                    model: [
                        { heading: Theme.say("Only in the saved list"), mods: comparePopup.comparison.onlySaved },
                        { heading: Theme.say("Only in the load order"), mods: comparePopup.comparison.onlyCurrent },
                        { heading: Theme.say("Placed differently"), mods: comparePopup.comparison.moved }
                    ]

                    ColumnLayout {
                        id: group
                        required property var modelData
                        visible: group.modelData.mods.length > 0
                        Layout.fillWidth: true
                        spacing: Theme.gapSmall

                        GroupTitle { text: group.modelData.heading }
                        Repeater {
                            model: group.modelData.mods
                            Text {
                                id: row
                                required property var modelData
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                                color: Theme.ink
                                font.pixelSize: Theme.body
                                text: row.modelData.name
                            }
                        }
                    }
                }
            }
        }

        Btn {
            Layout.fillWidth: true
            enabled: comparePopup.comparison.addable.length > 0
            text: Theme.plural(comparePopup.comparison.addable.length, "Activate the {} missing mod", "Activate the {} missing mods")
            tip: Theme.fill("Activates the mods {} holds that the load order lacks; nothing else changes, and Save still writes it", comparePopup.listName)
            onClicked: {
                Hub.mods.takeAll(false, comparePopup.comparison.addable)
                comparePopup.comparison = Hub.lists.compareList(comparePopup.listName)
            }
        }
    }
}
