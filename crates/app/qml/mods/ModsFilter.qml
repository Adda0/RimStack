pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

FilterField {
    id: filter
    z: 5
    icon: "funnel"
    keys: Theme.say("Ctrl + F")
    chips: true
    placeholder: Theme.say("Filter active and inactive mods")
    found: Mods.filtered === "" ? "" : Theme.fill("{} active, {} inactive", Theme.count(Mods.active.matching), Theme.count(Mods.inactive.matching))
    Rectangle {
        id: suggestions
        visible: offers.count > 0 && (filter.editing || suggestHover.hovered)
        z: 5
        y: parent.height + 4
        width: Math.min(parent.width, 420)
        height: suggestList.height + 12
        radius: Theme.cornerLarge
        color: Theme.bar
        border.width: 1
        border.color: Theme.line
        HoverHandler { id: suggestHover }
        // Qt hands a press taken here to the pointer handlers of the rows beneath
        // too; holding it alone keeps them from answering.
        TapHandler { gesturePolicy: TapHandler.WithinBounds; grabPermissions: PointerHandler.TakeOverForbidden }
        Column {
            id: suggestList
            x: 6
            y: 6
            width: parent.width - 12
            Repeater {
                id: offers
                model: [
                    ["is:error", Theme.plural(Hub.mods.errors, "{} mod", "{} mods")],
                    ["is:warning", Theme.plural(Hub.mods.warnings, "{} mod", "{} mods")],
                    ["is:conflict", Theme.plural(Hub.mods.conflicts, "{} mod sharing textures", "{} mods sharing textures")],
                    ["is:changed", Theme.say("moved or activated since the last save")],
                    ["is:new", Theme.say("installed since the game was last started")],
                    ["is:updated", Hub.downloads.behindActive > 0 ? Theme.fill("{} with an update", Hub.downloads.behindActive) : Theme.say("after checking for updates")],
                    ["is:local", Theme.say("not from the Workshop")],
                    ["is:notinsave", Theme.say("active, and not in the newest save")],
                    ["is:recent", Theme.say("updated by Steam in the last week")],
                    ["is:duplicate", Theme.say("installed more than once")],
                    ["has:code", Theme.say("ships compiled code")],
                    ["patches:", Theme.say("patches this method of the game")],
                    ["tag:", Theme.say("has this tag")],
                    ["author:", Theme.say("by this author")],
                ].filter(offer => offer[0].startsWith(filter.partial) && offer[0] !== filter.partial && filter.terms.indexOf(offer[0]) < 0)
                Rectangle {
                    id: offer
                    required property var modelData
                    width: suggestList.width
                    height: Theme.sized(26)
                    radius: Theme.corner
                    color: offerHover.hovered ? Theme.chip : "transparent"
                    Accessible.role: Accessible.Button
                    Accessible.name: Theme.fill("Filter by {}", modelData[0])
                    Text { x: 8; anchors.verticalCenter: parent.verticalCenter; color: Theme.ink; font.family: "monospace"; font.pixelSize: Theme.small; text: offer.modelData[0] }
                    Text { x: 110; anchors.verticalCenter: parent.verticalCenter; color: Theme.ink2; font.pixelSize: Theme.small; text: offer.modelData[1] }
                    HoverHandler { id: offerHover }
                    TapHandler {
                        onTapped: {
                            filter.take(offer.modelData[0])
                            filter.focusInput()
                        }
                    }
                }
            }
        }
    }
}
