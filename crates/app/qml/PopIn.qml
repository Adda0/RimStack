import QtQuick
import RimStack.Ui

Transition {
    NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Theme.quick }
    NumberAnimation { property: "scale"; from: 0.97; to: 1; duration: Theme.quick; easing.type: Easing.OutCubic }
}
