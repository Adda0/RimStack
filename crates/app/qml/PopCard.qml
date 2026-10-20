import QtQuick
import QtQuick.Controls.Basic
import RimStack.Ui

// A card over the window that takes the keys: its first control has them, Tab stays
// inside, Esc or a click beside it closes it, and then they go back where they were.
Popup {
    enter: PopIn {}
    exit: PopOut {}
    modal: true
    dim: false
    focus: true
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }
    property Item back: null
    // On show, not once faded in: keys typed straight after opening reach the card.
    onAboutToShow: {
        back = parent.Window.activeFocusItem
        contentItem.nextItemInFocusChain(true).forceActiveFocus()
    }
    // Unless what it ran moved them on.
    onClosed: if (back && parent.Window.activeFocusItem === parent.Window.contentItem) back.forceActiveFocus()
}
