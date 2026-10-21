// Driven by tools/check-app.sh: a saved list is compared with the load order and with
// the game's ModsConfig.xml, and the mods only the list holds join the current run. Nothing is saved.
// Exits with 1 if any check failed.
pragma ComponentBehavior: Bound
import QtCore
import QtQuick
import RimStack.Ui

Item {
    id: checks
    property var window

    Component.onCompleted: checks.runListsChecks()

    ListsCompareView { id: compareCheck }

    function every(item, test, into) {
        for (const child of item.children) {
            if (test(child))
                into.push(child)
            checks.every(child, test, into)
        }
        return into
    }

    // A list kept from the load order, changed so it differs from the game's ModsConfig.xml
    // every way: a mod dropped, one never installed added, the last moved first.
    function compareWithGame(check, kept) {
        const last = kept[kept.length - 1]
        const changed = [last].concat(kept.slice(0, -1).filter((id, at) => at !== 1), ["check.never.installed"])
        check("a changed list is kept", Hub.lists.keepIds("check-compare", changed) === "")
        compareCheck.left = "list:check-compare"
        compareCheck.right = "game"
        compareCheck.open()
        const comp = compareCheck.comparison
        const same = (a, b) => a.toLowerCase() === b.toLowerCase()
        check("against the game's file the list alone holds the uninstalled mod",
              comp.onlyLeft.mods.length === 1 && comp.onlyLeft.mods[0].id === "check.never.installed" && !comp.onlyLeft.mods[0].installed)
        check("the game's file alone holds the dropped mod, installed",
              comp.onlyRight.mods.length === 1 && same(comp.onlyRight.mods[0].id, kept[1]) && comp.onlyRight.mods[0].installed)
        check("the mod moved first is the one placed elsewhere, by every other shared mod",
              comp.moved.mods.length === 1 && same(comp.moved.mods[0].id, last) && comp.moved.mods[0].by === kept.length - 2 && comp.addable.length === 0)
        const gameTitle = Theme.say("the game's ModsConfig.xml")
        const headings = [Theme.plural(1, "{} mod only in {}", "{} mods only in {}", "check-compare"),
                          Theme.plural(1, "{} mod only in {}", "{} mods only in {}", gameTitle),
                          Theme.plural(1, "{} mod placed elsewhere in {}", "{} mods placed elsewhere in {}", gameTitle)]
        const shown = checks.every(compareCheck.contentItem, item => item.objectName === "compareHeading" && item.visible, []).map(item => item.text)
        check("the headings count each group: " + shown.join(" | "), shown.join("\n") === headings.join("\n"))
        const copy = checks.every(compareCheck.contentItem, item => item.spoken === Theme.say("Copy the comparison") && item.visible, [])[0]
        if (copy)
            copy.clicked()
        const copied = Theme.pasted()
        const lines = copied.split("\n")
        const placed = Theme.plural(kept.length - 2, "{} place later", "{} places later")
        check("copy gives each heading, then a - line per mod with its package id",
              !!copy && copied === comp.report && headings.every(heading => lines[lines.indexOf(heading) + 1].startsWith("- "))
              && lines.some(line => line.startsWith("- ") && line.endsWith("[check.never.installed], " + Theme.say("not installed")))
              && lines.some(line => line.startsWith("- ") && line.toLowerCase().endsWith("[" + last.toLowerCase() + "], " + placed.toLowerCase())))
        compareCheck.close()
        Hub.lists.remove("check-compare")
    }

    function runListsChecks() {
        let failed = 0
        const check = (what, ok) => {
            console.log("check " + (ok ? "ok   " : "FAIL ") + what)
            failed = ok ? failed : failed + 1
        }

        const count = Mods.active.count
        check("several mods are active", count >= 2)

        const name = "check-list"
        check("list kept", Hub.lists.keep(name, false) === "")
        const kept = Hub.lists.named(name)
        check("enough mods to move one past the others", kept.length >= 4)
        checks.compareWithGame(check, kept)

        compareCheck.withLoadOrder("list:" + name)
        let comp = compareCheck.comparison
        check("Compare on a list sets it against the load order, where it differs nowhere", compareCheck.right === "current" && comp.left === name && comp.right === Theme.say("the load order") && comp.same && comp.onlyLeft.mods.length === 0 && comp.onlyRight.mods.length === 0 && comp.moved.mods.length === 0 && comp.addable.length === 0)
        compareCheck.close()

        Hub.mods.take(true, 0, -1)
        check("the first mod is switched off", Mods.active.count === count - 1)

        comp = Hub.lists.compareOrders("list:" + name, "current")
        check("the saved list alone still holds it", comp.onlyLeft.mods.length === 1)
        check("it is offered to join again", comp.addable.length === 1)
        check("nothing sits only in the load order", comp.onlyRight.mods.length === 0)

        Hub.mods.takeAll(false, comp.addable)
        check("merging brings it back", Mods.active.count === count)
        comp = Hub.lists.compareOrders("list:" + name, "current")
        check("the lists hold the same again", comp.onlyLeft.mods.length === 0 && comp.onlyRight.mods.length === 0)

        const one = Mods.nthMod(Mods.inactive, 1)
        const another = Mods.nthMod(Mods.inactive, 3)
        check("an's local mod can be selected", one >= 0 && another >= 0 && one !== another)
        Mods.select(one, false)
        Mods.selectAlso(another, false)
        const cleanup = Hub.mods.cleanup(Mods.selectedIds)
        check("a multi-selection of inactive local mods offers both for the bin, none for Steam", cleanup.bin === 2 && cleanup.unsubscribe === 0 && cleanup.binBytes > 0)
        Mods.select(-1, false)

        Hub.lists.remove(name)
        comp = Hub.lists.compareOrders("list:" + name, "current")
        check("a deleted list does not compare", comp !== null && !comp.same && comp.onlyLeft.mods.length === 0 && comp.onlyRight.mods.length === 0 && comp.moved.mods.length === 0)

        const code = Hub.mods.shareCode()
        check("a share code is made", code.startsWith("rimstack-list:1:"))
        let ids = Hub.lists.idsInText(code)
        check("the share code reads back as the active mods", ids.length === Mods.active.count && ids.indexOf("fixture.workshop") >= 0 && ids.indexOf("ludeon.rimworld") >= 0)
        ids = Hub.lists.idsInText("<ModsConfigData><activeMods><li>fixture.workshop</li></activeMods></ModsConfigData>")
        check("a pasted ModsConfig reads back", ids.length === 1 && ids[0] === "fixture.workshop")
        ids = Hub.lists.idsInText(code + "garbage!")
        check("garbage after a code is no list", ids.length === 0)
        ids = Hub.lists.idsInText("rimstack-list:1:not-a-code")
        check("a bad code gives no mods", ids.length === 0)

        const where = StandardPaths.writableLocation(StandardPaths.TempLocation)
        Hub.mods.exportList(Qt.url(where + "/rimstack-check.rml"))
        ids = Hub.lists.idsInFile(Qt.url(where + "/rimstack-check.rml")).ids
        check("an exported .rml reads back with the workshop mod", ids.length === Mods.active.count && ids.indexOf("fixture.workshop") >= 0)
        Hub.mods.exportList(Qt.url(where + "/rimstack-check.xml"))
        ids = Hub.lists.idsInFile(Qt.url(where + "/rimstack-check.xml")).ids
        check("an exported ModsConfig reads back the same way", ids.length === Mods.active.count && ids.indexOf("fixture.workshop") >= 0)

        const links = Hub.mods.workshopLinks()
        check("the workshop links name the one workshop mod", links === "https://steamcommunity.com/sharedfiles/filedetails/?id=2941000101\n")

        console.log(failed === 0 ? "check ok    a saved list compares with the load order and the game's file, and merges" : "check FAIL " + failed + " list checks failed")
        Qt.exit(failed)
    }
}
