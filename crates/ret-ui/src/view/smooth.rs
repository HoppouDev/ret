//! Smooth scrolling for the scrollables of the window
//!
//! A scrollable moves by whole lines at once for each notch of a mouse wheel. Wrapped in
//! [`smooth`], the same distance is covered over a few frames, quickly at first then slowing
//! down. Touchpads already scroll by pixels, smoothly, so their events are left as they are.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector, keyboard, window};
use std::time::{Duration, Instant};

use crate::app::Message;

/// Pixels of a line of a mouse wheel, as a scrollable counts them
const LINE: f32 = 60.0;
/// Time for the scrolling to cover 63% of the remaining distance
const TIME_CONSTANT: f32 = 0.05;
/// Shortest step, in pixels
///
/// Without it the end of a movement would crawl by fractions of a pixel for many frames, moving
/// nothing visible.
const MIN_STEP: f32 = 1.0;

/// `scrollable`, scrolled smoothly by the mouse wheel
pub fn smooth<'a>(scrollable: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    Element::new(Smooth {
        content: scrollable.into(),
    })
}

struct Smooth<'a> {
    content: Element<'a, Message>,
}

#[derive(Default)]
struct State {
    /// Distance left to scroll, in the pixels of a wheel event
    pending: Vector,
    /// When the movement started or its last step was taken, `None` while still
    last_step: Option<Instant>,
    /// Shift turns vertical wheel movements horizontal, left to the scrollable
    shift: bool,
}

impl State {
    /// The distance to scroll in a frame at `now`, taken from what is pending
    fn step(&mut self, now: Instant) -> Vector {
        let elapsed = self.last_step.map_or(Duration::ZERO, |last| now - last);
        self.last_step = Some(now);
        let fraction = 1.0 - (-elapsed.as_secs_f32() / TIME_CONSTANT).exp();
        let step = Vector::new(
            axis_step(self.pending.x, fraction),
            axis_step(self.pending.y, fraction),
        );
        self.pending -= step;
        if self.pending == Vector::ZERO {
            self.last_step = None;
        }
        step
    }
}

/// The part of `pending` to scroll on one axis: `fraction` of it, at least [`MIN_STEP`], at most
/// all of it
fn axis_step(pending: f32, fraction: f32) -> f32 {
    let distance = pending.abs();
    let step = (distance * fraction).max(MIN_STEP).min(distance);
    step.copysign(pending)
}

impl widget::Widget<Message, Theme, Renderer> for Smooth<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        match event {
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                state.shift = modifiers.shift();
            }
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x, y },
            }) if !state.shift && cursor.is_over(layout.bounds()) => {
                state.pending += Vector::new(*x, *y) * LINE;
                // the first frame then covers the time since the wheel turned, at any frame rate
                state.last_step.get_or_insert_with(Instant::now);
                shell.capture_event();
                shell.request_redraw();
                return;
            }
            _ => {}
        }

        let content = self.content.as_widget_mut();
        let child = &mut tree.children[0];
        content.update(
            child, event, layout, cursor, renderer, clipboard, shell, viewport,
        );

        let state = tree.state.downcast_mut::<State>();
        if let Event::Window(window::Event::RedrawRequested(now)) = event
            && state.pending != Vector::ZERO
        {
            let step = state.step(*now);
            let wheel = Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels {
                    x: step.x,
                    y: step.y,
                },
            });
            // the scrollable only takes the wheel events of a cursor over it
            let over = mouse::Cursor::Available(layout.bounds().center());
            // through its own shell: the scrollable captures the wheel event, and that status must
            // not mark the redraw event as captured, which would make iced drop the open overlays
            // (such as a device menu) and the scrollables after this one skip their own step
            let mut messages = Vec::new();
            let mut scroll = Shell::new(&mut messages);
            content.update(
                &mut tree.children[0],
                &wheel,
                layout,
                over,
                renderer,
                clipboard,
                &mut scroll,
                viewport,
            );
            let redraw = scroll.redraw_request();
            let layout_invalid = scroll.is_layout_invalid();
            let widgets_invalid = scroll.are_widgets_invalid();
            drop(scroll);
            for message in messages {
                shell.publish(message);
            }
            shell.request_redraw_at(redraw);
            if layout_invalid {
                shell.invalidate_layout();
            }
            if widgets_invalid {
                shell.invalidate_widgets();
            }
            if state.pending != Vector::ZERO {
                shell.request_redraw();
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame at 60 frames per second
    const FRAME: Duration = Duration::from_micros(16_667);

    /// A movement of `pending` pixels down, started by a wheel at `start`
    fn moving(pending: f32, start: Instant) -> State {
        State {
            pending: Vector::new(0.0, pending),
            last_step: Some(start),
            shift: false,
        }
    }

    #[test]
    fn ease_out_then_stop_exactly() {
        let start = Instant::now();
        let mut state = moving(3.0 * LINE, start);
        let steps: Vec<f32> = (1..=40)
            .map(|frame| state.step(start + FRAME * frame).y)
            .collect();

        // each step is shorter than the previous one, until it stops
        let moving: Vec<f32> = steps.iter().copied().take_while(|&y| y > 0.0).collect();
        assert!(
            moving.windows(2).all(|pair| pair[1] <= pair[0]),
            "{moving:?}"
        );
        // the whole distance is covered in a fraction of a second, then it stops exactly
        let covered: f32 = steps.iter().sum();
        assert!((covered - 3.0 * LINE).abs() < 0.01, "{covered}");
        assert!(moving.len() < 20, "{} frames", moving.len());
        assert_eq!(Vector::ZERO, state.pending);
        assert_eq!(None, state.last_step);
    }

    #[test]
    fn cover_the_same_distance_at_any_frame_rate() {
        let start = Instant::now();
        // 100 ms after the wheel turned, at 60 and at 144 frames per second, first frame included
        let mut slow = moving(LINE, start);
        let mut fast = moving(LINE, start);
        for frame in 1..=6 {
            slow.step(start + FRAME * frame);
        }
        for frame in 1..=14 {
            fast.step(start + Duration::from_micros(7_143) * frame);
        }
        assert!(
            (slow.pending.y - fast.pending.y).abs() < 1.0,
            "{} and {}",
            slow.pending.y,
            fast.pending.y
        );
    }
}
