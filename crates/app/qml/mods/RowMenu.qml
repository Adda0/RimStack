import QtQuick
import RimStack.Ui

QtObject {
    function open(row, x, y) {
        const inActive = row.ownModel === Mods.active
        if (!(row.current))
            Mods.select(row.modId, inActive)
        const about = Hub.mods.notInstalled(inActive, row.modIndex) || Mods.about(row.modId)
        const many = Mods.selectedCount > 1
        const workshop = !!about.workshop
        const page = about.workshopPage || ""
        const groups = []
        groups.push([
            { text: about.takeOff || (many ? (inActive ? Theme.plural(Mods.selectedCount, "Deactivate {} mod", "Deactivate {} mods") : Theme.plural(Mods.selectedCount, "Activate {} mod", "Activate {} mods")) : inActive ? Theme.say("Deactivate") : Theme.say("Activate")), icon: inActive ? "minus" : "plus", off: Hub.mods.previewing,
              run: () => many ? Mods.moveSelected() : Mods.moveAcross(row.ownModel, row.modIndex, row.modId) },
        ])
        const open = []
        if (about.path)
            open.push({ text: Theme.say("Open folder"), icon: "folder-open", run: () => Qt.openUrlExternally(Theme.fileUrl(about.path)) })
        if (about.url)
            open.push({ text: Theme.say("Mod's own page"), icon: "link", run: () => Qt.openUrlExternally(about.url) })
        open.push({ text: Theme.say("Copy name"), icon: "copy", run: () => Theme.copy(about.name || "") })
        if (about.packageId)
            open.push({ text: Theme.say("Copy package id"), icon: "copy", run: () => Theme.copy(about.packageId) })
        groups.push(open)
        const fetch = []
        for (const fix of about.fixes || [])
            fetch.push({ text: Theme.say(fix.label), icon: "external-link", run: () => Mods.applyFix(row.modId, fix) })
        for (const id of about.resubscribe || [])
            fetch.push({ text: Theme.say("Subscribe again"), icon: "download", run: () => Hub.downloads.subscribe(id) })
        groups.push(fetch)
        // A local copy has the page of the Workshop item it was copied from, when it is known.
        const steam = page ? [
            { title: Theme.say("Steam Workshop") },
            { text: Theme.say("Page in Steam"), icon: "external-link", run: () => Qt.openUrlExternally(about.steamPage) },
            { text: Theme.say("Page in the browser"), icon: "external-link", run: () => Qt.openUrlExternally(page) },
            { text: Theme.say("Copy link"), icon: "copy", run: () => Theme.copy(page) },
        ] : []
        if (workshop) {
            if (many)
                steam.push({ text: Theme.plural(Mods.selectedCount, "Copy {} mod to the game's Mods folder…", "Copy {} mods to the game's Mods folder…"), icon: "copy", run: () => Notices.ask({
                    title: Theme.plural(Mods.selectedCount, "Copy {} mod to the game's Mods folder", "Copy {} mods to the game's Mods folder"),
                    lines: [Theme.say("Whole folders are copied, which can take a while. Into"), { path: Hub.settings.report.localMods || "" }, Theme.say("The Workshop copies stay where they are; Steam keeps updating those, not these. A mod that is there already stays as it is.")],
                    label: Theme.say("Copy"), run: () => Mods.copySelectedToLocal() }) })
            else
                steam.push({ text: Theme.say("Copy to the game's Mods folder…"), icon: "copy", run: () => Notices.ask({
                    title: Theme.fill("Copy {} to the game's Mods folder", (about.name || "")),
                    lines: [Theme.say("From"), { path: about.path }, Theme.say("Into"), { path: Hub.settings.report.localMods || "" }, Theme.say("The Workshop copy stays where it is; Steam keeps updating that one, not this.")],
                    label: Theme.say("Copy"), run: () => Mods.copyToLocal(row.modId) }) })
            if (Hub.settings.report.steamcmd)
                steam.push({ text: Theme.say("Download again with SteamCMD…"), icon: "download", run: () => Notices.ask({
                    title: Theme.fill("Download {} again", (about.name || "")),
                    lines: [Theme.say("SteamCMD downloads the mod anew, and the fresh copy replaces this folder:"), { path: about.path }],
                    label: Theme.say("Download"), run: () => Hub.downloads.redownload(row.modId) }) })
            const kept = Hub.mods.keptCopiesOf(row.modId) || []
            if (kept.length > 0) {
                steam.push({ title: Theme.say("Kept copies") })
                for (const copy of kept)
                    steam.push({ text: Theme.fill("Restore the copy of {}", Theme.day(copy)), icon: "history", run: () => Notices.ask({
                        title: Theme.fill("Restore the copy of {}", Theme.day(copy)),
                        lines: [Theme.say("Kept before a launch the Workshop's update reached. It is copied into the game's Mods folder, where a copy wins over Steam's newer one:"), { path: Hub.settings.report.localMods || "" }],
                        label: Theme.say("Restore"), run: () => Hub.mods.restoreKeptCopy(row.modId, copy) }) })
            }
            if (!inActive && !many)
                steam.push({ text: Theme.say("Unsubscribe…"), icon: "minus", danger: true, run: () => Mods.unsubscribe(row.modId) })
        }
        groups.push(steam)
        if (about.repository)
            groups.push([
                { title: Theme.say("Git") },
                { text: Theme.say("Repository"), icon: "link", run: () => Qt.openUrlExternally(about.repository) },
                { text: Theme.say("Update"), icon: "download", run: () => Hub.downloads.updateFromGit(row.modId) },
            ])
        // One entry, as the tags in use may be many: the groups open in a menu of their own.
        if (about.groupChoices)
            groups.push([{ text: Theme.say("Group under a tag"), icon: "tag", run: () => Mods.menuRequested(row, x, y, Mods.groupEntries(row.modId, about)) }])
        // Paints the selected mods' rows, as RimSort and RimPy users do to spot mods at a glance.
        if (row.modId >= 0)
            groups.push([{ title: Theme.say("Colour") }].concat(Mods.swatchEntries(row.colour, name => Hub.mods.paint(Mods.selectedIds, name)),
                [{ text: Theme.say("No colour"), icon: "x", off: !row.colour, run: () => Hub.mods.paint(Mods.selectedIds, "") }]))
        const last = []
        if (many && Mods.versionWarned.length > 0)
            last.push({ text: Theme.plural(Mods.versionWarned.length, "Ignore the game version mismatch of {} mod", "Ignore the game version mismatch of {} mods"), icon: "check", run: () => Mods.trustSelected(true) })
        else if (many && about.versionOk)
            last.push({ text: Theme.plural(Mods.selectedCount, "Warn about the game version of {} mod again", "Warn about the game version of {} mods again"), run: () => Mods.trustSelected(false) })
        else if (about.versionOk)
            last.push({ text: Theme.say("Warn about its game version again"), run: () => Mods.trust(row.modId, false) })
        else if (about.offerTrust)
            last.push({ text: Theme.say("Ignore the game version mismatch"), icon: "check", run: () => Mods.trust(row.modId, true) })
        for (const choice of about.binChoices || [])
            last.push({ text: Theme.say(choice.text), icon: choice.icon, danger: true, run: () => Mods.binRequested(row.modId, choice.kind, about.path || "") })
        if (many) {
            const cleanup = Hub.mods.cleanup(Mods.selectedIds) || ({})
            if (!inActive && cleanup.unsubscribe > 0)
                last.push({ text: Theme.plural(cleanup.unsubscribe, "Unsubscribe {} Workshop mod…", "Unsubscribe {} Workshop mods…"), icon: "minus", danger: true,
                  run: () => Notices.ask({
                    title: Theme.plural(cleanup.unsubscribe, "Unsubscribe {} Workshop mod", "Unsubscribe {} Workshop mods"),
                    lines: [Theme.say("Steam, which must be running, unsubscribes every one and deletes its folder"), Theme.fill("Together {} MB on disk", (cleanup.unsubscribeBytes / 1048576).toFixed(1))],
                    label: Theme.say("Unsubscribe"), danger: true, run: () => Hub.downloads.unsubscribeMany(Mods.selectedIds) }) })
            if (cleanup.bin > 0)
                last.push({ text: Theme.plural(cleanup.bin, "Delete {} mod…", "Delete {} mods…"), icon: "trash-2", danger: true,
                  run: () => Notices.ask({
                    title: Theme.plural(cleanup.bin, "Move {} mod to the bin", "Move {} mods to the bin"),
                    lines: [Theme.fill("Together {} MB on disk", (cleanup.binBytes / 1048576).toFixed(1)), Theme.say("The game's own mods and mods in use stay")],
                    label: Theme.say("Move to the bin"), danger: true, run: () => Hub.mods.binMany(Mods.selectedIds) }) })
        }
        groups.push(last)
        const entries = []
        for (const group of groups.filter(group => group.length > 0)) {
            if (entries.length > 0)
                entries.push({ line: true })
            entries.push(...group)
        }
        Mods.menuRequested(row, x, y, entries)
    }
}
