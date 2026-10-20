pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts
import RimStack.Ui

Popup {
    enter: PopIn {}
    exit: PopOut {}
    id: listsMenu
    modal: true
    dim: false

    signal useList(var ids)
    // The load order now goes by this name; "" when it has none.
    signal named(string name)

    readonly property var lists: Hub.lists.saved || []
    readonly property var snapshots: Hub.lists.snapshots || []
    property string problem

    width: Theme.sized(380)
    padding: Theme.gapLarge
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutsideParent
    onAboutToShow: problem = ""
    background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }

    contentItem: Column {
        spacing: Theme.gapSmall

        Text { color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: Theme.say("Saved lists") }
        Text {
            visible: listsMenu.lists.length === 0
            width: parent.width
            color: Theme.ink2
            font.pixelSize: Theme.body
            text: Theme.say("None")
        }
        Repeater {
            model: listsMenu.lists
            RowLayout {
                id: saved
                required property var modelData
                width: listsMenu.availableWidth
                height: Theme.sized(28)
                spacing: Theme.gapSmall
                Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: saved.modelData.name }
                Text { visible: !!saved.modelData.settings; color: Theme.ink3; font.pixelSize: Theme.small; text: Theme.say("settings kept"); Tip { text: Theme.say("The mods' settings are kept with this list; using it offers them") } }
                Text { color: Theme.ink3; font.pixelSize: Theme.small; text: Theme.plural(saved.modelData.mods, "{} mod", "{} mods") }
                Btn {
                    compact: Mods.prefs.listName !== saved.modelData.name
                    icon: "save"
                    text: Theme.say("Update")
                    tip: Theme.fill("Keeps the load order as it is now under {}, in place of what it held", saved.modelData.name)
                    onClicked: {
                        listsMenu.problem = Hub.lists.keep(saved.modelData.name, Mods.prefs.listWithSettings)
                        if (listsMenu.problem === "")
                            listsMenu.named(saved.modelData.name)
                    }
                }
                Btn {
                    text: Theme.say("Use")
                    onClicked: {
                        if (saved.modelData.settings) {
                            useSettingsBox.take(saved.modelData.name)
                        } else {
                            listsMenu.useSaved(saved.modelData.name)
                        }
                    }
                }
                Btn {
                    text: Theme.say("Compare")
                    tip: Theme.fill("Shows the mods {} and the load order hold apart or in a different order, and merges the missing ones", saved.modelData.name)
                    onClicked: {
                        compareView.listName = saved.modelData.name
                        compareView.open()
                    }
                }
                Btn {
                    icon: "trash-2"
                    tip: Theme.fill("Delete {}", saved.modelData.name)
                    onClicked: Hub.lists.remove(saved.modelData.name)
                }
            }
        }

        Rectangle { width: parent.width; height: 1; color: Theme.line }
        RowLayout {
            width: parent.width
            spacing: Theme.gapSmall
            FilterField {
                id: name
                Layout.fillWidth: true
                placeholder: Theme.say("Name for the current list")
                onAccepted: keep.clicked()
            }
            Btn {
                id: keep
                enabled: name.text.trim() !== ""
                icon: "save"
                text: Theme.say("Keep")
                onClicked: {
                    listsMenu.problem = Hub.lists.keep(name.text, keepSettings.on)
                    if (listsMenu.problem === "") {
                        listsMenu.named(name.text.trim())
                        name.text = ""
                    }
                }
            }
        }
        Row {
            width: parent.width
            spacing: Theme.gapSmall
            Toggle { silent: true; id: keepSettings; anchors.verticalCenter: parent.verticalCenter; on: Mods.prefs.listWithSettings; onFlipped: Mods.prefs.listWithSettings = !keepSettings.on }
            Text { anchors.verticalCenter: parent.verticalCenter; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("Keep the mods' settings too") }
        }
        Text { visible: listsMenu.problem !== ""; width: parent.width; wrapMode: Text.WordWrap; color: Theme.err; font.pixelSize: Theme.body; text: listsMenu.problem }

        Text { visible: listsMenu.snapshots.length > 0; topPadding: Theme.gapSmall; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: Theme.say("Earlier") }
        Repeater {
            model: listsMenu.snapshots
            RowLayout {
                id: earlier
                required property var modelData
                width: listsMenu.availableWidth
                height: Theme.sized(28)
                spacing: Theme.gapSmall
                Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink2; font.pixelSize: Theme.body; text: earlier.modelData.title }
                Btn {
                    text: Theme.say("Use")
                    onClicked: {
                        const ids = Hub.lists.snapshotNamed(earlier.modelData.name)
                        if (ids) {
                            listsMenu.useList(ids)
                            listsMenu.named("")
                        }
                        listsMenu.close()
                    }
                }
            }
        }

        ListsCompareView { id: compareView }

        Btn {
            icon: "history"
            text: Theme.say("History…")
            tip: Theme.say("Every order saved or written by the game or another tool, with a way to use one again or keep it as a saved list")
            onClicked: ordersView.open()
        }
        ListsHistory {
            id: ordersView
            onUseList: ids => {
                listsMenu.useList(ids)
                listsMenu.named("")
                listsMenu.close()
            }
        }

        Text { topPadding: Theme.gapSmall; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: Theme.say("Clipboard") }
        Flow {
            width: parent.width
            spacing: Theme.gapSmall
            Btn { icon: "copy"; text: Theme.say("Copy the load order"); tip: Theme.say("The active mods as text, one a line"); onClicked: { Theme.copy(Hub.mods.listing()); listsMenu.close() } }
            Btn {
                icon: "copy"
                text: Theme.say("Copy the share code")
                tip: Theme.say("The load order as a short code; this clipboard read into a friend's RimStack gives the same list, with the Workshop pages of its mods")
                onClicked: { Theme.copy(Hub.mods.shareCode()); listsMenu.close() }
            }
            Btn {
                text: Theme.say("Use the copied list")
                tip: Theme.say("A list of mods copied as text, links or a share code becomes the load order. Save writes it.")
                onClicked: {
                    const ids = Hub.lists.idsInText(Theme.pasted())
                    if (ids.length > 0) {
                        listsMenu.useList(ids)
                        listsMenu.named("")
                        listsMenu.close()
                    } else {
                        listsMenu.problem = Theme.say("No mod found on the clipboard")
                    }
                }
            }
        }

        Text { topPadding: Theme.gapSmall; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: Theme.say("Files") }
        Flow {
            width: parent.width
            spacing: Theme.gapSmall
            Btn { icon: "download"; text: Theme.say("Export as CSV"); tip: Theme.say("Writes the active mods as a CSV file, the columns of RimSort's export"); onClicked: { exportList.currentFile = "modlist.csv"; exportList.nameFilters = ["CSV (*.csv)"]; listsMenu.close(); exportList.open() } }
            Btn { text: Theme.say("Export as ModsConfig"); tip: Theme.say("Writes the load order as the game's ModsConfig.xml"); onClicked: { exportList.currentFile = "ModsConfig.xml"; exportList.nameFilters = ["ModsConfig (*.xml)"]; listsMenu.close(); exportList.open() } }
            Btn { text: Theme.say("Export as a RimSort list"); tip: Theme.say("Writes the load order as a saved mod list (.rml), which RimSort and the game read"); onClicked: { exportList.currentFile = "modlist.rml"; exportList.nameFilters = ["RimSort (*.rml)"]; listsMenu.close(); exportList.open() } }
            Btn { text: Theme.say("Use a list from a file"); tip: Theme.say("A RimSort or RimPy mod list, a save's list, a share code or a text list becomes the load order. Save writes it."); onClicked: { listsMenu.close(); importList.open() } }
        }

        FileDialog {
            id: exportList
            fileMode: FileDialog.SaveFile
            currentFile: "modlist.csv"
            nameFilters: ["CSV (*.csv)"]
            onAccepted: Hub.mods.exportList(selectedFile)
        }

        FileDialog {
            id: importList
            nameFilters: [
                Theme.say("Mod lists") + " (*.xml *.rml *.rws *.json *.txt)",
                Theme.say("All files") + " (*)"
            ]
            onAccepted: {
                const found = Hub.lists.idsInFile(selectedFile)
                if (found.ids.length > 0) {
                    listsMenu.useList(found.ids)
                    listsMenu.named("")
                } else {
                    Notices.say(found.problem, "err")
                }
            }
        }

        Text { topPadding: Theme.gapSmall; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: Theme.say("For a friend") }
        Flow {
            width: parent.width
            spacing: Theme.gapSmall
            Btn {
                icon: "copy"
                text: Theme.say("Copy the Workshop links")
                tip: Theme.say("A link a line, one for each active Workshop mod; a friend's RimStack reads them into the downloads")
                onClicked: { Theme.copy(Hub.mods.workshopLinks()); listsMenu.close() }
            }
            Btn {
                icon: "send"
                text: Theme.say("Write a lockfile…")
                tip: Theme.say("The load order, with each mod's source and when the Workshop last published its version, as a small file to give a friend")
                onClicked: listsMenu.writeLockfile()
            }
            Btn {
                text: Theme.say("Check a lockfile…")
                tip: Theme.say("Tells which of a friend's mods are missing, extra, in another order or another version here")
                onClicked: listsMenu.checkLockfile()
            }
        }
    }

    function useSaved(listName) {
        const ids = Hub.lists.named(listName)
        if (ids) {
            listsMenu.useList(ids)
            listsMenu.named(listName)
        }
        listsMenu.close()
    }

    Prompt {
        id: useSettingsBox
        property string list: ""
        function take(list) {
            useSettingsBox.list = list
            open()
        }
        title: Theme.fill("Use the mod settings kept with {} too?", useSettingsBox.list)
        Text { width: useSettingsBox.width - useSettingsBox.padding * 2; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("The current settings of those mods are set aside, then the kept ones take their place") }
        Row {
            spacing: Theme.gap
            anchors.right: parent.right
            Btn { text: Theme.say("Only the list"); onClicked: { useSettingsBox.close(); listsMenu.useSaved(useSettingsBox.list) } }
            Btn {
                text: Theme.say("Use them too")
                onClicked: {
                    listsMenu.problem = Hub.lists.useListSettings(useSettingsBox.list)
                    if (listsMenu.problem === "") {
                        useSettingsBox.close()
                        listsMenu.useSaved(useSettingsBox.list)
                    }
                }
            }
        }
    }

    function writeLockfile() {
        close()
        lockWrite.open()
    }

    function checkLockfile() {
        close()
        lockRead.open()
    }

    FileDialog {
        id: lockWrite
        fileMode: FileDialog.SaveFile
        nameFilters: [Theme.say("RimStack lockfile") + " (*.lock.json)", Theme.say("Any file") + " (*)"]
        defaultSuffix: "lock.json"
        onAccepted: {
            const said = Hub.lists.writeLock(lockWrite.selectedFile.toString().replace(/^file:\/\//, ""))
            listsMenu.open()
            listsMenu.problem = said
        }
    }
    FileDialog {
        id: lockRead
        nameFilters: [Theme.say("RimStack lockfile") + " (*.lock.json)", Theme.say("Any file") + " (*)"]
        onAccepted: {
            const found = Hub.lists.checkLock(lockRead.selectedFile.toString().replace(/^file:\/\//, ""))
            if ((found.problem || "") !== "") {
                listsMenu.open()
                listsMenu.problem = found.problem
            } else {
                lockReport.report = found
                lockReport.open()
            }
        }
    }
    Prompt {
        id: lockReport
        property var report: ({ said: [], ids: [], present: false })
        title: Theme.say("Lockfile of the load order")
        Repeater {
            model: lockReport.report.same ? [Theme.say("The load order is the lockfile's, every mod the same.")] : lockReport.report.said
            Text { required property var modelData; width: lockReport.width - lockReport.padding * 2; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; text: modelData }
        }
        Btn { text: Theme.say("Close"); onClicked: lockReport.close() }
        Btn {
            enabled: lockReport.report.present && lockReport.report.ids.length > 0
            text: Theme.say("Use this order")
            tip: Theme.say("The lockfile's order becomes the load order. Save writes it.")
            onClicked: { listsMenu.useList(lockReport.report.ids); listsMenu.named(""); lockReport.close() }
        }
    }
}
