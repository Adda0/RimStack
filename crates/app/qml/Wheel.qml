// A notch is 120 of angle, and may arrive in small parts that add up. On Wayland
// only touchpad events carry a pixel distance, so touchpads are accepted too,
// told apart by the phase.
pragma ComponentBehavior: Bound
import QtQuick
import RimStack.Ui

WheelHandler {
    id: wheel
    readonly property real step: Qt.styleHints.wheelScrollLines * 40
    // A handler declared in a scrolling view belongs to its content.
    readonly property Flickable view: (parent instanceof Flickable ? parent : parent?.parent) as Flickable
    property real goal: 0

    target: null
    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
    onWheel: (event) => {
        if (!view)
            return
        const first = view.originY - view.topMargin
        const end = Math.max(first, view.originY + view.contentHeight + view.bottomMargin - view.height)
        const within = (y) => Math.max(first, Math.min(end, y))
        if (Theme.wheelLog)
            console.log("wheel angle=" + event.angleDelta.y + " pixel=" + event.pixelDelta.y + " phase=" + event.phase)
        if (event.phase !== Qt.NoScrollPhase) {
            ease.stop()
            view.contentY = within(view.contentY - event.pixelDelta.y)
            return
        }
        if (event.angleDelta.y === 0)
            return
        const from = ease.running ? goal : view.contentY
        goal = within(from - event.angleDelta.y / 120 * step)
        ease.from = view.contentY
        ease.to = goal
        ease.restart()
    }

    property NumberAnimation ease: NumberAnimation {
        target: wheel.view
        property: "contentY"
        duration: Theme.stored.reduceMotion ? 0 : 200
        easing.type: Easing.OutCubic
    }
}
