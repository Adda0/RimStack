pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Column {
    id: side
    width: parent.width
    visible: Mods.tab === 2
    GroupTitle { visible: patched.count > 0; text: Theme.say("Patched methods") }
    Repeater {
        id: patched
        model: Mods.info.patches || []
        Column {
            id: aim
            required property var modelData
            width: side.width
            bottomPadding: Theme.gapTiny
            Text { width: parent.width; elide: Text.ElideLeft; color: Theme.ink; font.pixelSize: Theme.body; text: aim.modelData.method }
            Text { visible: text !== ""; width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink3; font.pixelSize: Theme.small; text: aim.modelData.note }
        }
    }
    GroupTitle { text: Theme.say("Folder") }
    PathText { width: parent.width; path: Mods.info.path || ""; copyable: true; size: Theme.body }
    Item { width: 1; height: 8 }
    Repeater {
        model: Mods.files.tree || []
        RowLayout {
            id: branch
            required property var modelData
            width: side.width
            height: Theme.sized(21)
            spacing: Theme.gap
            Text { Layout.fillWidth: true; Layout.maximumWidth: implicitWidth + 1; elide: Text.ElideMiddle; color: Theme.ink; font.family: "monospace"; font.pixelSize: Theme.small; text: branch.modelData.folder }
            Text { color: Theme.ink3; font.pixelSize: Theme.small; text: Theme.plural(branch.modelData.files, "{} file", "{} files") }
            Item { Layout.fillWidth: true }
        }
    }
    GroupTitle { text: Theme.say("Folders the game loads from") }
    Text {
        width: parent.width
        wrapMode: Text.WrapAnywhere
        color: Theme.ink
        font.pixelSize: Theme.body
        lineHeight: 1.3
        text: (Mods.info.loadFolders || []).join("\n") || Theme.say("None")
    }
    Text {
        width: parent.width
        topPadding: Theme.gapSmall
        wrapMode: Text.WordWrap
        color: Theme.ink2
        font.pixelSize: Theme.small
        lineHeight: 1.3
        text: Theme.fill("For RimWorld {} and the active mods", (Mods.overview.gameVersion || ""))
    }
    GroupTitle { visible: hiddenHere.count > 0; text: Theme.say("Textures set aside") }
    Column {
        id: hiddenHere
        readonly property int count: Mods.info && Mods.selectedId >= 0 ? Hub.textures.hiddenIn(Mods.selectedId) : 0
        property string problem
        visible: count > 0
        width: parent.width
        spacing: Theme.gapSmall
        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            color: Theme.ink
            font.pixelSize: Theme.body
            lineHeight: 1.3
            text: Theme.plural(hiddenHere.count, "{} texture set aside: renamed with .rimstack-hidden, so the game skips them.", "{} textures set aside: renamed with .rimstack-hidden, so the game skips them.")
        }
        Btn {
            icon: "eye-off"
            text: Theme.say("Bring back all")
            onClicked: {
                hiddenHere.problem = Hub.textures.unhideMod(Mods.selectedId)
            }
        }
        Text { visible: hiddenHere.problem !== ""; width: parent.width; wrapMode: Text.WordWrap; color: Theme.err; font.pixelSize: Theme.body; text: hiddenHere.problem }
    }
    GroupTitle { visible: contents.count > 0; text: Theme.say("Contents") }
    Repeater {
        id: contents
        model: [
            [Theme.say("Kinds of content"), Mods.info.contains],
            [Theme.say("Assemblies"), Mods.files.assemblies || ""],
            [Theme.say("Textures"), Mods.textures.count || ""],
            [Theme.say("DDS made by RimStack"), Mods.textures.ddsMade],
        ].filter(pair => pair[1])
        Item {
            id: content
            required property var modelData
            width: side.width
            implicitHeight: contentValue.implicitHeight + 4
            Text { id: contentName; width: Theme.sized(150); color: Theme.ink2; font.pixelSize: Theme.body; text: content.modelData[0] }
            Text { id: contentValue; anchors.left: contentName.right; anchors.leftMargin: 8; anchors.right: parent.right; wrapMode: Text.WrapAnywhere; color: Theme.ink; font.pixelSize: Theme.body; text: content.modelData[1] }
        }
    }
}
