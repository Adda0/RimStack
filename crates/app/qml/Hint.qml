pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import RimStack.Ui

Rectangle {
    id: hint
    property int step
    property string text
    property bool atRight: false
    visible: Mods.hintStep === step
    readonly property bool due: Mods.hintStep === step
    onDueChanged: {
        if (due && parent && !parent.visible) {
            if (step < 3)
                Mods.hintStep = step + 1
            else
                Mods.endHints()
        }
    }
    z: 30
    x: atRight ? parent.width - width : 0
    y: parent ? parent.height + 9 : 0
    width: Theme.sized(280)
    height: words.implicitHeight + 56
    radius: Theme.cornerLarge
    color: Theme.accent
    Rectangle { x: hint.atRight ? parent.width - 26 : 16; y: -5; width: 10; height: 10; rotation: 45; color: Theme.accent }
    Text { id: words; x: 12; y: 10; width: parent.width - 24; wrapMode: Text.WordWrap; color: Theme.accentInk; font.pixelSize: Theme.body; lineHeight: 1.25; text: hint.text }
    RowLayout {
        x: 12
        width: parent.width - 24
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.gapMedium
        Text { Layout.fillWidth: true; color: Theme.onSelectedDim; font.pixelSize: Theme.small; text: hint.step + " / 3" }
        Btn { text: Theme.say("Skip"); onClicked: Mods.endHints() }
        Btn { text: hint.step < 3 ? Theme.say("Next") : Theme.say("Done"); onClicked: hint.step < 3 ? Mods.hintStep = hint.step + 1 : Mods.endHints() }
    }
}
