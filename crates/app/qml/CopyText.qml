pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

TextEdit {
    // What Ctrl+C copies with nothing selected: a rich text gives its words without the markup.
    property string whole: text
    readOnly: true
    selectByMouse: true
    // Shift with the arrows selects; Ctrl+C with nothing selected copies it all.
    selectByKeyboard: true
    activeFocusOnTab: true
    Accessible.role: Accessible.StaticText
    Accessible.name: whole
    textFormat: TextEdit.PlainText
    wrapMode: TextEdit.Wrap
    color: Theme.ink
    selectionColor: Theme.accent
    selectedTextColor: Theme.accentInk
    font.pixelSize: Theme.body
    onActiveFocusChanged: if (!activeFocus) deselect()
    // A text shown with invisible places to break at is copied without them; a rich text's
    // paragraphs are selected with their own separators.
    Keys.onPressed: event => {
        if (event.matches(StandardKey.Copy)) {
            Theme.copy((selectedText.replace(/[\u2028\u2029]/g, "\n") || whole).replace(/\u200b/g, ""))
            event.accepted = true
        }
    }
    FocusRing {}
    HoverHandler { cursorShape: parent.hoveredLink !== "" ? Qt.PointingHandCursor : Qt.IBeamCursor }
}
