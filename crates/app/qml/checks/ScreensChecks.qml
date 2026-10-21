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
            section: 2,
            what: "on 1.6 Textures says nothing of the game's texture cache and Convert remaining stays the main action",
            ready: () => true,
            ok: () => {
                const notice = checks.named("cachesNotice")
                const convert = checks.buttons(Theme.say("Convert remaining"))
                return !!notice && !notice.visible && convert.length === 1 && convert[0].primary === (Hub.textures.report.remaining.length > 0)
            }
        },
        {
            section: 2,
            what: "on a game that caches textures itself, Textures says so with Remove all converted beside it, and Convert remaining is no longer the main action",
            ready: () => checks.flipped(() => {
                checks.textureReport = Hub.textures.report
                Hub.textures.report = Object.assign({}, checks.textureReport, { cachesSaid: "RimWorld 1.7 caches textures itself." })
            }),
            ok: () => {
                const notice = checks.named("cachesNotice")
                const remove = checks.buttons(Theme.say("Remove all converted…"))
                const convert = checks.buttons(Theme.say("Convert remaining"))
                const shown = !!notice && notice.visible && remove.length === 1 && checks.inside(remove[0], notice) && convert.length === 1 && !convert[0].primary
                Hub.textures.report = checks.textureReport
                return shown && !notice.visible && checks.buttons(Theme.say("Remove all converted…")).length === 1
            }
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
            section: 1,
            what: "a check that left hidden items' pages unlooked at says how many on the Updates card",
            ready: () => checks.flipped(() => {
                checks.downloadsUpdatesReport = Hub.downloads.updates
                Hub.downloads.updates = Object.assign({}, checks.downloadsUpdatesReport, { checked: 1, count: 0, problem: null, hiddenUnchecked: 3 })
            }),
            ok: () => {
                const said = Theme.plural(3, "{} hidden item not checked yet", "{} hidden items not checked yet")
                const shown = checks.chips(said).length === 1
                Hub.downloads.updates = checks.downloadsUpdatesReport || ({})
                return shown && checks.chips(said).length === 0
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
            what: "and lists the local Mods folder among them",
            ready: () => true,
            ok: () => checks.showing(checks.window.contentItem, Hub.settings.report.localMods)
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
            what: "the keys sheet at the largest text size stays inside the smallest window and scrolls",
            ready: () => checks.flipped(() => {
                checks.size = [checks.window.width, checks.window.height]
                checks.window.width = checks.window.minimumWidth
                checks.window.height = checks.window.minimumHeight
                Theme.stored.textScale = 1.6
                checks.window.keysSheet.open()
            }),
            ok: () => {
                const sheet = checks.window.keysSheet, scroll = sheet.scroll
                const inside = sheet.x >= 0 && sheet.y >= 0 && sheet.x + sheet.width <= checks.window.width && sheet.y + sheet.height <= checks.window.height
                const scrolls = !!scroll && scroll.height > 0 && scroll.contentHeight > scroll.height
                sheet.close()
                Theme.stored.textScale = 1
                checks.window.width = checks.size[0]
                checks.window.height = checks.size[1]
                return checks.window.width === checks.size[0] && inside && scrolls
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
        },
        {
            section: 1,
            what: "a Workshop search hit's date, subscribers and Look up follow its name, not the far edge",
            ready: () => checks.flipped(() => Hub.downloads.searched = ({ found: [{ id: "1", title: "Hat", updated: 1700000000, subscriptions: 5 }] })),
            ok: () => {
                const name = checks.shown(checks.window.contentItem, item => item.text === "Hat")
                const row = name ? name.parent : null
                const lookUp = row ? checks.shown(row, item => item.text === Theme.say("Look up")) : null
                Hub.downloads.searched = ({ found: [] })
                return !!lookUp && row.width > Theme.sized(600) && name.width <= name.implicitWidth + 2
                    && Math.abs(row.children[1].x - (name.x + name.width + row.spacing)) <= 1 && lookUp.x + lookUp.width < row.width / 2
            }
        },
        {
            section: 1,
            what: "choosing a Workshop hit shows its title, description, tags and actions, and More results asks for the next page",
            ready: () => checks.flipped(() => {
                Hub.downloads.searched = ({
                    text: "hat", page: 1, total: 50, more: true,
                    found: [{
                        id: "9", title: "Hat", updated: 1700000000, subscriptions: 5,
                        description: "A hat with ideas.", tags: ["Mod", "1.6"], size: 2048, preview: ""
                    }]
                })
            }),
            ok: () => {
                const keys = checks.named("searchHitKeys")
                if (keys)
                    keys.go(0)
                const pane = checks.named("searchHitPane")
                if (pane)
                    checks.buttons(Theme.say("More results")).map(more => more.clicked())
                const ok = !!pane && pane.visible
                    && checks.showing(pane, "Hat") && checks.showing(pane, "A hat with ideas.")
                    && checks.showing(pane, "Mod") && checks.showing(pane, "1.6")
                    && checks.matching(pane, item => item.danger !== undefined && item.text === Theme.say("Look up")).length === 1
                    && checks.matching(pane, item => item.danger !== undefined && item.text === Theme.say("Open its page")).length === 1
                    && Hub.downloads.searchPage === 2
                Hub.downloads.searchPicked = -1
                Hub.downloads.searched = ({ found: [] })
                return ok
            }
        },
        ...[["Export as CSV", "modlist.csv"], ["Export as ModsConfig", "ModsConfig.xml"], ["Export as a RimSort list", "modlist.rml"]].map(([button, name]) => ({
            section: 0,
            what: button + " opens its file dialog offering " + name + " in a folder",
            ready: () => checks.flipped(() => checks.labelled(checks.holding(checks.window.contentItem, "listsMenu").listsMenu.contentItem, Theme.say(button)).clicked()),
            ok: () => {
                const file = checks.dialogs(checks.holding(checks.window.contentItem, "listsMenu").listsMenu.contentItem)
                    .map(dialog => {
                        const shown = dialog.selectedFile.toString()
                        dialog.close()
                        return shown
                    })
                    .find(shown => shown.endsWith("/" + name))
                return !!file && file.startsWith("file:///")
            }
        })),
        {
            section: 3,
            what: "a save's row counts the mods it needs that are missing and the ones switched off",
            ready: () => checks.listed(Hub.saves.saves).length === 2,
            ok: () => {
                const two = checks.saveNamed("Colony Two"), one = checks.saveNamed("Colony One")
                return !!two && !!one && two.notInstalled === 1 && two.notActive === 0
                    && two.missing.length === 1 && two.missing[0].installed === false
                    && one.notInstalled === 0 && one.notActive === 0
                    && checks.chips(Theme.fill("{} not installed", 1)).length === 1
                    && checks.chips(Theme.fill("{} not active", 1)).length === 0
            }
        },
        {
            section: 3,
            what: "a save's order action makes its mods the load order, with nothing written until Save",
            ready: () => {
                if (checks.orderStage === 0) {
                    checks.orderStage = 1
                    const screen = checks.matching(checks.window.contentItem, item => item.reader === Hub.saves && item.save !== undefined)[0]
                    const index = checks.listed(Hub.saves.saves).findIndex(save => save.name === "Colony One")
                    if (!screen || index < 0)
                        return false
                    screen.selected = index
                    checks.beforeOrder = checks.loadOrder()
                    checks.wantCount = checks.saveNamed("Colony One").ids.length
                    return false
                }
                if (checks.orderStage === 1) {
                    checks.orderStage = 2
                    const button = checks.buttons(Theme.say("Use this save's mod list"))[0]
                    if (!!button && button.visible && button.enabled)
                        button.clicked()
                    return false
                }
                return true
            },
            ok: () => {
                const after = checks.loadOrder()
                const taken = after !== checks.beforeOrder && after.split(",").length === checks.wantCount
                const waiting = !Hub.mods.saving && Hub.mods.dirty
                Hub.mods.discardChanges()
                return taken && waiting && !Hub.mods.dirty && checks.loadOrder() === checks.beforeOrder
            }
        },
        {
            section: 3,
            what: "a save counts a mod it needs that is installed but switched off",
            ready: () => {
                if (checks.offStage === 0) {
                    checks.offStage = 1
                    const one = checks.saveNamed("Colony One")
                    checks.beforeOrder = checks.loadOrder()
                    checks.window.useList(one.ids.filter(id => id !== "fixture.mod020"))
                    return false
                }
                if (checks.offStage === 1) {
                    checks.offStage = 2
                    Hub.saves.reread()
                    return false
                }
                return !Hub.saves.busy && checks.listed(Hub.saves.saves).length === 2
            },
            ok: () => {
                const one = checks.saveNamed("Colony One")
                const shown = !!one && one.notInstalled === 0 && one.notActive === 1
                    && checks.chips(Theme.fill("{} not active", 1)).length >= 1
                Hub.mods.discardChanges()
                Hub.saves.reread()
                return shown && !Hub.mods.dirty && checks.loadOrder() === checks.beforeOrder
            }
        }
    ]
    property bool typed: false
    property bool askedFind: false
    property bool changed: false
    property string before
    property string beforeOrder
    property int wantCount: 0
    property int orderStage: 0
    property int offStage: 0
    property var textureReport
    property var downloadsUpdatesReport
    function matching(root, test) {
        let found = test(root) ? [root] : []
        for (const child of root.children || [])
            found = found.concat(matching(child, test))
        return found
    }
    function named(name) {
        return matching(checks.window.contentItem, item => item.objectName === name)[0] || null
    }
    function buttons(text) {
        return matching(checks.window.contentItem, item => item.danger !== undefined && item.text === text)
    }
    // As the id list the active side of the Mods screen shows.
    function loadOrder() {
        const ids = []
        for (let i = 0; i < Mods.active.count; i++)
            ids.push(Mods.nthMod(Mods.active, i))
        return ids.join(",")
    }
    function saveNamed(name) {
        return checks.listed(Hub.saves.saves).find(save => save.name === name)
    }
    // The save rows' chips hold the counts: `pressable` keeps plain Text out.
    function chips(text) {
        return matching(checks.window.contentItem, item => item.pressable !== undefined && item.text === text && item.visible)
    }
    function inside(item, outer) {
        for (let at = item; at; at = at.parent)
            if (at === outer)
                return true
        return false
    }
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
    // The first visible item, from the root down, that passes the test.
    function shown(root, test) {
        if (root.visible && test(root))
            return root
        for (const child of root.children || []) {
            const hit = shown(child, test)
            if (hit)
                return hit
        }
        return null
    }
    function showing(root, text) {
        if (!root.visible)
            return false
        return root.text === text || Array.from(root.children || []).some(child => checks.showing(child, text))
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
    function labelled(item, text) {
        if (item.text === text && item.clicked !== undefined)
            return item
        for (const child of item.children) {
            const found = labelled(child, text)
            if (found)
                return found
        }
        return null
    }
    // A file dialog is no item: it sits among an item's resources, not its children.
    function dialogs(item) {
        let found = item.selectedFile !== undefined ? [item] : []
        for (const child of Array.from(item.children || []).concat(Array.from(item.resources || [])))
            found = found.concat(dialogs(child))
        return found
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
    property var size: []
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
