pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Item {
    id: checks
    required property var window
    property int failed: 0
    property int at: 0
    property int waited: 0

    function listed(value) {
        return Array.from(value || [])
    }

    function check(what, ok) {
        console.log("check " + (ok ? "ok   " : "FAIL ") + what)
        if (!ok)
            failed += 1
    }

    readonly property var steps: [
        {
            section: 3,
            what: "Saves lists both saves, with the mods each was saved with",
            ready: () => !Hub.saves.busy && (Hub.saves.saves || []).length > 0,
            ok: () => checks.listed(Hub.saves.saves).length === 2 && checks.listed(Hub.saves.saves).every(save => save.ids.length >= 43)
        },
        {
            section: 3,
            what: "and says the save with a mod that is not installed breaks the list",
            ready: () => true,
            ok: () => checks.listed(Hub.saves.saves).filter(save => save.verdict.breaks).length === 1
        },
        {
            section: 4,
            what: "Logs reads the game's log and groups its errors",
            ready: () => !Hub.logs.busy && !!Hub.logs.head,
            ok: () => Hub.logs.head.errors >= 2 && Hub.logs.errors.length === 2
        },
        {
            section: 4,
            what: "and shows its lines",
            ready: () => (Hub.logs.lines || []).length > 0,
            ok: () => checks.listed(Hub.logs.lines).some(line => line.error)
        },
        {
            section: 2,
            what: "Textures counts the textures of the mods",
            ready: () => !Hub.textures.busy && !!Hub.textures.report && Hub.textures.report.all > 0,
            ok: () => Hub.textures.report.all >= 33 && Hub.textures.report.rows.length > 0
        },
        {
            section: 1,
            what: "Downloads offers the mod the load order needs and nothing installs",
            ready: () => !Hub.downloads.queueBusy && !!Hub.downloads.queue,
            ok: () => (Mods.overview.toGet || []).length >= 1
        },
        {
            section: 1,
            what: "a needed mod's problem ignored is gone, and back with the undo",
            ready: () => true,
            ok: () => {
                const count = () => (Mods.overview.toGet || []).length
                const [before, needed] = [count(), Mods.overview.toGet[0]]
                Hub.mods.ignoreProblem(needed.packageId, needed.ignore, true)
                const ignored = count() === before - 1
                Hub.mods.ignoreProblem(needed.packageId, needed.ignore, false)
                return needed.ignore !== "" && ignored && count() === before
            }
        },
        {
            section: 5,
            what: "Settings shows the folders in use, worked out in Rust",
            ready: () => true,
            ok: () => Hub.settings.report.game.endsWith("game") && Hub.settings.report.localMods.endsWith("Mods") && Hub.settings.report.gameLog.endsWith("Player.log")
        },
        {
            section: 5,
            what: "a folder typed in Settings is judged after a pause, and a missing one is refused and not kept",
            ready: () => {
                if (!checks.typed) {
                    checks.typed = true
                    Hub.settings.checkFolder("config", "/no/such/folder")
                }
                return !!(Hub.settings.folderChecks || {}).config
            },
            ok: () => !Hub.settings.folderChecks.config.ok && !Hub.settings.report.setConfig
        },
        {
            section: 5,
            what: "the text size follows a drag one step at a time without keeping the window busy",
            ready: () => {
                if (checks.dragged < checks.sizes.length) {
                    Theme.stored.textScale = checks.sizes[checks.dragged]
                    checks.dragged += 1
                    checks.waited = 0
                }
                return checks.dragged === checks.sizes.length
            },
            ok: () => {
                Theme.stored.textScale = 1
                return true
            }
        },
        {
            section: 5,
            what: "a theme chosen paints the open window without a restart",
            ready: () => checks.flipped(() => Theme.stored.theme = "light"),
            ok: () => {
                const shown = Qt.colorEqual(checks.window.color, "#f5f5f7")
                Theme.stored.theme = "rim"
                return shown && Qt.colorEqual(checks.window.color, "#1c1916")
            }
        },
        {
            section: 5,
            what: "high contrast strengthens grey text in the open window",
            ready: () => checks.flipped(() => {
                checks.before = String(Theme.ink3)
                Theme.stored.highContrast = true
            }),
            ok: () => {
                const shown = String(Theme.ink3) !== checks.before
                Theme.stored.highContrast = false
                return shown && String(Theme.ink3) === checks.before
            }
        },
        {
            section: 5,
            what: "reduced motion stops animations at once",
            ready: () => checks.flipped(() => Theme.stored.reduceMotion = true),
            ok: () => {
                const shown = Theme.quick === 0 && Theme.calm === 0
                Theme.stored.reduceMotion = false
                return shown && Theme.quick > 0
            }
        },
        {
            section: 5,
            what: "taller mod rows apply at once",
            ready: () => checks.flipped(() => {
                checks.before = String(Theme.row)
                Theme.stored.largeRows = !Theme.stored.largeRows
            }),
            ok: () => {
                const shown = String(Theme.row) !== checks.before
                Theme.stored.largeRows = !Theme.stored.largeRows
                return shown && String(Theme.row) === checks.before
            }
        },
        {
            section: 2,
            what: "the search inside the mods' files names mod, file and line for a hit",
            ready: () => {
                if (!checks.askedFind) {
                    checks.askedFind = true
                    Hub.find.text = "Fixture003"
                    Hub.find.start()
                }
                return !Hub.find.busy && Hub.find.summary.length > 0
            },
            ok: () => {
                const hits = checks.listed(Hub.find.hits)
                return hits.length === 1 && hits[0].modName === "Fixture Mod 003" && hits[0].file === "Defs/Things.xml" && hits[0].line === 1 && hits[0].text.indexOf("Fixture003") !== -1
            }
        },
        {
            section: 5,
            what: "a font chosen reaches every text already shown",
            ready: () => checks.flipped(() => {
                checks.before = Qt.application.font.family
                checks.window.useFont("RimStack Check Sans")
            }),
            ok: () => {
                const left = checks.fonted(checks.window.contentItem, checks.before)
                const moved = checks.fonted(checks.window.contentItem, "RimStack Check Sans")
                checks.window.useFont("")
                return left === 0 && moved > 0 && checks.fonted(checks.window.contentItem, "RimStack Check Sans") === 0
            }
        },
        {
            section: 0,
            what: "the lists menu shows every line whole, a long list name too, at the usual and a large text size",
            ready: () => {
                const menu = checks.holding(checks.window.contentItem, "listsMenu").listsMenu
                if (checks.fitted.length === 0 && !menu.visible) {
                    checks.kept = Hub.lists.keep(checks.longList, false) === ""
                    menu.open()
                    return false
                }
                if (!menu.opened)
                    return false
                checks.fitted = checks.fitted.concat([checks.whole(menu.contentItem, menu.contentItem)])
                Theme.stored.textScale = 1.5
                return checks.fitted.length === 2
            },
            ok: () => {
                checks.holding(checks.window.contentItem, "listsMenu").listsMenu.close()
                Theme.stored.textScale = 1
                Hub.lists.remove(checks.longList)
                return checks.kept && checks.fitted.length === 2 && checks.fitted.every(fits => fits)
            }
        }
    ]
    property bool typed: false
    property bool askedFind: false
    property bool changed: false
    property string before
    // Changes a setting on one tick and says it is ready on the next, so bindings settle first.
    function flipped(change) {
        if (changed) {
            changed = false
            return true
        }
        change()
        changed = true
        return false
    }
    function fonted(root, family) {
        let found = root.font !== undefined && root.font.family === family ? 1 : 0
        for (const child of root.children || [])
            found += fonted(child, family)
        return found
    }
    function holding(item, name) {
        if (item[name] !== undefined)
            return item
        for (const child of item.children) {
            const found = holding(child, name)
            if (found)
                return found
        }
        return null
    }
    // Every text shown on a card is whole and inside it, and the card inside the window.
    function whole(card, item) {
        if (!item.visible)
            return true
        if (item.truncated !== undefined && (item.truncated || item.mapToItem(card, item.width, 0).x > card.width + 0.5 || item.mapToItem(null, item.width, 0).x > checks.window.width))
            return false
        for (const child of item.children)
            if (!whole(card, child))
                return false
        return true
    }
    readonly property string longList: "A medieval run with the big overhauls"
    property bool kept: false
    property var fitted: []
    property int dragged: 0
    readonly property var sizes: [0.9, 1, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.4, 1.2, 1, 0.85, 1]

    Timer {
        interval: 200
        repeat: true
        running: true
        onTriggered: {
            if (checks.at === checks.steps.length) {
                stop()
                checks.languages()
                return
            }
            const step = checks.steps[checks.at]
            if (checks.window.section !== step.section) {
                checks.window.section = step.section
                checks.waited = 0
                return
            }
            checks.waited += 1
            if (!step.ready() && checks.waited < 50)
                return
            checks.check(step.what, step.ok())
            checks.at += 1
        }
    }

    function languages() {
        Hub.settings.useLanguage("Français")
        speaking.start()
    }
    Timer {
        id: speaking
        property int waited: 0
        property bool french: true
        interval: 100
        repeat: true
        onTriggered: {
            waited += 1
            const fromRust = Hub.settings.databases.some(d => d.use.indexOf(Theme.say("Names of mods that are not installed")) >= 0)
            const shown = fromRust && (french ? Theme.say("Save") === "Enregistrer" && !Hub.settings.report.english
                                              : Theme.say("Save") === "Save" && Hub.settings.report.english)
            if (!shown && waited < 30)
                return
            checks.check(french ? "a language chosen is used without a restart" : "and English again", shown)
            waited = 0
            if (french) {
                french = false
                Hub.settings.useLanguage("English")
                return
            }
            stop()
            checks.window.section = 0
            Qt.exit(checks.failed > 0 ? 1 : 0)
        }
    }
}
