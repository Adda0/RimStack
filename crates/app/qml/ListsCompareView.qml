pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

PopCard {
    id: comparePopup
    parent: Overlay.overlay
    anchors.centerIn: Overlay.overlay

    // Keys as Hub.lists.compareOrders takes them: a saved list's against the load order unless picked.
    property string left: ""
    property string right: "current"
    // Read fresh on each open: the load order changes while the menu is up.
    property var comparison: ({ left: "", right: "", onlyLeft: { heading: "", mods: [] }, onlyRight: { heading: "", mods: [] }, moved: { heading: "", mods: [] }, same: false, addable: [], report: "" })

    function refresh() {
        comparison = Hub.lists.compareOrders(left, right)
    }
    function withLoadOrder(key) {
        left = key
        right = "current"
        open()
    }
    // `side` is "left" or "right": the menu of every order puts the one picked there.
    function choose(button, side) {
        const entries = Hub.lists.orderChoices().map(choice => Object.assign({ run: () => { comparePopup[side] = choice.key; comparePopup.refresh() } }, choice))
        sideMenu.openAt(button, 0, button.height, entries)
    }

    onAboutToShow: refresh()

    width: Theme.sized(540)
    height: Math.min(implicitHeight, parent.height - Theme.gapLarge)
    padding: Theme.gapLarge

    contentItem: ColumnLayout {
        spacing: Theme.gapLarge

        RowLayout {
            Layout.fillWidth: true
            height: Theme.header
            spacing: Theme.gap

            Text {
                color: Theme.ink
                font.pixelSize: Theme.title
                font.weight: Font.DemiBold
                text: Theme.say("Compare")
            }
            Btn {
                icon: "copy"
                compact: true
                spoken: Theme.say("Copy the comparison")
                tip: Theme.say("Copies each group as text, a line per mod with its package id")
                onClicked: Theme.copy(comparePopup.comparison.report)
            }
            Item { Layout.fillWidth: true }
            Btn { text: Theme.say("Close"); onClicked: comparePopup.close() }
        }

        Flow {
            Layout.fillWidth: true
            spacing: Theme.gapSmall

            Btn {
                id: leftPick
                menu: true
                text: comparePopup.comparison.left
                tip: Theme.say("The order compared: a saved list, an earlier order, the game's file or the load order")
                onClicked: comparePopup.choose(leftPick, "left")
            }
            Text { height: leftPick.height; verticalAlignment: Text.AlignVCenter; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("with") }
            Btn {
                id: rightPick
                menu: true
                text: comparePopup.comparison.right
                tip: Theme.say("The order it is compared with")
                onClicked: comparePopup.choose(rightPick, "right")
            }
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
                    visible: comparePopup.comparison.same
                    wrapMode: Text.WordWrap
                    color: Theme.ink2
                    font.pixelSize: Theme.body
                    text: Theme.say("Both hold the same mods in the same order")
                }

                Repeater {
                    model: [comparePopup.comparison.onlyLeft, comparePopup.comparison.onlyRight, comparePopup.comparison.moved]

                    ColumnLayout {
                        id: group
                        required property var modelData
                        visible: group.modelData.mods.length > 0
                        Layout.fillWidth: true
                        spacing: Theme.gapSmall

                        GroupTitle { objectName: "compareHeading"; text: group.modelData.heading }
                        Repeater {
                            model: group.modelData.mods
                            Row {
                                id: row
                                required property var modelData
                                Layout.fillWidth: true
                                spacing: Theme.gap
                                Text {
                                    id: modName
                                    width: Math.min(implicitWidth, row.width - said.implicitWidth - row.spacing)
                                    elide: Text.ElideRight
                                    color: Theme.ink
                                    font.pixelSize: Theme.body
                                    text: row.modelData.name
                                    Tip { text: row.modelData.id }
                                }
                                Text { id: said; anchors.baseline: modName.baseline; color: Theme.ink3; font.pixelSize: Theme.small; text: row.modelData.said }
                            }
                        }
                    }
                }
            }
        }

        Btn {
            Layout.fillWidth: true
            visible: comparePopup.comparison.addable.length > 0
            text: Theme.plural(comparePopup.comparison.addable.length, "Activate the {} missing mod", "Activate the {} missing mods")
            tip: Theme.fill("Activates the mods {} holds that the load order lacks; nothing else changes, and Save still writes it", comparePopup.comparison.left)
            onClicked: {
                Hub.mods.takeAll(false, comparePopup.comparison.addable)
                comparePopup.refresh()
            }
        }
    }

    PopMenu { id: sideMenu }
}
