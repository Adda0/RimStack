pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Item {
    id: term
    property string text
    readonly property string meaning: Theme.meaning(text)
    implicitWidth: label.implicitWidth + (meaning !== "" ? mark.width + Theme.gapSmall : 0)
    implicitHeight: label.implicitHeight
    activeFocusOnTab: meaning !== ""
    Accessible.role: Accessible.StaticText
    Accessible.name: text
    Accessible.description: meaning
    FocusRing {}
    HoverHandler { id: pointed }
    Text { id: label; color: Theme.ink2; font.pixelSize: Theme.body; text: term.text }
    Dots { visible: term.meaning !== ""; y: label.height + 1; width: label.contentWidth; tone: Theme.ink3 }
    Icon { id: mark; visible: term.meaning !== ""; x: label.contentWidth + Theme.gapSmall; anchors.verticalCenter: label.verticalCenter; name: "info"; size: Theme.sized(12); color: Theme.ink3; Accessible.ignored: true }
    Tip { text: term.meaning; over: pointed.hovered || term.activeFocus }
}
