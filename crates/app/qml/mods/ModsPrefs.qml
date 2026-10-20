import QtCore
import QtQuick

Settings {
    category: "Mods"
    property real inactive: 0
    property real details: 0
    property bool inactiveHidden: false
    property string listName: ""
    property string lastMod: ""
    property int lastTab: 0
    property string lastFilter: ""
    property real lastScroll: 0
    property string columns: "author,version,load"
    // "category", "name", "updated", "downloaded" or "supports".
    property string inactiveOrder: "category"
    property bool inactiveReversed: false
    // The load order under category headings: 1 always, 0 never (one
    // numbered list), -1 when the order is close to category order.
    property int headings: -1
    property string columnSizes: ""
    // Saving a list also keeps the active mods' Mod_*.xml settings with it.
    property bool listWithSettings: false
}
