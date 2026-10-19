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
    // A click into the field offers again what a click away put away; focus alone does not change.
    onTapped: suggestions.shut = false
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
        width: Math.min(filter.width, Theme.sized(420))
        padding: 6
        background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }
        FontMetrics { id: keyFont; font.family: "monospace"; font.pixelSize: Theme.small }
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
                    Text { id: offerKey; width: keyFont.advanceWidth("is:notinsave"); anchors.left: parent.left; anchors.leftMargin: 8; anchors.verticalCenter: parent.verticalCenter; color: Theme.ink; font: keyFont.font; text: offer.modelData.term }
                    Text { anchors.left: offerKey.right; anchors.leftMargin: Theme.sized(16); anchors.right: parent.right; anchors.rightMargin: 8; anchors.verticalCenter: parent.verticalCenter; elide: Text.ElideRight; color: Theme.ink2; font.pixelSize: Theme.small; text: offer.modelData.note }
                    HoverHandler { id: offerHover }
                    TapHandler {
                        gesturePolicy: TapHandler.ReleaseWithinBounds
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
