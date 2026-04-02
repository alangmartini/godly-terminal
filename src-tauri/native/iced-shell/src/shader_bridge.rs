//! Iced shader trait implementations for the atlas GPU pipeline.
//!
//! `godly-terminal-surface` defines the atlas types (`AtlasShaderProgram`,
//! `AtlasPrimitive`, `AtlasPipeline`) without any iced dependency.  This module
//! provides newtype wrappers that implement the `iced::widget::shader` traits so
//! the Shader widget can use them.

use godly_terminal_surface::atlas_shader::{AtlasPipeline, AtlasPrimitive, AtlasShaderProgram};
use iced::widget::shader;
use iced::wgpu;
use iced::{mouse, Rectangle};

// ---------------------------------------------------------------------------
// Program newtype
// ---------------------------------------------------------------------------

/// Wraps [`AtlasShaderProgram`] to implement [`shader::Program`].
pub struct IcedAtlasProgram(pub AtlasShaderProgram);

impl<Message> shader::Program<Message> for IcedAtlasProgram {
    type State = ();
    type Primitive = IcedAtlasPrimitive;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        _bounds: Rectangle,
    ) -> Self::Primitive {
        IcedAtlasPrimitive(self.0.build_primitive())
    }
}

// ---------------------------------------------------------------------------
// Primitive newtype
// ---------------------------------------------------------------------------

/// Wraps [`AtlasPrimitive`] to implement [`shader::Primitive`].
#[derive(Debug)]
pub struct IcedAtlasPrimitive(AtlasPrimitive);

impl shader::Primitive for IcedAtlasPrimitive {
    type Pipeline = IcedAtlasPipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        _viewport: &shader::Viewport,
    ) {
        self.0.prepare(&mut pipeline.0, device, queue);
    }

    fn draw(
        &self,
        pipeline: &Self::Pipeline,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> bool {
        self.0.draw(&pipeline.0, render_pass)
    }
}

// ---------------------------------------------------------------------------
// Pipeline newtype
// ---------------------------------------------------------------------------

/// Wraps [`AtlasPipeline`] to implement [`shader::Pipeline`].
pub struct IcedAtlasPipeline(AtlasPipeline);

impl shader::Pipeline for IcedAtlasPipeline {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self(AtlasPipeline::new(device, queue, format))
    }
}
