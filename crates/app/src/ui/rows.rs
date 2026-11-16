//! A list whose rows' heights are known before they are drawn: only the rows in view are made,
//! each at its exact place, and the wheel glides and the wheel glides.

use super::theme::{self, Theme};
use super::widgets::with_alpha;
use gpui::{
    AnyElement, App as Cx, AvailableSpace, Bounds, ContentMask, Element, ElementId, EntityId,
    GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, ScrollDelta, ScrollEvent,
    Size, Style, Styled, Window, canvas, fill, point, px, relative, size,
};
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::time::{Duration, Instant};

const GLIDE: Duration = Duration::from_millis(200);
/// How long a bar hidden while idle stays in view after the list last moved.
const SHOWN_AFTER_MOVING: Duration = Duration::from_millis(1200);

#[derive(Default)]
pub struct Scroll {
    top: Pixels,
    from: Pixels,
    goal: Pixels,
    started: Option<Instant>,
    total: Pixels,
    shown: Pixels,
    /// While the bar's thumb is held: how far below the thumb's top it was taken.
    pub held: Option<Pixels>,
    hovered: bool,
    reduce_motion: bool,
    /// A row to bring into view once the list knows its height: its top and bottom, and
    /// whether to put it in the middle rather than just inside.
    asked: Option<(Pixels, Pixels, bool)>,
    /// When the list last moved, for a bar shown only while scrolling.
    moved: Option<Instant>,
}

pub type Scrolling = Rc<RefCell<Scroll>>;

impl Scroll {
    pub fn most(&self) -> Pixels {
        (self.total - self.shown).max(px(0.))
    }

    pub fn total(&self) -> Pixels {
        self.total
    }

    pub fn shown(&self) -> Pixels {
        self.shown
    }

    pub fn top(&self) -> Pixels {
        self.top
    }

    pub fn set_top(&mut self, top: Pixels) {
        let top = top.clamp(px(0.), self.most());
        if top != self.top {
            self.moved = Some(Instant::now());
        }
        self.top = top;
        self.goal = self.top;
        self.started = None;
    }

    /// Moved too recently for a bar shown only while scrolling to hide.
    fn lately_moved(&self) -> bool {
        self.started.is_some()
            || self
                .moved
                .is_some_and(|at| at.elapsed() < SHOWN_AFTER_MOVING)
    }

    pub fn set_reduce_motion(&mut self, reduce: bool) {
        self.reduce_motion = reduce;
    }

    pub fn wheel(&mut self, delta: ScrollDelta, line: Pixels) {
        match delta {
            ScrollDelta::Pixels(moved) => self.set_top(self.top - moved.y),
            ScrollDelta::Lines(lines) if lines.y != 0. => {
                let from = if self.started.is_some() {
                    self.goal
                } else {
                    self.top
                };
                self.goal = (from - line * lines.y).clamp(px(0.), self.most());
                self.from = self.top;
                self.started = Some(Instant::now());
                if self.reduce_motion {
                    self.set_top(self.goal);
                }
            }
            ScrollDelta::Lines(_) => {}
        }
    }

    pub(crate) fn advance(&mut self) -> bool {
        let Some(started) = self.started else {
            return false;
        };
        let part = (started.elapsed().as_secs_f32() / GLIDE.as_secs_f32()).min(1.);
        let eased = 1. - (1. - part).powi(3);
        self.top = self.from + (self.goal - self.from) * eased;
        if part >= 1. {
            self.started = None;
            self.moved = Some(Instant::now());
        }
        part < 1.
    }

    pub(crate) fn fit(&mut self, total: Pixels, shown: Pixels) {
        (self.total, self.shown) = (total, shown);
        self.top = self.top.clamp(px(0.), self.most());
        self.goal = self.goal.clamp(px(0.), self.most());
        if let Some((top, bottom, middle)) = self.asked.take() {
            let to = match middle {
                true => top - (shown - (bottom - top)) / 2.,
                false if top < self.top => top,
                false if bottom > self.top + shown => bottom - shown,
                false => return,
            };
            self.set_top(to);
        }
    }

