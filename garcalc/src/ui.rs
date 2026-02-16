//! UI rendering using gartk-render

use anyhow::Result;
use garcalc_cas::expr::{Expr, Symbol};
use garcalc_cas::parser;
use garcalc_graph::{Graph2D, Graph3D, ViewportPreset, COLOR_PALETTE};
use garcalc_ipc::Mode;
use garcalc_math::{from_expr, MathBox, MathInput, MathLayoutEngine, MathRenderer};
use gartk_core::{Color, Point, Rect, Theme};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalcButtonAction {
    InsertText(&'static str),
    Command(&'static str),
    Backspace,
    Delete,
    MoveLeft,
    MoveRight,
    Tab,
    Clear,
    Evaluate,
    ToggleExtended,
}

#[derive(Debug, Clone, Copy)]
enum CalcButtonRole {
    Numeric,
    Operator,
    Function,
    Command,
    Control,
    Evaluate,
}

#[derive(Debug, Clone, Copy)]
struct CalcButtonSpec {
    label: &'static str,
    action: CalcButtonAction,
    role: CalcButtonRole,
}

#[derive(Debug, Clone, Copy)]
struct CalcButtonRender {
    rect: Rect,
    label: &'static str,
    action: CalcButtonAction,
    role: CalcButtonRole,
}

#[derive(Debug, Clone)]
struct CalcButtonLayout {
    panel_rect: Rect,
    toggle_rect: Option<Rect>,
    buttons: Vec<CalcButtonRender>,
    hidden_optional_rows: usize,
}

impl CalculatorUI {
    const HELP_BUTTON_SIZE: u32 = 28;

    pub fn new(window: Window, width: u32, height: u32) -> Result<Self> {
        let theme = Theme::dark();
        let renderer = Renderer::with_theme(width, height, theme.clone())?;

        // Create a GC for blitting
        let conn = window.connection();
        let gc = conn.generate_id()?;
        conn.inner()
            .create_gc(gc, window.id(), &Default::default())?;
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

    pub fn size(&self) -> (u32, u32) {
        let s = self.renderer.size();
        (s.width, s.height)
    }

    /// Resize the renderer to match new window dimensions
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if width > 0 && height > 0 {
            self.renderer = Renderer::with_theme(width, height, self.theme.clone())?;
        }
        Ok(())
    }

    pub fn render(
        &mut self,
        input: &str,
        cursor: usize,
        cursor_visible: bool,
        math_input: Option<&MathInput>,
        history: &[HistoryEntry],
        mode: Mode,
        graph: &Graph2D,
        graph3d: &Graph3D,
        show_help_modal: bool,
        calc_buttons_extended: bool,
        show_function_list: bool,
        zeros_visible: bool,
        cached_zeros: &[(usize, Vec<(f64, f64)>)],
        cached_intersections: &[((usize, usize), Vec<(f64, f64)>)],
        table_visible: bool,
        table_scroll_offset: usize,
        table_step: f64,
        table_data: &[(f64, Option<f64>)],
        viewport_panel_open: bool,
        viewport_edit_field: usize,
        viewport_edit_buffer: &str,
        color_picker_open: bool,
        color_picker_func_index: usize,
        settings3d_panel_open: bool,
        auto_rotate: bool,
    ) -> Result<()> {
        let size = self.renderer.size();

        if mode == Mode::Graph {
            // Graph mode: render 2D graph with overlay input
            self.render_graph_mode(
                input,
                cursor,
                cursor_visible,
                history,
                graph,
                show_function_list,
                zeros_visible,
                cached_zeros,
                cached_intersections,
                table_visible,
                table_scroll_offset,
                table_step,
                table_data,
                viewport_panel_open,
                viewport_edit_field,
                viewport_edit_buffer,
                color_picker_open,
                color_picker_func_index,
            )?;
        } else if mode == Mode::Graph3D {
            // Graph3D mode: render 3D surface with overlay input
            self.render_graph3d_mode(input, cursor, cursor_visible, history, graph3d, settings3d_panel_open, auto_rotate)?;
        } else {
            // Calculator mode: standard layout
            // Clear background with darker color
            self.renderer
                .clear_color(self.theme.background.darken(0.1))?;

            // Mode indicator
            self.draw_mode_indicator(mode)?;

            let input_height: i32 = if math_input.is_some() { 88 } else { 50 };
            let input_y = size.height as i32 - input_height - 10;
            let button_layout = if math_input.is_some() {
                self.calculator_button_layout(input_y, calc_buttons_extended)
            } else {
                None
            };

            // History area
            let history_start_y = 40;
            let history_end_y = button_layout
                .as_ref()
                .map(|layout| layout.panel_rect.y - 10)
                .unwrap_or(size.height as i32 - input_height - 20);
            self.draw_history(history, history_start_y, history_end_y)?;

            if let Some(layout) = button_layout.as_ref() {
                self.draw_calculator_buttons(layout, calc_buttons_extended)?;
            }

            // Input area
            if let Some(math_input) = math_input {
                self.draw_math_input(math_input, cursor_visible, input_y, input_height as u32)?;
            } else {
                self.draw_text_input(input, cursor, cursor_visible, input_y)?;
            }
        }

        self.draw_help_button()?;
        if show_help_modal {
            self.draw_help_modal(mode)?;
        }

        // Copy to window
        self.copy_to_window()?;

        Ok(())
    }

    pub fn is_help_button_hit(&self, x: f64, y: f64) -> bool {
        self.help_button_rect()
            .contains_point(Point::new(x as i32, y as i32))
    }

    pub fn is_help_modal_hit(&self, x: f64, y: f64) -> bool {
        self.help_modal_rect()
            .contains_point(Point::new(x as i32, y as i32))
    }

    pub fn is_help_close_hit(&self, x: f64, y: f64) -> bool {
        self.help_close_rect()
            .contains_point(Point::new(x as i32, y as i32))
    }

    pub fn calculator_button_action_at(
        &self,
        x: f64,
        y: f64,
        calc_buttons_extended: bool,
    ) -> Option<CalcButtonAction> {
        let size = self.renderer.size();
        let input_y = size.height as i32 - 88 - 10;
        let layout = self.calculator_button_layout(input_y, calc_buttons_extended)?;
        let point = Point::new(x as i32, y as i32);

        if let Some(toggle) = layout.toggle_rect {
            if toggle.contains_point(point) {
                return Some(CalcButtonAction::ToggleExtended);
            }
        }

        layout
            .buttons
            .iter()
            .find(|button| button.rect.contains_point(point))
            .map(|button| button.action)
    }

    fn render_graph_mode(
        &mut self,
        input: &str,
        cursor: usize,
        cursor_visible: bool,
        history: &[HistoryEntry],
        graph: &Graph2D,
        show_function_list: bool,
        zeros_visible: bool,
        cached_zeros: &[(usize, Vec<(f64, f64)>)],
        cached_intersections: &[((usize, usize), Vec<(f64, f64)>)],
        table_visible: bool,
        table_scroll_offset: usize,
        table_step: f64,
        table_data: &[(f64, Option<f64>)],
        viewport_panel_open: bool,
        viewport_edit_field: usize,
        viewport_edit_buffer: &str,
        color_picker_open: bool,
        color_picker_func_index: usize,
    ) -> Result<()> {
        let size = self.renderer.size();
        let width = size.width;
        let height = size.height;

        // Get Cairo context from renderer surface
        let ctx = self.renderer.surface().context()?;

        // Render the graph (fills entire area)
        graph.render(&ctx, width, height);

        // Draw zeros/intersection markers on the graph surface
        if zeros_visible {
            let zero_color = garcalc_graph::Color { r: 255, g: 80, b: 80, a: 255 };
            let isect_color = garcalc_graph::Color { r: 80, g: 255, b: 80, a: 255 };
            for (_idx, pts) in cached_zeros {
                graph.draw_markers(&ctx, pts, zero_color, width, height);
            }
            for (_pair, pts) in cached_intersections {
                graph.draw_markers(&ctx, pts, isect_color, width, height);
            }
        }

        // Mode indicator (overlay)
        self.draw_mode_indicator(Mode::Graph)?;

        // Input area at bottom (overlay with semi-transparent background)
        let input_height = 50;
        let input_y = height as i32 - input_height - 10;

        // Semi-transparent background for input area
        let input_bg = Rect::new(10, input_y - 5, width - 20, input_height as u32 + 10);
        self.renderer
            .fill_rounded_rect(input_bg, 8.0, self.theme.background.with_alpha(0.85))?;

        // Draw input
        self.draw_text_input(input, cursor, cursor_visible, input_y)?;

        // Show most recent history entry as overlay (if any)
        if let Some(entry) = history.last() {
            let result_style = TextStyle::new()
                .font_family(&self.theme.font_family)
                .font_size(12.0)
                .color(self.theme.foreground.with_alpha(0.8));

            let text = if let Some(ref error) = entry.error {
                format!("Error: {error}")
            } else {
                entry.result.clone()
            };

            // Background for result
            let result_bg = Rect::new(10, input_y - 30, width - 20, 22);
            self.renderer.fill_rounded_rect(
                result_bg,
                4.0,
                self.theme.background.with_alpha(0.75),
            )?;

            self.renderer
                .text(&text, 20.0, (input_y - 26) as f64, &result_style)?;
        }

        // Show function count
        let func_count = graph.functions.len();
        if func_count > 0 {
            let func_style = TextStyle::new()
                .font_family(&self.theme.font_family)
                .font_size(11.0)
                .color(self.theme.foreground.with_alpha(0.7));

            let func_text = format!(
                "{} function{}",
                func_count,
                if func_count == 1 { "" } else { "s" }
            );
            self.renderer
                .text(&func_text, (width - 80) as f64, 12.0, &func_style)?;
        }

        // Function list overlay
        if show_function_list {
            self.draw_function_list(graph)?;
        }

        // Table view overlay
        if table_visible {
            self.draw_table_view(table_data, table_scroll_offset, table_step)?;
        }

        // Viewport settings panel
        if viewport_panel_open {
            self.draw_viewport_panel(graph, viewport_edit_field, viewport_edit_buffer)?;
        }

        // Color picker popup
        if color_picker_open {
            self.draw_color_picker(graph, color_picker_func_index)?;
        }

        // Help text
        let help_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(10.0)
            .color(self.theme.foreground.with_alpha(0.5));
        self.renderer.text(
            "Scroll: zoom | Drag: pan | Ctrl+W: viewport | Ctrl+K: color | Ctrl+L: funcs",
            90.0,
            12.0,
            &help_style,
        )?;

        Ok(())
    }

    fn render_graph3d_mode(
        &mut self,
        input: &str,
        cursor: usize,
        cursor_visible: bool,
        history: &[HistoryEntry],
        graph3d: &Graph3D,
        settings3d_panel_open: bool,
        auto_rotate: bool,
    ) -> Result<()> {
        let size = self.renderer.size();
        let width = size.width;
        let height = size.height;

        // Get Cairo context from renderer surface
        let ctx = self.renderer.surface().context()?;

        // Render the 3D graph (fills entire area)
        graph3d.render(&ctx, width, height);

        // Mode indicator (overlay)
        self.draw_mode_indicator(Mode::Graph3D)?;

        // Input area at bottom (overlay with semi-transparent background)
        let input_height = 50;
        let input_y = height as i32 - input_height - 10;

        // Semi-transparent background for input area
        let input_bg = Rect::new(10, input_y - 5, width - 20, input_height as u32 + 10);
        self.renderer
            .fill_rounded_rect(input_bg, 8.0, self.theme.background.with_alpha(0.85))?;

        // Draw input
        self.draw_text_input(input, cursor, cursor_visible, input_y)?;

        // Show most recent history entry as overlay (if any)
        if let Some(entry) = history.last() {
            let result_style = TextStyle::new()
                .font_family(&self.theme.font_family)
                .font_size(12.0)
                .color(self.theme.foreground.with_alpha(0.8));

            let text = if let Some(ref error) = entry.error {
                format!("Error: {error}")
            } else {
                entry.result.clone()
            };

            // Background for result
            let result_bg = Rect::new(10, input_y - 30, width - 20, 22);
            self.renderer.fill_rounded_rect(
                result_bg,
                4.0,
                self.theme.background.with_alpha(0.75),
            )?;

            self.renderer
                .text(&text, 20.0, (input_y - 26) as f64, &result_style)?;
        }

        // Show surface count
        let surface_count = graph3d.surfaces.len();
        if surface_count > 0 {
            let func_style = TextStyle::new()
                .font_family(&self.theme.font_family)
                .font_size(11.0)
                .color(self.theme.foreground.with_alpha(0.7));

            let func_text = format!(
                "{} surface{}",
                surface_count,
                if surface_count == 1 { "" } else { "s" }
            );
            self.renderer
                .text(&func_text, (width - 80) as f64, 12.0, &func_style)?;
        }

        // 3D settings panel
        if settings3d_panel_open {
            self.draw_3d_settings_panel(graph3d, auto_rotate)?;
        }

        // Help text
        let help_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(10.0)
            .color(self.theme.foreground.with_alpha(0.5));
        self.renderer.text(
            "Drag: rotate | Ctrl+M: mode | Ctrl+A: spin | Ctrl+G: grid | Ctrl+S: settings",
            55.0,
            12.0,
            &help_style,
        )?;

        Ok(())
    }

    fn draw_mode_indicator(&mut self, mode: Mode) -> Result<()> {
        let mode_text = match mode {
            Mode::Calculator => "CALC",
            Mode::Graph => "GRAPH",
            Mode::Graph3D => "3D",
            Mode::Geometry => "GEO",
            Mode::Spreadsheet => "SHEET",
            Mode::Notes => "NOTES",
        };

        // Background pill
        let pill_rect = Rect::new(10, 8, 70, 24);
        self.renderer
            .fill_rounded_rect(pill_rect, 4.0, self.theme.selection_background)?;

        // Text
        let style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(12.0)
            .color(self.theme.foreground);
        self.renderer.text(mode_text, 20.0, 8.0, &style)?;

        Ok(())
    }

    fn calc_core_rows() -> Vec<Vec<CalcButtonSpec>> {
        vec![
            vec![
                CalcButtonSpec {
                    label: "7",
                    action: CalcButtonAction::InsertText("7"),
                    role: CalcButtonRole::Numeric,
                },
                CalcButtonSpec {
                    label: "8",
                    action: CalcButtonAction::InsertText("8"),
                    role: CalcButtonRole::Numeric,
                },
                CalcButtonSpec {
                    label: "9",
                    action: CalcButtonAction::InsertText("9"),
                    role: CalcButtonRole::Numeric,
                },
                CalcButtonSpec {
                    label: "/",
                    action: CalcButtonAction::InsertText("/"),
                    role: CalcButtonRole::Operator,
                },
                CalcButtonSpec {
                    label: "(",
                    action: CalcButtonAction::InsertText("("),
                    role: CalcButtonRole::Operator,
                },
                CalcButtonSpec {
                    label: ")",
                    action: CalcButtonAction::InsertText(")"),
                    role: CalcButtonRole::Operator,
                },
            ],
            vec![
                CalcButtonSpec {
                    label: "4",
                    action: CalcButtonAction::InsertText("4"),
                    role: CalcButtonRole::Numeric,
                },
                CalcButtonSpec {
                    label: "5",
                    action: CalcButtonAction::InsertText("5"),
                    role: CalcButtonRole::Numeric,
                },
                CalcButtonSpec {
                    label: "6",
                    action: CalcButtonAction::InsertText("6"),
                    role: CalcButtonRole::Numeric,
                },
                CalcButtonSpec {
                    label: "*",
                    action: CalcButtonAction::InsertText("*"),
                    role: CalcButtonRole::Operator,
                },
                CalcButtonSpec {
                    label: "x",
                    action: CalcButtonAction::InsertText("x"),
                    role: CalcButtonRole::Function,
                },
                CalcButtonSpec {
                    label: "y",
                    action: CalcButtonAction::InsertText("y"),
                    role: CalcButtonRole::Function,
                },
            ],
            vec![
                CalcButtonSpec {
                    label: "1",
                    action: CalcButtonAction::InsertText("1"),
                    role: CalcButtonRole::Numeric,
                },
                CalcButtonSpec {
                    label: "2",
                    action: CalcButtonAction::InsertText("2"),
                    role: CalcButtonRole::Numeric,
                },
                CalcButtonSpec {
                    label: "3",
                    action: CalcButtonAction::InsertText("3"),
                    role: CalcButtonRole::Numeric,
                },
                CalcButtonSpec {
                    label: "-",
                    action: CalcButtonAction::InsertText("-"),
                    role: CalcButtonRole::Operator,
                },
                CalcButtonSpec {
                    label: "^",
                    action: CalcButtonAction::InsertText("^"),
                    role: CalcButtonRole::Operator,
                },
                CalcButtonSpec {
                    label: "!",
                    action: CalcButtonAction::InsertText("!"),
                    role: CalcButtonRole::Operator,
                },
            ],
            vec![
                CalcButtonSpec {
                    label: "0",
                    action: CalcButtonAction::InsertText("0"),
                    role: CalcButtonRole::Numeric,
                },
                CalcButtonSpec {
                    label: ".",
                    action: CalcButtonAction::InsertText("."),
                    role: CalcButtonRole::Numeric,
                },
                CalcButtonSpec {
                    label: ",",
                    action: CalcButtonAction::InsertText(","),
                    role: CalcButtonRole::Operator,
                },
                CalcButtonSpec {
                    label: "+",
                    action: CalcButtonAction::InsertText("+"),
                    role: CalcButtonRole::Operator,
                },
                CalcButtonSpec {
                    label: "π",
                    action: CalcButtonAction::InsertText("pi"),
                    role: CalcButtonRole::Function,
                },
                CalcButtonSpec {
                    label: "=",
                    action: CalcButtonAction::Evaluate,
                    role: CalcButtonRole::Evaluate,
                },
            ],
        ]
    }

    fn calc_scientific_rows() -> Vec<Vec<CalcButtonSpec>> {
        vec![
            vec![
                CalcButtonSpec {
                    label: "sin(",
                    action: CalcButtonAction::InsertText("sin("),
                    role: CalcButtonRole::Function,
                },
                CalcButtonSpec {
                    label: "cos(",
                    action: CalcButtonAction::InsertText("cos("),
                    role: CalcButtonRole::Function,
                },
                CalcButtonSpec {
                    label: "tan(",
                    action: CalcButtonAction::InsertText("tan("),
                    role: CalcButtonRole::Function,
                },
                CalcButtonSpec {
                    label: "ln(",
                    action: CalcButtonAction::InsertText("ln("),
                    role: CalcButtonRole::Function,
                },
                CalcButtonSpec {
                    label: "log(",
                    action: CalcButtonAction::InsertText("log("),
                    role: CalcButtonRole::Function,
                },
                CalcButtonSpec {
                    label: "√",
                    action: CalcButtonAction::InsertText("sqrt("),
                    role: CalcButtonRole::Function,
                },
            ],
            vec![
                CalcButtonSpec {
                    label: "sin⁻¹",
                    action: CalcButtonAction::InsertText("asin("),
                    role: CalcButtonRole::Function,
                },
                CalcButtonSpec {
                    label: "cos⁻¹",
                    action: CalcButtonAction::InsertText("acos("),
                    role: CalcButtonRole::Function,
                },
                CalcButtonSpec {
                    label: "tan⁻¹",
                    action: CalcButtonAction::InsertText("atan("),
                    role: CalcButtonRole::Function,
                },
                CalcButtonSpec {
                    label: "eˣ",
                    action: CalcButtonAction::InsertText("exp("),
                    role: CalcButtonRole::Function,
                },
                CalcButtonSpec {
                    label: "abs(",
                    action: CalcButtonAction::InsertText("abs("),
                    role: CalcButtonRole::Function,
                },
                CalcButtonSpec {
                    label: "gamma(",
                    action: CalcButtonAction::InsertText("gamma("),
                    role: CalcButtonRole::Function,
                },
            ],
        ]
    }

    fn calc_extended_rows() -> Vec<Vec<CalcButtonSpec>> {
        vec![
            vec![
                CalcButtonSpec {
                    label: "a⁄b",
                    action: CalcButtonAction::Command("frac"),
                    role: CalcButtonRole::Command,
                },
                CalcButtonSpec {
                    label: "∑",
                    action: CalcButtonAction::Command("sum"),
                    role: CalcButtonRole::Command,
                },
                CalcButtonSpec {
                    label: "∏",
                    action: CalcButtonAction::Command("prod"),
                    role: CalcButtonRole::Command,
                },
                CalcButtonSpec {
                    label: "∫",
                    action: CalcButtonAction::Command("int"),
                    role: CalcButtonRole::Command,
                },
                CalcButtonSpec {
                    label: "∫ᵇₐ",
                    action: CalcButtonAction::Command("dint"),
                    role: CalcButtonRole::Command,
                },
                CalcButtonSpec {
                    label: "d/dx",
                    action: CalcButtonAction::Command("diff"),
                    role: CalcButtonRole::Command,
                },
            ],
            vec![
                CalcButtonSpec {
                    label: "limₓ→a",
                    action: CalcButtonAction::Command("lim"),
                    role: CalcButtonRole::Command,
                },
                CalcButtonSpec {
                    label: "x=?",
                    action: CalcButtonAction::Command("solve"),
                    role: CalcButtonRole::Command,
                },
                CalcButtonSpec {
                    label: "ⁿ√",
                    action: CalcButtonAction::Command("nthroot"),
                    role: CalcButtonRole::Command,
                },
                CalcButtonSpec {
                    label: "▦",
                    action: CalcButtonAction::Command("matrix"),
                    role: CalcButtonRole::Command,
                },
                CalcButtonSpec {
                    label: "↓min",
                    action: CalcButtonAction::InsertText("min("),
                    role: CalcButtonRole::Function,
                },
                CalcButtonSpec {
                    label: "↑max",
                    action: CalcButtonAction::InsertText("max("),
                    role: CalcButtonRole::Function,
                },
            ],
        ]
    }

    fn calc_control_row() -> Vec<CalcButtonSpec> {
        vec![
            CalcButtonSpec {
                label: "Bksp",
                action: CalcButtonAction::Backspace,
                role: CalcButtonRole::Control,
            },
            CalcButtonSpec {
                label: "Del",
                action: CalcButtonAction::Delete,
                role: CalcButtonRole::Control,
            },
            CalcButtonSpec {
                label: "Clr",
                action: CalcButtonAction::Clear,
                role: CalcButtonRole::Control,
            },
            CalcButtonSpec {
                label: "<",
                action: CalcButtonAction::MoveLeft,
                role: CalcButtonRole::Control,
            },
            CalcButtonSpec {
                label: ">",
                action: CalcButtonAction::MoveRight,
                role: CalcButtonRole::Control,
            },
            CalcButtonSpec {
                label: "Tab",
                action: CalcButtonAction::Tab,
                role: CalcButtonRole::Control,
            },
        ]
    }

    fn calculator_button_layout(&self, input_y: i32, extended: bool) -> Option<CalcButtonLayout> {
        let size = self.renderer.size();
        let min_history_height = 90i32;
        let top_reserved = 46i32;
        let max_panel_height = input_y - top_reserved - min_history_height - 8;
        if max_panel_height < 132 {
            return None;
        }

        let cols = 6i32;
        let gap = 8i32;
        let panel_side_padding = 12i32;
        let panel_margin = 12i32;
        let panel_max_width = size.width as i32 - panel_margin * 2;
        if panel_max_width < 320 {
            return None;
        }

        let mut button_w = (panel_max_width - panel_side_padding * 2 - (cols - 1) * gap) / cols;
        button_w = button_w.clamp(44, 92);
        let button_h = ((button_w as f64) * 0.58).round() as i32;
        let button_h = button_h.clamp(30, 42);
        let inner_width = cols * button_w + (cols - 1) * gap;
        let panel_width = inner_width + panel_side_padding * 2;
        let panel_x = (size.width as i32 - panel_width) / 2;

        let mut rows = Vec::new();
        rows.push(Self::calc_control_row());
        rows.extend(Self::calc_core_rows());

        let mut optional_rows = Self::calc_scientific_rows();
        if extended {
            optional_rows.extend(Self::calc_extended_rows());
        }

        let toolbar_h = 34i32;
        let row_gap = gap;
        let panel_vertical_padding = 12i32;
        let max_rows_fit = ((max_panel_height - toolbar_h - panel_vertical_padding * 2 + row_gap)
            / (button_h + row_gap))
            .max(0) as usize;
        let required_rows = rows.len();
        if max_rows_fit < required_rows {
            return None;
        }

        let extra_fit = max_rows_fit - required_rows;
        let optional_visible = optional_rows.len().min(extra_fit);
        let hidden_optional_rows = optional_rows.len().saturating_sub(optional_visible);
        rows.extend(optional_rows.into_iter().take(optional_visible));

        let row_count = rows.len() as i32;
        let panel_height = panel_vertical_padding * 2
            + toolbar_h
            + row_count * button_h
            + (row_count - 1).max(0) * row_gap;
        let panel_bottom = input_y - 10;
        let panel_y = panel_bottom - panel_height;

        if panel_y < top_reserved + min_history_height {
            return None;
        }

        let panel_rect = Rect::new(panel_x, panel_y, panel_width as u32, panel_height as u32);
        let toggle_rect = if panel_width >= 340 {
            Some(Rect::new(
                panel_x + (panel_width - 190) / 2,
                panel_y + 7,
                190,
                22,
            ))
        } else {
            None
        };

        let mut buttons = Vec::new();
        let grid_top = panel_y + panel_vertical_padding + toolbar_h;
        for (row_idx, row) in rows.iter().enumerate() {
            let cols_this_row = row.len() as i32;
            if cols_this_row == 0 {
                continue;
            }
            let row_width = cols_this_row * button_w + (cols_this_row - 1) * gap;
            let row_x = panel_x + (panel_width - row_width) / 2;
            let y = grid_top + row_idx as i32 * (button_h + row_gap);

            for (col_idx, spec) in row.iter().enumerate() {
                let x = row_x + col_idx as i32 * (button_w + gap);
                buttons.push(CalcButtonRender {
                    rect: Rect::new(x, y, button_w as u32, button_h as u32),
                    label: spec.label,
                    action: spec.action,
                    role: spec.role,
                });
            }
        }

        Some(CalcButtonLayout {
            panel_rect,
            toggle_rect,
            buttons,
            hidden_optional_rows,
        })
    }

    fn draw_calculator_buttons(
        &mut self,
        layout: &CalcButtonLayout,
        calc_buttons_extended: bool,
    ) -> Result<()> {
        self.renderer.fill_rounded_rect(
            layout.panel_rect,
            10.0,
            self.theme.background.lighten(0.03).with_alpha(0.96),
        )?;
        self.renderer.stroke_rounded_rect(
            layout.panel_rect,
            10.0,
            self.theme.border.with_alpha(0.75),
            1.0,
        )?;

        if let Some(toggle_rect) = layout.toggle_rect {
            let toggle_bg = if calc_buttons_extended {
                self.theme.selection_background.with_alpha(0.95)
            } else {
                self.theme.item_hover_background.with_alpha(0.95)
            };
            self.renderer
                .fill_rounded_rect(toggle_rect, 7.0, toggle_bg)?;
            self.renderer.stroke_rounded_rect(
                toggle_rect,
                7.0,
                self.theme.border.with_alpha(0.85),
                1.0,
            )?;

            let toggle_label = if calc_buttons_extended {
                "Mode: Extended"
            } else {
                "Mode: Scientific"
            };
            let toggle_style = TextStyle::new()
                .font_family(&self.theme.font_family)
                .font_size(11.0)
                .color(self.theme.selection_foreground);
            let text_size = self.renderer.measure_text(toggle_label, &toggle_style)?;
            self.renderer.text(
                toggle_label,
                toggle_rect.x as f64 + (toggle_rect.width as f64 - text_size.width as f64) * 0.5,
                toggle_rect.y as f64 + (toggle_rect.height as f64 - text_size.height as f64) * 0.5,
                &toggle_style,
            )?;
        }

        if layout.hidden_optional_rows > 0 {
            let hint = format!(
                "{} function row(s) hidden by size",
                layout.hidden_optional_rows
            );
            let hint_style = TextStyle::new()
                .font_family(&self.theme.font_family)
                .font_size(9.5)
                .color(self.theme.foreground.with_alpha(0.62));
            self.renderer.text(
                &hint,
                (layout.panel_rect.x + 10) as f64,
                (layout.panel_rect.y + 10) as f64,
                &hint_style,
            )?;
        }

        for button in &layout.buttons {
            let bg = match button.role {
                CalcButtonRole::Numeric => self.theme.input_background.lighten(0.08),
                CalcButtonRole::Operator => self.theme.selection_background.with_alpha(0.45),
                CalcButtonRole::Function => self.theme.item_hover_background.with_alpha(0.88),
                CalcButtonRole::Command => Color::rgb(0.22, 0.34, 0.52).with_alpha(0.92),
                CalcButtonRole::Control => self.theme.background.lighten(0.09),
                CalcButtonRole::Evaluate => self.theme.selection_background.with_alpha(0.92),
            };
            self.renderer.fill_rounded_rect(button.rect, 7.0, bg)?;
            self.renderer.stroke_rounded_rect(
                button.rect,
                7.0,
                self.theme.border.with_alpha(0.7),
                1.0,
            )?;

            let label_style = TextStyle::new()
                .font_family(&self.theme.font_family)
                .font_size(11.0)
                .color(self.theme.foreground.with_alpha(0.96));
            let text_size = self.renderer.measure_text(button.label, &label_style)?;
            self.renderer.text(
                button.label,
                button.rect.x as f64 + (button.rect.width as f64 - text_size.width as f64) * 0.5,
                button.rect.y as f64 + (button.rect.height as f64 - text_size.height as f64) * 0.5,
                &label_style,
            )?;
        }

        Ok(())
    }

    fn help_button_rect(&self) -> Rect {
        let size = self.renderer.size();
        let margin = 10i32;
        Rect::new(
            size.width as i32 - Self::HELP_BUTTON_SIZE as i32 - margin,
            margin,
            Self::HELP_BUTTON_SIZE,
            Self::HELP_BUTTON_SIZE,
        )
    }

    fn help_modal_rect(&self) -> Rect {
        let size = self.renderer.size();
        let width = ((size.width as f64 * 0.82).clamp(520.0, 920.0)).round() as u32;
        let height = ((size.height as f64 * 0.82).clamp(420.0, 700.0)).round() as u32;
        Rect::new(
            ((size.width - width) / 2) as i32,
            ((size.height - height) / 2) as i32,
            width,
            height,
        )
    }

    fn help_close_rect(&self) -> Rect {
        let modal = self.help_modal_rect();
        let size = 26u32;
        Rect::new(modal.right() - size as i32 - 14, modal.y + 12, size, size)
    }

    fn draw_help_button(&mut self) -> Result<()> {
        let rect = self.help_button_rect();
        self.renderer.fill_rounded_rect(
            rect,
            8.0,
            self.theme.selection_background.with_alpha(0.9),
        )?;
        self.renderer
            .stroke_rounded_rect(rect, 8.0, self.theme.border.with_alpha(0.95), 1.0)?;

        let style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(16.0)
            .color(self.theme.selection_foreground);
        let label = "?";
        let text_size = self.renderer.measure_text(label, &style)?;
        let text_x = rect.x as f64 + (rect.width as f64 - text_size.width as f64) * 0.5;
        let text_y = rect.y as f64 + (rect.height as f64 - text_size.height as f64) * 0.5;
        self.renderer.text(label, text_x, text_y, &style)?;
        Ok(())
    }

    fn draw_help_modal(&mut self, mode: Mode) -> Result<()> {
        let size = self.renderer.size();
        let scrim = Rect::new(0, 0, size.width, size.height);
        self.renderer
            .fill_rect(scrim, Color::rgb(0.0, 0.0, 0.0).with_alpha(0.62))?;

        let modal = self.help_modal_rect();
        self.renderer
            .fill_rounded_rect(modal, 12.0, self.theme.background.lighten(0.08))?;
        self.renderer
            .stroke_rounded_rect(modal, 12.0, self.theme.border.with_alpha(0.95), 1.0)?;

        let close = self.help_close_rect();
        self.renderer.fill_rounded_rect(
            close,
            6.0,
            self.theme.item_hover_background.with_alpha(0.95),
        )?;
        self.renderer
            .stroke_rounded_rect(close, 6.0, self.theme.border.with_alpha(0.9), 1.0)?;

        let close_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(14.0)
            .color(self.theme.foreground);
        let close_size = self.renderer.measure_text("x", &close_style)?;
        self.renderer.text(
            "x",
            close.x as f64 + (close.width as f64 - close_size.width as f64) * 0.5,
            close.y as f64 + (close.height as f64 - close_size.height as f64) * 0.5,
            &close_style,
        )?;

        let title_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(17.0)
            .color(self.theme.foreground);
        self.renderer.text(
            "Quick Help",
            (modal.x + 20) as f64,
            (modal.y + 16) as f64,
            &title_style,
        )?;

        let sub_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(11.0)
            .color(self.theme.foreground.with_alpha(0.75));
        self.renderer.text(
            "Shortcuts, structured math input, and mode controls",
            (modal.x + 20) as f64,
            (modal.y + 36) as f64,
            &sub_style,
        )?;

        let heading_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(13.0)
            .color(self.theme.selection_foreground);
        let body_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(11.5)
            .color(self.theme.foreground.with_alpha(0.92));
        let hint_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(11.0)
            .color(self.theme.foreground.with_alpha(0.75));

        let content_top = modal.y + 56;
        let content_left = modal.x + 20;
        let content_width = modal.width as i32 - 40;
        let column_gap = 24;
        let column_width = (content_width - column_gap) / 2;
        let left_x = content_left;
        let right_x = content_left + column_width + column_gap;
        let divider_x = content_left + column_width + (column_gap / 2);
        let divider_rect = Rect::new(
            divider_x,
            content_top + 2,
            1,
            modal.height.saturating_sub(94),
        );
        self.renderer
            .fill_rect(divider_rect, self.theme.border.with_alpha(0.45))?;

        let mut left_y = content_top;
        let mut right_y = content_top;

        self.draw_help_section(
            left_x,
            &mut left_y,
            "Quick Start",
            &[
                "Enter evaluates the current expression.",
                "Esc clears input (or closes this help).",
                "Type '?' or click ? to toggle this panel.",
                "F1 calculator, F2 graph, F3 graph 3D.",
            ],
            &heading_style,
            &body_style,
        )?;
        left_y += 6;

        self.draw_help_section(
            left_x,
            &mut left_y,
            "Structured Templates",
            &[
                "Type '\\' or ':' then command + Space/Enter.",
                "\\frac \\sqrt \\nthroot \\abs \\matrix",
                "\\sum \\prod \\int \\dint \\lim \\diff",
                "\\solve \\sin \\cos \\tan \\ln \\exp",
                "Tab / Shift+Tab moves between template slots.",
            ],
            &heading_style,
            &body_style,
        )?;
        left_y += 6;

        self.draw_help_section(
            left_x,
            &mut left_y,
            "Editing And History",
            &[
                "Arrow keys move inside boxes and fractions.",
                "Backspace/Delete remove chars or empty boxes.",
                "Home/End jumps to start/end of input.",
                "Use '^', '_', '/', '!' for power/sub/frac/factorial.",
                "Up/Down on blank input recalls history.",
                "Ctrl+Up/Down forces history browsing.",
            ],
            &heading_style,
            &body_style,
        )?;

        self.draw_help_section(
            right_x,
            &mut right_y,
            "Calculator Examples",
            &[
                "2+2",
                "diff(x^2, x)",
                "integrate(sin(x), x)",
                "sum(k^2+k, k, 1, n)",
                "solve(x^2-4, x)",
            ],
            &heading_style,
            &body_style,
        )?;
        right_y += 6;

        self.draw_help_section(
            right_x,
            &mut right_y,
            "Graph Mode",
            &[
                "Enter y=... or r=... (polar) to add a curve.",
                "Ctrl+W: viewport panel, Ctrl+K: color picker.",
                "Ctrl+L: func list, Ctrl+Z: zeros, F4: table.",
                "Ctrl+R reset view, Ctrl+C clear, Ctrl+T trace.",
            ],
            &heading_style,
            &body_style,
        )?;
        right_y += 6;

        self.draw_help_section(
            right_x,
            &mut right_y,
            "3D Mode",
            &[
                "Enter z=... or expression to add a surface.",
                "sphere: expr | cyl: expr | level: expr = c",
                "Ctrl+M: render mode, Ctrl+A: auto-rotate.",
                "Ctrl+1/3/5/7: preset views, Ctrl+G: grid.",
                "Ctrl+S: settings panel, Ctrl+R: reset camera.",
            ],
            &heading_style,
            &body_style,
        )?;
        right_y += 6;

        self.draw_help_section(
            right_x,
            &mut right_y,
            "Behavior Notes",
            &[
                "Pretty output renders many symbolic forms directly.",
                "Definite integrals with numeric bounds evaluate numerically.",
                "Hard symbolic forms may remain as integrate(...).",
                "Click outside modal (or x) to close.",
            ],
            &heading_style,
            &body_style,
        )?;

        let mode_hint = match mode {
            Mode::Graph => "Active mode: Graph.",
            Mode::Graph3D => "Active mode: Graph 3D.",
            _ => "Active mode: Calculator.",
        };
        let hint_y = modal.bottom() - 24;
        self.renderer
            .text(mode_hint, (modal.x + 20) as f64, hint_y as f64, &hint_style)?;

        Ok(())
    }

    fn draw_help_section(
        &mut self,
        x: i32,
        y: &mut i32,
        heading: &str,
        lines: &[&str],
        heading_style: &TextStyle,
        body_style: &TextStyle,
    ) -> Result<()> {
        self.renderer
            .text(heading, x as f64, *y as f64, heading_style)?;
        *y += 20;
        for line in lines {
            self.renderer
                .text(&format!("- {line}"), x as f64, *y as f64, body_style)?;
            *y += 16;
        }
        Ok(())
    }

    fn draw_history(&mut self, history: &[HistoryEntry], start_y: i32, end_y: i32) -> Result<()> {
        let min_entry_height = 50;
        let max_entries = ((end_y - start_y) / min_entry_height).max(1) as usize;
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
        let start_idx = if history.len() > max_entries {
            history.len() - max_entries
        } else {
            0
        };

        let mut y = start_y + 12;
        for entry in history.iter().skip(start_idx) {
            if y > end_y {
                break;
            }

            let input_math = Self::parse_history_mathbox(&entry.input);
            let result_math = if entry.error.is_none() {
                Self::parse_history_mathbox(&entry.result)
            } else {
                None
            };

            // Input line (pretty if parseable)
            if let Some(mathbox) = input_math.as_ref() {
                let line_height = {
                    let ctx = self.renderer.surface().context()?;
                    let layout_engine = MathLayoutEngine::new(&self.theme.font_family, 14.0);
                    let layout = layout_engine.layout(mathbox, &ctx);
                    let baseline = y as f64 + layout.ascent + 2.0;
                    let prompt = MathBox::Number(">".to_string());
                    let prompt_layout = layout_engine.layout(&prompt, &ctx);
                    let input_gap = 8.0;

                    let mut math_renderer = MathRenderer::new(&ctx, &self.theme.font_family, 14.0);
                    math_renderer.fg_color = self.theme.foreground.with_alpha(0.85);
                    math_renderer.slot_bg_color = self.theme.input_background.lighten(0.15);
                    math_renderer.slot_focus_color = self.theme.selection_background;
                    math_renderer.cursor_color = self.theme.input_cursor;
                    math_renderer.render(&prompt, padding as f64, baseline);
                    math_renderer.render(
                        mathbox,
                        padding as f64 + prompt_layout.width + input_gap,
                        baseline,
                    );

                    (layout.height().ceil() as i32 + 6).max(20)
                };
                y += line_height;
            } else {
                self.renderer.text(
                    &format!("> {}", entry.input),
                    padding as f64,
                    y as f64,
                    &input_style,
                )?;
                y += 24;
            }

            // Result or error
            if let Some(ref error) = entry.error {
                self.renderer.text(
                    &format!("  Error: {error}"),
                    (padding + 10) as f64,
                    y as f64,
                    &error_style,
                )?;
                y += 24;
            } else if let Some(mathbox) = result_math.as_ref() {
                let line_height = {
                    let ctx = self.renderer.surface().context()?;
                    let layout_engine = MathLayoutEngine::new(&self.theme.font_family, 14.0);
                    let layout = layout_engine.layout(mathbox, &ctx);
                    let baseline = y as f64 + layout.ascent + 2.0;
                    let equals = MathBox::Number("=".to_string());
                    let equals_layout = layout_engine.layout(&equals, &ctx);
                    let result_gap = 12.0;
                    let equals_x = (padding + 10) as f64;

                    let mut math_renderer = MathRenderer::new(&ctx, &self.theme.font_family, 14.0);
                    math_renderer.fg_color = self.theme.selection_foreground;
                    math_renderer.slot_bg_color = self.theme.input_background.lighten(0.15);
                    math_renderer.slot_focus_color = self.theme.selection_background;
                    math_renderer.cursor_color = self.theme.input_cursor;
                    math_renderer.render(&equals, equals_x, baseline);
                    math_renderer.render(
                        mathbox,
                        equals_x + equals_layout.width + result_gap,
                        baseline,
                    );

                    (layout.height().ceil() as i32 + 6).max(20)
                };
                y += line_height;
            } else {
                self.renderer.text(
                    &format!("  = {}", entry.result),
                    (padding + 10) as f64,
                    y as f64,
                    &result_style,
                )?;
                y += 24;
            }
            y += 5;
        }

        Ok(())
    }

    fn parse_history_mathbox(text: &str) -> Option<MathBox> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        Self::parse_display_derivative(trimmed)
            .or_else(|| parser::parse(trimmed).ok())
            .map(|expr| from_expr(&expr))
    }

    fn parse_display_derivative(text: &str) -> Option<Expr> {
        let open = text.find('(')?;
        if !text.ends_with(')') || open + 1 >= text.len() {
            return None;
        }

        let prefix = &text[..open];
        if !(prefix.starts_with("d/d") || prefix.starts_with("d^")) {
            return None;
        }

        let body_text = &text[open + 1..text.len() - 1];
        let body_expr = parser::parse(body_text).ok()?;

        if let Some(var_name) = prefix.strip_prefix("d/d") {
            if var_name.is_empty() {
                return None;
            }
            return Some(Expr::Derivative {
                expr: Box::new(body_expr),
                var: Symbol::new(var_name),
                order: 1,
            });
        }

        let rest = prefix.strip_prefix("d^")?;
        let (order_text, den_part) = rest.split_once("/d")?;
        let order: u32 = order_text.parse().ok()?;
        if order == 0 {
            return None;
        }

        let suffix = format!("^{order}");
        let var_name = den_part.strip_suffix(&suffix)?;
        if var_name.is_empty() {
            return None;
        }

        Some(Expr::Derivative {
            expr: Box::new(body_expr),
            var: Symbol::new(var_name),
            order,
        })
    }

    fn draw_text_input(
        &mut self,
        input: &str,
        cursor: usize,
        show_cursor: bool,
        y: i32,
    ) -> Result<()> {
        let size = self.renderer.size();
        let padding = 15;
        let input_width = size.width - 2 * padding;
        let input_height = 40u32;

        // Input background
        let input_rect = Rect::new(padding as i32, y, input_width, input_height);
        self.renderer
            .fill_rounded_rect(input_rect, 6.0, self.theme.input_background)?;

        // Input text
        let text_x = padding + 10;
        let text_y = y + 8;

        let input_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(16.0)
            .color(self.theme.foreground);

        self.renderer
            .text(input, text_x as f64, text_y as f64, &input_style)?;

        // Cursor
        let cursor_text = if cursor < input.len() {
            &input[..cursor]
        } else {
            input
        };
        let cursor_size = self.renderer.measure_text(cursor_text, &input_style)?;
        let cursor_x = text_x as f64 + cursor_size.width as f64;

        // Draw cursor line
        if show_cursor {
            let cursor_rect = Rect::new(cursor_x as i32, y + 8, 2, input_height - 16);
            self.renderer
                .fill_rect(cursor_rect, self.theme.input_cursor)?;
        }

        Ok(())
    }

    fn draw_math_input(
        &mut self,
        math_input: &MathInput,
        cursor_visible: bool,
        y: i32,
        input_height: u32,
    ) -> Result<()> {
        let size = self.renderer.size();
        let padding = 15;
        let input_width = size.width - 2 * padding;

        // Input background
        let input_rect = Rect::new(padding as i32, y, input_width, input_height);
        self.renderer
            .fill_rounded_rect(input_rect, 6.0, self.theme.input_background)?;

        // Draw structured math content
        {
            let ctx = self.renderer.surface().context()?;
            let mut math_renderer = MathRenderer::new(&ctx, &self.theme.font_family, 18.0);
            math_renderer.fg_color = self.theme.foreground;
            math_renderer.slot_bg_color = self.theme.input_background.lighten(0.15);
            math_renderer.slot_focus_color = self.theme.selection_background;
            math_renderer.cursor_color = self.theme.input_cursor;
            math_renderer.render_with_cursor(
                math_input.mathbox(),
                (padding + 10) as f64,
                y as f64 + input_height as f64 * 0.62,
                math_input.cursor_path(),
                math_input.cursor_offset(),
                cursor_visible,
            );
        }

        // Show command buffer in main input area and corner while command mode is active.
        if let Some(cmd) = math_input.command_buffer.as_deref() {
            let inline_cmd = format!("\\{}", cmd);
            let inline_style = TextStyle::new()
                .font_family(&self.theme.font_family)
                .font_size(14.0)
                .color(self.theme.foreground.with_alpha(0.85));
            self.renderer.text(
                &inline_cmd,
                (padding + 12) as f64,
                y as f64 + 20.0,
                &inline_style,
            )?;

            let cmd_text = format!("\\{}  [Enter/Space]", cmd);
            let cmd_style = TextStyle::new()
                .font_family(&self.theme.font_family)
                .font_size(11.0)
                .color(self.theme.foreground.with_alpha(0.7));
            let cmd_size = self.renderer.measure_text(&cmd_text, &cmd_style)?;
            let cmd_x = (padding as f64 + input_width as f64 - cmd_size.width as f64 - 10.0)
                .max((padding + 10) as f64);
            let cmd_y = y as f64 + input_height as f64 - 10.0;
            self.renderer.text(&cmd_text, cmd_x, cmd_y, &cmd_style)?;
        }

        Ok(())
    }

    fn draw_function_list(&mut self, graph: &Graph2D) -> Result<()> {
        let size = self.renderer.size();
        let panel_w = 200u32;
        let row_h = 22i32;
        let count = graph.functions.len();
        let panel_h = (count as u32 * row_h as u32 + 10).max(30);
        let px = size.width as i32 - panel_w as i32 - 10;
        let py = 36;

        // Background
        let bg = Rect::new(px, py, panel_w, panel_h);
        self.renderer
            .fill_rounded_rect(bg, 6.0, self.theme.background.with_alpha(0.88))?;

        let label_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(11.0)
            .color(self.theme.foreground);

        let dim_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(11.0)
            .color(self.theme.foreground.with_alpha(0.4));

        for (i, pf) in graph.functions.iter().enumerate() {
            let y = py + 6 + (i as i32 * row_h);
            let graph_color = pf.func.color().unwrap_or(garcalc_graph::Color::BLACK);
            let swatch_color =
                Color::from_u8(graph_color.r, graph_color.g, graph_color.b, graph_color.a);
            let swatch = Rect::new(px + 8, y + 2, 12, 12);
            self.renderer.fill_rect(swatch, swatch_color)?;

            let style = if pf.visible { &label_style } else { &dim_style };
            let prefix = format!("{}. ", i + 1);
            let label = format!("{}{}", prefix, pf.label);
            self.renderer
                .text(&label, (px + 26) as f64, y as f64, style)?;
        }

        Ok(())
    }

    fn draw_table_view(
        &mut self,
        table_data: &[(f64, Option<f64>)],
        scroll_offset: usize,
        step: f64,
    ) -> Result<()> {
        let size = self.renderer.size();
        let panel_w = 180u32;
        let row_h = 18i32;
        let max_rows = 20usize;
        let header_h = 24i32;
        let panel_h = header_h as u32 + (max_rows as u32 * row_h as u32) + 8;
        let px = size.width as i32 - panel_w as i32 - 10;
        let py = 36;

        // Background
        let bg = Rect::new(px, py, panel_w, panel_h);
        self.renderer
            .fill_rounded_rect(bg, 6.0, self.theme.background.with_alpha(0.92))?;

        // Header
        let header_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(11.0)
            .color(self.theme.selection_foreground);
        self.renderer.text(
            &format!("x (step={:.3})", step),
            (px + 8) as f64,
            (py + 4) as f64,
            &header_style,
        )?;
        self.renderer.text(
            "f(x)",
            (px + panel_w as i32 / 2 + 8) as f64,
            (py + 4) as f64,
            &header_style,
        )?;

        // Divider
        let div = Rect::new(px + 4, py + header_h - 2, panel_w - 8, 1);
        self.renderer
            .fill_rect(div, self.theme.border.with_alpha(0.5))?;

        // Rows
        let val_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(10.0)
            .color(self.theme.foreground.with_alpha(0.9));
        let undef_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(10.0)
            .color(self.theme.foreground.with_alpha(0.4));

        let visible_rows = table_data.iter().skip(scroll_offset).take(max_rows);
        for (i, (x, y)) in visible_rows.enumerate() {
            let ry = py + header_h + (i as i32 * row_h);
            self.renderer.text(
                &format!("{:.4}", x),
                (px + 8) as f64,
                ry as f64,
                &val_style,
            )?;
            match y {
                Some(yv) => {
                    self.renderer.text(
                        &format!("{:.4}", yv),
                        (px + panel_w as i32 / 2 + 8) as f64,
                        ry as f64,
                        &val_style,
                    )?;
                }
                None => {
                    self.renderer.text(
                        "undef",
                        (px + panel_w as i32 / 2 + 8) as f64,
                        ry as f64,
                        &undef_style,
                    )?;
                }
            }
        }

        // Scroll indicators
        let hint_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(9.0)
            .color(self.theme.foreground.with_alpha(0.5));
        let bottom_y = py as f64 + panel_h as f64 - 14.0;
        self.renderer.text(
            "Up/Dn scroll | [/] step | Ctrl+E export",
            (px + 6) as f64,
            bottom_y,
            &hint_style,
        )?;

        Ok(())
    }

    // --- Viewport settings panel (#16) ---

    fn viewport_panel_rect(&self) -> Rect {
        Rect::new(10, 40, 200, 160)
    }

    fn draw_viewport_panel(
        &mut self,
        graph: &Graph2D,
        edit_field: usize,
        edit_buffer: &str,
    ) -> Result<()> {
        let panel = self.viewport_panel_rect();
        self.renderer
            .fill_rounded_rect(panel, 8.0, self.theme.background.with_alpha(0.92))?;
        self.renderer
            .stroke_rounded_rect(panel, 8.0, self.theme.border.with_alpha(0.8), 1.0)?;

        let heading_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(11.0)
            .color(self.theme.selection_foreground);
        let val_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(11.0)
            .color(self.theme.foreground.with_alpha(0.9));
        let active_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(11.0)
            .color(self.theme.selection_foreground);

        self.renderer.text(
            "Viewport Settings",
            (panel.x + 10) as f64,
            (panel.y + 6) as f64,
            &heading_style,
        )?;

        let labels = ["X min:", "X max:", "Y min:", "Y max:"];
        let values = [
            graph.viewport.x_min,
            graph.viewport.x_max,
            graph.viewport.y_min,
            graph.viewport.y_max,
        ];

        for (i, (label, val)) in labels.iter().zip(values.iter()).enumerate() {
            let y = panel.y + 26 + i as i32 * 22;
            self.renderer
                .text(label, (panel.x + 10) as f64, y as f64, &val_style)?;

            if i == edit_field {
                let field_rect = Rect::new(panel.x + 70, y - 2, 118, 18);
                self.renderer.fill_rounded_rect(
                    field_rect,
                    3.0,
                    self.theme.selection_background.with_alpha(0.4),
                )?;
                self.renderer.text(
                    edit_buffer,
                    (panel.x + 74) as f64,
                    y as f64,
                    &active_style,
                )?;
            } else {
                self.renderer.text(
                    &format!("{:.4}", val),
                    (panel.x + 74) as f64,
                    y as f64,
                    &val_style,
                )?;
            }
        }

        // Preset buttons
        let btn_y = panel.y + 118;
        let btn_labels = ["Standard", "Trig", "Fit"];
        let btn_w = 56i32;
        let btn_gap = 6i32;
        for (i, label) in btn_labels.iter().enumerate() {
            let bx = panel.x + 10 + i as i32 * (btn_w + btn_gap);
            let btn = Rect::new(bx, btn_y, btn_w as u32, 22);
            self.renderer.fill_rounded_rect(
                btn,
                4.0,
                self.theme.item_hover_background.with_alpha(0.85),
            )?;
            self.renderer.stroke_rounded_rect(
                btn,
                4.0,
                self.theme.border.with_alpha(0.7),
                1.0,
            )?;
            let text_size = self.renderer.measure_text(label, &val_style)?;
            self.renderer.text(
                label,
                bx as f64 + (btn_w as f64 - text_size.width as f64) * 0.5,
                btn_y as f64 + 3.0,
                &val_style,
            )?;
        }

        // Hint
        let hint_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(9.0)
            .color(self.theme.foreground.with_alpha(0.5));
        self.renderer.text(
            "Tab: next | Enter: apply | Esc: close",
            (panel.x + 10) as f64,
            (panel.y + 146) as f64,
            &hint_style,
        )?;

        Ok(())
    }

    pub fn viewport_preset_hit(&self, x: f64, y: f64) -> Option<ViewportPreset> {
        let panel = self.viewport_panel_rect();
        let btn_y = panel.y + 118;
        let btn_w = 56i32;
        let btn_gap = 6i32;
        let presets = [ViewportPreset::Standard, ViewportPreset::Trig, ViewportPreset::ZoomFit];

        for (i, preset) in presets.iter().enumerate() {
            let bx = panel.x + 10 + i as i32 * (btn_w + btn_gap);
            let btn = Rect::new(bx, btn_y, btn_w as u32, 22);
            if btn.contains_point(Point::new(x as i32, y as i32)) {
                return Some(*preset);
            }
        }
        None
    }

    // --- Color picker (#17) ---

    fn color_picker_rect(&self) -> Rect {
        let size = self.renderer.size();
        // Position near top-right, below function list area
        let px = size.width as i32 - 130;
        Rect::new(px, 40, 120, 90)
    }

    fn draw_color_picker(
        &mut self,
        graph: &Graph2D,
        func_index: usize,
    ) -> Result<()> {
        let panel = self.color_picker_rect();
        self.renderer
            .fill_rounded_rect(panel, 8.0, self.theme.background.with_alpha(0.92))?;
        self.renderer
            .stroke_rounded_rect(panel, 8.0, self.theme.border.with_alpha(0.8), 1.0)?;

        let heading_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(10.0)
            .color(self.theme.foreground.with_alpha(0.8));
        self.renderer.text(
            &format!("Color: func {}", func_index + 1),
            (panel.x + 8) as f64,
            (panel.y + 4) as f64,
            &heading_style,
        )?;

        let current_color = graph
            .functions
            .get(func_index)
            .and_then(|pf| pf.func.color());

        let swatch_size = 20i32;
        let gap = 4i32;
        let cols = 4;
        for (i, palette_color) in COLOR_PALETTE.iter().enumerate() {
            let col = (i % cols) as i32;
            let row = (i / cols) as i32;
            let sx = panel.x + 10 + col * (swatch_size + gap);
            let sy = panel.y + 22 + row * (swatch_size + gap);
            let swatch = Rect::new(sx, sy, swatch_size as u32, swatch_size as u32);

            let c = Color::from_u8(palette_color.r, palette_color.g, palette_color.b, palette_color.a);
            self.renderer.fill_rounded_rect(swatch, 3.0, c)?;

            // White border on current color
            if let Some(cur) = current_color {
                if cur.r == palette_color.r && cur.g == palette_color.g && cur.b == palette_color.b {
                    self.renderer.stroke_rounded_rect(
                        swatch,
                        3.0,
                        Color::rgb(1.0, 1.0, 1.0),
                        2.0,
                    )?;
                }
            }
        }

        Ok(())
    }

    pub fn color_picker_hit(&self, x: f64, y: f64) -> Option<usize> {
        let panel = self.color_picker_rect();
        let swatch_size = 20i32;
        let gap = 4i32;
        let cols = 4;

        for i in 0..COLOR_PALETTE.len() {
            let col = (i % cols) as i32;
            let row = (i / cols) as i32;
            let sx = panel.x + 10 + col * (swatch_size + gap);
            let sy = panel.y + 22 + row * (swatch_size + gap);
            let swatch = Rect::new(sx, sy, swatch_size as u32, swatch_size as u32);
            if swatch.contains_point(Point::new(x as i32, y as i32)) {
                return Some(i);
            }
        }
        None
    }

    // --- 3D settings panel (#23) ---

    fn draw_3d_settings_panel(
        &mut self,
        graph3d: &Graph3D,
        auto_rotate: bool,
    ) -> Result<()> {
        let size = self.renderer.size();
        let panel_w = 220u32;
        let panel_h = 240u32;
        let px = size.width as i32 - panel_w as i32 - 10;
        let py = 36;
        let panel = Rect::new(px, py, panel_w, panel_h);

        self.renderer
            .fill_rounded_rect(panel, 8.0, self.theme.background.with_alpha(0.92))?;
        self.renderer
            .stroke_rounded_rect(panel, 8.0, self.theme.border.with_alpha(0.8), 1.0)?;

        let heading_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(11.0)
            .color(self.theme.selection_foreground);
        let val_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(11.0)
            .color(self.theme.foreground.with_alpha(0.9));
        let label_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(10.0)
            .color(self.theme.foreground.with_alpha(0.7));

        self.renderer.text(
            "3D Settings",
            (px + 10) as f64,
            (py + 6) as f64,
            &heading_style,
        )?;

        let mut y = py + 28;
        let lx = (px + 10) as f64;
        let vx = (px + 90) as f64;

        // Domain
        self.renderer.text("Domain:", lx, y as f64, &label_style)?;
        y += 16;
        self.renderer.text(
            &format!("X: [{:.1}, {:.1}]", graph3d.viewport.x_min, graph3d.viewport.x_max),
            lx + 8.0,
            y as f64,
            &val_style,
        )?;
        y += 16;
        self.renderer.text(
            &format!("Y: [{:.1}, {:.1}]", graph3d.viewport.y_min, graph3d.viewport.y_max),
            lx + 8.0,
            y as f64,
            &val_style,
        )?;
        y += 16;
        self.renderer.text(
            &format!("Z: [{:.1}, {:.1}]", graph3d.viewport.z_min, graph3d.viewport.z_max),
            lx + 8.0,
            y as f64,
            &val_style,
        )?;
        y += 20;

        // Grid
        self.renderer.text("Grid:", lx, y as f64, &label_style)?;
        self.renderer.text(
            &format!("{}", graph3d.config.grid_lines),
            vx,
            y as f64,
            &val_style,
        )?;
        y += 18;

        // Colormap
        self.renderer.text("Colormap:", lx, y as f64, &label_style)?;
        self.renderer.text(
            graph3d.config.colormap.name(),
            vx,
            y as f64,
            &val_style,
        )?;
        y += 18;

        // Render mode
        self.renderer.text("Render:", lx, y as f64, &label_style)?;
        self.renderer.text(
            graph3d.config.render_mode.name(),
            vx,
            y as f64,
            &val_style,
        )?;
        y += 18;

        // Alpha
        self.renderer.text("Alpha:", lx, y as f64, &label_style)?;
        self.renderer.text(
            &format!("{:.2}", graph3d.config.surface_alpha),
            vx,
            y as f64,
            &val_style,
        )?;
        y += 18;

        // View angles
        let az_deg = graph3d.camera.azimuth.to_degrees();
        let el_deg = graph3d.camera.elevation.to_degrees();
        self.renderer.text("View:", lx, y as f64, &label_style)?;
        self.renderer.text(
            &format!("az={:.0} el={:.0}", az_deg, el_deg),
            vx,
            y as f64,
            &val_style,
        )?;
        y += 18;

        // Auto-rotate status
        let rotate_text = if auto_rotate { "ON" } else { "OFF" };
        self.renderer.text("Rotate:", lx, y as f64, &label_style)?;
        self.renderer.text(rotate_text, vx, y as f64, &val_style)?;
        y += 16;

        // Hints
        let hint_style = TextStyle::new()
            .font_family(&self.theme.font_family)
            .font_size(9.0)
            .color(self.theme.foreground.with_alpha(0.5));
        self.renderer.text(
            "[/]: grid | </>: alpha | c: cmap | m: mode",
            (px + 6) as f64,
            y as f64,
            &hint_style,
        )?;

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

#[cfg(test)]
mod tests {
    use super::CalculatorUI;
    use garcalc_cas::expr::Expr;

    #[test]
    fn parse_display_derivative_first_order() {
        let parsed = CalculatorUI::parse_display_derivative("d/dx(e^x)").unwrap();
        assert!(matches!(parsed, Expr::Derivative { order: 1, .. }));
    }

    #[test]
    fn parse_display_derivative_higher_order() {
        let parsed = CalculatorUI::parse_display_derivative("d^2/dx^2(sin(x))").unwrap();
        assert!(matches!(parsed, Expr::Derivative { order: 2, .. }));
    }

    #[test]
    fn parse_history_mathbox_prefers_display_derivative_form() {
        let parsed = CalculatorUI::parse_history_mathbox("d/dx(e^x)").unwrap();
        assert!(matches!(parsed, garcalc_math::MathBox::Derivative { .. }));
    }
}
