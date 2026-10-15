pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Column {
    id: cmpSummary
    width: parent.width
    visible: Mods.comparing !== null
    readonly property var cmp: Mods.compareView
    readonly property var report: Hub.textures.comparison || ({})
    readonly property var names: report.earlier ? [report.earlier.name, report.later.name] : ["", ""]
    Text { color: Theme.ink; font.pixelSize: Theme.heading; font.weight: Font.DemiBold; text: Theme.say("Shared textures") }
    Text { width: parent.width; topPadding: Theme.gapHair; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.fill("{} and {}", cmpSummary.names[0], cmpSummary.names[1]) }

    GroupTitle { text: Theme.say("So far") }
    Repeater {
        model: cmpSummary.report.earlier ? [
            [Theme.fill("{} kept", cmpSummary.names[1]), cmpSummary.report.total - cmpSummary.report.keptEarlier],
            [Theme.fill("{} kept", cmpSummary.names[0]), cmpSummary.report.keptEarlier],
            [Theme.say("Chosen by hand"), Theme.fill("{} of {}", cmpSummary.report.yours, cmpSummary.report.total)],
        ] : []
        RowLayout {
            id: tally
            required property var modelData
            width: cmpSummary.width
            height: Theme.sized(24)
            Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: tally.modelData[0] }
            Text { color: Theme.ink2; font.pixelSize: Theme.body; text: tally.modelData[1] }
        }
    }

    GroupTitle { text: Theme.say("All at once") }
    Flow {
        width: parent.width
        spacing: Theme.gapSmall
        Btn { text: Theme.fill("Keep all from {}", cmpSummary.names[1]); onClicked: cmpSummary.cmp.keepAll(false) }
        Btn {
            text: Theme.fill("Keep all from {}", cmpSummary.names[0])
            // Keeping the earlier mod's versions renames files in the other mod.
            tip: cmpSummary.report.earlier ? Theme.plural(cmpSummary.report.total, "Sets aside {} file in {} by renaming it; Undo names it back.", "Sets aside {} files in {} by renaming them; Undo names them back.", cmpSummary.names[1]) : ""
            onClicked: {
                const later = cmpSummary.report.later.id
                const done = cmpSummary.cmp.keepAll(true)
                if (done.said)
                    Notices.offer(done.said, Theme.say("Undo"), () => Hub.textures.unhideMod(later))
            }
        }
    }

    GroupTitle { text: Theme.say("Default") }
    Text {
        width: parent.width
        wrapMode: Text.WordWrap
        color: Theme.ink
        font.pixelSize: Theme.body
        text: Theme.fill("{} wins: it loads later.", cmpSummary.names[1])
        Tip { text: Theme.say("A choice against the load order sets the losing file aside: it is renamed inside its mod, with .rimstack-hidden added, so the game skips it. Nothing is deleted; choosing the other version renames it back. Steam restores renamed files when it updates a mod.") }
    }
}
