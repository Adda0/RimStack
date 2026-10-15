import QtCore
import QtQuick

Settings {
    category: "Appearance"
    property string theme: "rim"
    property bool comfortable: true
    property bool largeRows: false
    property bool reduceMotion: false
    property bool highContrast: false
    property real textScale: 1
    property bool hintsSeen: false
    property string folded: ""
    property int windowWidth: 1440
    property int windowHeight: 860
    property bool workshopInSteam: false
    // Add new preferences last: a file written before one existed gives those declared
    // after it their defaults.
    property bool ownColours: false
    property bool wrapLog: false
    property bool wrapTrace: true
    // A category's own colour, by its number and a name of `Theme.swatches`: "2=Rose,5=Sky".
    property string categoryColours: ""
}