    pub fn centre(&mut self, top: Pixels, bottom: Pixels) {
        self.asked = Some((top, bottom, true));
    }

    pub fn contain(&mut self, top: Pixels, bottom: Pixels) {
        self.asked = Some((top, bottom, false));
    }
}

/// How far Page Up and Page Down step: a view full of rows, less one to land on.
pub fn page_of(scroll: &Scrolling, line: Pixels) -> usize {
    let shown = f32::from(scroll.borrow().shown());
    let rows = (shown / f32::from(line).max(1.)).floor() as usize;
    rows.saturating_sub(1).max(1)
}

type Draw = Box<dyn Fn(Range<usize>, Pixels, &mut Window, &mut Cx) -> Vec<AnyElement>>;

/// `tops` holds each row's top and, last, the whole height.
pub fn rows(
    id: impl Into<ElementId>,
    scroll: Scrolling,
    tops: Rc<[Pixels]>,
    line: Pixels,
    view: EntityId,
    draw: impl Fn(Range<usize>, Pixels, &mut Window, &mut Cx) -> Vec<AnyElement> + 'static,
) -> Rows {
    Rows {
        id: id.into(),
        scroll,
        tops,
        line,
        view,
        draw: Box::new(draw),
    }
}

pub struct Rows {
    id: ElementId,
    scroll: Scrolling,
    tops: Rc<[Pixels]>,
    line: Pixels,
    view: EntityId,
    draw: Draw,
}

impl IntoElement for Rows {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Rows {
    type RequestLayoutState = ();
    type PrepaintState = (Vec<AnyElement>, Hitbox);

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut Cx,
    ) -> (LayoutId, ()) {
        let style = Style {
            size: Size {
                width: relative(1.).into(),
                height: relative(1.).into(),
            },
            ..Style::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut Cx,
    ) -> (Vec<AnyElement>, Hitbox) {
        let total = self.tops.last().copied().unwrap_or_default();
        let gliding = {
            let mut scroll = self.scroll.borrow_mut();
            scroll.fit(total, bounds.size.height);
            scroll.advance()
        };
        if gliding {
            window.request_animation_frame();
        }
        let top = self.scroll.borrow().top();
        let count = self.tops.len().saturating_sub(1);
        let first = self
            .tops
            .partition_point(|row| *row <= top)
            .saturating_sub(1);
        let end = self
            .tops
            .partition_point(|row| *row < top + bounds.size.height)
            .min(count);
        let mut drawn = (self.draw)(first..end, bounds.size.width, window, cx);
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for (at, row) in (first..end).zip(drawn.iter_mut()) {
                let height = self.tops[at + 1] - self.tops[at];
                let room = size(
                    AvailableSpace::Definite(bounds.size.width),
                    AvailableSpace::Definite(height),
                );
                row.layout_as_root(room, window, cx);
                row.prepaint_at(
                    bounds.origin + point(px(0.), self.tops[at] - top),
                    window,
                    cx,
                );
            }
        });
        (drawn, window.insert_hitbox(bounds, HitboxBehavior::Normal))
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        (drawn, hitbox): &mut (Vec<AnyElement>, Hitbox),
        window: &mut Window,
        cx: &mut Cx,
    ) {
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for row in drawn.iter_mut() {
                row.paint(window, cx);
            }
        });
        let (scroll, line, view, hitbox) =
            (self.scroll.clone(), self.line, self.view, hitbox.clone());
        window.on_mouse_event(move |event: &ScrollEvent, phase, window, cx| {
            let sideways = match event.delta {
                ScrollDelta::Pixels(moved) => moved.x.abs() > moved.y.abs(),
                ScrollDelta::Lines(lines) => lines.x.abs() > lines.y.abs(),
            };
            if sideways || event.modifiers.shift {
                return;
            }
            if phase.bubble() && hitbox.should_handle_scroll(window) {
                scroll.borrow_mut().wheel(event.delta, line);
                cx.notify(view);
                cx.stop_propagation();
            }
        });
    }
}

