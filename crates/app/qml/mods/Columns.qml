import QtQuick
import RimStack.Ui

QtObject {
    id: columns

    required property ModsPrefs prefs
    // No load times are known until the game has been started from here
    // on its present version.
    required property bool loadTimes
    signal shownByAsked()

    readonly property ModsData rules: Hub.mods
    readonly property var labels: ({ author: "Author", version: "Version", game: "Game", load: "Load", size: "Size" })
    readonly property var choices: rules.columnChoices(prefs.columns, loadTimes)
    // Why the load column is not offered; empty while load times are known.
    readonly property string note: rules.columnNote(loadTimes)
    readonly property var chosen: prefs.columns === "" ? [] : prefs.columns.split(",")
    readonly property var shown: chosen.filter(column => column !== "load" || loadTimes)
    // At 100% text, where a column was dragged.
    readonly property var chosenWidths: rules.columnWidths(prefs.columnSizes)
    readonly property var widths: ({ author: Theme.sized(chosenWidths.author || 142), version: Theme.sized(chosenWidths.version || 84), game: Theme.sized(chosenWidths.game || 80), load: Theme.sized(chosenWidths.load || 56), size: Theme.sized(chosenWidths.size || 76) })

    // "" for the load order itself.
    property string shownBy: ""
    property bool descending: false
    function showBy(column) {
        const now = rules.columnPressed(shownBy, descending, column)
        shownBy = now[0]
        descending = now[1]
        shownByAsked()
    }

    function toggle(column) { prefs.columns = rules.columnsToggled(prefs.columns, column) }
    function move(column, to) { prefs.columns = rules.columnsMoved(prefs.columns, column, to) }
    function size(column, width) { prefs.columnSizes = rules.columnSized(prefs.columnSizes, column, width / Theme.grow) }

    function menu() {
        return choices.map(choice => ({ text: choice.mark + choice.text, off: choice.off, run: () => toggle(choice.column) }))
    }
}
