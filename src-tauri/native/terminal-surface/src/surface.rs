use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{self, Widget};
use iced::mouse;
use iced::{Color, Element, Length, Rectangle, Size, Theme};

/// Stub adapter that will hold the wgpu rendering pipeline in Phase 1.
pub struct SurfaceAdapter {
    _placeholder: (),
}

impl SurfaceAdapter {
    pub fn new() -> Self {
        Self { _placeholder: () }
    }
}

impl Default for SurfaceAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Custom Iced widget that renders the terminal surface.
///
/// Phase 0: draws a solid dark rectangle.
/// Phase 1: uses `SurfaceAdapter` to paint RichGridData via wgpu.
pub struct TerminalSurface {
    width: Length,
    height: Length,
}

impl TerminalSurface {
    pub fn new() -> Self {
        Self {
            width: Length::Fill,
            height: Length::Fill,
        }
    }
}

impl Default for TerminalSurface {
    fn default() -> Self {
        Self::new()
    }
}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for TerminalSurface
where
    Renderer: renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.max())
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                ..renderer::Quad::default()
            },
            Color::from_rgb(0.07, 0.07, 0.10),
        );
    }
}

impl<'a, Message: 'a, Renderer: renderer::Renderer + 'a> From<TerminalSurface>
    for Element<'a, Message, Theme, Renderer>
{
    fn from(surface: TerminalSurface) -> Self {
        Self::new(surface)
    }
}
