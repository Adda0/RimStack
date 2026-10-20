// Driven by tools/check-app.sh: a saved list is compared with the load order,
// and the mods only the list holds join the current run. Nothing is saved.
// Exits with 1 if any check failed.
pragma ComponentBehavior: Bound
import QtCore
import QtQuick
import RimStack.Ui

Item {
    id: checks
    property var window

    Component.onCompleted: checks.runListsChecks()

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

        let comp = Hub.lists.compareList(name)
        check("a list next to itself differs nowhere", comp !== null && comp.onlySaved.length === 0 && comp.onlyCurrent.length === 0 && comp.moved.length === 0 && comp.addable.length === 0)

        Hub.mods.take(true, 0, -1)
        check("the first mod is switched off", Mods.active.count === count - 1)

        comp = Hub.lists.compareList(name)
        check("the saved list alone still holds it", comp.onlySaved.length === 1)
        check("it is offered to join again", comp.addable.length === 1)
        check("nothing sits only in the load order", comp.onlyCurrent.length === 0)

        Hub.mods.takeAll(false, comp.addable)
        check("merging brings it back", Mods.active.count === count)
        comp = Hub.lists.compareList(name)
        check("the lists hold the same again", comp.onlySaved.length === 0 && comp.onlyCurrent.length === 0)

        const one = Mods.nthMod(Mods.inactive, 1)
        const another = Mods.nthMod(Mods.inactive, 3)
        check("an's local mod can be selected", one >= 0 && another >= 0 && one !== another)
        Mods.select(one, false)
        Mods.selectAlso(another, false)
        const cleanup = Hub.mods.cleanup(Mods.selectedIds)
        check("a multi-selection of inactive local mods offers both for the bin, none for Steam", cleanup.bin === 2 && cleanup.unsubscribe === 0 && cleanup.binBytes > 0)
        Mods.select(-1, false)

        Hub.lists.remove(name)
        comp = Hub.lists.compareList(name)
        check("a deleted list does not compare", comp !== null && comp.onlySaved.length === 0 && comp.onlyCurrent.length === 0 && comp.moved.length === 0)

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

        console.log(failed === 0 ? "check ok    a saved list compares and merges" : "check FAIL " + failed + " list checks failed")
        Qt.exit(failed)
    }
}
