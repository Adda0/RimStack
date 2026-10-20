// Opened by address only when asked for, so RimStack itself never loads QtTest.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtTest
import RimStack.Ui

Item {
    id: checks
    property ModsScreen screen
    property Item activePane
    readonly property ListView activeList: activePane?.list ?? null
    property ListView inactiveList
    property Item details
    readonly property Item viewButton: activePane?.viewButton ?? null
    readonly property Popup viewMenu: activePane?.viewMenu ?? null
    property int failed: 0

    function check(what, ok) {
        console.log("check " + (ok ? "ok   " : "FAIL ") + what)
        if (!ok)
            failed += 1
    }

    function divider() {
        const wide = details.width, edge = details.mapToItem(screen, 0, 200)
        events.mouseMove(screen, edge.x - 1, edge.y, -1, Qt.NoButton, Qt.NoModifier)
        check("the pointer over the divider moves nothing", details.width === wide)
        events.mousePress(screen, edge.x - 1, edge.y, Qt.LeftButton, Qt.NoModifier, -1)
        for (let i = 1; i <= 6; i++)
            events.mouseMove(screen, edge.x - 1 - 10 * i, edge.y, 10, Qt.LeftButton, Qt.NoModifier)
        events.mouseRelease(screen, edge.x - 61, edge.y, Qt.LeftButton, Qt.NoModifier, 10)
        check("dragging the divider sixty pixels widens the details by as much", Math.abs(details.width - wide - 60) <= 2)
        Mods.prefs.details = wide
    }

    function dueHint(item) {
        for (const child of item.data) {
            if (child.due === true && child.step > 0)
                return child
            const found = child.data ? checks.dueHint(child) : null
            if (found)
                return found
        }
        return null
    }

    function hints() {
        const selected = Mods.selectedId
        let shown = 0, below = true
        Mods.hintStep = 1
        for (let hint = checks.dueHint(screen); hint && shown < 3; hint = checks.dueHint(screen)) {
            const top = hint.background.mapToItem(null, 0, 0), under = hint.parent.mapToItem(null, 0, hint.parent.height)
            below = below && Math.abs(top.y - under.y - 9) < 1 && hint.opened
            const row = hint.contentItem.children[1], next = row.children[row.children.length - 1]
            events.mouseClick(next, next.width / 2, next.height / 2, Qt.LeftButton, Qt.NoModifier, -1)
            if (checks.dueHint(screen) === hint)
                break
            shown += 1
        }
        check("Next and Done on the first-run hints go through each and close the last", shown > 0 && Mods.hintStep === 0)
        check("and reach nothing under the hints", Mods.selectedId === selected)
        check("each hint shows just below its button", below)
        Mods.endHints()
    }

    function suggestions() {
        const away = () => events.mouseClick(details, details.width / 2, details.height - 8, Qt.LeftButton, Qt.NoModifier, -1)
        Mods.filterFocusRequested()
        const filter = screen.Window.activeFocusItem.parent
        const opened = filter.suggesting
        away()
        check("a click away from the filter closes its suggestions", opened && !filter.suggesting && filter.editing)
        events.mouseClick(filter, filter.width / 2, filter.height / 2, Qt.LeftButton, Qt.NoModifier, -1)
        check("a click back in the filter opens them again", filter.suggesting)
        away()
        events.keyClick(Qt.Key_I, Qt.NoModifier, -1)
        check("and typing opens them again", filter.suggesting)
        const selected = Mods.selectedId
        events.mouseClick(filter, 40, filter.height + 4 + 6 + 13, Qt.LeftButton, Qt.NoModifier, -1)
        check("a click on a suggestion takes it and reaches nothing under it", Mods.filtered.startsWith("is:") && Mods.selectedId === selected)
        events.keyClick(Qt.Key_Escape, Qt.NoModifier, -1)
        check("Esc empties the filter and closes its suggestions", !filter.suggesting && filter.editing && Mods.filtered === "")
        events.keyClick(Qt.Key_I, Qt.NoModifier, -1)
        away()
        events.keyClick(Qt.Key_Escape, Qt.NoModifier, -1)
        check("and empties it with them closed, ahead of the screen's own Esc", Mods.filtered === "")
    }

    function field(item, placeholder) {
        for (const child of item.data) {
            if (child.placeholder === placeholder)
                return child
            const found = child.data ? checks.field(child, placeholder) : null
            if (found)
                return found
        }
        return null
    }

    function tagOffers() {
        const mine = Mods.nthMod(Mods.active, 0), other = Mods.nthMod(Mods.active, 1)
        Hub.mods.addTags(other, "pointer-pick")
        Mods.select(mine, true)
        Mods.tab = 0
        const box = checks.field(details, "Add tags, separated by commas")
        box.focusInput()
        for (const c of "pointer-p")
            events.keyClickChar(c, Qt.NoModifier, -1)
        check("typing part of another mod's tag offers it", box.suggesting)
        events.keyClick(Qt.Key_Return, Qt.NoModifier, -1)
        check("Enter with no offer picked adds what is typed", (Mods.info.tags || "").split(", ").indexOf("pointer-p") >= 0)
        for (const c of "pointer-p")
            events.keyClickChar(c, Qt.NoModifier, -1)
        events.keyClick(Qt.Key_Down, Qt.NoModifier, -1)
        events.keyClick(Qt.Key_Return, Qt.NoModifier, -1)
        check("Down then Enter adds the offered tag", (Mods.info.tags || "").split(", ").indexOf("pointer-pick") >= 0)
        for (const tag of ["pointer-p", "pointer-pick"])
            Hub.mods.removeTag(mine, tag)
        Hub.mods.removeTag(other, "pointer-pick")
        events.keyClick(Qt.Key_Escape, Qt.NoModifier, -1)
        Mods.select(-1, false)
    }

    function nameAt(row) {
        return Mods.active.at(row).name
    }
    function entry(menu, text) {
        menu.column.forceLayout()
        for (const child of menu.column.children)
            if (child.modelData && child.modelData.text === text)
                return child
        return null
    }
    function labelled(item, text) {
        for (const child of item.children)
            if (child.text === text)
                return child
        return null
    }
    function terms(item, into) {
        for (const child of item.children) {
            if (child.objectName === "term")
                into.push(child)
            checks.terms(child, into)
        }
        return into
    }

    function drag(from, fromY, to, toY) {
        from.parent.ensurePolished()
        to.parent.ensurePolished()
        from.forceLayout()
        to.forceLayout()
        const a = from.mapToItem(screen, 120, fromY), b = to.mapToItem(screen, 120, toY)
        events.mousePress(from, 120, fromY, Qt.LeftButton, Qt.NoModifier, -1)
        for (let i = 1; i <= 12; i++)
            events.mouseMove(screen, a.x + (b.x - a.x) * i / 12, a.y + (b.y - a.y) * i / 12, 20, Qt.LeftButton, Qt.NoModifier)
        events.mouseRelease(screen, b.x, b.y, Qt.LeftButton, Qt.NoModifier, 20)
    }

    Choice {
        id: probe
        property int at: 0
        visible: false
        options: ["One", "Two", "Three"]
        current: at
        onPicked: index => at = index
    }
    Item {
        width: 200
        height: 90
        RowKeys {
            id: rows
            property int at: 0
            property int presses: 0
            visible: false
            count: 3
            current: at
            onChosen: index => at = index
            onEntered: presses += 1
        }
    }
    function keys() {
        if (Mods.compareView)
            events.keyClick(Qt.Key_Escape, Qt.NoModifier, -1)
        if (screen.menuOpen)
            events.keyClick(Qt.Key_Escape, Qt.NoModifier, -1)
        probe.visible = true
        probe.forceActiveFocus()
        events.keyClick(Qt.Key_Right, Qt.NoModifier, -1)
        events.keyClick(Qt.Key_Right, Qt.NoModifier, -1)
        events.keyClick(Qt.Key_Right, Qt.NoModifier, -1)
        events.keyClick(Qt.Key_Left, Qt.NoModifier, -1)
        check("the arrow keys move a choice and stop at its ends", probe.at === 1)
        probe.visible = false
        rows.visible = true
        rows.forceActiveFocus()
        for (const key of [Qt.Key_Down, Qt.Key_Down, Qt.Key_Down, Qt.Key_Up])
            events.keyClick(key, Qt.NoModifier, -1)
        const stepped = rows.at === 1
        events.keyClick(Qt.Key_End, Qt.NoModifier, -1)
        const last = rows.at === 2
        events.keyClick(Qt.Key_Home, Qt.NoModifier, -1)
        events.keyClick(Qt.Key_Return, Qt.NoModifier, -1)
        check("a list of rows takes Up, Down, Home, End and Enter", stepped && last && rows.at === 0 && rows.presses === 1)
        events.keyClick(Qt.Key_PageDown, Qt.NoModifier, -1)
        const paged = rows.at === 2
        events.keyClick(Qt.Key_PageUp, Qt.NoModifier, -1)
        check("and Page Up and Page Down, stopping at its ends", paged && rows.at === 0)
        rows.visible = false
        checks.listKeys()
        events.keyClick(Qt.Key_2, Qt.ControlModifier, -1)
        const away = !screen.visible
        events.keyClick(Qt.Key_1, Qt.ControlModifier, -1)
        check("Ctrl with a figure goes to that section and back", away && screen.visible)
    }

    function listKeys() {
        const first = Mods.active.modIdAt(Hub.mods.stepFrom(true, -1, 1)), last = Mods.active.modIdAt(Hub.mods.stepFrom(true, -1, -1))
        Mods.select(first, true)
        Mods.focusList(Mods.active)
        events.keyClick(Qt.Key_End, Qt.NoModifier, -1)
        const atEnd = Mods.selectedId === last
        events.keyClick(Qt.Key_Home, Qt.NoModifier, -1)
        const atTop = Mods.selectedId === first
        events.keyClick(Qt.Key_PageDown, Qt.NoModifier, -1)
        check("a mod list takes Home, End and Page Down", atEnd && atTop && Mods.active.indexOf(Mods.selectedId) > 1)
        events.keyClick(Qt.Key_Home, Qt.NoModifier, -1)
        events.keyClick(Qt.Key_Down, Qt.ShiftModifier, -1)
        events.keyClick(Qt.Key_Down, Qt.ShiftModifier, -1)
        const three = Mods.selectedCount === 3
        events.keyClick(Qt.Key_Up, Qt.ShiftModifier, -1)
        check("Shift with the arrows selects through from the first mod selected", three && Mods.selectedCount === 2 && Mods.selectedId === first)
        events.keyClick(Qt.Key_Down, Qt.ControlModifier, -1)
        const kept = Mods.selectedCount === 2
        events.keyClick(Qt.Key_Space, Qt.ControlModifier, -1)
        check("Ctrl with the arrows goes on without selecting and Ctrl+Space adds the mod", kept && Mods.selectedCount === 3 && Mods.also[Mods.cursor] === true)
        events.keyClick(Qt.Key_A, Qt.ControlModifier, -1)
        check("Ctrl+A selects every mod shown", Mods.selectedCount === Hub.mods.shownIds(true).length && Mods.selectedId === first)
        Mods.select(first, true)
        events.keyClick(Qt.Key_Down, Qt.ShiftModifier, -1)
        const second = Mods.cursor, at = Mods.active.indexOf(first), count = Mods.active.count
        events.keyClick(Qt.Key_Down, Qt.AltModifier, -1)
        check("Alt+Down moves every selected mod one place", Mods.active.indexOf(first) === at + 1 && Mods.active.indexOf(second) === at + 2)
        events.keyClick(Qt.Key_Home, Qt.AltModifier, -1)
        check("and Alt+Home to the top of their category", Mods.active.indexOf(first) === at && Mods.active.indexOf(second) === at + 1)
        events.keyClick(Qt.Key_Return, Qt.NoModifier, -1)
        check("Enter moves every selected mod to the other list", Mods.active.count === count - 2 && Mods.inactive.indexOf(second) >= 0)
        Hub.mods.undo()
        Mods.prefs.headings = 1
        const mod = Mods.active.modIdAt(Hub.mods.stepFrom(true, -1, 1)), shown = Mods.active.rows()
        Mods.select(mod, true)
        Mods.focusList(Mods.active)
        events.keyClick(Qt.Key_Left, Qt.NoModifier, -1)
        const heading = checks.activeList.place, folded = Mods.active.rows() < shown && heading >= 0 && Mods.active.at(heading).isHeader
        events.keyClick(Qt.Key_Down, Qt.NoModifier, -1)
        const past = Mods.selectedId !== mod
        events.keyClick(Qt.Key_Up, Qt.NoModifier, -1)
        const back = Mods.selectedId === mod && checks.activeList.place === heading
        events.keyClick(Qt.Key_Right, Qt.NoModifier, -1)
        check("Left folds the selected mod's heading, the arrows stop on it and Right shows its mods again", folded && past && back && Mods.active.rows() === shown)
        Mods.prefs.headings = 0
        Mods.select(-1, false)
        checks.fieldKeys()
    }

    function fieldKeys() {
        Mods.filterFocusRequested()
        for (const c of "Fixture")
            events.keyClickChar(c, Qt.NoModifier, -1)
        const filter = screen.Window.activeFocusItem.parent, offered = filter.suggesting
        events.keyClick(Qt.Key_Down, Qt.NoModifier, -1)
        check("Down in the filter without offers goes to the list and its first mod", !offered && checks.activeList.activeFocus && Mods.selectedId === Hub.mods.stopOn(true, Hub.mods.stepFrom(true, -1, 1)))
        Mods.filterBy("")
        Mods.filterFocusRequested()
        events.keyClick(Qt.Key_Escape, Qt.NoModifier, -1)
        check("Esc in the empty filter goes back to the list", checks.activeList.activeFocus)
        Mods.tab = 0
        if (Theme.folded.indexOf("about-notes") >= 0)
            Theme.fold("about-notes")
        checks.field(details, "Add tags, separated by commas").focusInput()
        events.keyClick(Qt.Key_Escape, Qt.NoModifier, -1)
        const fromTags = checks.activeList.activeFocus
        checks.editable(details).forceActiveFocus()
        events.keyClick(Qt.Key_Escape, Qt.NoModifier, -1)
        check("Esc in the empty tag box, and in the note box, goes back to the list", fromTags && checks.activeList.activeFocus && Mods.selectedId >= 0)
        const inside = pane => {
            let item = screen.Window.activeFocusItem
            while (item && item !== pane)
                item = item.parent
            return !!item
        }
        events.keyClick(Qt.Key_F6, Qt.NoModifier, -1)
        const toDetails = inside(details)
        events.keyClick(Qt.Key_F6, Qt.NoModifier, -1)
        const toInactive = checks.inactiveList.activeFocus
        events.keyClick(Qt.Key_F6, Qt.NoModifier, -1)
        const round = checks.activeList.activeFocus
        events.keyClick(Qt.Key_F6, Qt.ShiftModifier, -1)
        check("F6 goes round the details, the inactive mods and the load order, Shift+F6 back", toDetails && toInactive && round && checks.inactiveList.activeFocus)
        events.keyClick(Qt.Key_PageDown, Qt.ControlModifier, -1)
        const next = Mods.tab === 1
        events.keyClick(Qt.Key_PageUp, Qt.ControlModifier, -1)
        events.keyClick(Qt.Key_PageUp, Qt.ControlModifier, -1)
        check("Ctrl+Page Down and Ctrl+Page Up go round the tabs of the details", next && Mods.tab === 2)
        Mods.tab = 0
        Mods.select(-1, false)
    }

    // The note box: the one text a person can edit that is not a field.
    function editable(item) {
        for (const child of item.data) {
            if (child.textDocument !== undefined && !child.readOnly && child.visible)
                return child
            const found = child.data ? checks.editable(child) : null
            if (found)
                return found
        }
        return null
    }

    TestEvent { id: events }
    Timer {
        interval: 300
        repeat: true
        running: true
        onTriggered: {
            if (Mods.active.count === 0 || !Hub.mods.filesRead)
                return
            stop()
            Mods.select(Mods.nthMod(Mods.active, 2), true)
            Mods.selectAlso(Mods.nthMod(Mods.active, 3), true)
            rescanned.chosen = Mods.about(Mods.selectedId).packageId
            selectionShown.start()
        }
    }
    Timer {
        id: selectionShown
        interval: Theme.calm + 200
        onTriggered: {
            for (const list of [checks.activeList, checks.inactiveList])
                list.contentY = Math.max(0, list.contentHeight - list.height)
            rescanned.places = [checks.activeList.contentY, checks.inactiveList.contentY]
            rescanned.resets = 0
            Hub.mods.rescan()
            rescanned.start()
        }
    }
    Connections { target: checks.activeList?.model ?? null; function onModelReset() { rescanned.resets += 1 } }
    Connections { target: checks.inactiveList?.model ?? null; function onModelReset() { rescanned.resets += 1 } }
    Timer {
        id: rescanned
        property var places: []
        property int resets: 0
        property string chosen
        interval: 300
        repeat: true
        onTriggered: {
            if (Hub.mods.rescanning || !Hub.mods.filesRead)
                return
            stop()
            checks.check("a rescan leaves both lists where they were scrolled", resets === 0 && checks.activeList.contentY === places[0] && checks.inactiveList.contentY === places[1])
            checks.check("and keeps the mods selected", Mods.selectedCount === 2 && Mods.selectedActive && Mods.about(Mods.selectedId).packageId === chosen && Mods.active.indexOf(Mods.selectedId) === 2)
            Mods.select(-1, false)
            checks.activeList.positionViewAtBeginning()
            checks.inactiveList.positionViewAtBeginning()
            checks.hints()
            checks.divider()
            checks.suggestions()
            checks.tagOffers()
            const middle = () => events.mouseClick(checks.viewButton, checks.viewButton.width / 2, checks.viewButton.height / 2, Qt.LeftButton, Qt.NoModifier, -1)
            checks.viewMenu.enter = null
            checks.viewMenu.exit = null
            middle()
            const shown = checks.viewMenu.visible
            middle()
            checks.check("a second click on View closes its menu", shown && !checks.viewMenu.visible)
            // `row` below assumes compact rows, 26 high.
            Mods.prefs.headings = 0
            Theme.stored.comfortable = false
            const row = y => 4 + 13 + 26 * y
            const moved = checks.nameAt(8), count = Mods.active.count

            checks.drag(checks.activeList, row(8), checks.activeList, row(12))
            checks.check("a mod dragged down the load order lands where it was dropped", checks.nameAt(12) === moved && checks.nameAt(8) !== moved)
            checks.check("and the list has unsaved changes", Hub.mods.dirty && Mods.active.count === count)

            const taken = Mods.inactive.at(2).name
            checks.drag(checks.inactiveList, 13 + 26 * 2, checks.activeList, row(5))
            checks.check("a mod dragged from the inactive list is activated where it was dropped", Mods.active.count === count + 1 && checks.nameAt(5) === taken)

            // Filtered to a few mods, a drop below the last row lands just after it (#14).
            Mods.filtered = "Fixture Mod 04"
            const shownRows = Mods.active.rows(), lastPlace = Mods.active.indexOf(Mods.active.modIdAt(shownRows - 1))
            let first = 0
            while (Mods.inactive.modIdAt(first) < 0)
                first += 1
            const added = Mods.inactive.modIdAt(first)
            checks.drag(checks.inactiveList, 13 + 26 * first, checks.activeList, row(shownRows) + 4)
            checks.check("a mod dropped below a filtered load order lands after the last mod it shows", Mods.active.indexOf(added) === lastPlace + 1 && lastPlace + 2 < Mods.active.count)
            Hub.mods.undo()
            Mods.filtered = ""
            checks.activeList.positionViewAtBeginning()

            events.mouseClick(checks.activeList, 120, row(3), Qt.RightButton, Qt.NoModifier, -1)
            checks.check("the right mouse button selects the mod and opens its menu", Mods.info.name === checks.nameAt(3) && checks.screen.menuOpen)
            menuShown.start()
        }
    }
    // A menu takes clicks once it has faded in, as a person's would.
    Timer {
        id: menuShown
        interval: 400
        onTriggered: {
            const menu = checks.screen.rowMenu
            const terms = checks.terms(details, [])
            // A Text's contentWidth runs past its width when it does not fit; a term with a meaning is no Text.
            checks.check("every term of the Facts fits before its value", terms.length >= 4 && terms.every(term => (term.contentWidth !== undefined ? term.contentWidth : term.implicitWidth) <= term.width + 0.5))
            checks.check("the row menu holds the groups in one entry", checks.entry(menu, Theme.say("Group under a tag")) !== null && checks.entry(menu, Theme.say("A new group…")) === null)
            checks.check("and the colours in one entry", checks.entry(menu, Theme.say("Colour")) !== null && checks.entry(menu, Theme.say("No colour")) === null)
            checks.check("a local copy's menu opens the page of the Workshop item it names, and offers no unsubscribing", !Mods.info.workshop && checks.entry(menu, Theme.say("Page in the browser")) !== null && checks.entry(menu, Theme.say("Unsubscribe…")) === null)
            const copy = checks.entry(menu, Theme.say("Copy name"))
            const selected = Mods.selectedId
            events.mouseClick(copy, copy.width / 2, copy.height / 2, Qt.LeftButton, Qt.NoModifier, -1)
            checks.check("a click on a menu entry closes the menu", !checks.screen.menuOpen)
            checks.check("and reaches no row under it", Mods.selectedId === selected)
            Hub.mods.addTags(Mods.selectedId, "pick-one, pick-two")
            events.mouseClick(checks.activeList, 120, 4 + 13 + 26 * 3, Qt.RightButton, Qt.NoModifier, -1)
            groupAsked.start()
        }
    }
    Timer {
        id: groupAsked
        interval: 400
        onTriggered: {
            const menu = checks.screen.rowMenu
            const group = checks.entry(menu, Theme.say("Group under a tag"))
            events.mouseClick(group, group.width / 2, group.height / 2, Qt.LeftButton, Qt.NoModifier, -1)
            checks.check("and that entry opens the groups in a menu of their own", checks.screen.menuOpen && checks.entry(menu, Theme.say("A new group…")) !== null)
            const picks = () => Array.from(menu.column.children).filter(child => child.modelData && child.modelData.pick)
            checks.check("the groups are listed by their tag names alone, after a search field", menu.search.visible && picks().map(child => child.modelData.text).indexOf("pick-two") >= 0)
            menu.search.focusInput()
            for (const key of [Qt.Key_Z, Qt.Key_Q, Qt.Key_X])
                events.keyClick(key, Qt.NoModifier, -1)
            checks.check("and typing in it leaves only the tags that hold the text", picks().every(child => !child.visible) && checks.entry(menu, Theme.say("A new group…")).visible)
            menu.close()
            Hub.mods.clearTags(Mods.selectedId)
            events.mouseClick(checks.activeList, 120, 4 + 13 + 26 * 3, Qt.RightButton, Qt.NoModifier, -1)
            colourAsked.start()
        }
    }
    Timer {
        id: colourAsked
        interval: 400
        onTriggered: {
            const menu = checks.screen.rowMenu
            const colour = checks.entry(menu, Theme.say("Colour"))
            events.mouseClick(colour, colour.width / 2, colour.height / 2, Qt.LeftButton, Qt.NoModifier, -1)
            checks.check("the Colour entry opens the swatches in a menu of their own, after a search field", checks.screen.menuOpen && menu.search.visible && checks.entry(menu, Theme.say("No colour")) !== null)
            const painted = () => Mods.active.at(Mods.active.rowOf(Mods.active.indexOf(Mods.selectedId))).colour
            for (const c of Theme.say("Rose").slice(0, 3))
                events.keyClickChar(c, Qt.NoModifier, -1)
            events.keyClick(Qt.Key_Return, Qt.NoModifier, -1)
            checks.check("typing a colour's name and Enter paint the mod with it", !checks.screen.menuOpen && painted() === "Rose")
            Hub.mods.paint([Mods.selectedId], "")
            const many = []
            for (let i = 0; i < 80; i++)
                many.push({ text: "check entry " + i, run: () => {} })
            Mods.menuRequested(checks.activeList, 0, 0, many)
            longShown.start()
        }
    }
    Timer {
        id: longShown
        interval: 400
        onTriggered: {
            const menu = checks.screen.rowMenu, scroll = menu.scroller
            checks.check("a menu longer than the window stays inside it", menu.y >= 0 && menu.y + menu.height <= menu.parent.height && scroll.contentHeight > scroll.height)
            events.keyClick(Qt.Key_Up, Qt.NoModifier, -1)
            const chosen = checks.entry(menu, "check entry " + menu.at), spot = chosen.mapToItem(scroll, 0, 0)
            checks.check("and scrolls to the entry the arrow keys are on", menu.at > 40 && spot.y >= 0 && spot.y + chosen.height <= scroll.height)
            events.keyClick(Qt.Key_Escape, Qt.NoModifier, -1)
            checks.screen.viewMenu.open()
            viewShown.start()
        }
    }
    Timer {
        id: viewShown
        interval: 400
        onTriggered: {
            const card = checks.screen.viewMenu
            const headed = checks.labelled(card.contentItem, Theme.say("Under category headings"))
            const selected = Mods.selectedId
            events.mouseClick(headed, headed.width / 2, headed.height / 2, Qt.LeftButton, Qt.NoModifier, -1)
            checks.check("a click on a button of the View card takes it and reaches no row under it", Mods.prefs.headings === 1 && Mods.selectedId === selected && card.visible)
            events.mouseClick(checks.activeList, 120, 4 + 13 + 26 * 2, Qt.LeftButton, Qt.NoModifier, -1)
            checks.check("a click beside the card closes it", !card.opened)
            checks.check("and selects no row", Mods.selectedId === selected)
            Mods.prefs.headings = 0
            cardGone.start()
        }
    }
    Timer {
        id: cardGone
        interval: 400
        onTriggered: {
            const before = checks.inactiveList.contentY
            events.mouseWheel(checks.inactiveList, 120, 200, Qt.NoButton, Qt.NoModifier, 0, -120, -1)
            wheelDone.before = before
            wheelDone.start()
        }
    }
    Timer {
        id: wheelDone
        property real before
        interval: 400
        onTriggered: {
            const moved = checks.inactiveList.contentY - before
            checks.check("one notch of the wheel scrolls the system's lines, not more", Math.abs(moved - Application.styleHints.wheelScrollLines * 40) < 2)
            Mods.select(Mods.nthMod(Mods.active, Math.min(20, Mods.active.count - 1)), true)
            Mods.focusList(Mods.active)
            checks.activeList.positionViewAtBeginning()
            events.keyClick(Qt.Key_Menu, Qt.NoModifier, -1)
            const menu = checks.screen.rowMenu
            checks.check("the Menu key opens the selected mod's menu", checks.screen.menuOpen && checks.entry(menu, Theme.say("Copy name")) !== null)
            events.keyClick(Qt.Key_Escape, Qt.NoModifier, -1)
            Mods.select(-1, false)
            comparing.start()
        }
    }
    Timer {
        id: comparing
        property int tries: 0
        interval: 500
        repeat: true
        onTriggered: {
            tries += 1
            if (Hub.mods.conflicts === 0 && tries < 20)
                return
            stop()
            seeking.start()
        }
    }
    Timer {
        id: seeking
        property int at: 0
        interval: 1
        repeat: true
        onTriggered: {
            if (at < Mods.active.count && Mods.comparing === null) {
                Mods.select(Mods.nthMod(Mods.active, at++), true)
                const other = (Mods.info.losesTo || []).concat(Mods.info.winsOver || [])[0]
                if (other)
                    Mods.compareWith(other.modId)
                return
            }
            stop()
            if (Mods.comparing === null) {
                checks.keys()
                popups.start()
            } else
                zooming.start()
        }
    }
    Timer {
        id: zooming
        interval: 600
        onTriggered: {
            const view = Mods.compareView, choice = view.zoomChoice
            const saved = !Hub.mods.dirty
            events.mouseClick(choice, choice.width - 16, choice.height / 2, Qt.LeftButton, Qt.NoModifier, -1)
            checks.check("a click on the last zoom makes the picture eight times its fitted size", view.zoom === 8)
            checks.check("and reaches nothing under the comparison", !Hub.mods.dirty === saved && !Hub.mods.previewing)
            const before = view.chosen
            events.keyClick(Qt.Key_Down, Qt.NoModifier, -1)
            checks.check("the down key shows the next shared texture", view.textures.length < 2 || view.chosen === before + 1)
            checks.keys()
            popups.start()
        }
    }

    // Dialogs, popups and shared controls by keyboard, one step every 400 ms: popups take
    // clicks and keys only once they have faded in.
    property int ran: 0
    function ask() {
        Notices.ask({ title: "Check", lines: [], label: "Run", run: () => checks.ran += 1 })
    }
    function asked() {
        return Notices.asking !== null
    }
    function focused() {
        return screen.Window.activeFocusItem
    }
    function find(item, test) {
        for (const child of item.children) {
            if (test(child))
                return child
            const found = checks.find(child, test)
            if (found)
                return found
        }
        return null
    }
    function every(item, test, into) {
        for (const child of item.children) {
            if (test(child))
                into.push(child)
            checks.every(child, test, into)
        }
        return into
    }
    property string chose
    function pick(texts) {
        return texts.map(text => text === "" ? { line: true } : { text: text, off: text === "Gamma", run: () => checks.chose = text })
    }
    function copied() {
        pasted.text = ""
        pasted.paste()
        return pasted.text
    }
    function logsScreen() {
        return checks.find(screen.Window.contentItem, item => item.recorded !== undefined && item.launches !== undefined)
    }
    property int waited: 0
    property Item order
    property string wasOrder
    Column {
        id: shared
        visible: false
        width: 300
        Slide { id: slider; from: 0; to: 1; step: 0.1; onMoved: value => slider.value = value }
        Chip { id: chipProbe; property int presses: 0; pressable: true; text: "Probe"; onClicked: presses += 1 }
        Card { id: cardProbe; key: "keys-check"; title: "Probe" }
        CopyText { id: copyProbe; text: "Copied\u200b text" }
        TextEdit { id: pasted; activeFocusOnTab: false }
    }
    readonly property var popupSteps: [
        () => {
            Mods.select(Mods.nthMod(Mods.active, 0), true)
            checks.activeList.forceActiveFocus()
            checks.ask()
        },
        () => {
            const count = Mods.active.count, selected = Mods.selectedId
            events.keyClick(Qt.Key_Return, Qt.NoModifier, -1)
            check("Enter in a confirmation runs its main action, and not the list's under it", checks.ran === 1 && Mods.active.count === count && Mods.selectedId === selected && Mods.selectedActive)
        },
        () => {
            check("and once it closes the keys are back on the list", !checks.asked() && checks.activeList.activeFocus)
            checks.ask()
        },
        () => {
            events.keyClick(Qt.Key_Tab, Qt.NoModifier, -1)
            events.keyClick(Qt.Key_Return, Qt.NoModifier, -1)
        },
        () => {
            check("Tab in a confirmation goes to Cancel, and Enter there runs nothing", checks.ran === 1 && !checks.asked())
            checks.ask()
        },
        () => {
            const selected = Mods.selectedId
            events.keyClick(Qt.Key_Tab, Qt.NoModifier, -1)
            events.keyClick(Qt.Key_Tab, Qt.NoModifier, -1)
            const inside = checks.focused().text === "Run"
            events.keyClick(Qt.Key_Escape, Qt.NoModifier, -1)
            check("Tab goes round a confirmation's buttons and Esc cancels it", inside && checks.ran === 1 && Mods.selectedId === selected)
        },
        () => {
            check("and closes it", !checks.asked())
            checks.screen.viewMenu.open()
        },
        () => {
            const card = checks.screen.viewMenu, headings = Mods.prefs.headings
            const first = checks.focused().text === Theme.say("Under category headings")
            events.keyClick(Qt.Key_Return, Qt.NoModifier, -1)
            const under = Mods.prefs.headings === 1
            events.keyClick(Qt.Key_Tab, Qt.NoModifier, -1)
            const next = checks.focused().text === Theme.say("One numbered list")
            events.keyClick(Qt.Key_Return, Qt.NoModifier, -1)
            check("a card of buttons opens on its first, Enter presses it and Tab goes to the next", first && under && next && Mods.prefs.headings === 0)
            events.keyClick(Qt.Key_Escape, Qt.NoModifier, -1)
            check("and Esc closes it", !card.opened)
            Mods.prefs.headings = headings
        },
        () => {
            checks.wasOrder = Mods.prefs.inactiveOrder
            Mods.prefs.inactiveOrder = Hub.mods.inactiveOrders.all[0].key
            checks.order = checks.find(screen, item => item.spoken === Theme.say("Order the inactive mods by"))
            checks.order.forceActiveFocus()
            events.keyClick(Qt.Key_Space, Qt.NoModifier, -1)
        },
        () => {
            const all = Hub.mods.inactiveOrders.all
            const listed = checks.screen.menuOpen && checks.screen.rowMenu.entries.length === all.length
            events.keyClick(Qt.Key_Down, Qt.NoModifier, -1)
            events.keyClick(Qt.Key_Up, Qt.NoModifier, -1)
            events.keyClick(Qt.Key_Return, Qt.NoModifier, -1)
            check("the inactive mods' order is a menu the keys pick from", listed && Mods.prefs.inactiveOrder === all[all.length - 1].key)
            Mods.prefs.inactiveOrder = checks.wasOrder
        },
        () => {
            checks.activeList.forceActiveFocus()
            Mods.menuRequested(checks.activeList, 0, 0, checks.pick(["Alpha", "Beta", "", "Bravo", "Gamma", "Delta"]))
        },
        () => {
            const menu = checks.screen.rowMenu, at = []
            for (const key of [Qt.Key_End, Qt.Key_Home, Qt.Key_B, Qt.Key_B, Qt.Key_B, Qt.Key_PageDown, Qt.Key_PageUp]) {
                events.keyClick(key, Qt.NoModifier, -1)
                at.push(menu.at)
            }
            check("a menu takes End, Home, Page Down and Page Up, and a letter goes to the next entry starting with it", at.join() === "5,0,1,3,1,5,0")
            events.keyClick(Qt.Key_Enter, Qt.KeypadModifier, -1)
            check("the keypad's Enter takes the entry, and the keys go back to the list", checks.chose === "Alpha" && checks.activeList.activeFocus)
            Mods.menuRequested(checks.activeList, 0, 0, checks.pick(["Alpha", "Beta"]))
            events.keyClick(Qt.Key_Down, Qt.NoModifier, -1)
            events.keyClick(Qt.Key_Down, Qt.NoModifier, -1)
            events.keyClick(Qt.Key_Space, Qt.NoModifier, -1)
            check("and so does Space", checks.chose === "Beta" && !checks.screen.menuOpen)
        },
        () => {
            shared.visible = true
            slider.forceActiveFocus()
            const moved = []
            for (const key of [Qt.Key_End, Qt.Key_Home, Qt.Key_PageUp, Qt.Key_PageDown]) {
                events.keyClick(key, Qt.NoModifier, -1)
                moved.push(Math.round(slider.value * 100))
            }
            check("a slider takes End, Home, Page Up and Page Down", moved.join() === "100,0,30,10")
            shared.visible = false
            probe.visible = true
            probe.forceActiveFocus()
            events.keyClick(Qt.Key_End, Qt.NoModifier, -1)
            const last = probe.at === 2
            events.keyClick(Qt.Key_Home, Qt.NoModifier, -1)
            check("a choice takes Home and End", last && probe.at === 0)
            probe.visible = false
        },
        () => {
            shared.visible = true
            chipProbe.forceActiveFocus()
            events.keyClick(Qt.Key_Enter, Qt.KeypadModifier, -1)
            check("a chip takes the keypad's Enter", chipProbe.presses === 1)
            events.keyClick(Qt.Key_Tab, Qt.NoModifier, -1)
            const folded = cardProbe.folded
            events.keyClick(Qt.Key_Enter, Qt.KeypadModifier, -1)
            const flipped = cardProbe.folded !== folded
            events.keyClick(Qt.Key_Enter, Qt.KeypadModifier, -1)
            check("a card's heading folds with the keypad's Enter", flipped && cardProbe.folded === folded)
            events.keyClick(Qt.Key_Tab, Qt.NoModifier, -1)
            const reached = copyProbe.activeFocus
            events.keyClick(Qt.Key_C, Qt.ControlModifier, -1)
            check("a text to copy is reached with Tab, and Ctrl+C copies all of it", reached && checks.copied() === "Copied text")
            shared.visible = false
        },
        () => events.keyClick(Qt.Key_6, Qt.ControlModifier, -1),
        () => {
            const size = checks.find(screen.Window.contentItem, item => item.title === Theme.say("Text size") && item.step !== undefined)
            let flick = size.parent
            while (flick.contentY === undefined)
                flick = flick.parent
            const shown = checks.every(flick.contentItem, item => item.activeFocusOnTab && item.visible, [])
            const last = shown[shown.length - 1]
            flick.contentY = 0
            const hidden = last.mapToItem(flick, 0, 0).y > flick.height
            last.forceActiveFocus()
            const y = last.mapToItem(flick, 0, 0).y
            check("a control focused below a page's fold scrolls into view", hidden && y >= 0 && y + last.height <= flick.height)
            events.keyClick(Qt.Key_1, Qt.ControlModifier, -1)
        },
        () => events.keyClick(Qt.Key_5, Qt.ControlModifier, -1),
        // Again until the log is read.
        () => {
            const logs = checks.logsScreen()
            logs.view = 2
            if (logs.lines.length === 0 && checks.waited++ < 25)
                return false
            const keys = checks.find(logs, item => item.title === Theme.say("Log") && item.count !== undefined)
            keys.forceActiveFocus()
            events.keyClick(Qt.Key_C, Qt.ControlModifier, -1)
            check("Ctrl+C on the log copies the marked line", logs.lines.length > 0 && checks.copied() === logs.lines[keys.current].text)
            events.keyClick(Qt.Key_1, Qt.ControlModifier, -1)
        },
    ]
    Timer {
        id: popups
        property int at: 0
        interval: 400
        repeat: true
        // A step that throws is not run again; one that returns false is, until it is ready.
        onTriggered: {
            if (at === checks.popupSteps.length) {
                stop()
                Qt.exit(checks.failed > 0 ? 1 : 0)
            } else if (checks.popupSteps[at++]() === false)
                at -= 1
        }
    }
}
