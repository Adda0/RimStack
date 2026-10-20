// Driven by tools/check-app.sh: a saved list is compared with the load order,
// and the mods only the list holds join the current run. Nothing is saved.
// Exits with 1 if any check failed.
pragma ComponentBehavior: Bound
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

        Hub.lists.remove(name)
        comp = Hub.lists.compareList(name)
        check("a deleted list does not compare", comp !== null && comp.onlySaved.length === 0 && comp.onlyCurrent.length === 0 && comp.moved.length === 0)

        console.log(failed === 0 ? "check ok    a saved list compares and merges" : "check FAIL " + failed + " list checks failed")
        Qt.exit(failed)
    }
}
