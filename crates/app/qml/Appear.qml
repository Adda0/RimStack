import QtQuick
import RimStack.Ui

Connections {
    id: appear
    required property Item of
    property NumberAnimation show: NumberAnimation { target: appear.of; property: "opacity"; from: 0; to: 1; duration: Theme.calm; easing.type: Easing.OutCubic }
    target: of
    function onVisibleChanged() {
        if (of.visible && Theme.calm > 0)
            show.restart()
    }
}
