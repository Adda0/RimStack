pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Row {
    id: dots
    property color tone: Theme.ink3
    height: 1
    clip: true
    spacing: Theme.gapHair
    Repeater {
        model: Math.ceil(dots.width / 4)
        Rectangle { width: 2; height: 1; color: dots.tone }
    }
}
