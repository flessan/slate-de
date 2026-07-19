use async_trait::async_trait;
use ratatui::layout::Rect;
use ratatui::Frame;
use serde::{Deserialize, Serialize};
use std::error::Error;

/// Trait for rendering Ratatui frames directly to Wayland SHM buffers.
pub trait WaylandTuiRenderer {
    type Error: Error + Send + Sync + 'static;

    /// Renders a frame to the provided shared memory buffer.
    fn render_to_shm(
        &mut self,
        area: Rect,
        f: &mut Frame,
        buffer: &mut [u8],
        stride: u32,
    ) -> Result<(), Self::Error>;

    /// Updates the internal state of the renderer (e.g., resizing buffers).
    fn resize(&mut self, width: u32, height: u32) -> Result<(), Self::Error>;
}

/// Interface for the modular vision framework.
#[async_trait]
pub trait VisionProvider: Send + Sync {
    /// Analyzes a frame for OCR or visual metadata.
    async fn process_frame(&self, rgba_data: &[u8], width: u32, height: u32) -> Result<VisionResult, anyhow::Error>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VisionResult {
    pub text_content: String,
    pub regions: Vec<VisionRegion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VisionRegion {
    pub x: u32,
    pub y: u32,
    pub label: String,
}

/// Plugin contract for WASM host-guest interaction.
pub trait PluginContract {
    /// Initialized when the plugin is loaded.
    fn on_init(&mut self, config: String) -> Result<(), String>;

    /// Hook for custom CLI commands.
    fn on_command(&mut self, command: String, args: Vec<String>) -> Result<Option<String>, String>;

    /// Lifecycle hook triggered on every compositor tick.
    fn on_tick(&mut self, delta_ms: u64) -> Result<(), String>;
    
    /// Cleanup before unloading.
    fn on_shutdown(&mut self);
}
