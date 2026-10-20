pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Prompt {
    id: sheet
    width: Theme.sized(620)
    title: Theme.say("Keyboard")
    readonly property var groups: [
        [Theme.say("Everywhere"), [[Theme.say("Ctrl + K"), Theme.say("Search mods, commands and settings")], [Theme.say("Ctrl + 1 to 6"), Theme.say("Mods, Downloads, Textures, Saves, Logs, Settings")], ["?", Theme.say("This page")], [Theme.say("Left, Right"), Theme.say("Move a choice or the section")], [Theme.say("Tab"), Theme.say("Next control")], [Theme.say("Home, End, Page Up, Page Down"), Theme.say("To either end of a list, or a page further")], [Theme.say("Esc"), Theme.say("Close, cancel, clear the selection")]]],
        [Theme.say("Load order"), [
            [Theme.say("Ctrl + S"), Theme.say("Save")],
            [Theme.say("Ctrl + Z"), Theme.say("Undo")],
            [Theme.say("Ctrl + Shift + Z"), Theme.say("Redo")],
            [Theme.say("Ctrl + F"), Theme.say("Filter the load order")],
            [Theme.say("Down, Enter"), Theme.say("From the filter to the list")],
            [Theme.say("Esc"), Theme.say("From an empty filter, tag box or note back to the list")],
            [Theme.say("F6, Shift + F6"), Theme.say("To the next or the previous pane")],
            [Theme.say("Ctrl + Page Up, Ctrl + Page Down"), Theme.say("The previous or the next tab of the details")]]],
        [Theme.say("A list of mods"), [
            [Theme.say("Up, Down"), Theme.say("Select the mod above or below")],
            [Theme.say("Enter"), Theme.say("Activate or deactivate the selected mods")],
            [Theme.say("Alt + Up, Alt + Down"), Theme.say("Move the selected mods one place")],
            [Theme.say("Alt + Home, Alt + End"), Theme.say("To the top or the bottom of their category")],
            [Theme.say("Shift + click, Shift + Up, Shift + Down"), Theme.say("Select through")],
            [Theme.say("Ctrl + Up, Ctrl + Down, Ctrl + Home, Ctrl + End"), Theme.say("Go on without selecting")],
            [Theme.say("Ctrl + click, Ctrl + Space"), Theme.say("Add or remove one from the selection")],
            [Theme.say("Shift + Home, Shift + End, Shift + Page Up, Shift + Page Down"), Theme.say("Select through to an end or a page further")],
            [Theme.say("Ctrl + A"), Theme.say("Select every mod shown")],
            ["T", Theme.say("Type its tags")],
            ["N", Theme.say("Type its note")],
            [Theme.say("Left, Right"), Theme.say("Fold the heading the mod is under, or show its mods again")],
            [Theme.say("Menu, Shift + F10"), Theme.say("The mod's menu")]]],
        [Theme.say("Texture comparison"), [[Theme.say("Up, Down"), Theme.say("Previous or next texture")], ["1, 2", Theme.say("Keep the first or the second mod's")], [Theme.say("Esc"), Theme.say("Back to the lists")]]],
        [Theme.say("Dialogs and menus"), [[Theme.say("Enter, Space"), Theme.say("Press the focused button or menu entry; a dialog opens on its main one")], [Theme.say("Up, Down"), Theme.say("Move through a menu")], [Theme.say("Home, End, Page Up, Page Down"), Theme.say("The ends or a page on, in a menu, a choice or a slider")], [Theme.say("A letter"), Theme.say("The next menu entry starting with it")], [Theme.say("Ctrl + C"), Theme.say("Copy a text, or the log's marked line")]]],
    ]
    GridLayout {
        width: parent.width
        columns: 2
        columnSpacing: 24
        rowSpacing: Theme.gapCard
        Repeater {
            model: sheet.groups
            Column {
                id: group
                required property var modelData
                Layout.alignment: Qt.AlignTop
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                Layout.minimumWidth: 0
                spacing: Theme.gapThin
                Text { bottomPadding: Theme.gapThin; color: Theme.ink2; font.pixelSize: Theme.small; font.weight: Font.DemiBold; text: Theme.say(group.modelData[0]) }
                Repeater {
                    model: group.modelData[1]
                    Row {
                        id: key
                        required property var modelData
                        spacing: Theme.gap
                        Flow {
                            width: Theme.sized(150)
                            spacing: Theme.gapTiny
                            Accessible.role: Accessible.StaticText
                            Accessible.name: key.modelData[0]
                            Repeater {
                                model: key.modelData[0].split(", ")
                                Row {
                                    id: combo
                                    required property string modelData
                                    required property int index
                                    spacing: Theme.gapTiny
                                    Text { visible: combo.index > 0; height: Theme.sized(20); verticalAlignment: Text.AlignVCenter; color: Theme.ink3; font.pixelSize: Theme.small; text: "," }
                                    Repeater {
                                        model: combo.modelData.split(" + ")
                                        KeyCap { required property string modelData; text: modelData }
                                    }
                                }
                            }
                        }
                        Text { width: group.width - Theme.sized(150) - Theme.gap; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.small; text: Theme.say(key.modelData[1]) }
                    }
                }
            }
        }
    }
    buttons: [ Btn { primary: true; text: Theme.say("Close"); onClicked: sheet.close() } ]
}
