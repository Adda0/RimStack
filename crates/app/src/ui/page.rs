use super::rows::{Scrolling, bar};
use super::theme::Theme;
use gpui::{
    Bounds, ElementId, EntityId, FocusHandle, HitboxBehavior, InteractiveElement, IntoElement,
    ParentElement, Pixels, ScrollEvent, ScrollHandle, StatefulInteractiveElement, Styled, canvas,
    div, point, px,
};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Default)]
pub struct Paging {
    pub scroll: Scrolling,
    handle: ScrollHandle,
    /// The control the keys were on when the page was last drawn.
    followed: Rc<RefCell<Option<FocusHandle>>>,
}

impl Paging {
    pub fn new(reduce_motion: bool) -> Self {
        let paging = Self::default();
        paging.scroll.borrow_mut().set_reduce_motion(reduce_motion);
        paging
    }

    /// Where the page's content shows, in its window.
    pub fn area(&self) -> Bounds<Pixels> {
        self.handle.bounds()
    }

    pub fn contain_drawn(&self, top: gpui::Pixels, bottom: gpui::Pixels) {
        let shift = self.scroll.borrow().top() - self.handle.bounds().top();
        self.scroll
            .borrow_mut()
            .contain(top + shift, bottom + shift);
    }
}

pub fn room(theme: &Theme) -> gpui::Pixels {
    theme.sized(9.) + super::theme::GAP_HAIR * 2.
}

/// `content` in a scrolled area that fills its place; `view` is drawn again as it glides.
pub fn page(
    id: impl Into<ElementId>,
    paging: &Paging,
    theme: &Theme,
    view: EntityId,
    content: impl IntoElement,
) -> gpui::Div {
    let line = theme.sized(40.);
    let (follow, handle) = (paging.scroll.clone(), paging.handle.clone());
    let followed = paging.followed.clone();
    let (wheeled, wheel_bounds) = (paging.scroll.clone(), paging.handle.clone());
    // Laid out after the content: the handle knows how tall it is by then.
    let steer = canvas(
        move |_, window, cx| {
            // A control the keys come to inside the page is shown, just enough.
            let reached = window.focused(cx);
            if reached != *followed.borrow() {
                let area = handle.bounds();
                if let Some(at) = reached.as_ref().and_then(super::focus::drawn_at)
                    && at.left() < area.right()
                    && at.right() > area.left()
                {
                    // From the window's place to the content's, the way `contain_drawn` does.
                    let shift = follow.borrow().top() - area.top();
                    let mut scroll = follow.borrow_mut();
                    scroll.contain(at.top() + shift, at.bottom() + shift);
                }
                *followed.borrow_mut() = reached;
            }
            let shown = handle.bounds().size.height;
            let total = shown + handle.max_offset().y;
            let gliding = {
                let mut scroll = follow.borrow_mut();
                scroll.fit(total, shown);
                scroll.advance()
            };
            let offset = point(px(0.), -follow.borrow().top());
            // gpui-fast takes a write as a scroll, which draws the page's view again.
            if handle.offset() != offset {
                handle.set_offset(offset);
            }
            if gliding {
                window.request_animation_frame();
            }
            window.insert_hitbox(wheel_bounds.bounds(), HitboxBehavior::Normal)
        },
        move |_, hitbox, window, _| {
            // Before the area's own handler: the wheel glides instead of jumping.
            window.on_mouse_event(move |event: &ScrollEvent, phase, window, cx| {
                if phase.capture() && hitbox.should_handle_scroll(window) {
                    wheeled.borrow_mut().wheel(event.delta, line);
                    cx.notify(view);
                    cx.stop_propagation();
                }
            });
        },
    )
    .absolute()
    .size_0();
    div()
        .relative()
        .size_full()
        .min_h_0()
        .child(
            div()
                .id(id)
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&paging.handle)
                .child(content),
        )
        .child(steer)
        .child(bar(theme, paging.scroll.clone(), view))
}
