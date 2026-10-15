pragma Singleton
import QtQuick
import RimStack.Ui

QtObject {
    id: theme

    readonly property AppPrefs stored: AppPrefs {}

    readonly property string name: stored.theme
    readonly property bool light: palette.bright === true
    readonly property var custom: Hub.settings.ownColours || ({})
    readonly property Binding themeName: Binding { target: Hub.settings; property: "theme"; value: theme.name }
    readonly property var colourNames: ["win", "panel", "bar", "ctl", "ctlHi", "line", "edge", "well", "seg", "chip", "hover", "ink", "ink2", "ink3", "brand", "fieldLine", "accent", "accentHi", "focus", "accentInk", "err", "warn", "ok", "moved", "banner", "dangerBg", "scrim"]

    readonly property real scale: Math.max(0.85, Math.min(1.6, stored.textScale))
    readonly property int caption: Math.round(11 * scale)
    readonly property int small: Math.round(12 * scale)
    readonly property int body: Math.round(13 * scale)
    readonly property int title: Math.round(15 * scale)
    readonly property int heading: Math.round(17 * scale)
    readonly property int display: Math.round(26 * scale)
    readonly property real grow: Math.max(1, scale)
    readonly property int row: Math.round((stored.largeRows ? 32 : 26) * grow)
    readonly property int control: Math.round(24 * grow)
    readonly property int controlLarge: Math.round(28 * grow)
    readonly property int field: Math.round(28 * grow)
    readonly property int segment: Math.round(24 * grow)
    readonly property int tag: Math.round(18 * grow)
    readonly property int strip: Math.round(26 * grow)
    readonly property int header: Math.round(42 * grow)
    readonly property int topBar: Math.round(52 * grow)
    readonly property int icon: Math.round(14 * grow)
    function sized(pixels) {
        return Math.round(pixels * grow)
    }
    readonly property int corner: 6
    readonly property int cornerLarge: 9
    readonly property int gapLine: 1
    readonly property int gapHair: 2
    readonly property int gapThin: 3
    readonly property int gapTiny: 4
    readonly property int gapSmall: 6
    readonly property int gap: 8
    readonly property int gapMedium: 10
    readonly property int gapLarge: 12
    readonly property int gapWide: 16
    readonly property int gapCard: 14
    readonly property int gapPage: 20
    readonly property int quick: stored.reduceMotion ? 0 : 140
    readonly property int calm: stored.reduceMotion ? 0 : 220
    readonly property bool contrast: stored.highContrast
    readonly property color onSelected: "#ffffff"
    readonly property color onSelectedDim: "#ebffffff"
    // Darker than the accent, so white on it keeps 7:1 in every theme.
    readonly property color selectedChip: "#38000000"

    readonly property string issues: "https://github.com/TheForgotten69/RimStack/issues"

    readonly property var words: Hub.settings.words || ({})
    function say(text) {
        return words[text] || text
    }
    function fill(text, ...parts) {
        let at = 0
        return say(text).replace(/\{\}/g, () => parts[at++])
    }
    // The locale is passed so a binding follows a change of language.
    function plural(n, one, many, ...parts) {
        const form = Hub.settings.counted(n, say(one), say(many), words["@locale"] || "")
        let at = 0
        const all = [count(n), ...parts]
        return form.replace(/\{\}/g, () => all[at++])
    }

    readonly property var palettes: ({
        dark: {
            win: "#191b23", panel: "#1f212b", bar: "#272a36", ctl: "#353949", ctlHi: "#454a5e", line: "#323646", edge: "#101117",
            well: "#13151b", seg: "#16181f", chip: "#14ffffff", hover: "#ffffff", ink: "#f3f1ec", ink2: "#b3b2ba", ink3: "#9b9ba6",
            brand: "#d9b88a", fieldLine: "#6c728b", accent: "#2468c8", accentHi: "#2f75d6", focus: "#4c96f5", accentInk: "#ffffff",
            err: "#ff6961", warn: "#f5a93b", ok: "#5fcf80", moved: "#6cb0ff", banner: "#1b2a44", dangerBg: "#3a2222", scrim: "#66000000",
            high: { ink2: "#e4e3ea", ink3: "#c9c8d2", line: "#7c8199", fieldLine: "#b9bdd0" },
        },
        oled: {
            win: "#000000", panel: "#0b0b0c", bar: "#111113", ctl: "#202023", ctlHi: "#303034", line: "#242427", edge: "#000000",
            well: "#060607", seg: "#0d0d0e", chip: "#1cffffff", hover: "#ffffff", ink: "#f3f1ec", ink2: "#a8a8ae", ink3: "#8f8f96",
            brand: "#d9b88a", fieldLine: "#62677c", accent: "#1f63c2", accentHi: "#2a70d4", focus: "#4c96f5", accentInk: "#ffffff",
            err: "#ff6961", warn: "#f5a93b", ok: "#5fcf80", moved: "#6cb0ff", banner: "#0d1d33", dangerBg: "#2a1414", scrim: "#80000000",
            high: { ink2: "#e4e3ea", ink3: "#c9c8d2", line: "#74788c", fieldLine: "#b9bdd0" },
        },
        light: {
            bright: true,
            win: "#f5f5f7", panel: "#ececef", bar: "#e3e3e7", ctl: "#d5d5da", ctlHi: "#ffffff", line: "#cdcdd3", edge: "#c3c3c9",
            well: "#ffffff", seg: "#dadade", chip: "#12000000", hover: "#000000", ink: "#1d1d1f", ink2: "#55555c", ink3: "#5c5c64",
            brand: "#7a4d22", fieldLine: "#85858f", accent: "#1f63c2", accentHi: "#2a70d4", focus: "#1f63c2", accentInk: "#ffffff",
            err: "#ab241e", warn: "#824b00", ok: "#14682a", moved: "#1558b0", banner: "#d7e5fb", dangerBg: "#f7dcda", scrim: "#40000000",
            high: { ink2: "#26262a", ink3: "#3a3a40", line: "#6d6d77", fieldLine: "#4a4a52" },
        },
        rim: {
            win: "#1c1916", panel: "#23201c", bar: "#2b2722", ctl: "#3b352d", ctlHi: "#4b4439", line: "#3a342c", edge: "#110f0d",
            well: "#151311", seg: "#181512", chip: "#14ffffff", hover: "#ffffff", ink: "#f2ebdd", ink2: "#bcb3a3", ink3: "#a0988a",
            brand: "#e2bb7c", fieldLine: "#7d7465", accent: "#2870ab", accentHi: "#2b76b3", focus: "#7db4dc", accentInk: "#ffffff",
            err: "#ff7b6b", warn: "#f2b04a", ok: "#8fce7a", moved: "#8fc3ea", banner: "#22303a", dangerBg: "#3d2320", scrim: "#66000000",
            high: { ink2: "#e6dfd0", ink3: "#cfc7b8", line: "#8a8172", fieldLine: "#c2b9a8" },
        },
    })
    readonly property var themeNames: ["rim", "dark", "oled", "light"]
    readonly property var themeLabels: ["Rim", "Dark", "OLED", "Light"]
    readonly property var palette: palettes[name] || palettes.rim
    readonly property var raised: contrast ? palette.high : ({})
    readonly property var own: stored.ownColours ? custom : ({})
    readonly property bool ownInUse: stored.ownColours && Object.keys(custom).length > 0
    function colour(key) {
        return raised[key] || own[key] || palette[key]
    }

    readonly property color win: colour("win")
    readonly property color panel: colour("panel")
    readonly property color bar: colour("bar")
    readonly property color ctl: colour("ctl")
    readonly property color ctlHi: colour("ctlHi")
    readonly property color line: colour("line")
    readonly property color edge: colour("edge")
    readonly property color well: colour("well")
    readonly property color seg: colour("seg")
    readonly property color chip: colour("chip")
    readonly property color hover: colour("hover")
    readonly property color ink: colour("ink")
    readonly property color ink2: colour("ink2")
    readonly property color ink3: colour("ink3")
    // For the app's name only, never for something to press.
    readonly property color brand: colour("brand")
    readonly property color fieldLine: colour("fieldLine")
    readonly property color accent: colour("accent")
    readonly property color accentHi: colour("accentHi")
    readonly property color focus: colour("focus")
    readonly property color accentInk: colour("accentInk")
    readonly property color err: colour("err")
    readonly property color warn: colour("warn")
    readonly property color ok: colour("ok")
    readonly property color moved: colour("moved")
    readonly property color banner: colour("banner")
    readonly property color dangerBg: colour("dangerBg")
    readonly property color scrim: colour("scrim")

    // Indexed by the category number Rust gives, in load order.
    readonly property var categoryIcons: ["plug", "gamepad-2", "library", "package", "wrench", "image", "layout-dashboard", "clock", "arrow-down-to-line"]
    // The first nine are the categories' own colours, in order.
    readonly property var swatches: [
        ["Violet", "#9a8fc2", "#6d5fa8"], ["Slate", "#8d99a6", "#5f6b78"], ["Blue", "#6fa3c7", "#3f7da6"],
        ["Green", "#86b06b", "#4f8a36"], ["Amber", "#d2a94e", "#9a7a1c"], ["Coral", "#c98277", "#a8574b"],
        ["Teal", "#7fb8b0", "#3f8a80"], ["Sand", "#c9a36a", "#8a6a2a"], ["Grey", "#a3a3ad", "#6f6f78"],
        ["Rose", "#d58fb3", "#a44f7c"], ["Lime", "#b5c95a", "#6f8420"], ["Sky", "#7cc4e8", "#2f86b3"], ["Orange", "#e09a5c", "#a8621e"],
    ]
    function swatch(name) {
        const found = swatches.find(s => s[0] === name)
        return found ? found[light ? 2 : 1] : ""
    }
    readonly property var chosenSwatches: {
        const chosen = {}
        for (const pair of stored.categoryColours.split(",")) {
            const [category, name] = pair.split("=")
            if (name)
                chosen[category] = name
        }
        return chosen
    }
    readonly property var categoryColours: categoryIcons.map((icon, category) => swatch(chosenSwatches[category] || swatches[category][0]))
    // "" gives the category its own colour back.
    function paintCategory(category, name) {
        const now = Object.assign({}, chosenSwatches)
        if (name === "" || name === swatches[category][0])
            delete now[category]
        else
            now[category] = name
        stored.categoryColours = Object.keys(now).map(key => key + "=" + now[key]).join(",")
    }

    readonly property var categoryNames: ["Loaders", "Game and DLC", "Libraries", "Content", "Patches", "Textures", "Interface", "Performance & tweaks", "Loads last"]

    readonly property bool wheelLog: Hub.given("--wheel-log")
    readonly property bool selfTest: Hub.given("--selftest")
    readonly property bool demo: selfTest || Hub.given("--demo")

    readonly property var terms: ({
        "Load order": "The order in which the game loads the active mods. A mod that changes another has to load after it.",
        "Own rules": "Rules you added: this mod loads after, or before, that one. Sort follows them before anything else.",
        "Community rules": "Load-order rules kept by players (RimSort's Community Rules Database), for mods whose own files do not say enough.",
        "Detected from files": "What a mod's files point at in another mod. A hint for Sort, not something the game requires.",
        "Texture conflicts": "Two active mods provide a picture under the same name. The game uses the one from the mod that loads later.",
        "Patched methods": "The methods of the game this mod's code changes, through the Harmony library. Two mods patching one method may clash.",
        "Group": "A mod's first tag. Inside a category, mods with the same first tag are kept side by side under its name.",
        "Textures set aside": "Files RimStack renamed inside this mod so the game skips them. Nothing is deleted.",
        "Folders the game loads from": "A mod can ship a folder per game version or per other mod; the game reads only those that apply.",
        "Requires": "Mods this one needs active, as its About.xml says.",
        "Filed under": "The category Sort keeps this mod in. Categories load in a fixed order: loaders, the game, libraries, content, patches, textures.",
        "DDS": "A texture format the game loads faster than PNG. RimStack writes DDS copies beside the PNG files.",
        "To review": "Not problems: things worth a look when you have a moment.",
    })
    function meaning(term) {
        // `term` may come in the chosen language or in English.
        const english = Object.keys(terms).find(key => key === term || say(key) === term)
        return english ? say(terms[english]) : ""
    }

    readonly property var folded: stored.folded === "" ? [] : stored.folded.split(",")
    function fold(key) {
        const now = folded.filter(k => k !== key)
        if (now.length === folded.length)
            now.push(key)
        stored.folded = now.join(",")
    }

    // Qt gives a new font only to texts made afterwards; the ones shown already are re-fonted here.
    function refont(root, from, to) {
        if (!root || from === to)
            return
        if (root.font !== undefined && root.font.family === from)
            root.font.family = to
        for (const child of root.children || [])
            refont(child, from, to)
        for (const kept of root.resources || []) {
            if (kept.contentItem)
                refont(kept.contentItem, from, to)
        }
    }

    function plain(text) {
        return String(text).replace(/&/g, "&amp;").replace(/</g, "&lt;")
    }

    function day(seconds) {
        return Qt.formatDateTime(new Date(seconds * 1000), "yyyy-MM-dd")
    }
    function moment(seconds) {
        return Qt.formatDateTime(new Date(seconds * 1000), "yyyy-MM-dd HH:mm")
    }

    function clock(seconds) {
        return Qt.formatDateTime(new Date(seconds * 1000), "HH:mm")
    }

    // Takes seconds since 1970.
    function ago(seconds) {
        const minutes = Math.floor((Date.now() / 1000 - seconds) / 60)
        if (minutes < 1)
            return say("just now")
        if (minutes < 60)
            return plural(minutes, "{} minute ago", "{} minutes ago")
        const hours = Math.floor(minutes / 60)
        if (hours < 48)
            return plural(hours, "{} hour ago", "{} hours ago")
        const days = Math.floor(hours / 24)
        if (days < 60)
            return plural(days, "{} day ago", "{} days ago")
        if (days < 730)
            return plural(Math.floor(days / 30), "{} month ago", "{} months ago")
        return plural(Math.floor(days / 365), "{} year ago", "{} years ago")
    }

    function count(n) {
        return Number(n).toLocaleString(Qt.locale(), "f", 0)
    }

    // On Windows a path has backslashes and starts with its drive: file:///C:/…
    function fileUrl(path) {
        const parts = String(path).replace(/\\/g, "/").split("/")
        return "file://" + (parts[0] === "" ? "" : "/") + parts.map(part => encodeURIComponent(part).replace(/%3A/g, ":")).join("/")
    }

    property TextEdit clip: TextEdit { visible: false }
    function copy(text) {
        clip.text = text
        clip.selectAll()
        clip.copy()
        clip.text = ""
    }

    function pasted() {
        clip.text = ""
        clip.paste()
        const text = clip.text
        clip.text = ""
        return text
    }

    function openWorkshop(id) {
        Qt.openUrlExternally(stored.workshopInSteam ? "steam://url/CommunityFilePage/" + id
            : "https://steamcommunity.com/sharedfiles/filedetails/?id=" + id)
    }

    function openChangeNotes(id) {
        const page = "https://steamcommunity.com/sharedfiles/filedetails/changelog/" + id
        Qt.openUrlExternally(stored.workshopInSteam ? "steam://openurl/" + page : page)
    }
}
