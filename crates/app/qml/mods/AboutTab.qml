pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Column {
    id: side
    Appear { of: side }
    width: parent.width
    visible: Mods.tab === 0
    topPadding: Theme.gapLarge
    spacing: Theme.gapCard
    readonly property var clashes: (Mods.info.losesTo || []).map(c => ({ loses: true, clash: c }))
        .concat((Mods.info.winsOver || []).map(c => ({ loses: false, clash: c })))
    readonly property string linkColour: "<style>a { color: " + Theme.focus + "; }</style>"
    readonly property var problems: Mods.info.problems || []
    component SavedMark: Text {
        opacity: Number(Hub.mods.notesSaved)
        color: Theme.ok
        font.pixelSize: Theme.small
        text: Theme.say("Saved")
        Behavior on opacity { NumberAnimation { duration: Theme.quick } }
    }

    Card {
        visible: side.problems.length > 0 || !!Mods.info.unreadable || !!Mods.info.versionOk || Mods.info.problemsIgnored > 0
        title: Theme.say("Before you play")
        tone: side.problems.some(p => p.error) || Mods.info.unreadable ? Theme.err : Theme.warn
        spacing: Theme.gap
        Repeater {
            model: side.problems
            ProblemLine {
                required property var modelData
                width: side.width
                error: modelData.error
                words: modelData.words
                fix: modelData.fix
                ignore: modelData.ignore
                packageId: modelData.packageId
                modId: Mods.selectedId
            }
        }
        Btn {
            readonly property int alike: Mods.overview.versionOnly || 0
            visible: alike > 1 && !!Mods.info.offerTrust && !Hub.mods.previewing
            text: Theme.fill("Ignore all {} for RimWorld {}", alike, (Mods.overview.gameVersion || ""))
            tip: Theme.say("No warning about the game version while RimWorld stays on this version.")
            onClicked: Mods.setVersionAside(Mods.overview.gameVersion || "")
        }
        Text { visible: !!Mods.info.unreadable; width: parent.width; wrapMode: Text.WordWrap; color: Theme.err; font.pixelSize: Theme.body; text: Theme.fill("Unreadable: {}", (Mods.info.unreadable || "")) }
        RowLayout {
            visible: !!Mods.info.versionOk
            width: parent.width
            spacing: Theme.gap
            Icon { name: "eye-off"; size: 14; color: Theme.ink3 }
            Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.fill("Made for RimWorld {}; the warning is ignored.", (Mods.info.madeFor || "")) }
            Btn { text: Theme.say("Warn again"); onClicked: Mods.trust(Mods.selectedId, false) }
        }
        RowLayout {
            visible: Mods.info.problemsIgnored > 0
            width: parent.width
            spacing: Theme.gap
            Icon { name: "eye-off"; size: 14; color: Theme.ink3 }
            Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.plural(Mods.info.problemsIgnored || 0, "{} problem is ignored.", "{} problems are ignored.") }
            Btn { text: Theme.say("Warn again"); onClicked: Hub.mods.warnAgain([Mods.selectedId]) }
        }
    }

    Card {
        title: Theme.say("Tags and note")
        key: "about-notes"
        actions: SavedMark {}
        // Each tag takes itself off; the first is the group the mod sits under.
        Flow {
            width: parent.width
            spacing: Theme.gapSmall
            Repeater {
                model: Mods.info.tagList || []
                Chip {
                    required property string modelData
                    required property int index
                    pressable: true
                    tone: index === 0 ? Theme.moved : Theme.ink2
                    icon: "x"
                    text: index === 0 ? Theme.fill("{}  group", modelData) : modelData
                    tip: Theme.fill("Remove the tag {}", modelData)
                    onClicked: Hub.mods.removeTag(Mods.selectedId, modelData)
                }
            }
        }
        RowLayout {
            width: parent.width
            spacing: Theme.gapSmall
            FilterField {
                id: tagBox
                Layout.fillWidth: true
                placeholder: Theme.say("Add tags, separated by commas")
                readonly property bool suggesting: tagOffers.visible
                onTypingChanged: {
                    tagOffers.shut = false
                    tagOffers.chosen = -1
                }
                onEditingChanged: tagOffers.shut = false
                onEscaped: tagOffers.shut = true
                // Enter takes the offer chosen with the arrows, else adds what is typed.
                onAccepted: {
                    const chosen = tagBox.suggesting && tagOffers.chosen >= 0 && tagOffers.offered[tagOffers.chosen].text
                    Hub.mods.addTags(Mods.selectedId, chosen || tagBox.text)
                    tagBox.text = ""
                }
                Keys.enabled: suggesting
                Keys.onUpPressed: tagOffers.chosen = Math.max(tagOffers.chosen - 1, 0)
                Keys.onDownPressed: tagOffers.chosen = Math.min(tagOffers.chosen + 1, tagOffers.offered.length - 1)
                // The tag after the last comma, from those given to any mod and not to this one.
                Popup {
                    id: tagOffers
                    readonly property var offered: Hub.mods.tagsFor(Mods.info.tags || "", tagBox.text)
                    property bool shut: false
                    property int chosen: -1
                    visible: offered.length > 0 && tagBox.editing && tagBox.visible && !shut
                    onClosed: shut = true
                    closePolicy: Popup.CloseOnPressOutsideParent
                    y: tagBox.height + 4
                    width: Math.min(tagBox.width, 320)
                    padding: 6
                    background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }
                    contentItem: Column {
                        id: tagOfferList
                        Repeater {
                            model: tagOffers.offered
                            Item {
                                id: tagOffer
                                required property var modelData
                                required property int index
                                width: tagOfferList.width
                                height: Theme.sized(26)
                                Accessible.role: Accessible.Button
                                Accessible.name: tagOffer.modelData.tag
                                Rectangle { anchors.fill: parent; radius: Theme.corner; color: Theme.chip; visible: tagOfferHover.hovered || tagOffer.index === tagOffers.chosen }
                                Text { x: 8; anchors.verticalCenter: parent.verticalCenter; color: Theme.ink; font.pixelSize: Theme.small; text: tagOffer.modelData.tag }
                                HoverHandler { id: tagOfferHover }
                                TapHandler {
                                    gesturePolicy: TapHandler.ReleaseWithinBounds
                                    onTapped: {
                                        Hub.mods.addTags(Mods.selectedId, tagOffer.modelData.text)
                                        tagBox.text = ""
                                        tagBox.focusInput()
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Btn {
                compact: true
                icon: "plus"
                tip: Theme.say("Add")
                onClicked: {
                    Hub.mods.addTags(Mods.selectedId, tagBox.text)
                    tagBox.text = ""
                }
            }
            Btn {
                visible: !!Mods.info.tags
                compact: true
                icon: "trash-2"
                tip: Theme.say("Remove all tags")
                onClicked: Hub.mods.clearTags(Mods.selectedId)
            }
        }
        Rectangle {
            width: parent.width
            height: Math.max(54, noteBox.implicitHeight + 14)
            radius: Theme.corner
            color: Theme.well
            border.width: 1
            border.color: noteBox.activeFocus ? Theme.focus : Theme.fieldLine
            Text { x: 10; y: 7; visible: noteBox.text === ""; color: Theme.ink3; font.pixelSize: Theme.body; text: Theme.say("Note") }
            TextEdit {
                id: noteBox
                Accessible.name: Theme.say("Note")
                x: 10
                y: 7
                width: parent.width - 20
                wrapMode: TextEdit.Wrap
                color: Theme.ink
                selectionColor: Theme.accent
                font.pixelSize: Theme.body
                onTextChanged: {
                    if (activeFocus)
                        Hub.mods.draftNote(Mods.selectedId, noteBox.text)
                }
                onActiveFocusChanged: Hub.mods.keepDrafts()
            }
            Binding { target: noteBox; property: "text"; value: Mods.info.note || ""; when: !noteBox.activeFocus; restoreMode: Binding.RestoreNone }
        }
    }

    Card {
        title: Theme.say("Facts")
        key: "about-facts"
        actions: SavedMark {}
        Repeater {
            model: [
                [Theme.say("Contains"), [Mods.info.contains, Mods.textures.count ? Theme.plural(Mods.textures.count, "{} texture", "{} textures") : ""].filter(s => s).join("; ")],
                [Theme.say("Code"), Mods.info.code],
                [Theme.say("Load time"), Mods.info.loadTime],
            ].filter(pair => pair[1])
            Item {
                id: fact
                required property var modelData
                width: side.width
                implicitHeight: factValue.implicitHeight
                Text { id: factName; width: Theme.sized(92); color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say(fact.modelData[0]) }
                CopyText { id: factValue; anchors.left: factName.right; anchors.leftMargin: 4; anchors.right: parent.right; text: fact.modelData[1] }
            }
        }
        RowLayout {
            visible: !!Mods.info.packageId
            width: parent.width
            spacing: Theme.gap
            Text { Layout.preferredWidth: Theme.sized(92) - Theme.gap; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("Package id") }
            CopyText { Layout.fillWidth: true; text: Mods.info.packageId || "" }
            Btn { compact: true; icon: "copy"; tip: Theme.say("Copy package id"); onClicked: Theme.copy(Mods.info.packageId) }
        }
        RowLayout {
            visible: !!Mods.info.path
            width: parent.width
            spacing: Theme.gap
            Text { Layout.preferredWidth: Theme.sized(92) - Theme.gap; color: Theme.ink2; font.pixelSize: Theme.body; text: Theme.say("Folder") }
            PathText { path: Mods.info.path || ""; copyable: true; openTip: Theme.say("Open folder") }
        }
        RowLayout {
            width: parent.width
            spacing: Theme.gap
            Defined { Layout.preferredWidth: Theme.sized(92); text: Theme.say("Filed under") }
            Icon { name: Theme.categoryIcons[Theme.categoryNames.indexOf(Mods.info.category || "")] || "package"; size: 14; color: Theme.categoryColours[Theme.categoryNames.indexOf(Mods.info.category || "")] || Theme.ink2 }
            Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: Mods.info.categoryChosen ? Theme.fill("{} (chosen)", Theme.say(Mods.info.category || "")) : Theme.say(Mods.info.category || "") }
            Btn {
                id: fileUnder
                text: Theme.say("Change")
                tip: Theme.say("RimStack files a mod by where it stands, what its folders hold and what its code is about. A category chosen here stands over that.")
                onClicked: Mods.menuRequested(fileUnder, 0, fileUnder.height + 2, Mods.categoryChoices(Mods.selectedId))
            }
        }
        RowLayout {
            width: parent.width
            spacing: Theme.gap
            Defined { Layout.preferredWidth: Theme.sized(92); text: Theme.say("Group") }
            Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: Mods.info.sub || Theme.say("None") }
            Btn {
                id: groupButton
                visible: !!Mods.info.groupChoices
                menu: true
                text: Theme.say("Change")
                tip: Theme.say("A group is a tag the mod is listed under, beside the other mods with it. Choose one in use, start a new one, or delete one for every mod in it.")
                onClicked: Mods.menuRequested(groupButton, 0, groupButton.height + 2, Mods.groupEntries(Mods.selectedId, Mods.info))
            }
        }
    }

    Card {
        visible: !!Mods.info.description
        title: Theme.say("Description")
        CopyText {
            width: parent.width
            textFormat: TextEdit.RichText
            text: side.linkColour + (Mods.info.descriptionRich || "")
            onLinkActivated: (link) => Qt.openUrlExternally(link)
            HoverHandler { enabled: parent.hoveredLink !== ""; cursorShape: Qt.PointingHandCursor }
        }
    }

    // The game keeps the texture of the mod that loads last.
    Card {
        visible: conflictLines.count > 0
        title: Theme.say("Texture conflicts")
        note: conflictLines.count
        tone: Theme.warn
        key: "about-conflicts"
        spacing: Theme.gap
        Repeater {
            id: conflictLines
            model: side.clashes
            Column {
                id: clash
                required property var modelData
                readonly property bool loses: modelData.loses
                readonly property var other: modelData.clash
                width: side.width
                spacing: Theme.gapTiny
                RowLayout {
                    width: parent.width
                    spacing: Theme.gapSmall
                    Chip { tone: clash.loses ? Theme.warn : Theme.ok; icon: clash.loses ? "arrow-down" : "arrow-up"; text: Theme.fill(clash.loses ? "loses {}" : "wins {}", clash.other.textures) }
                    Text { Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: clash.other.name }
                }
                Row {
                    spacing: Theme.gapSmall
                    Btn { text: Theme.say("Compare"); tip: Theme.say("Both mods' versions of each shared texture, side by side"); onClicked: Mods.compareWith(clash.other.modId) }
                    Btn { visible: clash.loses; enabled: !Hub.mods.previewing; text: Theme.say("Win all"); tip: Theme.say("Adds an own rule: this mod loads after the other."); onClicked: Mods.winAll(clash.other.modId) }
                }
            }
        }
    }

    Card {
        visible: !!Mods.info.replacedBy
        title: Theme.say("Use this instead")
        RowLayout {
            width: parent.width
            spacing: Theme.gap
            Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; lineHeight: 1.3; text: Mods.info.replacedByVersions ? Theme.fill("{}, for RimWorld {}", Mods.info.replacedBy || "", Mods.info.replacedByVersions) : Mods.info.replacedBy || "" }
            Btn { icon: "external-link"; text: Theme.say("Open"); onClicked: Theme.openWorkshop(Mods.info.replacedById) }
        }
        Row {
            spacing: Theme.gapSmall
            Btn { visible: Mods.selectedActive && !Hub.mods.previewing; icon: "minus"; text: Theme.say("Deactivate this one"); onClicked: Mods.toggleSelected() }
            Btn { visible: !!Mods.info.workshop; danger: true; text: Theme.say("Unsubscribe…"); onClicked: Mods.unsubscribe(Mods.info.name || "", Mods.info.folder, Mods.info.path) }
        }
    }

    Card {
        visible: !!Mods.info.copies
        title: Theme.say("Two copies")
        Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.body; lineHeight: 1.3; text: Mods.info.copies || "" }
        Btn {
            visible: !!Mods.info.binLocalCopy
            danger: true
            icon: "trash-2"
            text: Theme.say("Move the local copy to the bin…")
            tip: Theme.say("The Workshop copy is then the one the game uses.")
            onClicked: Mods.binRequested(Mods.info.binLocalCopy.modId, 0, Mods.info.binLocalCopy.path)
        }
    }

    Card {
        visible: (Mods.info.settingsFiles || []).length > 0
        title: Theme.say("Mod settings")
        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            color: Theme.ink
            font.pixelSize: Theme.body
            lineHeight: 1.3
            text: Theme.say("Its Mod_*.xml files of the game's Config folder:")
        }
        Column {
            width: parent.width
            spacing: Theme.gapSmall
            Repeater {
                model: Mods.info.settingsFiles || []
                Text {
                    required property string modelData
                    width: parent.width
                    elide: Text.ElideMiddle
                    color: Theme.ink3
                    font.pixelSize: Theme.small
                    text: modelData
                }
            }
        }
        Btn {
            danger: true
            icon: "trash-2"
            text: Theme.say("Reset its settings…")
            tip: Theme.say("Its settings files are set aside, so the mod starts from its defaults.")
            onClicked: Notices.ask({
                title: Theme.fill("Reset the settings of {}", (Mods.info.name || "")),
                lines: [Theme.say("Its Mod_*.xml are moved out of"), { path: Hub.settings.report.configFolder || "" }, Theme.say("into a new folder, named mod-settings and the time, in"), { path: Hub.settings.report.dataFolder || "" }, Theme.say("Moving the files back restores them.")],
                label: Theme.say("Set aside"), run: Hub.mods.resetSettings })
        }
    }
    Card {
        id: latest
        readonly property bool workshop: !!Mods.info.workshop
        readonly property bool git: !workshop && !!Mods.info.repository
        readonly property var notes: workshop ? ((Hub.downloads.notes || {})[Mods.info.folder] || []) : git ? Hub.downloads.commitsOf(Mods.selectedId) || [] : []
        visible: !Theme.demo && (workshop || git)
        title: Theme.say("Latest changes")
        key: "about-changes"
        spacing: Theme.gap
        actions: [
            AskSteam {
                visible: latest.workshop
                idle: latest.notes.length > 0 ? Theme.say("Fetch again") : Theme.say("Fetch")
                tip: Theme.say("Reads this mod's change notes from its Workshop page. Only its Workshop id is sent.")
                onClicked: Hub.downloads.fetchNotes(Mods.selectedId)
            },
            Btn { visible: latest.workshop; icon: "external-link"; spoken: Theme.fill("Change notes of {}", Mods.info.name || ""); tip: Theme.say("The mod's change notes on the Workshop."); onClicked: Theme.openChangeNotes(Mods.info.folder) }
        ]
        Text {
            visible: latest.notes.length === 0
            width: parent.width
            wrapMode: Text.WordWrap
            color: Theme.ink3
            font.pixelSize: Theme.body
            text: !latest.workshop ? Theme.say("No commits") : Mods.info.updated ? Theme.fill("Last updated {}; its notes are not fetched", Theme.day(Mods.info.updated)) : Theme.say("Not fetched")
        }
        Repeater {
            model: latest.notes
            Column {
                id: note
                required property var modelData
                width: latest.width
                spacing: Theme.gapHair
                Text { visible: note.modelData.time > 0; color: Theme.ink3; font.pixelSize: Theme.small; text: Theme.moment(note.modelData.time) }
                Text {
                    width: parent.width
                    wrapMode: Text.WordWrap
                    maximumLineCount: 14
                    elide: Text.ElideRight
                    textFormat: Text.PlainText
                    color: note.modelData.time > 0 ? Theme.ink : Theme.err
                    font.pixelSize: Theme.body
                    lineHeight: 1.25
                    text: note.modelData.text !== "" ? note.modelData.text : Theme.say("No note written")
                }
            }
        }
    }

    Connections {
        target: Mods
        function onEditRequested(what) {
            Mods.tab = 0
            if (Theme.folded.indexOf("about-notes") >= 0)
                Theme.fold("about-notes")
            Qt.callLater(() => what === "note" ? noteBox.forceActiveFocus() : tagBox.focusInput())
        }
    }
}
