import QtQuick
import QtQuick.Controls.Basic
import RimStack.Ui

// A list's scrollbar: wide enough to grab, never a sliver, and in view while the
// list is longer than its pane, unless chosen to hide it when idle.
ScrollBar {
    id: bar
    // What a list leaves free at its right for the bar, so it covers no row.
    property real room: 0
    policy: ScrollBar.AlwaysOn
    minimumSize: 0.04
    padding: Theme.gapLine
    contentItem: Rectangle {
        implicitWidth: Theme.sized(8)
        implicitHeight: Theme.sized(8)
        radius: width / 2
        color: Theme.ink3
        opacity: 0
        states: State {
            name: "shown"
            when: bar.size < 1 && (bar.policy === ScrollBar.AlwaysOn || bar.active)
            PropertyChanges { bar.contentItem.opacity: 0.75 }
        }
        transitions: Transition { from: "shown"; NumberAnimation { property: "opacity"; duration: Theme.calm } }
    }
    // The first state that holds wins: a bar hidden when idle lies over the rows and takes no room.
    states: [
        State {
            when: Theme.stored.idleScrollbars
            PropertyChanges { bar.policy: ScrollBar.AsNeeded }
        },
        State {
            when: bar.size < 1
            PropertyChanges { bar.room: bar.width }
        }
    ]
}
