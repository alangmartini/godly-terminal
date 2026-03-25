//! A thin wrapper widget that redirects vertical mouse-wheel deltas
//! into horizontal scrolling.  Wrap any horizontal `scrollable` with
//! [`horizontal_wheel()`] so that a normal (vertical) mouse wheel
//! scrolls its content sideways.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::{mouse, Element, Event, Length, Rectangle, Size, Vector};

/// Wrapper that converts vertical wheel events to horizontal.
pub struct HorizontalWheel<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
}

/// Wrap `content` so that vertical mouse-wheel deltas are redirected
/// into horizontal scrolling.
pub fn horizontal_wheel<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> HorizontalWheel<'a, Message, Theme, Renderer> {
    HorizontalWheel {
        content: content.into(),
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for HorizontalWheel<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
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
        // If the cursor is over our bounds and we get a vertical wheel
        // event, rewrite it so the Y delta becomes X delta.
        let redirected;
        let event = match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if cursor.is_over(layout.bounds()) =>
            {
                let new_delta = match *delta {
                    mouse::ScrollDelta::Lines { x, y } => {
                        mouse::ScrollDelta::Lines { x: x + y, y: 0.0 }
                    }
                    mouse::ScrollDelta::Pixels { x, y } => {
                        mouse::ScrollDelta::Pixels { x: x + y, y: 0.0 }
                    }
                };
                redirected = Event::Mouse(mouse::Event::WheelScrolled {
                    delta: new_delta,
                });
                &redirected
            }
            _ => event,
        };

        self.content.as_widget_mut().update(
            &mut tree.children[0],
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
        self.content
            .as_widget()
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
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

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<HorizontalWheel<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(widget: HorizontalWheel<'a, Message, Theme, Renderer>) -> Self {
        Element::new(widget)
    }
}
