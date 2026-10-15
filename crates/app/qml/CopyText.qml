pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

TextEdit {
    readOnly: true
    selectByMouse: true
    activeFocusOnTab: false
    Accessible.role: Accessible.StaticText
    Accessible.name: text
    textFormat: TextEdit.PlainText
    wrapMode: TextEdit.Wrap
    color: Theme.ink
    selectionColor: Theme.accent
    selectedTextColor: Theme.accentInk
    font.pixelSize: Theme.body
    onActiveFocusChanged: if (!activeFocus) deselect()
    // A text shown with invisible places to break at is copied without them.
    Keys.onPressed: event => {
        if (event.matches(StandardKey.Copy)) {
            Theme.copy(selectedText.replace(/\u200b/g, ""))
            event.accepted = true
        }
    }
    HoverHandler { cursorShape: Qt.IBeamCursor }
}
