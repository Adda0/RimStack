pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import RimStack.Ui

ScrollBar {
    id: bar
    policy: ScrollBar.AlwaysOn
    padding: Theme.gapHair
    minimumSize: Theme.sized(32) / Math.max(1, height)
    contentItem: Rectangle {
        id: thumb
        visible: bar.size < 1
        implicitWidth: Theme.sized(9)
        implicitHeight: Theme.sized(9)
        radius: width / 2
        color: Theme.ink3
        opacity: 0.6
        states: State {
            when: bar.hovered || bar.pressed
            PropertyChanges { thumb.color: Theme.ink2; thumb.opacity: 1 }
        }
    }
}
