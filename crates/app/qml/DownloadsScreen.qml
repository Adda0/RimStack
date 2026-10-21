pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Item {
    id: screen
    Appear { of: screen }

    readonly property DownloadsData downloads: Hub.downloads
    signal showMod(int modId)

    readonly property var report: downloads.updates || ({})
    readonly property var behind: report.steamcmd || []
    readonly property var found: downloads.queue || ({})
    readonly property var searchBox: downloads.searched || ({ found: [], problem: "" })
    readonly property var searchHits: searchBox.found || []
    // The Workshop hit whose preview shows; -1 for none.
    property int searchPicked: -1
    readonly property var searchHit: searchHits[searchPicked] || null
    readonly property string searchPreview: searchHit ? (downloads.previews || ({}))[searchHit.id] || "" : ""
    onSearchHitChanged: if (searchHit) downloads.fetchPreview(searchHit.id, searchHit.preview || "")
    readonly property var queue: found.items || []
    readonly property var repositories: found.repositories || []
    readonly property var wanted: found.wanted || []
    readonly property bool steamcmd: downloads.steamcmd
    readonly property var needed: Mods.overview.toGet || []
    readonly property var toDownload: Mods.overview.toDownload || []
    readonly property var info: Hub.settings.report
    readonly property var since: downloads.since || ({ time: 0, count: 0, active: 0, rows: [], workshop: 0 })

    property bool sinceOpen: false
    readonly property var changedShown: since.rows
    function day(seconds) {
        return seconds ? Theme.day(seconds) : ""
    }

    function when(seconds) {
        return Theme.moment(seconds)
    }

    function megabytes(n) {
        return !n ? "" : n < 10 ? Theme.fill("{} MB", n.toFixed(1)) : Theme.fill("{} MB", Theme.count(Math.round(n)))
    }

    readonly property var activeUpdates: report.active || []
    readonly property var inactiveUpdates: report.inactive || []
    property bool inactiveOpen: false
    readonly property var checkouts: report.checkouts || []
    readonly property var checkoutsBehind: report.checkoutsBehind || []
    property bool checkoutsOpen: false
    readonly property int ignoredReplacements: (Hub.settings.report.replacementsIgnored || []).length
    readonly property var replacementRows: downloads.replacements || []
    readonly property int swappable: replacementRows.filter(row => row.active && row.haveNew).length
    readonly property var lackingReplacements: replacementRows.filter(row => !row.haveNew).map(row => row.newId)

    component UpdateRow: RowLayout {
        id: row
        required property var modelData
        width: page.width
        height: Theme.sized(30)
        spacing: Theme.gapLarge
        Text { Layout.fillWidth: true; Layout.maximumWidth: implicitWidth + 1; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: row.modelData.name }
        Chip { visible: row.modelData.asked; tone: Theme.ok; icon: "check"; text: Theme.say("Asked of Steam") }
        Chip { tone: Theme.moved; text: screen.day(row.modelData.have) + " → " + screen.day(row.modelData.now); tip: Theme.fill("Published {}", Theme.ago(row.modelData.now)) }
        Btn { text: Theme.say("Show"); spoken: Theme.fill("Show {}", row.modelData.name); onClicked: screen.showMod(row.modelData.modId) }
        Btn { text: Theme.say("What changed"); spoken: Theme.fill("What changed in {}", row.modelData.name); tip: Theme.say("The mod's change notes on the Workshop."); onClicked: Theme.openChangeNotes(row.modelData.id) }
        Item { Layout.fillWidth: true }
    }

    component Cell: Text {
        elide: Text.ElideRight
        color: Theme.ink2
        font.pixelSize: Theme.body
    }

    Flickable {
        id: scroller
        anchors.fill: parent
        contentHeight: page.implicitHeight + 2 * Theme.gapWide
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        Wheel {}
        ScrollBar.vertical: ListBar {}

        Column {
            id: page
            x: Math.max(Theme.gapWide, (parent.width - width) / 2)
            y: Theme.gapWide
            width: screen.width - 2 * Theme.gapWide
            spacing: Theme.gapPage

            Column {
                width: parent.width
                spacing: Theme.gapSmall
                Text { color: Theme.ink2; font.pixelSize: Theme.small; font.weight: Font.DemiBold; text: Theme.say("Add mods") }
                RowLayout {
                    width: parent.width
                    spacing: Theme.gap
                    FilterField {
                        id: pasted
                        Layout.fillWidth: true
                        implicitHeight: Theme.sized(30)
                        placeholder: Theme.say("Paste Workshop links, collection links, mod ids or Git links")
                        onAccepted: add.clicked()
                    }
                    Btn {
                        id: add
                        enabled: !screen.downloads.queueBusy && pasted.text.trim() !== ""
                        primary: pasted.text.trim() !== ""
                        text: screen.downloads.queueBusy ? Theme.say("Asking Steam…") : Theme.say("Look up")
                        tip: Theme.say("Looks up what is pasted beside it: paste a link or an id first.")
                        onClicked: screen.downloads.lookUp(pasted.text)
                    }
                }
                RowLayout {
                    width: parent.width
                    spacing: Theme.gap
                    visible: screen.downloads.canSearch
                    FilterField {
                        id: seeked
                        Layout.fillWidth: true
                        implicitHeight: Theme.sized(30)
                        placeholder: Theme.say("Search the Workshop by name")
                        onAccepted: find.clicked()
                    }
                    Btn {
                        id: find
                        enabled: !screen.downloads.searchBusy && seeked.text.trim() !== ""
                        icon: "search"
                        text: screen.downloads.searchBusy && screen.downloads.searchPage <= 1 ? Theme.say("Asking Steam…") : Theme.say("Search")
                        tip: Theme.say("Searches the whole Workshop with your Steam WebAPI key, sent only to api.steampowered.com.")
                        onClicked: { screen.searchPicked = -1; screen.downloads.search(seeked.text, 1) }
                    }
                }
                Text {
                    visible: !screen.downloads.canSearch
                    width: parent.width
                    wrapMode: Text.WordWrap
                    color: Theme.ink2
                    font.pixelSize: Theme.body
                    text: Theme.say("Searching the Workshop by name needs a Steam WebAPI key, kept on the Settings screen.")
                }
                Btn {
                    visible: !screen.downloads.canSearch
                    icon: "external-link"
                    text: Theme.say("Get a key")
                    tip: Theme.say("Opens steamcommunity.com/dev/apikey in the browser; the key is sent only to api.steampowered.com.")
                    onClicked: Qt.openUrlExternally("https://steamcommunity.com/dev/apikey")
                }
                Column {
                    width: parent.width
                    spacing: Theme.gap
                    visible: !!searchBox.problem || screen.searchHits.length > 0
                    Text {
                        visible: !!searchBox.problem
                        width: parent.width
                        wrapMode: Text.WordWrap
                        color: Theme.err
                        font.pixelSize: Theme.body
                        text: searchBox.problem || ""
                    }
                    RowLayout {
                        visible: screen.searchHits.length > 0
                        width: parent.width
                        spacing: Theme.gapLarge
                        ListView {
                            id: hitList
                            Layout.fillWidth: true
                            Layout.alignment: Qt.AlignTop
                            implicitHeight: Math.min(contentHeight, Theme.sized(420))
                            clip: true
                            boundsBehavior: Flickable.StopAtBounds
                            Wheel {}
                            ScrollBar.vertical: ListBar {}
                            model: screen.searchHits
                            RowKeys {
                                objectName: "searchHitKeys"
                                parent: hitList
                                view: hitList
                                title: Theme.say("Workshop hits")
                                count: hitList.count
                                current: screen.searchPicked
                                onChosen: index => screen.searchPicked = index
                                onEntered: if (screen.searchHit) screen.downloads.lookUp(screen.searchHit.id)
                            }
                            delegate: Item {
                                id: hit
                                required property int index
                                required property var modelData
                                readonly property bool current: index === screen.searchPicked
                                width: hitList.width
                                height: Theme.sized(34)
                                Accessible.role: Accessible.ListItem
                                Accessible.name: modelData.title
                                Rectangle {
                                    anchors.fill: parent
                                    radius: Theme.corner
                                    color: hit.current ? Theme.accent : Theme.hover
                                    opacity: hit.current ? 1 : hitHover.hovered ? 0.05 : 0
                                }
                                HoverHandler { id: hitHover }
                                TapHandler { onTapped: screen.searchPicked = hit.index }
                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: Theme.gapSmall
                                    spacing: Theme.gap
                                    Text { Layout.fillWidth: true; Layout.maximumWidth: implicitWidth + 1; elide: Text.ElideRight; color: hit.current ? Theme.onSelected : Theme.ink; font.pixelSize: Theme.body; text: hit.modelData.title }
                                    Chip { onAccent: hit.current; tone: Theme.moved; icon: "history"; text: screen.day(hit.modelData.updated); tip: Theme.fill("Updated {}", Theme.day(hit.modelData.updated)) }
                                    Chip { visible: hit.modelData.subscriptions > 0; onAccent: hit.current; tone: Theme.ok; icon: "download"; text: Theme.fill("{} subscribed", Theme.count(hit.modelData.subscriptions)) }
                                    Btn { text: Theme.say("Look up"); onClicked: screen.downloads.lookUp(hit.modelData.id) }
                                    Item { Layout.fillWidth: true }
                                }
                            }
                        }
                        Column {
                            objectName: "searchHitPane"
                            visible: !!screen.searchHit
                            Layout.preferredWidth: Math.min(Theme.sized(360), page.width * 0.4)
                            Layout.alignment: Qt.AlignTop
                            spacing: Theme.gapSmall
                            // Held open while the picture is fetched, so the pane does not jump.
                            Rectangle {
                                visible: !!screen.searchHit && !!screen.searchHit.preview
                                width: parent.width
                                height: width * 9 / 16
                                radius: Theme.cornerLarge
                                color: Theme.well
                                clip: true
                                Image {
                                    anchors.fill: parent
                                    asynchronous: true
                                    fillMode: Image.PreserveAspectFit
                                    sourceSize.width: Theme.sized(720)
                                    source: screen.searchPreview ? Theme.fileUrl(screen.searchPreview) : ""
                                }
                            }
                            Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: Theme.heading; font.weight: Font.DemiBold; text: screen.searchHit ? screen.searchHit.title : "" }
                            Flow {
                                width: parent.width
                                spacing: Theme.gapTiny
                                Chip { tone: Theme.moved; icon: "history"; text: screen.searchHit ? Theme.fill("Updated {}", screen.day(screen.searchHit.updated)) : "" }
                                Chip { visible: !!screen.searchHit && screen.searchHit.size > 0; text: screen.searchHit ? screen.megabytes(screen.searchHit.size / 1048576) : "" }
                                Chip { visible: !!screen.searchHit && screen.searchHit.subscriptions > 0; tone: Theme.ok; icon: "download"; text: screen.searchHit ? Theme.fill("{} subscribed", Theme.count(screen.searchHit.subscriptions)) : "" }
                                Repeater {
                                    model: screen.searchHit ? screen.searchHit.tags : []
                                    Chip { required property string modelData; text: modelData }
                                }
                            }
                            Text {
                                visible: text !== ""
                                width: parent.width
                                wrapMode: Text.WordWrap
                                maximumLineCount: 10
                                elide: Text.ElideRight
                                color: Theme.ink2
                                font.pixelSize: Theme.body
                                text: screen.searchHit ? screen.searchHit.description : ""
                            }
                            Flow {
                                width: parent.width
                                spacing: Theme.gapSmall
                                Btn { primary: true; text: Theme.say("Look up"); onClicked: screen.downloads.lookUp(screen.searchHit.id) }
                                Btn { icon: "external-link"; text: Theme.say("Open its page"); onClicked: Theme.openWorkshop(screen.searchHit.id) }
                            }
                        }
                    }
                    Btn {
                        visible: !!searchBox.more
                        enabled: !screen.downloads.searchBusy
                        icon: "chevron-down"
                        text: screen.downloads.searchBusy && screen.downloads.searchPage > 1 ? Theme.say("Asking Steam…") : Theme.say("More results")
                        onClicked: screen.downloads.search(searchBox.text, searchBox.page + 1)
                    }
                }
            }

            Card {
                visible: screen.queue.length + screen.repositories.length > 0 || !!(screen.found.problem || screen.found.note)
                title: Theme.say("Look up")
                note: screen.queue.length + screen.repositories.length > 0 ? screen.queue.length + screen.repositories.length : ""
                actions: [
                    Btn {
                        visible: screen.wanted.length > 0
                        icon: "plus"
                        text: Theme.say("Subscribe in Steam")
                        tip: Theme.say("Steam, which must be running, subscribes to each and downloads it; it keeps them up to date afterwards.")
                        onClicked: screen.downloads.subscribeWanted()
                    },
                    Btn {
                        visible: screen.wanted.length > 0 && screen.steamcmd
                        icon: "download"
                        text: Theme.say("Download with SteamCMD…")
                        tip: Theme.fill("Into the game's own Mods folder, {}, without subscribing: Steam will not update these.", (screen.info.localMods || ""))
                        onClicked: Notices.ask({
                            title: Theme.plural(screen.wanted.length, "Download {} mod with SteamCMD", "Download {} mods with SteamCMD"),
                            lines: [Theme.say("Each is downloaded and copied into the game's own Mods folder:"), { path: screen.info.localMods || "" }, Theme.say("Steam does not update mods installed this way.")],
                            label: Theme.say("Download"), run: () => screen.downloads.downloadWanted() })
                    }
                ]
                Text { visible: screen.queue.length === 0 && !!(screen.found.problem || screen.found.note); width: parent.width; wrapMode: Text.WordWrap; color: screen.found.problem ? Theme.err : Theme.ink2; font.pixelSize: Theme.body; text: screen.found.problem || screen.found.note || "" }
                Repeater {
                    model: screen.repositories
                    RowLayout {
                        id: repository
                        required property var modelData
                        required property int index
                        readonly property bool cloning: screen.downloads.cloning === index
                        width: page.width
                        height: Theme.sized(32)
                        spacing: Theme.gapLarge
                        Icon { name: "git-branch"; color: Theme.ink3 }
                        Text { Layout.fillWidth: true; Layout.maximumWidth: implicitWidth + 1; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: repository.modelData.name }
                        Chip { visible: repository.modelData.installed; tone: Theme.ok; icon: "check"; text: Theme.say("Installed") }
                        Btn { icon: "external-link"; text: Theme.say("Repository page"); onClicked: Qt.openUrlExternally(repository.modelData.link) }
                        Btn {
                            visible: !repository.modelData.installed
                            enabled: !!screen.info.git && !!repository.modelData.into && screen.downloads.cloning < 0
                            primary: !repository.cloning
                            icon: "download"
                            text: repository.cloning ? Theme.say("Cloning…") : Theme.say("Clone…")
                            tip: screen.info.git ? Theme.say("git clone into the game's own Mods folder; Check for updates then asks its repository") : Theme.say("Git is not installed")
                            onClicked: Notices.ask({
                                title: Theme.fill("Clone {}", repository.modelData.name),
                                lines: [Theme.say("git clone, from"), { path: repository.modelData.link }, Theme.say("Into"), { path: repository.modelData.into }],
                                label: Theme.say("Clone"), run: () => screen.downloads.cloneRepository(repository.index) })
                        }
                        Item { Layout.fillWidth: true }
                    }
                }
                Repeater {
                    model: screen.queue
                    RowLayout {
                        id: item
                        required property var modelData
                        width: page.width
                        height: Theme.sized(32)
                        spacing: Theme.gapLarge
                        Text { Layout.fillWidth: true; Layout.maximumWidth: implicitWidth + 1; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: item.modelData.title || item.modelData.id }
                        Cell { text: screen.day(item.modelData.updated) }
                        Cell { text: screen.megabytes(item.modelData.megabytes) }
                        Chip { visible: !item.modelData.title; tone: Theme.err; icon: "circle-alert"; text: Theme.say("Not on the Workshop") }
                        Chip { visible: !!item.modelData.title && item.modelData.installed; tone: Theme.ok; icon: "check"; text: Theme.say("Installed") }
                        Btn { visible: !!item.modelData.title && !item.modelData.installed; icon: "external-link"; text: Theme.say("Workshop page"); onClicked: Theme.openWorkshop(item.modelData.id) }
                        Item { Layout.fillWidth: true }
                    }
                }
            }

            Flow {
                id: cards
                width: parent.width
                spacing: Theme.gapLarge
                readonly property int across: Math.max(1, Math.min(3, Math.floor((width + spacing) / (Theme.sized(300) + spacing))))
                readonly property real each: (width - (across - 1) * spacing) / across
                readonly property real tallest: Math.max(updatesCard.natural, missingCard.natural, steamcmdCard.natural)

                StatusCard {
                    id: updatesCard
                    width: cards.each
                    minHeight: cards.tallest
                    title: Theme.say("Updates")
                    note: screen.report.checkedSaid || Theme.say("Never checked")
                    status: [
                        Chip {
                            visible: screen.downloads.updatesBusy
                            text: Theme.say("Asking Steam…")
                        },
                        Chip {
                            visible: !screen.downloads.updatesBusy && !!screen.report.problem
                            tone: Theme.err
                            icon: "circle-alert"
                            text: Theme.say("Steam not reached")
                        },
                        Chip {
                            visible: !screen.downloads.updatesBusy && !screen.report.problem && !!screen.report.checked && screen.report.count === 0
                            tone: Theme.ok
                            icon: "check"
                            text: Theme.say("All Workshop mods are current")
                        },
                        Chip {
                            visible: !screen.downloads.updatesBusy && !screen.report.problem && screen.report.count > 0
                            tone: Theme.moved
                            icon: "download"
                            text: Theme.plural(screen.report.count, "{} update available", "{} updates available")
                        },
                        Chip {
                            visible: !screen.downloads.updatesBusy && !screen.report.problem && screen.report.hiddenUnchecked > 0
                            icon: "eye-off"
                            text: Theme.plural(screen.report.hiddenUnchecked, "{} hidden item not checked yet", "{} hidden items not checked yet")
                            tip: Theme.say("Steam hides removed, private and unlisted items alike, so each one's Workshop page is looked at, a few per check: Steam refuses many at once. The next check goes on with these.")
                        }
                    ]
                    actions: Btn {
                        primary: !screen.report.checked
                        enabled: !screen.downloads.updatesBusy
                        icon: "refresh-cw"
                        text: Theme.say("Check now")
                        tip: screen.info.gitForWorkshop ? Theme.say("Asks Steam for the current version of each installed Workshop mod (only their Workshop ids are sent), and Git for new commits on each checkout in the game's Mods folder and in the Workshop folder.")
                            : Theme.say("Asks Steam for the current version of each installed Workshop mod (only their Workshop ids are sent), and Git for new commits on each checkout in the game's Mods folder.")
                        onClicked: screen.downloads.checkUpdates()
                    }
                }

                StatusCard {
                    id: missingCard
                    width: cards.each
                    minHeight: cards.tallest
                    title: Theme.say("Missing")
                    note: Theme.say("Needed by the load order")
                    status: [
                        Chip { visible: screen.needed.length === 0; tone: Theme.ok; icon: "check"; text: Theme.say("Nothing missing") },
                        Chip { visible: screen.needed.length > 0; tone: Theme.warn; icon: "triangle-alert"; text: Theme.plural(screen.needed.length, "{} mod missing", "{} mods missing") }
                    ]
                    actions: [
                        Btn {
                            visible: screen.toDownload.length > 0
                            enabled: !screen.downloads.queueBusy
                            icon: "search"
                            text: Theme.say("Look up")
                            tip: Theme.say("Puts the missing mods that have a Workshop page in Look up, to subscribe to them in Steam or download them.")
                            onClicked: screen.downloads.lookUp(screen.toDownload.join(" "))
                        },
                        Btn {
                            visible: screen.steamcmd && screen.toDownload.length > 0
                            enabled: !screen.downloads.taskRunning
                            icon: "download"
                            text: Theme.plural(screen.toDownload.length, "Download {} with SteamCMD", "Download all {} with SteamCMD")
                            tip: Theme.say("SteamCMD downloads each missing mod that has a Workshop page into the game's Mods folder, without subscribing.")
                            onClicked: screen.downloads.downloadAll(screen.toDownload)
                        }
                    ]
                    Repeater {
                        model: screen.needed
                        ProblemLine {
                            required property var modelData
                            width: parent.width
                            stacked: true
                            name: modelData.name
                            words: modelData.words
                            items: modelData.items
                            fix: modelData.fix
                            ignore: modelData.ignore
                            packageId: modelData.packageId
                            modId: modelData.modId
                        }
                    }
                }

                StatusCard {
                    id: steamcmdCard
                    width: cards.each
                    minHeight: cards.tallest
                    title: Theme.say("SteamCMD")
                    status: [
                        Chip { visible: screen.steamcmd; tone: Theme.ok; icon: "check"; text: Theme.say("Installed") },
                        Chip { visible: !screen.steamcmd; text: Theme.say("Not installed") },
                        Chip { visible: screen.steamcmd && screen.behind.length > 0; tone: Theme.moved; icon: "download"; text: Theme.plural(screen.behind.length, "{} update available", "{} updates available") }
                    ]
                    Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink3; font.pixelSize: Theme.small; text: Theme.say("Fetches a mod without subscribing."); Tip { text: Theme.say("Valve's own downloader, fetched from Valve into RimStack's data folder. It downloads Workshop mods without a Steam account, into the game's Mods folder. It runs with a home folder of its own, so it cannot touch Steam's settings.") } }
                    actions: [
                        Btn {
                            visible: !screen.steamcmd
                            text: Theme.say("Install…")
                            onClicked: Notices.ask({
                                title: Theme.say("Install SteamCMD"),
                                lines: [Theme.say("Valve's own downloader is fetched from Valve into"), { path: screen.info.steamcmdFolder || "" }, Theme.say("It runs with a home folder of its own there, apart from Steam's settings.")],
                                label: Theme.say("Install"), run: () => screen.downloads.installSteamcmd() })
                        },
                        Btn {
                            visible: screen.steamcmd && screen.behind.length > 0
                            enabled: !screen.downloads.taskRunning
                            icon: "download"
                            text: Theme.say("Update…")
                            tip: Theme.say("Asked of Steam when the check above last ran.")
                            onClicked: Notices.ask({
                                title: Theme.plural(screen.behind.length, "Update {} mod with SteamCMD", "Update {} mods with SteamCMD"),
                                lines: [Theme.say("Each's folder in the game's Mods folder is replaced by a fresh download:"), { path: screen.info.localMods || "" }, Theme.say("Only the downloads SteamCMD made are replaced: copies made for a purpose of your own stay.")],
                                label: Theme.say("Download"), run: () => screen.downloads.updateWithSteamcmd() })
                        }
                    ]
                }
            }

            Card {
                visible: screen.downloads.updatesBusy || !!screen.report.problem || (screen.report.count || 0) > 0 || screen.behind.length > 0
                title: Theme.say("Newer versions")
                note: screen.downloads.updatesBusy ? Theme.say("Asking Steam…") : Theme.fill("{} for active mods", screen.activeUpdates.length)
                actions: [
                    Btn {
                        visible: (screen.report.count || 0) > 0
                        primary: screen.activeUpdates.length > 0
                        enabled: !screen.downloads.taskRunning
                        icon: "download"
                        text: Theme.say("Update in Steam")
                        tip: Theme.say("Steam, which must be running, downloads the newer version of each now. Left alone it does so the next time it starts.")
                        onClicked: screen.downloads.updateInSteam()
                    }
                ]
                Rectangle {
                    visible: screen.downloads.updatesBusy
                    width: parent.width
                    height: 3
                    radius: Math.min(width, height) / 2
                    color: Theme.chip
                    clip: true
                    Rectangle {
                        width: parent.width / 4
                        height: parent.height
                        radius: Math.min(width, height) / 2
                        color: Theme.accent
                        NumberAnimation on x { running: screen.downloads.updatesBusy && !Theme.stored.reduceMotion; from: -page.width / 4; to: page.width; duration: 1200; loops: Animation.Infinite }
                    }
                }
                Text { visible: !!screen.report.problem; width: parent.width; wrapMode: Text.WordWrap; color: Theme.err; font.pixelSize: Theme.body; text: Theme.fill("Steam not reached: {}", (screen.report.problem || "")) }
                Repeater {
                    model: screen.activeUpdates
                    UpdateRow {}
                }
                Row {
                    visible: screen.inactiveUpdates.length > 0
                    topPadding: screen.activeUpdates.length > 0 ? 8 : 0
                    spacing: Theme.gap
                    Text { color: Theme.ink2; font.pixelSize: Theme.small; font.weight: Font.DemiBold; text: Theme.say("Inactive mods") }
                    Text { color: Theme.ink3; font.pixelSize: Theme.small; text: screen.inactiveUpdates.length }
                }
                Repeater {
                    model: screen.inactiveOpen ? screen.inactiveUpdates : screen.inactiveUpdates.slice(0, 3)
                    UpdateRow {}
                }
                Btn {
                    visible: screen.inactiveUpdates.length > 3
                    text: screen.inactiveOpen ? Theme.fill("Show {}", 3) : Theme.fill("{} more", (screen.inactiveUpdates.length - 3))
                    onClicked: screen.inactiveOpen = !screen.inactiveOpen
                }
                Row {
                    visible: screen.behind.length > 0
                    topPadding: 8
                    spacing: Theme.gap
                    Text { color: Theme.ink2; font.pixelSize: Theme.small; font.weight: Font.DemiBold; text: Theme.say("SteamCMD downloads") }
                    Text { color: Theme.ink3; font.pixelSize: Theme.small; text: Theme.say("Update them with SteamCMD, from the card above") }
                }
                Column {
                    visible: screen.behind.length > 0
                    width: parent.width
                    Repeater {
                        model: screen.behind
                        UpdateRow {}
                    }
                }
            }

            Card {
                visible: screen.checkouts.length > 0
                title: Theme.say("From Git")
                note: screen.checkoutsBehind.length > 0 ? Theme.fill("{} behind", screen.checkoutsBehind.length) : Theme.say("all up to date")
                Repeater {
                    model: screen.checkoutsOpen ? screen.checkouts : screen.checkoutsBehind
                    RowLayout {
                        id: checkout
                        required property var modelData
                        readonly property bool updating: screen.downloads.updating === modelData.modId
                        width: page.width
                        height: Theme.sized(30)
                        spacing: Theme.gapLarge
                        Icon { name: "git-branch"; color: Theme.ink3 }
                        Text { Layout.fillWidth: true; Layout.maximumWidth: implicitWidth + 1; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; text: checkout.modelData.name }
                        Chip {
                            maxWidth: Theme.sized(340)
                            tone: checkout.modelData.problem || !checkout.modelData.checked ? Theme.ink2 : checkout.modelData.behind > 0 ? Theme.moved : Theme.ok
                            text: checkout.modelData.problem ? Theme.fill("Not checked: {}", checkout.modelData.problem)
                                : !checkout.modelData.checked ? Theme.say("Not checked")
                                : checkout.modelData.behind > 0 ? Theme.plural(checkout.modelData.behind, "{} commit behind", "{} commits behind") : Theme.say("Up to date")
                        }
                        Btn { text: Theme.say("Show"); spoken: Theme.fill("Show {}", checkout.modelData.name); onClicked: screen.showMod(checkout.modelData.modId) }
                        Btn {
                            visible: !checkout.modelData.workshop && checkout.modelData.behind > 0
                            enabled: screen.downloads.updating < 0
                            primary: !checkout.updating
                            text: checkout.updating ? Theme.say("Updating…") : Theme.say("Update")
                            spoken: Theme.fill("Update {}", checkout.modelData.name)
                            tip: Theme.say("Moves forward to the remote; stops if the folder has changes of its own.")
                            onClicked: screen.downloads.updateFromGit(checkout.modelData.modId)
                        }
                        Item { Layout.fillWidth: true }
                    }
                }
                RowLayout {
                    readonly property int settled: screen.checkouts.length - screen.checkoutsBehind.length
                    visible: settled > 0
                    width: page.width
                    height: Theme.sized(30)
                    spacing: Theme.gapLarge
                    Text { Layout.fillWidth: true; Layout.maximumWidth: implicitWidth + 1; color: Theme.ink2; font.pixelSize: Theme.body; text: screen.checkoutsOpen ? "" : Theme.fill("{} not behind", parent.settled) }
                    Btn { text: screen.checkoutsOpen ? Theme.say("Only those behind") : Theme.say("Show all"); onClicked: screen.checkoutsOpen = !screen.checkoutsOpen }
                    Item { Layout.fillWidth: true }
                }
            }

            Card {
                visible: screen.since.count > 0
                title: Theme.say("Changed since the game last started")
                note: Theme.fill("{}, since {}", screen.since.count, screen.when(screen.since.time))
                key: "downloads-changed"
                actions: [
                    Choice {
                        readonly property int active: screen.since.active
                        visible: active > 0 && active < screen.since.count
                        title: Theme.say("Changed mods shown")
                        options: [Theme.say("All"), Theme.fill("Active {}", active), Theme.fill("Inactive {}", screen.since.count - active)]
                        current: ["", "active", "inactive"].indexOf(screen.downloads.sinceOnly)
                        onPicked: index => Hub.downloads.sinceOnly = ["", "active", "inactive"][index]
                    },
                    AskSteam {
                    visible: screen.since.workshop > 0
                    idle: Theme.say("Fetch change notes")
                    tip: Theme.say("Reads each mod's change notes from its Workshop page. Only the Workshop ids are sent.")
                    onClicked: screen.downloads.fetchSinceNotes()
                    }
                ]
                spacing: Theme.gapLarge
                Column {
                id: changedRows
                property int at: 0
                readonly property var shown: screen.sinceOpen ? screen.changedShown : screen.changedShown.slice(0, 12)
                function go(to) {
                    at = Math.max(0, Math.min(shown.length - 1, to))
                    const row = rows.itemAt(at)
                    if (!row)
                        return
                    const top = row.mapToItem(scroller.contentItem, 0, 0).y
                    if (top < scroller.contentY)
                        scroller.contentY = Math.max(0, top - Theme.gapLarge)
                    else if (top + row.height > scroller.contentY + scroller.height)
                        scroller.contentY = top + row.height + Theme.gapLarge - scroller.height
                }
                width: page.width
                spacing: Theme.gapLarge
                activeFocusOnTab: shown.length > 0
                Accessible.role: Accessible.List
                Accessible.name: Theme.say("Changed since the game last started")
                Keys.onUpPressed: go(at - 1)
                Keys.onDownPressed: go(at + 1)
                Keys.onReturnPressed: if (shown[at]) screen.showMod(shown[at].modId)
                Keys.onEnterPressed: if (shown[at]) screen.showMod(shown[at].modId)
                Keys.onPressed: event => {
                    if (event.key !== Qt.Key_N || !shown[at] || shown[at].git)
                        return
                    Theme.openChangeNotes(shown[at].id)
                    event.accepted = true
                }
                Repeater {
                    id: rows
                    model: changedRows.shown
                    Column {
                        id: changed
                        readonly property bool reached: changedRows.activeFocus && changedRows.at === index
                        required property var modelData
                        required property int index
                        readonly property string day: screen.day(modelData.now)
                        readonly property bool opensDay: modelData.opensDay
                        readonly property var said: modelData.said
                        width: page.width
                        spacing: Theme.gapTiny
                        Text {
                            visible: changed.opensDay
                            topPadding: changed.index === 0 ? 0 : Theme.gap
                            bottomPadding: Theme.gapThin
                            color: Theme.ink2
                            font.pixelSize: Theme.small
                            font.weight: Font.DemiBold
                            text: changed.day + "  " + Theme.plural(changed.modelData.dayCount, "{} mod", "{} mods")
                        }
                        RowLayout {
                            width: parent.width
                            spacing: Theme.gap
                            Icon { name: changed.modelData.git ? "git-branch" : "cloud"; color: changed.reached ? Theme.focus : Theme.ink3 }
                            Text { Layout.fillWidth: true; Layout.maximumWidth: implicitWidth + 1; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; font.weight: Font.Medium; font.underline: changed.reached; text: changed.modelData.name; Accessible.role: Accessible.ListItem; Accessible.name: text }
                            Chip { visible: !changed.modelData.active; text: Theme.say("inactive") }
                            Chip { visible: changed.modelData.git; tone: Theme.moved; text: Theme.plural(changed.modelData.notes.length, "{} commit", "{} commits") }
                            Text {
                                visible: !changed.modelData.git
                                color: Theme.ink3
                                font.pixelSize: Theme.small
                                text: changed.modelData.sameDay ? Theme.clock(changed.modelData.was) + " → " + Theme.clock(changed.modelData.now)
                                    : screen.day(changed.modelData.was) + " → " + (changed.day === screen.day(Date.now() / 1000) ? Theme.say("today") : changed.day)
                            }
                            Btn { activeFocusOnTab: false; text: Theme.say("Show"); spoken: Theme.fill("Show {}", changed.modelData.name); tip: Theme.say("Enter"); onClicked: screen.showMod(changed.modelData.modId) }
                            Btn { activeFocusOnTab: false; visible: !changed.modelData.git; icon: "external-link"; spoken: Theme.fill("Change notes of {}", changed.modelData.name); tip: Theme.say("The mod's change notes on the Workshop."); onClicked: Theme.openChangeNotes(changed.modelData.id) }
                            Item { Layout.fillWidth: true }
                        }
                        Repeater {
                            model: changed.said
                            RowLayout {
                                id: entry
                                required property var modelData
                                width: changed.width
                                spacing: Theme.gap
                                Text { Layout.preferredWidth: Theme.sized(22); color: Theme.ink3; font.pixelSize: Theme.small; text: "" }
                                Text { Layout.preferredWidth: Theme.sized(74); Layout.alignment: Qt.AlignTop; color: Theme.ink3; font.pixelSize: Theme.small; text: entry.modelData.time ? screen.day(entry.modelData.time) : "" }
                                Text {
                                    Layout.fillWidth: true
                                    wrapMode: Text.WordWrap
                                    maximumLineCount: 12
                                    elide: Text.ElideRight
                                    textFormat: Text.PlainText
                                    color: entry.modelData.time ? Theme.ink2 : Theme.err
                                    font.pixelSize: Theme.small
                                    lineHeight: 1.25
                                    text: entry.modelData.text !== "" ? entry.modelData.text : Theme.say("No note written")
                                }
                            }
                        }
                    }
                }
                }
                Btn {
                    visible: screen.changedShown.length > 12
                    text: screen.sinceOpen ? Theme.fill("Show {}", 12) : Theme.fill("{} more", (screen.changedShown.length - 12))
                    onClicked: screen.sinceOpen = !screen.sinceOpen
                }
            }

            Card {
                visible: replaced.count > 0 || screen.ignoredReplacements > 0
                title: Theme.say("Replacements suggested")
                note: Theme.fill("{}, from the Use This Instead list", replaced.count)
                key: "downloads-replaced"
                actions: [
                    Btn {
                        visible: screen.swappable > 0
                        icon: "corner-down-right"
                        text: Theme.plural(screen.swappable, "Use {} replacement", "Use {} replacements")
                        tip: Theme.say("Each active mod whose replacement is installed gives its place in the load order to it. Nothing reaches the game before Save, and Undo takes it back.")
                        onClicked: Hub.mods.useReplacements()
                    },
                    Btn {
                        visible: screen.lackingReplacements.length > 0
                        enabled: !screen.downloads.queueBusy
                        icon: "search"
                        text: Theme.plural(screen.lackingReplacements.length, "Look up {} replacement", "Look up {} replacements")
                        tip: Theme.say("Puts the replacements that are not installed in Look up, to subscribe to them in Steam or download them.")
                        onClicked: {
                            screen.downloads.lookUp(screen.lackingReplacements.join(" "))
                            scroller.contentY = 0
                        }
                    },
                    Chip {
                        visible: screen.ignoredReplacements > 0
                        pressable: true
                        icon: "eye-off"
                        text: Theme.fill("{} ignored", screen.ignoredReplacements)
                        tip: Theme.say("Suggest them again")
                        onClicked: screen.downloads.suggestReplacementsAgain()
                    }
                ]
                Repeater {
                    id: replaced
                    model: screen.replacementRows
                    RowLayout {
                        id: old
                        required property var modelData
                        width: page.width
                        height: Theme.sized(46)
                        spacing: Theme.gapLarge
                        Column {
                            Layout.fillWidth: true
                            Layout.maximumWidth: Math.max(oldLine.implicitWidth, newLine.implicitWidth) + 1
                            Layout.minimumWidth: 0
                            spacing: Theme.gapHair
                            RowLayout {
                                id: oldLine
                                width: parent.width
                                spacing: Theme.gap
                                Text { Layout.fillWidth: true; Layout.maximumWidth: implicitWidth + 1; elide: Text.ElideRight; color: Theme.ink2; font.pixelSize: Theme.body; text: old.modelData.name }
                                Chip { visible: !old.modelData.active; text: Theme.say("inactive") }
                                Item { Layout.fillWidth: true }
                            }
                            RowLayout {
                                id: newLine
                                width: parent.width
                                spacing: Theme.gap
                                Icon { name: "corner-down-right"; color: Theme.ink3 }
                                Text { Layout.fillWidth: true; Layout.maximumWidth: implicitWidth + 1; elide: Text.ElideRight; color: Theme.ink; font.pixelSize: Theme.body; font.weight: Font.Medium; text: old.modelData.newName }
                                Chip { visible: old.modelData.haveNew; tone: Theme.ok; icon: "check"; text: Theme.say("installed") }
                                Item { Layout.fillWidth: true }
                            }
                        }
                        Btn { text: Theme.say("Show old mod"); spoken: Theme.fill("Show {}", old.modelData.name); tip: Theme.say("The installed mod the list would replace"); onClicked: screen.showMod(old.modelData.modId) }
                        Btn { visible: !old.modelData.haveNew; icon: "external-link"; text: Theme.say("Replacement"); tip: Theme.say("Workshop page of the replacement"); onClicked: Theme.openWorkshop(old.modelData.newId) }
                        Btn { icon: "eye-off"; text: Theme.say("Ignore"); spoken: Theme.fill("Ignore the replacement of {}", old.modelData.name); tip: Theme.say("No replacement is suggested for this mod again"); onClicked: screen.downloads.ignoreReplacement(old.modelData.modId) }
                        Btn {
                            danger: true
                            text: Theme.say("Unsubscribe…")
                            spoken: Theme.fill("Unsubscribe from {}", old.modelData.name)
                            tip: Theme.say("Lets the replaced mod go: Steam unsubscribes and deletes its folder")
                            onClicked: Mods.unsubscribe(old.modelData.modId)
                        }
                        Item { Layout.fillWidth: true }
                    }
                }
            }
        }
    }
}
