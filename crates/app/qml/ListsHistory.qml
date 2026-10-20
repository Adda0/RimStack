pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import RimStack.Ui

Popup {
    id: ordersPopup
    parent: Overlay.overlay
    anchors.centerIn: Overlay.overlay
    enter: PopIn {}
    exit: PopOut {}
    modal: true
    dim: false
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    signal useList(var ids)

    // Read fresh on each open: the load order changes while the menu is up.
    property var orders: []

    function saidOf(order) {
        let parts = []
        if (order.added > 0)
            parts.push("+" + order.added)
        if (order.removed > 0)
            parts.push("−" + order.removed)
        if (order.moved > 0)
            parts.push(Theme.plural(order.moved, "{} moved", "{} moved"))
        return parts.length > 0 ? parts.join(", ") : Theme.say("the same order as now")
    }

    onAboutToShow: orders = Hub.lists.pastOrders()

    width: Theme.sized(480)
    height: Math.min(implicitHeight, parent.height - Theme.gapLarge)
    padding: Theme.gapLarge
    background: Rectangle { radius: Theme.cornerLarge; color: Theme.bar; border.width: 1; border.color: Theme.line }

    contentItem: ColumnLayout {
        spacing: Theme.gapLarge

        RowLayout {
            Layout.fillWidth: true
            height: Theme.header
            spacing: Theme.gap
            Text { Layout.fillWidth: true; color: Theme.ink; font.pixelSize: Theme.title; font.weight: Font.DemiBold; text: Theme.say("Load order history") }
            Btn { text: Theme.say("Close"); onClicked: ordersPopup.close() }
        }

        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Theme.ink2
            font.pixelSize: Theme.body
            text: Theme.say("Every order saved from RimStack, and every order the game or another tool wrote instead. Using one makes it the load order; Save writes it.")
        }

        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true

            ColumnLayout {
                width: ordersPopup.availableWidth
                spacing: Theme.gapSmall

                Text {
                    Layout.fillWidth: true
                    visible: ordersPopup.orders.length === 0
                    color: Theme.ink2
                    font.pixelSize: Theme.body
                    text: Theme.say("Nothing yet")
                }

                Repeater {
                    model: ordersPopup.orders

                    RowLayout {
                        id: order
                        required property var modelData
                        required property int index
                        Layout.fillWidth: true
                        spacing: Theme.gap

                        Text { Layout.preferredWidth: Theme.sized(112); color: Theme.ink3; font.pixelSize: Theme.small; text: Theme.moment(order.modelData.time) }

                        Column {
                            Layout.fillWidth: true
                            Text { color: Theme.ink; font.pixelSize: Theme.body; text: order.modelData.said + ", " + Theme.plural(order.modelData.mods, "{} mod", "{} mods") }
                            Text { color: Theme.ink3; font.pixelSize: Theme.small; text: ordersPopup.saidOf(order.modelData); visible: text !== Theme.say("the same order as now") }
                        }

                        Btn {
                            text: Theme.say("Use")
                            tip: Theme.say("This order becomes the load order. Save writes it.")
                            onClicked: {
                                const ids = Hub.lists.pastOrder(order.index)
                                if (ids.length > 0) {
                                    ordersPopup.useList(ids)
                                    ordersPopup.close()
                                }
                            }
                        }
                        Btn {
                            icon: "save"
                            tip: Theme.say("Keeps this order as a saved list named by its time")
                            onClicked: {
                                const name = Theme.moment(order.modelData.time)
                                const done = Hub.lists.keepIds(name, Hub.lists.pastOrder(order.index))
                                if (done === "") {
                                    Notices.say(Theme.fill("Kept as {}", name), "ok")
                                    ordersPopup.close()
                                } else
                                    Notices.say(done, "err")
                            }
                        }
                    }
                }
            }
        }
    }
}