pub fn bar_room(theme: &Theme) -> Pixels {
    theme::GAP_HAIR + theme.sized(9.) + theme::GAP_LINE + px(2.) + theme::GAP_HAIR
}

/// The bar at a scrolled area's right, its thumb held to scroll; `view` is drawn
/// again as the thumb moves.
pub fn bar(theme: &Theme, scroll: Scrolling, view: EntityId) -> impl IntoElement {
    let (colour, strong) = (with_alpha(theme.colours.ink3, 0.6), theme.colours.ink2);
    let (width, shortest) = (theme.sized(9.), theme.sized(32.));
    let thumb = move |scroll: &Scroll, track: Pixels| {
        let length = (track * (scroll.shown() / scroll.total().max(px(1.)))).max(shortest);
        let part = if scroll.most() > px(0.) {
            scroll.top() / scroll.most()
        } else {
            0.
        };
        (length, (track - length) * part)
    };
    let drawn = scroll;
    let only_scrolling = theme.appearance.idle_scrollbars;
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let scroll = drawn.borrow();
            if scroll.total() <= scroll.shown() {
                return;
            }
            let track = bounds.size.height - theme::GAP_HAIR * 2.;
            let (length, top) = thumb(&scroll, track);
            let at = point(
                bounds.left() + theme::GAP_HAIR,
                bounds.top() + theme::GAP_HAIR + top,
            );
            let pointed = scroll.hovered || scroll.held.is_some();
            let shown = match pointed {
                true => strong,
                false => colour,
            };
            let idle = only_scrolling && !pointed && !scroll.lately_moved();
            if !idle {
                window.paint_quad(
                    fill(gpui::Bounds::new(at, size(width, length)), shown)
                        .corner_radii(width / 2.),
                );
            }
            // Drawn again until the moved list's bar may hide.
            if only_scrolling && !pointed && !idle {
                window.request_animation_frame();
            }
            drop(scroll);
            let (held, moved) = (drawn.clone(), drawn.clone());
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if !phase.bubble()
                    || event.button != MouseButton::Left
                    || !bounds.contains(&event.position)
                {
                    return;
                }
                let mut scroll = held.borrow_mut();
                let (length, top) = thumb(&scroll, track);
                let within = event.position.y - bounds.top() - theme::GAP_HAIR;
                let grip = if within >= top && within <= top + length {
                    within - top
                } else {
                    length / 2.
                };
                scroll.held = Some(grip);
                let room = (track - length).max(px(1.));
                let most = scroll.most();
                scroll.set_top(most * ((within - grip) / room).clamp(0., 1.));
                drop(scroll);
                cx.notify(view);
                cx.stop_propagation();
                let _ = window;
            });
            window.on_mouse_event(move |event: &MouseMoveEvent, _, _, cx| {
                let mut scroll = moved.borrow_mut();
                let over = bounds.contains(&event.position);
                if over != scroll.hovered {
                    scroll.hovered = over;
                    cx.notify(view);
                }
                let Some(grip) = scroll.held else {
                    return;
                };
                if !event.dragging() {
                    scroll.held = None;
                    return;
                }
                let (length, _) = thumb(&scroll, track);
                let within = event.position.y - bounds.top() - theme::GAP_HAIR;
                let room = (track - length).max(px(1.));
                let most = scroll.most();
                scroll.set_top(most * ((within - grip) / room).clamp(0., 1.));
                cx.notify(view);
            });
            let released = drawn.clone();
            window.on_mouse_event(move |_: &MouseUpEvent, _, _, cx| {
                if released.borrow_mut().held.take().is_some() {
                    cx.notify(view);
                }
            });
        },
    )
    .absolute()
    .top_0()
    .right_0()
    .bottom_0()
    .w(bar_room(theme))
}
