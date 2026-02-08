//! UI rendering using gartk-render

use anyhow::Result;
use garcalc_ipc::Mode;
use gartk_core::{Color, Rect, Theme};
use gartk_render::{Renderer, Surface, TextStyle};
use gartk_x11::Window;
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};

use crate::app::HistoryEntry;

/// Calculator UI renderer
pub struct CalculatorUI {
    window: Window,
    renderer: Renderer,
    theme: Theme,
    gc: u32,
}

impl CalculatorUI {
    pub fn new(window: Window, width: u32, height: u32) -> Result<Self> {
        let theme = Theme::dark();
        let renderer = Renderer::with_theme(width, height, theme.clone())?;

        // Create a GC for blitting
        let conn = window.connection();
        let gc = conn.generate_id()?;
        conn.inner().create_gc(gc, window.id(), &Default::default())?;
        conn.flush()?;

        Ok(Self {
            window,
            renderer,
            theme,
            gc,
        })
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn render(
        &mut self,
        input: &str,
        cursor: usize,
        history: &[HistoryEntry],
        mode: Mode,
    ) -> Result<()> {
        let size = self.renderer.size();

        // Clear background with darker color
        self.renderer.clear_color(self.theme.background.darken(0.1))?;

        // Mode indicator
        self.draw_mode_indicator(mode)?;

        // History area
        let history_start_y = 40;
        let input_height = 50;
        let history_end_y = size.height as i32 - input_height - 20;
        self.draw_history(history, history_start_y, history_end_y)?;

        // Input area
        let input_y = size.height as i32 - input_height - 10;
        self.draw_input(input, cursor, input_y)?;

        // Copy to window
        self.copy_to_window()?;

        Ok(())
    }

    fn draw_mode_indicator(&mut self, mode: Mode) -> Result<()> {
        let mode_text = match mode {
            Mode::Calculator => "CALC",
            Mode::Graph => "GRAPH",
            Mode::Geometry => "GEO",
            Mode::Spreadsheet => "SHEET",
            Mode::Notes => "NOTES",
        };

        // Background pill
        let pill_rect = Rect::new(10, 8, 70, 24);
        self.renderer.fill_rounded_rect(pill_rect, 4.0, self.theme.selection_background)?;

        // Text
        let style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(12.0)
            .color(self.theme.foreground);
        self.renderer.text(mode_text, 20.0, 8.0, &style)?;

        Ok(())
    }

    fn draw_history(
        &mut self,
        history: &[HistoryEntry],
        start_y: i32,
        end_y: i32,
    ) -> Result<()> {
        let line_height = 24;
        let max_lines = ((end_y - start_y) / line_height) as usize;
        let padding = 15;

        let input_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(14.0)
            .color(self.theme.foreground.with_alpha(0.7));

        let result_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(14.0)
            .color(self.theme.selection_foreground);

        // Error color - red
        let error_color = Color::rgb(0.9, 0.4, 0.4);
        let error_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(14.0)
            .color(error_color);

        // Show most recent entries that fit
        let start_idx = if history.len() > max_lines / 2 {
            history.len() - max_lines / 2
        } else {
            0
        };

        let mut y = start_y + 20;
        for entry in history.iter().skip(start_idx) {
            if y > end_y {
                break;
            }

            // Input line
            self.renderer.text(&format!("> {}", entry.input), padding as f64, y as f64, &input_style)?;
            y += line_height;

            // Result or error
            if let Some(ref error) = entry.error {
                self.renderer.text(&format!("  Error: {error}"), (padding + 10) as f64, y as f64, &error_style)?;
            } else {
                self.renderer.text(&format!("  = {}", entry.result), (padding + 10) as f64, y as f64, &result_style)?;
            }
            y += line_height + 5;
        }

        Ok(())
    }

    fn draw_input(&mut self, input: &str, cursor: usize, y: i32) -> Result<()> {
        let size = self.renderer.size();
        let padding = 15;
        let input_width = size.width - 2 * padding;
        let input_height = 40u32;

        // Input background
        let input_rect = Rect::new(padding as i32, y, input_width, input_height);
        self.renderer.fill_rounded_rect(input_rect, 6.0, self.theme.input_background)?;

        // Input text
        let text_x = padding + 10;
        let text_y = y + 8;

        let input_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(16.0)
            .color(self.theme.foreground);

        self.renderer.text(input, text_x as f64, text_y as f64, &input_style)?;

        // Cursor
        let cursor_text = if cursor < input.len() {
            &input[..cursor]
        } else {
            input
        };
        let cursor_size = self.renderer.measure_text(cursor_text, &input_style)?;
        let cursor_x = text_x as f64 + cursor_size.width as f64;

        // Draw cursor line
        let cursor_rect = Rect::new(cursor_x as i32, y + 8, 2, input_height - 16);
        self.renderer.fill_rect(cursor_rect, self.theme.input_cursor)?;

        Ok(())
    }

    fn copy_to_window(&mut self) -> Result<()> {
        let size = self.renderer.size();
        let conn = self.window.connection();

        // Need to copy surface to a temp surface to get the data
        let mut temp_surface = Surface::new(size.width, size.height)?;
        let temp_ctx = temp_surface.context()?;
        temp_ctx.set_source_surface(self.renderer.surface().cairo_surface(), 0.0, 0.0)?;
        temp_ctx.paint()?;
        drop(temp_ctx);

        let data = temp_surface.data()?;

        conn.inner().put_image(
            ImageFormat::Z_PIXMAP,
            self.window.id(),
            self.gc,
            size.width as u16,
            size.height as u16,
            0,
            0,
            0,
            self.window.depth(),
            &data,
        )?;

        conn.flush()?;

        Ok(())
    }
}

impl Drop for CalculatorUI {
    fn drop(&mut self) {
        let _ = self.window.connection().inner().free_gc(self.gc);
    }
}
