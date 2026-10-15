pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Text {
    id: title
    readonly property string meaning: Theme.meaning(text)
    topPadding: Theme.gapWide
    bottomPadding: Theme.gapSmall
    color: Theme.ink2
    font.pixelSize: Theme.small
    font.weight: Font.DemiBold
    activeFocusOnTab: meaning !== ""
    font.underline: activeFocus
    Accessible.role: Accessible.Heading
    Accessible.name: text
    Accessible.description: meaning
    HoverHandler { id: pointed }
    Dots { visible: title.meaning !== ""; y: parent.height - 5; width: title.contentWidth; tone: title.color }
    Icon { visible: title.meaning !== ""; x: title.contentWidth + Theme.gapSmall; y: title.topPadding + (title.contentHeight - height) / 2; name: "info"; size: Theme.sized(11); color: Theme.ink3; Accessible.ignored: true }
    Tip { text: title.meaning; over: pointed.hovered || title.activeFocus }
}
