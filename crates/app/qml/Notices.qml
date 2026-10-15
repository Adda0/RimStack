pragma Singleton
import QtQuick
import RimStack.Ui

QtObject {
    id: notices

    // Each is { key, text, tone: "", "ok" or "err", label, run }.
    property var shown: []
    // This run's notices, newest first: { text, tone, time }.
    property var earlier: []
    property int counted: 0
    property string hint

    function add(notice) {
        notice.key = ++counted
        const kept = notice.run ? shown.filter(other => !other.run) : shown
        shown = kept.concat([notice]).slice(-4)
        earlier = [{ text: notice.text, tone: notice.tone || "", time: Date.now() }].concat(earlier).slice(0, 40)
    }
    function say(text, tone) {
        add({ text: text, tone: tone || "" })
    }
    function offer(text, label, run) {
        add({ text: text, tone: "", label: label, run: run })
    }
    // Confirmed before anything touches files outside RimStack:
    // { title, lines, label, danger, run }.
    property var asking: null
    function ask(question) {
        asking = question
    }

    function dismiss(key) {
        shown = shown.filter(notice => notice.key !== key)
    }
}
