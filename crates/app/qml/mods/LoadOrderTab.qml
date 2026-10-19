pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Column {
    id: side
    width: parent.width
    visible: Mods.tab === 1
    GroupTitle { text: Theme.say("Position") }
    Text {
        width: parent.width
        wrapMode: Text.WordWrap
        color: Theme.ink
        font.pixelSize: Theme.body
        text: Mods.selectedActive ? Theme.fill(Mods.info.category ? "{} of {}, in {}" : "{} of {}", Mods.active.indexOf(Mods.selectedId) + 1, Mods.active.count, Theme.say(Mods.info.category || "")) : Theme.say("Not active")
    }
    NameList { width: parent.width; title: Theme.say("Requires"); lines: Mods.info.requires || [] }
    NameList { width: parent.width; title: Theme.say("Needed by"); lines: Mods.info.neededBy || [] }
    Btn {
        visible: (Mods.info.offWith || []).length > 0
        text: Theme.plural((Mods.info.offWith || []).length - 1, "Switch off, with the {} mod needing it", "Switch off, with the {} mods needing it")
        onClicked: Hub.mods.takeAll(true, Mods.info.offWith)
    }
    GroupTitle { visible: Mods.selectedActive; text: Theme.say("Own rules") }
    Repeater {
        model: Mods.selectedActive ? (Mods.info.yourRules || []) : []
        RowLayout {
            id: own
            required property var modelData
            width: side.width
            height: Theme.sized(26)
            spacing: Theme.gap
            Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: own.modelData.said }
            Btn {
                icon: "x"
                tip: Theme.say("Remove this rule")
                onClicked: Hub.mods.removeRule(own.modelData.first, own.modelData.then)
            }
        }
    }
    Text {
        visible: Mods.selectedActive && Mods.picking !== ""
        width: parent.width
        bottomPadding: Theme.gapSmall
        wrapMode: Text.WordWrap
        color: Theme.moved
        font.pixelSize: Theme.body
        lineHeight: 1.3
        text: Theme.fill("Click the mod this one loads {} (Esc cancels)", Mods.picking)
    }
    Flow {
        visible: Mods.selectedActive
        width: parent.width
        spacing: Theme.gapSmall
        Btn { visible: Mods.picking === ""; icon: "plus"; text: Theme.say("Load after…"); onClicked: Mods.picking = "after" }
        Btn { visible: Mods.picking === ""; icon: "plus"; text: Theme.say("Load before…"); onClicked: Mods.picking = "before" }
        Btn { visible: Mods.picking !== ""; text: Theme.say("Cancel"); onClicked: Mods.picking = "" }
    }
    NameList {
        readonly property int repeated: Mods.info.requiredAbove || 0
        width: parent.width
        title: Theme.say("Loads after")
        lead: repeated > 0 ? Theme.plural(repeated, "{} required, listed above", "{} required, listed above") : ""
        lines: Mods.info.loadsAfter || []
    }
    NameList { width: parent.width; title: Theme.say("Loads before"); lines: Mods.info.loadsBefore || [] }
    GroupTitle { visible: detected.count > 0; text: Theme.say("Detected from files") }
    Repeater {
        id: detected
        model: Mods.info.detected || []
        Column {
            id: found
            required property var modelData
            width: side.width
            bottomPadding: Theme.gapSmall
            Text { width: parent.width; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: Theme.fill("After {}", found.modelData.name) }
            Text {
                width: parent.width
                wrapMode: Text.WrapAnywhere
                color: Theme.ink3
                font.pixelSize: Theme.small
                text: found.modelData.code ? found.modelData.why : Theme.fill("{}  (a hint; the game does not need this order)", found.modelData.why)
            }
        }
    }
    GroupTitle { visible: !!Mods.info.placement; text: Theme.say("Placement") }
    Text { visible: !!Mods.info.placement; width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; lineHeight: 1.3; text: Mods.info.placement || "" }
}
