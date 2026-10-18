pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import RimStack.Ui

FilterField {
    id: filter
    z: 5
    icon: "funnel"
    keys: Theme.say("Ctrl + F")
    chips: true
    placeholder: Theme.say("Filter active and inactive mods")
    found: Mods.filtered === "" ? "" : Theme.fill("{} active, {} inactive", Theme.count(Mods.active.matching), Theme.count(Mods.inactive.matching))
    readonly property alias suggesting: suggestions.visible
    onTypingChanged: {
        suggestions.shut = false
        suggestions.chosen = 0
    }
    onEditingChanged: suggestions.shut = false
    onEscaped: suggestions.shut = true
    // Enter takes the offer chosen with the arrows; without offers it is the field's own.
    onAccepted: suggesting && filter.take(offers.itemAt(suggestions.chosen).modelData.term)
    Keys.enabled: suggesting
    Keys.onUpPressed: suggestions.chosen = Math.max(suggestions.chosen - 1, 0)
    Keys.onDownPressed: suggestions.chosen = Math.min(suggestions.chosen + 1, offers.count - 1)
    Popup {
        id: suggestions
        property bool shut: false
        property int chosen: 0
        visible: offers.count > 0 && filter.editing && filter.visible && !shut
        onClosed: shut = true
        closePolicy: Popup.CloseOnPressOutsideParent
        y: filter.height + 4
        width: Math.min(filter.width, 420)
        padding: 6
        background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }
        contentItem: Column {
            id: suggestList
            Repeater {
                id: offers
                // Terms, or for author:, tag: and patches: the values the mods have.
                model: Hub.mods.filterOffers(filter.partial, filter.bang, filter.terms, Hub.downloads.behindActive)
                Rectangle {
                    id: offer
                    required property var modelData
                    required property int index
                    width: suggestList.width
                    height: Theme.sized(26)
                    radius: Theme.corner
                    color: offerHover.hovered || offer.index === suggestions.chosen ? Theme.chip : "transparent"
                    Accessible.role: Accessible.Button
                    Accessible.name: Theme.fill("Filter by {}", offer.modelData.term)
                    Row {
                        x: 8
                        width: parent.width - 16
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: Theme.gap
                        Text { id: termText; width: Math.min(implicitWidth, parent.width * 0.6); elide: Text.ElideRight; color: Theme.ink; font.family: "monospace"; font.pixelSize: Theme.small; text: offer.modelData.term }
                        Text { width: parent.width - termText.width - Theme.gap; elide: Text.ElideRight; color: Theme.ink2; font.pixelSize: Theme.small; text: offer.modelData.note }
                    }
                    HoverHandler { id: offerHover }
                    TapHandler {
                        onTapped: {
                            filter.take(offer.modelData.term)
                            filter.focusInput()
                        }
                    }
                }
            }
        }
    }
}
