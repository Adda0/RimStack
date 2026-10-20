pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Rectangle {
    id: field
    property string text
    property string placeholder
    property alias echoMode: fieldInput.echoMode
    property bool chips: false
    property string found
    property string icon
    property string keys
    readonly property real lead: icon !== "" ? Theme.icon + 8 : 0
    property var terms: []
    readonly property string typing: fieldInput.text
    readonly property string lastWord: typing.split(" ").pop()
    // A "!" before a term turns it round; the term after it is what is offered.
    readonly property string bang: lastWord.match(/^!?/)[0]
    readonly property string partial: lastWord.slice(bang.length)
    function take(term) {
        const words = fieldInput.text.split(" ").slice(0, -1)
        if (term.endsWith(":")) {
            fieldInput.text = words.concat([term]).join(" ")
            return
        }
        if (terms.indexOf(term) < 0)
            terms = terms.concat([term])
        syncing = true
        fieldInput.text = words.join(" ")
        syncing = false
        compose()
    }
    property bool syncing: false
    signal accepted()
    signal escaped()
    signal tapped()
    function focusInput() { fieldInput.forceActiveFocus() }
    readonly property bool editing: fieldInput.activeFocus

    function isTerm(word) {
        return /^!?(is|has|tag|author|patches):.+$/.test(word)
    }
    function compose() {
        syncing = true
        text = terms.concat(fieldInput.text.trim() === "" ? [] : [fieldInput.text.trim()]).join(" ")
        syncing = false
    }
    function dropTerm(at) {
        terms = terms.filter((term, index) => index !== at)
        compose()
        fieldInput.forceActiveFocus()
    }
    onTextChanged: {
        if (syncing)
            return
        syncing = true
        const words = text.split(" ").filter(word => word !== "")
        terms = chips ? words.filter(isTerm) : []
        fieldInput.text = chips ? words.filter(word => !isTerm(word)).join(" ") : text
        syncing = false
    }

    implicitHeight: Theme.field
    radius: Theme.corner
    color: Theme.well
    border.width: 1
    border.color: fieldInput.activeFocus ? Theme.focus : Theme.fieldLine
    Behavior on border.color { ColorAnimation { duration: Theme.quick } }
    Row {
        id: shown
        x: 6 + field.lead
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.gapTiny
        Repeater {
            model: field.terms
            Chip {
                required property string modelData
                required property int index
                pressable: true
                tone: Theme.moved
                icon: "x"
                text: modelData
                tip: Theme.say("Remove")
                onClicked: field.dropTerm(index)
            }
        }
    }
    Text {
        id: foundText
        visible: field.found !== ""
        anchors.right: parent.right
        anchors.rightMargin: clear.visible ? 30 : 10
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.ink2
        font.pixelSize: Theme.small
        font.features: { "tnum": 1 }
        text: field.found
    }
    Text {
        anchors.fill: fieldInput
        verticalAlignment: Text.AlignVCenter
        visible: fieldInput.text === "" && field.terms.length === 0
        text: field.placeholder
        elide: Text.ElideRight
        color: Theme.ink3
        font.pixelSize: Theme.body
    }
    TextInput {
        id: fieldInput
        activeFocusOnTab: true
        Accessible.role: Accessible.EditableText
        Accessible.name: field.placeholder
        anchors.fill: parent
        anchors.leftMargin: field.lead + (field.terms.length > 0 ? shown.width + 12 : 10)
        anchors.rightMargin: (clear.visible ? 26 : 10) + (foundText.visible ? foundText.width + 8 : 0)
        verticalAlignment: TextInput.AlignVCenter
        color: Theme.ink
        selectionColor: Theme.accent
        font.pixelSize: Theme.body
        clip: true
        // On the input itself: it takes the press, so a handler on the field would not see it.
        TapHandler { onTapped: field.tapped() }
        onTextChanged: {
            if (field.syncing)
                return
            const words = text.split(" ")
            const done = field.chips ? words.slice(0, -1).filter(field.isTerm) : []
            if (done.length > 0) {
                field.terms = field.terms.concat(done)
                field.syncing = true
                text = words.filter((word, at) => at === words.length - 1 || !field.isTerm(word)).join(" ")
                field.syncing = false
            }
            field.compose()
        }
        Keys.onEscapePressed: {
            field.text = ""
            field.escaped()
        }
        Keys.onReturnPressed: field.accepted()
        Keys.onEnterPressed: field.accepted()
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Backspace && text === "" && field.terms.length > 0) {
                field.dropTerm(field.terms.length - 1)
                event.accepted = true
            }
        }
    }
    Icon { visible: field.icon !== ""; x: 9; anchors.verticalCenter: parent.verticalCenter; name: field.icon; size: Theme.icon - 1; color: Theme.ink3 }
    Text {
        visible: field.keys !== "" && field.text === "" && !fieldInput.activeFocus && field.width > Theme.sized(300)
        anchors.right: parent.right
        anchors.rightMargin: Theme.gap
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.ink3
        font.pixelSize: Theme.caption
        text: field.keys
    }
    Icon {
        id: clear
        visible: field.text !== ""
        anchors.right: parent.right
        anchors.rightMargin: Theme.gap
        anchors.verticalCenter: parent.verticalCenter
        name: "x"
        size: 12
        color: clearHover.hovered ? Theme.ink : Theme.ink3
        HoverHandler { id: clearHover; cursorShape: Qt.PointingHandCursor }
        TapHandler { onTapped: { field.text = ""; fieldInput.forceActiveFocus() } }
        Tip { text: Theme.say("Clear (Esc)") }
    }
}
