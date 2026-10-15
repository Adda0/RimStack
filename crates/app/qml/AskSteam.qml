pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

Btn {
    id: ask
    property string idle
    readonly property bool busy: Hub.downloads.notesBusy
    readonly property var far: Hub.downloads.notesState || ({})
    property int waits: 0
    enabled: !busy
    icon: "download"
    text: !busy ? idle
        : waits > 0 ? Theme.fill("Steam asks to wait, {} s", waits)
        : far.asked > 1 ? Theme.fill("Asking Steam, {} of {}", Math.min(far.done + 1, far.asked), far.asked)
        : Theme.say("Asking Steam…")
    Timer {
        interval: 1000
        repeat: true
        triggeredOnStart: true
        running: ask.busy && ask.far.resume > 0
        onTriggered: ask.waits = Math.max(0, ask.far.resume - Math.floor(Date.now() / 1000))
        onRunningChanged: if (!running) ask.waits = 0
    }
}
