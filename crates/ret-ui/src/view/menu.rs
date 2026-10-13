//! A menu opened by a button, floating over the window below it

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Operation, Tree};
use iced::advanced::{Clipboard, Shell, mouse, overlay, renderer};
use iced::keyboard::{self, key};
use iced::{Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector, touch};

use crate::app::Message;

/// Space between the button and its menu
const GAP: f32 = 6.0;

/// `button`, with `menu` shown under it while `open`
///
/// The menu is right-aligned with the button, and shown above it when the window has no room
/// below. A click outside of both, or Escape, sends `on_dismiss`. A click on the button itself is
/// left to the button, so that it can close the menu.
pub fn menu<'a>(
    button: impl Into<Element<'a, Message>>,
    menu: impl Into<Element<'a, Message>>,
    open: bool,
    width: f32,
    on_dismiss: Message,
) -> Element<'a, Message> {
    Element::new(Menu {
        button: button.into(),
        menu: menu.into(),
        open,
        width,
        on_dismiss,
    })
}

struct Menu<'a> {
    button: Element<'a, Message>,
    menu: Element<'a, Message>,
    open: bool,
    width: f32,
    on_dismiss: Message,
}

impl Menu<'_> {
    fn button_tree(tree: &mut Tree) -> &mut Tree {
        &mut tree.children[0]
    }
}

impl widget::Widget<Message, Theme, Renderer> for Menu<'_> {
    fn size(&self) -> Size<Length> {
        self.button.as_widget().size()
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.button), Tree::new(&self.menu)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.button, &self.menu]);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.button
            .as_widget_mut()
            .layout(Self::button_tree(tree), renderer, limits)
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
        self.button.as_widget().draw(
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
        self.button
            .as_widget_mut()
            .operate(Self::button_tree(tree), layout, renderer, operation);
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
        self.button.as_widget_mut().update(
            Self::button_tree(tree),
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.button.as_widget().mouse_interaction(
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
        let [button_tree, menu_tree] = tree.children.as_mut_slice() else {
            return None;
        };
        if !self.open {
            return self.button.as_widget_mut().overlay(
                button_tree,
                layout,
                renderer,
                viewport,
                translation,
            );
        }
        Some(overlay::Element::new(Box::new(Overlay {
            menu: &mut self.menu,
            tree: menu_tree,
            button: layout.bounds() + translation,
            width: self.width,
            on_dismiss: &self.on_dismiss,
        })))
    }
}

struct Overlay<'a, 'b> {
    menu: &'b mut Element<'a, Message>,
    tree: &'b mut Tree,
    /// Bounds of the button in the window
    button: Rectangle,
    width: f32,
    on_dismiss: &'b Message,
}

impl overlay::Overlay<Message, Theme, Renderer> for Overlay<'_, '_> {
    fn layout(&mut self, renderer: &Renderer, window: Size) -> layout::Node {
        let limits =
            layout::Limits::new(Size::ZERO, Size::new(self.width, window.height)).width(self.width);
        let node = self
            .menu
            .as_widget_mut()
            .layout(self.tree, renderer, &limits);
        let size = node.size();
        let position = place(self.button, size, window);
        node.move_to(position)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        self.menu.as_widget().draw(
            self.tree,
            renderer,
            theme,
            style,
            layout,
            cursor,
            &layout.bounds(),
        );
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let dismissed = match event {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(key::Named::Escape),
                ..
            }) => true,
            Event::Mouse(mouse::Event::ButtonPressed(_))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                !cursor.is_over(layout.bounds()) && !cursor.is_over(self.button)
            }
            _ => false,
        };
        if dismissed {
            shell.publish(self.on_dismiss.clone());
        }
        self.menu.as_widget_mut().update(
            self.tree,
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            &layout.bounds(),
        );
        // the menu covers other widgets, such as the buttons of the next device: what happens over
        // it, on its padding or a disabled item too, must not reach them
        let pointer = matches!(event, Event::Mouse(_) | Event::Touch(_));
        if pointer && cursor.is_over(layout.bounds()) {
            shell.capture_event();
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let interaction = self.menu.as_widget().mouse_interaction(
            self.tree,
            layout,
            cursor,
            &layout.bounds(),
            renderer,
        );
        // iced hands the cursor to the widgets below an overlay which reports no interaction
        if interaction == mouse::Interaction::None && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Idle
        } else {
            interaction
        }
    }

    fn operate(&mut self, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn Operation) {
        self.menu
            .as_widget_mut()
            .operate(self.tree, layout, renderer, operation);
    }
}

/// Where a menu of `size` goes for the `button`: under it and right-aligned with it, above it if
/// the window has no room below, always inside the window
fn place(button: Rectangle, size: Size, window: Size) -> Point {
    let x = (button.x + button.width - size.width).clamp(0.0, (window.width - size.width).max(0.0));
    let below = button.y + button.height + GAP;
    let above = button.y - GAP - size.height;
    let y = if below + size.height <= window.height || above < 0.0 {
        below
    } else {
        above
    };
    Point::new(x, y.clamp(0.0, (window.height - size.height).max(0.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Size = Size::new(1000.0, 800.0);
    const MENU: Size = Size::new(280.0, 120.0);

    fn button_at(x: f32, y: f32) -> Rectangle {
        Rectangle::new(Point::new(x, y), Size::new(44.0, 44.0))
    }

    #[test]
    fn open_below_right_aligned() {
        let position = place(button_at(900.0, 200.0), MENU, WINDOW);
        assert_eq!(Point::new(944.0 - 280.0, 244.0 + GAP), position);
    }

    #[test]
    fn open_above_without_room_below() {
        let position = place(button_at(900.0, 700.0), MENU, WINDOW);
        assert_eq!(700.0 - GAP - 120.0, position.y);
    }

    #[test]
    fn stay_inside_the_window() {
        // a button near the left edge: the menu cannot extend past it
        assert_eq!(0.0, place(button_at(10.0, 200.0), MENU, WINDOW).x);
        // a window too short for the menu either way: as low as it fits
        let short = Size::new(1000.0, 150.0);
        assert_eq!(150.0 - 120.0, place(button_at(900.0, 50.0), MENU, short).y);
    }
}
