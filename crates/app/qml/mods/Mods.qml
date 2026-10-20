pragma Singleton
pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Item {
    id: mods

    signal reveal(var model, int row, bool centre)
    signal focusList(var model)
    signal filterRequested(string text)
    property string filtered: ""
    signal filterFocusRequested()
    // Entries: { text, icon, off, danger, run }.
    signal menuRequested(Item item, real x, real y, var entries)
    signal firstFrameShown()

    readonly property ModRows active: Hub.mods.active
    readonly property ModRows inactive: Hub.mods.inactive
    readonly property ModsPrefs prefs: ModsPrefs {}

    readonly property int activeCount: active.count
    readonly property int inactiveCount: inactive.count
    signal rescanned()
    // "after" or "before" while the other mod of a rule of your own is chosen.
    property string picking

    function pick(otherId) {
        if (picking === "after")
            Hub.mods.addRule(otherId, selectedId)
        else
            Hub.mods.addRule(selectedId, otherId)
        picking = ""
    }

    readonly property bool dirty: Hub.mods.dirty
    readonly property int errors: Hub.mods.errors
    // Library id -> `{ loses }`: whether that mod loses its shared textures to this one.
    readonly property var conflictMarks: info.conflictMarks || ({})
    readonly property string saveProblem: Hub.mods.saveProblem
    readonly property bool fileChanged: Hub.mods.fileChanged

    property var afterSave: null
    function save(then) {
        afterSave = then || null
        return Hub.mods.save()
    }

    function about(modId) {
        return Hub.mods.detailsOf(modId) || ({})
    }

    // The mod reached, from the cursor; -1 in a list without mods.
    function step(model, by) {
        const row = Hub.mods.stepFrom(model.active, cursor, by)
        if (row >= 0)
            reveal(model, row, false)
        return Hub.mods.stopOn(model.active, row)
    }

    // A mod list's keys; false for those it leaves to others.
    function listKey(list, key, modifiers) {
        const model = list.model, page = Math.max(1, Math.floor(list.height / Theme.row) - 1)
        const by = new Map([[Qt.Key_Up, -1], [Qt.Key_Down, 1], [Qt.Key_PageUp, -page], [Qt.Key_PageDown, page], [Qt.Key_Home, -list.count], [Qt.Key_End, list.count]]).get(key)
        const how = modifiers & ~Qt.KeypadModifier
        switch (by === undefined ? key | how : how) {
        case Qt.NoModifier:
            select(step(model, by), model.active)
            break
        case Qt.ShiftModifier:
            selectThrough(step(model, by), model.active)
            break
        case Qt.ControlModifier:
            cursor = step(model, by)
            break
        case Qt.Key_Space | Qt.ControlModifier:
            selectAlso(cursor, model.active)
            break
        case Qt.Key_A | Qt.ControlModifier:
            selectAll(model)
            break
        case Qt.AltModifier:
            model.active && columns.shownBy === "" && Math.abs(by) === list.count ? selectedToEdge(by < 0) : nudge(model, by)
            break
        case Qt.Key_Return:
        case Qt.Key_Enter:
            activateSelected(model)
            break
        case Qt.Key_Menu:
        case Qt.Key_F10 | Qt.ShiftModifier:
            rowWanted(row => row.isHeader ? categoryMenu(row, Theme.sized(60), row.height, row.category) : rowMenu(row, Theme.sized(60), row.height))
            break
        case Qt.Key_Left:
        case Qt.Key_Right:
            Hub.mods.fold(model.active, selectedId, key === Qt.Key_Right)
            break
        case Qt.Key_T:
        case Qt.Key_N:
            return selectedCount === 1 && (editRequested(key === Qt.Key_N ? "note" : "tags"), true)
        default:
            return false
        }
        return true
    }

    // Runs `run(row)` with the selected mod's row, scrolled to; not when its list does not show it.
    signal rowWanted(var run)

    function activateSelected(model) {
        const position = selectedId >= 0 ? model.indexOf(selectedId) : -1
        if (position < 0 || Hub.mods.previewing)
            return
        if (selectedCount > 1)
            return moveSelected()
        const next = Hub.mods.besideMod(model.active, selectedId)
        moveAcross(model, position, selectedId)
        if (next >= 0)
            select(next, model.active)
    }

    function compareWith(otherId) {
        const pair = Hub.mods.inLoadOrder(selectedId, otherId)
        if (pair)
            comparing = { earlier: pair[0], later: pair[1] }
    }

    readonly property var overview: Hub.mods.overview || ({})
    readonly property var troubles: overview.troubles || []
    readonly property var needed: overview.needed || []
    readonly property int fixable: needed.length
    property bool problemsOpen: false
    signal problemsRequested()
    onProblemsRequested: problemsOpen = true

    property bool leftOffRestored: false
    function restoreLeftOff() {
        const picture = Hub.given("--shot") && !Hub.given("--resume")
        if (leftOffRestored || Theme.demo || checkMode !== "" || picture || active.count === 0)
            return
        leftOffRestored = true
        // "is:changed" is left out: nothing is unsaved at the start.
        const kept = prefs.lastFilter.split(" ").filter(term => term !== "" && term !== "is:changed").join(" ")
        if (kept !== "")
            filterBy(kept)
        const key = prefs.lastMod
        const here = key === "" ? -1 : Hub.mods.packageMod(true, key)
        const there = key === "" || here >= 0 ? -1 : Hub.mods.packageMod(false, key)
        if (here >= 0 || there >= 0) {
            showMod(here >= 0 ? here : there)
            tab = prefs.lastTab
        }
        leftOffScroll(prefs.lastScroll)
    }
    signal leftOffScroll(real y)
    onTabChanged: if (leftOffRestored) prefs.lastTab = tab

    // Which of the three first-run hints shows; 0 for none.
    property int hintStep: 0
    function startHints() {
        if (!Theme.stored.hintsSeen && !Theme.demo && checkMode === "")
            hintStep = 1
    }
    function endHints() {
        hintStep = 0
        Theme.stored.hintsSeen = true
    }

    // `fix` is { action, target, label, page }, as a problem carries it.
    function applyFix(modId, fix) {
        if (Hub.mods.fix(modId, fix.action, fix.target))
            return
        if (fix.page)
            Qt.openUrlExternally(fix.page)
        else
            Theme.openWorkshop(fix.target)
    }

    function fixAll() { Hub.mods.fixAll() }
    function winAll(otherId) { Hub.mods.winOver(selectedId, otherId) }

    // No warning about the game version while the game stays on `version`;
    // "" brings the warnings back.
    function setVersionAside(version) {
        const before = Hub.settings.report.versionAsideFor || ""
        Hub.settings.keep("versionAsideFor", version)
        if (version !== "")
            Notices.offer(Theme.fill("Game version warnings are ignored for RimWorld {}", version), Theme.say("Undo"), () => setVersionAside(before))
    }

    readonly property var versionWarned: selectedCount < 2 || !Hub.mods.overview ? [] : Hub.mods.versionWarned(selectedIds)
    function trustSelected(yes) {
        Hub.mods.trust(yes ? versionWarned : selectedIds, yes)
    }
    function trust(modId, yes) {
        Hub.mods.trust([modId], yes)
    }

    function putInSub(modId, name) { Hub.mods.group([modId], name) }
    function removeSub(model, row) { Hub.mods.removeSub(model.active, row) }
    signal subNameRequested(int modId)
    signal bulkRequested(string what)
    readonly property var selectedIds: selectedId < 0 ? [] : [selectedId].concat(Object.keys(also).map(Number))

    function tagSelected(tag) { Hub.mods.addTag(selectedIds, tag) }
    function subSelected(name) { Hub.mods.group(selectedIds, name) }
    function selectedToEdge(top) {
        Hub.mods.toEdge(selectedIds, top)
        reveal(active, active.rowOf(active.indexOf(selectedId)), false)
    }

    signal editRequested(string what)

    readonly property int sortPending: Hub.mods.sortPending
    property Connections sortWatch: Connections {
        target: Hub.mods
        function onLibraryVersionChanged() {
            mods.select(-1, false)
            mods.rescanned()
        }
        function onDone(said) {
            Notices.offer(said, Theme.say("Undo"), () => Hub.mods.undo())
        }
        function onTold(said, tone) {
            Notices.say(said, tone)
        }
        function onSaved(ok) {
            const then = mods.afterSave
            mods.afterSave = null
            if (ok && then)
                then()
        }
    }
    property Connections countWatch: Connections {
        target: Hub.mods.active
        function onCountChanged() { Qt.callLater(mods.restoreLeftOff) }
    }
    property Binding filterHeld: Binding { target: Hub.mods; property: "filter"; value: mods.filtered }
    property Binding headingsHeld: Binding { target: Hub.mods; property: "headings"; value: mods.prefs.headings !== 0 }
    property Binding inactiveOrderHeld: Binding { target: Hub.mods; property: "inactiveOrder"; value: mods.prefs.inactiveOrder }
    property Binding inactiveReversedHeld: Binding { target: Hub.mods; property: "inactiveReversed"; value: mods.prefs.inactiveReversed }
    property Binding selectedHeld: Binding { target: Hub.mods; property: "selected"; value: mods.selectedId }

    signal binRequested(int modId, int kind, string folder)

    readonly property string upkeepProblem: Hub.mods.upkeepProblem
    function copyToLocal(modId) { Hub.mods.copyToLocal(modId) }
    function copySelectedToLocal() { Hub.mods.copyManyToLocal(selectedIds) }

    readonly property Columns columns: Columns {
        prefs: mods.prefs
        loadTimes: mods.overview.loadTimes === true
        onShownByAsked: Hub.mods.showBy(shownBy, descending)
    }
    // A menu entry for each swatch, the chosen one ticked.
    function swatchEntries(chosen, pick) {
        const entries = []
        for (const swatch of Theme.swatches)
            entries.push({ text: Theme.say(swatch[0]) + (swatch[0] === chosen ? "  ✓" : ""), colour: Theme.swatch(swatch[0]), run: () => pick(swatch[0]) })
        return entries
    }
    // The swatches for the selected mods, searchable so a colour is picked by typing its name.
    function colourEntries(chosen) {
        const entries = [{ title: Theme.say("Colour") }].concat(swatchEntries(chosen, name => Hub.mods.paint(selectedIds, name)).map(entry => Object.assign(entry, { pick: true })))
        entries.push({ text: Theme.say("No colour"), icon: "x", off: !chosen, run: () => Hub.mods.paint(selectedIds, "") })
        return entries
    }
    function categoryMenu(item, x, y, category) {
        const chosen = Theme.chosenSwatches[category] || Theme.swatches[category][0]
        const entries = [{ title: Theme.categoryNames[category] }].concat(swatchEntries(chosen, name => Theme.paintCategory(category, name)))
        entries.push({ line: true })
        entries.push({ text: Theme.say("Its own colour"), icon: "undo-2", off: !Theme.chosenSwatches[category], run: () => Theme.paintCategory(category, "") })
        menuRequested(item, x, y, entries)
    }

    function fileUnder(modId, label) { Hub.mods.fileUnder(modId, label) }

    function categoryChoices(modId) {
        const chosen = about(modId).categoryChosen || ""
        const entries = [{ text: Theme.say("Automatic"), icon: chosen === "" ? "check" : "", run: () => fileUnder(modId, "") }, { line: true }]
        for (const label of overview.categories || [])
            entries.push({ text: Theme.say(label), icon: chosen === label ? "check" : "", run: () => fileUnder(modId, label) })
        return entries
    }

    // The groups beside the mod, a new one, out of its group, and its group deleted for every mod in it.
    function groupEntries(modId, about) {
        const entries = [{ title: Theme.say("Group under a tag") }]
        for (const tag of about.groupChoices || [])
            entries.push({ text: tag, icon: "tag", pick: true, run: () => putInSub(modId, tag) })
        entries.push({ text: Theme.say("A new group…"), icon: "plus", run: () => subNameRequested(modId) })
        if (about.sub) {
            entries.push({ text: Theme.fill("Remove the tag {}", about.sub), run: () => putInSub(modId, "") })
            entries.push({ text: Theme.fill("Delete the group {}…", about.sub), icon: "trash-2", danger: true, run: () => Notices.ask({
                title: Theme.fill("Delete the group {}", about.sub),
                lines: [Theme.plural(about.groupSize, "{} mod is in it. It keeps its place in the load order and loses only this tag.", "{} mods are in it. They keep their places in the load order and lose only this tag.")],
                label: Theme.say("Delete the group"), run: () => Hub.mods.deleteGroup(about.sub) }) })
        }
        return entries
    }

    readonly property RowMenu menus: RowMenu {}
    function rowMenu(row, x, y) {
        menus.open(row, x, y)
    }

    function unsubscribe(modId) {
        const mod = about(modId)
        Notices.ask({
            title: Theme.fill("Unsubscribe from {}", mod.name || ""),
            lines: [Theme.say("Steam, which must be running, unsubscribes and then deletes the mod's folder:"), { path: mod.path || "" }],
            label: Theme.say("Unsubscribe"), danger: true, run: () => Hub.downloads.unsubscribe(modId) })
    }

    function openFolder(modId) {
        Qt.openUrlExternally(Theme.fileUrl(about(modId).path))
    }

    property var slid: ({ modId: -1, by: 0 })
    property bool sorting: false
    property Timer slideEnd: Timer { interval: 400; onTriggered: { mods.slid = ({ modId: -1, by: 0 }); mods.sorting = false } }
    function slide(modId, by) {
        slid = ({ modId: modId, by: by })
        slideEnd.restart()
    }

    function nudge(model, by) {
        if (Hub.mods.previewing || (model.active && columns.shownBy !== "") || !Hub.mods.nudge(model.active, selectedIds, by))
            return
        slide(selectedId, -by)
        reveal(model, model.rowOf(model.indexOf(cursor)), false)
    }

    readonly property var fresh: Theme.demo ? [] : (Hub.mods.fresh || [])
    function activateFresh() { Hub.mods.activateFresh() }

    function keepNote(tags, note) { Hub.mods.keepNote(selectedId, tags, note) }

    function showMod(modId) {
        const at = active.indexOf(modId)
        select(modId, at >= 0)
        if (at >= 0)
            reveal(active, active.rowOf(at), true)
        else if (inactive.indexOf(modId) >= 0) {
            prefs.inactiveHidden = false
            reveal(inactive, inactive.rowOf(inactive.indexOf(modId)), true)
        }
    }

    function filterBy(text) {
        filterRequested(text)
    }

    function useList(ids, quiet) { Hub.mods.useList(ids, !!quiet) }

    // By library id; -1 for none.
    property int selectedId: -1
    property bool selectedActive: false
    readonly property var info: Hub.mods.details || ({})
    readonly property var textures: Hub.mods.textures || ({})
    readonly property var files: tab === 2 && selectedId >= 0 ? (Hub.mods.filesOf(selectedId) || ({})) : ({})

    function select(modId, inActive) {
        selectedId = modId
        selectedActive = inActive
        cursor = modId
        also = ({})
        if (leftOffRestored)
            prefs.lastMod = modId >= 0 ? String(info.packageId || "").toLowerCase() : ""
    }

    // Which tab of the details panel shows: 0 About, 1 Load order, 2 Files.
    property int tab: 0

    // Library id -> true.
    property var also: ({})
    readonly property int selectedCount: selectedId < 0 ? 0 : 1 + Object.keys(also).length
    // The mod the arrow keys go on from: the one last added to the selection or reached with Ctrl.
    property int cursor: -1

    function selectAlso(modId, inActive) {
        if (selectedId < 0 || inActive !== selectedActive)
            return select(modId, inActive)
        if (modId === selectedId || modId < 0)
            return
        cursor = modId
        const next = Object.assign({}, also)
        if (next[modId])
            delete next[modId]
        else
            next[modId] = true
        also = next
    }

    function selectThrough(modId, inActive) {
        if (selectedId < 0 || inActive !== selectedActive)
            return select(modId, inActive)
        const others = Hub.mods.othersThrough(inActive, selectedId, modId)
        if (!others)
            return
        cursor = modId
        alsoOf(others)
    }

    // Keeps the first one selected when it is shown.
    function selectAll(model) {
        const shown = Hub.mods.shownIds(model.active)
        if (selectedActive !== model.active || shown.indexOf(selectedId) < 0)
            select(shown.length > 0 ? shown[0] : -1, model.active)
        alsoOf(shown)
    }

    function alsoOf(ids) {
        const next = {}
        for (const id of ids)
            next[id] = true
        delete next[selectedId]
        also = next
    }

    function moveSelected() {
        Hub.mods.takeAll(selectedActive, selectedIds)
        select(-1, false)
    }

    signal showSection(int section)
    signal findBrokenRequested()

    property int problemRow: -1
    property int problemPlace: 0
    property int problemMod: -1
    readonly property int problemCount: Hub.mods.troubled
    function showNextProblem() {
        showProblem(true)
    }
    function showProblem(forward) {
        problemRow = Hub.mods.nextProblem(problemRow, forward)
        problemPlace = Hub.mods.problemPlace(problemRow)
        if (problemRow < 0)
            return
        reveal(active, problemRow, false)
        select(active.modIdAt(problemRow), true)
        problemMod = selectedId
    }

    function previewSort() {
        if (Hub.mods.previewing || !Hub.mods.filesRead)
            return
        sorting = true
        slideEnd.restart()
        Hub.mods.previewSort()
        // Once the list has laid its new rows out: before that it has no place to go to.
        Qt.callLater(() => {
            const row = Hub.mods.firstMoved()
            if (row >= 0)
                reveal(active, row, true)
        })
    }

    function endSort(keep) { Hub.mods.endSort(keep) }

    function moveAcross(fromModel, modIndex, modId) {
        Hub.mods.take(fromModel.active, modIndex, -1)
        if (modId >= 0 && modId === selectedId)
            selectedActive = !fromModel.active
    }

    function toggleSelected() {
        const from = selectedActive ? active : inactive
        const index = from.indexOf(selectedId)
        if (index >= 0)
            moveAcross(from, index, selectedId)
    }

    readonly property Dragging drag: Dragging {
        onDropped: (from, to) => mods.drop(from, to)
    }
    function drop(from, to) {
        if (from.model === to.model) {
            const place = Math.min(to.modIndex, to.model.count - 1)
            if (from.modIndex !== place) {
                slide(from.modId, from.modIndex - place)
                Hub.mods.moveMod(to.model.active, from.modIndex, place)
            }
        } else {
            Hub.mods.take(from.model.active, from.modIndex, to.modIndex)
            if (from.modId >= 0 && from.modId === selectedId)
                selectedActive = to.model.active
        }
    }

    // `{ earlier, later }` by library id; null when the lists are showing.
    property var comparing: null
    property var compareView: null

    function nthMod(model, position) {
        const row = model.rowOf(position)
        return row < 0 ? -1 : model.modIdAt(row)
    }
    readonly property string checkMode: {
        return Hub.given("--check") ? Hub.argument("--check") || "all" : ""
    }

    function firstFrame() {
        Hub.mods.markReady()
        firstFrameShown()
    }
}
