//! Application state and event loop

use anyhow::Result;
use garcalc_cas::{Evaluator, parser};
use garcalc_graph::{CameraPreset, Graph2D, Graph3D, COLOR_PALETTE};
use garcalc_ipc::Mode;
use garcalc_math::input::SpecialKey;
use garcalc_math::{
    ConvertError, InputResult as MathInputResult, MathBox, MathInput, from_expr, to_expr,
};
use gartk_core::{InputEvent, Key, Modifiers, MouseButton};
use gartk_x11::{Connection, EventLoop, EventLoopConfig, Window, WindowConfig};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::ui::{CalcButtonAction, CalculatorUI};

/// Calculator entry (input + result)
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub input: String,
    pub result: String,
    pub error: Option<String>,
}

/// Application state
pub struct App {
    /// UI renderer
    ui: CalculatorUI,
    /// CAS evaluator
    evaluator: Evaluator,
    /// 2D graph state
    graph: Graph2D,
    /// 3D graph state
    graph3d: Graph3D,
    /// Current mode
    mode: Mode,
    /// Plain text input (used in graph modes)
    input: String,
    /// Cursor position
    cursor: usize,
    /// Structured input (used in calculator mode)
    math_input: MathInput,
    /// Structured expression history for calculator input recall
    calc_history: Vec<MathBox>,
    /// History navigation index in calculator mode
    calc_history_index: Option<usize>,
    /// Calculation history
    history: Vec<HistoryEntry>,
    /// History navigation index
    history_index: Option<usize>,
    /// Whether running in popup mode
    popup_mode: bool,
    /// Whether app should quit
    should_quit: bool,
    /// Whether the help modal overlay is open
    help_modal_open: bool,
    /// Whether calculator buttons are in extended mode
    calc_buttons_extended: bool,
    /// Whether the cursor is currently visible (blink state)
    cursor_visible: bool,
    /// Last time the cursor blink state toggled
    last_cursor_blink: Instant,
    /// Whether we have focus
    has_focus: bool,
    /// Mouse drag state for graph panning
    drag_start: Option<(f64, f64)>,
    /// Whether to show the function list panel
    show_function_list: bool,
    /// Whether zeros markers are visible
    zeros_visible: bool,
    /// Cached zero points: (func_index, points)
    cached_zeros: Vec<(usize, Vec<(f64, f64)>)>,
    /// Cached intersection points: ((idx_i, idx_j), points)
    cached_intersections: Vec<((usize, usize), Vec<(f64, f64)>)>,
    /// Whether table view is visible
    table_visible: bool,
    /// Which function index to tabulate
    table_func_index: usize,
    /// Table scroll offset
    table_scroll_offset: usize,
    /// Table step size
    table_step: f64,
    /// Cached table data
    table_data: Vec<(f64, Option<f64>)>,
    /// Whether auto-rotate is enabled (3D mode)
    auto_rotate: bool,
    /// Auto-rotate speed (radians per tick)
    auto_rotate_speed: f64,
    /// Whether the viewport settings panel is open (2D graph)
    viewport_panel_open: bool,
    /// Which viewport field is being edited (0=xmin, 1=xmax, 2=ymin, 3=ymax)
    viewport_edit_field: usize,
    /// Buffer for editing viewport values
    viewport_edit_buffer: String,
    /// Whether the color picker popup is open
    color_picker_open: bool,
    /// Which function the color picker targets
    color_picker_func_index: usize,
    /// Whether the 3D settings panel is open
    settings3d_panel_open: bool,
    /// Configuration
    #[allow(dead_code)]
    config: Config,
}

impl App {
    pub fn new(mode: Mode, popup: bool) -> Result<Self> {
        let config = Config::load().unwrap_or_default();

        // Connect to X11
        let conn = Connection::connect(None)?;

        // Window dimensions
        let (width, height) = if popup {
            (config.popup.width, config.popup.height)
        } else {
            (900, 600)
        };

        // Position
        let (x, y) = if popup {
            let monitor = gartk_x11::monitor_at_pointer(&conn)?;
            let x = monitor.rect.x + (monitor.rect.width as i32 - width as i32) / 2;
            let y = monitor.rect.y + (monitor.rect.height as i32 - height as i32) / 3;
            (x, y)
        } else {
            (100, 100)
        };

        // Create window
        let window_config = if popup {
            WindowConfig::popup()
                .title("garcalc")
                .class("garcalc")
                .position(x, y)
                .size(width, height)
                .transparent(true)
        } else {
            WindowConfig::new()
                .title("garcalc")
                .class("garcalc")
                .position(x, y)
                .size(width, height)
        };

        let window = Window::create(conn.clone(), window_config)?;
        window.focus()?;

        let ui = CalculatorUI::new(window, width, height)?;

        Ok(Self {
            ui,
            evaluator: Evaluator::new(),
            graph: Graph2D::new(),
            graph3d: Graph3D::new(),
            mode,
            input: String::new(),
            cursor: 0,
            math_input: MathInput::new(),
            calc_history: Vec::new(),
            calc_history_index: None,
            history: Vec::new(),
            history_index: None,
            popup_mode: popup,
            should_quit: false,
            help_modal_open: false,
            calc_buttons_extended: false,
            cursor_visible: true,
            last_cursor_blink: Instant::now(),
            has_focus: false,
            drag_start: None,
            show_function_list: false,
            zeros_visible: false,
            cached_zeros: Vec::new(),
            cached_intersections: Vec::new(),
            table_visible: false,
            table_func_index: 0,
            table_scroll_offset: 0,
            table_step: 1.0,
            table_data: Vec::new(),
            auto_rotate: false,
            auto_rotate_speed: 0.02,
            viewport_panel_open: false,
            viewport_edit_field: 0,
            viewport_edit_buffer: String::new(),
            color_picker_open: false,
            color_picker_func_index: 0,
            settings3d_panel_open: false,
            config,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        let window = self.ui.window();
        let mut event_loop = EventLoop::new(window, EventLoopConfig::default())?;

        // Initial render
        self.render()?;

        event_loop.run(|ev, event| {
            match event {
                InputEvent::Key(key_event) if key_event.pressed => {
                    self.handle_key(&key_event.key, key_event.modifiers);
                    self.reset_cursor_blink();
                    ev.request_redraw();
                }
                InputEvent::MousePress(mouse_ev) => {
                    let x = mouse_ev.position.x as f64;
                    let y = mouse_ev.position.y as f64;

                    if mouse_ev.button == Some(MouseButton::Left)
                        && self.ui.is_help_button_hit(x, y)
                    {
                        self.help_modal_open = !self.help_modal_open;
                        self.drag_start = None;
                        ev.request_redraw();
                    } else if self.help_modal_open {
                        if mouse_ev.button == Some(MouseButton::Left)
                            && (self.ui.is_help_close_hit(x, y) || !self.ui.is_help_modal_hit(x, y))
                        {
                            self.help_modal_open = false;
                            ev.request_redraw();
                        }
                    } else if self.mode == Mode::Calculator {
                        if mouse_ev.button == Some(MouseButton::Left) {
                            if let Some(action) = self.ui.calculator_button_action_at(
                                x,
                                y,
                                self.calc_buttons_extended,
                            ) {
                                self.handle_calculator_button_action(action);
                                self.reset_cursor_blink();
                                ev.request_redraw();
                            }
                        }
                    } else if self.mode == Mode::Graph {
                        if mouse_ev.button == Some(MouseButton::Left) {
                            // Check viewport panel preset clicks
                            if self.viewport_panel_open {
                                if let Some(preset) = self.ui.viewport_preset_hit(x, y) {
                                    self.graph.apply_viewport_preset(preset);
                                    self.viewport_panel_open = false;
                                    self.invalidate_caches();
                                    ev.request_redraw();
                                    // Don't start drag
                                } else {
                                    self.drag_start = Some((x, y));
                                }
                            } else if self.color_picker_open {
                                if let Some(palette_idx) = self.ui.color_picker_hit(x, y) {
                                    self.graph.set_function_color(self.color_picker_func_index, COLOR_PALETTE[palette_idx]);
                                    self.color_picker_open = false;
                                    ev.request_redraw();
                                } else {
                                    self.color_picker_open = false;
                                    self.drag_start = Some((x, y));
                                }
                            } else {
                                // Left click - start drag for pan
                                self.drag_start = Some((x, y));
                            }
                        } else if mouse_ev.button == Some(MouseButton::Right) {
                            // Right click - toggle trace mode
                            self.graph.trace_enabled = !self.graph.trace_enabled;
                            if self.graph.trace_enabled {
                                self.graph.set_trace_pos(x, y);
                            }
                            ev.request_redraw();
                        }
                    } else if self.mode == Mode::Graph3D {
                        if mouse_ev.button == Some(MouseButton::Left) {
                            // Left click - start drag for rotation
                            self.drag_start = Some((x, y));
                        }
                    }
                }
                InputEvent::MouseRelease(mouse_ev) => {
                    if self.help_modal_open {
                        self.drag_start = None;
                    } else if (self.mode == Mode::Graph || self.mode == Mode::Graph3D)
                        && mouse_ev.button == Some(MouseButton::Left)
                    {
                        self.drag_start = None;
                    }
                }
                InputEvent::MouseMove(mouse_ev) => {
                    let x = mouse_ev.position.x as f64;
                    let y = mouse_ev.position.y as f64;
                    if self.help_modal_open {
                        self.drag_start = None;
                    } else if self.mode == Mode::Graph {
                        if let Some((start_x, start_y)) = self.drag_start {
                            let (width, height) = self.ui.size();
                            let dx = x - start_x;
                            let dy = y - start_y;
                            self.graph.pan(dx, dy, width, height);
                            self.drag_start = Some((x, y));
                            ev.request_redraw();
                        } else if self.graph.trace_enabled {
                            self.graph.set_trace_pos(x, y);
                            ev.request_redraw();
                        }
                    } else if self.mode == Mode::Graph3D {
                        if let Some((start_x, start_y)) = self.drag_start {
                            let dx = x - start_x;
                            let dy = y - start_y;
                            // Rotate camera: horizontal drag = azimuth, vertical = elevation
                            self.graph3d.camera.rotate(dx * 0.01, -dy * 0.01);
                            self.drag_start = Some((x, y));
                            ev.request_redraw();
                        }
                    }
                }
                InputEvent::Scroll(scroll_ev) => {
                    if self.help_modal_open {
                        // Disable background interactions while modal is visible.
                    } else if self.mode == Mode::Graph {
                        let factor = if scroll_ev.delta_y > 0 { 1.1 } else { 0.9 };
                        let (width, height) = self.ui.size();
                        let x = scroll_ev.position.x as f64;
                        let y = scroll_ev.position.y as f64;
                        self.graph.zoom(factor, x, y, width, height);
                        ev.request_redraw();
                    } else if self.mode == Mode::Graph3D {
                        // More aggressive zoom for 3D (25% per scroll)
                        let factor = if scroll_ev.delta_y > 0 { 1.25 } else { 0.8 };
                        self.graph3d.camera.zoom(factor);
                        ev.request_redraw();
                    }
                }
                InputEvent::Resize { width, height } => {
                    let _ = self.ui.resize(width, height);
                    ev.request_redraw();
                }
                InputEvent::Expose => {
                    ev.request_redraw();
                }
                InputEvent::CloseRequested => {
                    self.should_quit = true;
                }
                InputEvent::FocusIn => {
                    self.has_focus = true;
                    self.reset_cursor_blink();
                }
                InputEvent::FocusOut => {
                    if self.has_focus && self.popup_mode {
                        self.should_quit = true;
                    }
                }
                InputEvent::Idle => {
                    if self.tick_cursor_blink() {
                        ev.request_redraw();
                    }
                    if self.auto_rotate && self.mode == Mode::Graph3D {
                        self.graph3d.camera.rotate(self.auto_rotate_speed, 0.0);
                        ev.request_redraw();
                    }
                }
                _ => {}
            }

            if ev.needs_redraw() {
                let _ = self.render();
                ev.redraw_done();
            }

            Ok(!self.should_quit)
        })?;

        Ok(())
    }

    fn handle_key(&mut self, key: &Key, modifiers: Modifiers) {
        let ctrl = modifiers.ctrl;

        if !modifiers.ctrl && !modifiers.alt && !modifiers.super_key && *key == Key::Char('?') {
            self.help_modal_open = !self.help_modal_open;
            return;
        }

        if self.help_modal_open {
            if matches!(key, Key::Escape | Key::Return) {
                self.help_modal_open = false;
            }
            return;
        }

        // Global keys
        match key {
            Key::Escape => {
                if self.popup_mode {
                    self.should_quit = true;
                } else if self.mode == Mode::Calculator {
                    if matches!(
                        self.math_input.handle_key(SpecialKey::Escape),
                        MathInputResult::Cancel
                    ) {
                        self.math_input.clear();
                    }
                    self.calc_history_index = None;
                } else {
                    self.input.clear();
                    self.cursor = 0;
                }
                return;
            }
            Key::Return => {
                if self.mode == Mode::Calculator {
                    let result = self.math_input.handle_key(SpecialKey::Enter);
                    match result {
                        MathInputResult::Evaluate => self.evaluate(),
                        MathInputResult::Consumed => {
                            self.history_index = None;
                            self.calc_history_index = None;
                        }
                        _ => {}
                    }
                } else {
                    self.evaluate();
                }
                return;
            }
            Key::F1 => {
                self.mode = Mode::Calculator;
                return;
            }
            Key::F2 => {
                self.mode = Mode::Graph;
                return;
            }
            Key::F3 => {
                self.mode = Mode::Graph3D;
                return;
            }
            Key::F4 if self.mode == Mode::Graph => {
                self.table_visible = !self.table_visible;
                if self.table_visible {
                    self.regenerate_table();
                }
                return;
            }
            _ => {}
        }

        // Graph shortcuts
        match key {
            Key::Char('r') if ctrl && self.mode == Mode::Graph => {
                self.graph.reset_viewport();
                self.invalidate_caches();
                return;
            }
            Key::Char('c') if ctrl && self.mode == Mode::Graph => {
                self.graph.clear_functions();
                self.invalidate_caches();
                return;
            }
            Key::Char('t') if ctrl && self.mode == Mode::Graph => {
                self.graph.trace_enabled = !self.graph.trace_enabled;
                return;
            }
            Key::Char('l') if ctrl && self.mode == Mode::Graph => {
                self.show_function_list = !self.show_function_list;
                return;
            }
            Key::Char(c @ '1'..='9') if ctrl && self.mode == Mode::Graph => {
                let idx = (*c as usize) - ('1' as usize);
                self.graph.toggle_visibility(idx);
                self.invalidate_caches();
                return;
            }
            Key::Char('z') if ctrl && self.mode == Mode::Graph => {
                self.zeros_visible = !self.zeros_visible;
                if self.zeros_visible {
                    self.cached_zeros = self.graph.find_zeros();
                    self.cached_intersections = self.graph.find_intersections();
                }
                return;
            }
            // 2D graph: viewport panel
            Key::Char('w') if ctrl && self.mode == Mode::Graph => {
                self.viewport_panel_open = !self.viewport_panel_open;
                if self.viewport_panel_open {
                    self.viewport_edit_field = 0;
                    self.viewport_edit_buffer = format!("{:.4}", self.graph.viewport.x_min);
                }
                return;
            }
            // 2D graph: color picker
            Key::Char('k') if ctrl && self.mode == Mode::Graph => {
                if !self.graph.functions.is_empty() {
                    self.color_picker_open = !self.color_picker_open;
                    self.color_picker_func_index =
                        self.color_picker_func_index.min(self.graph.functions.len().saturating_sub(1));
                }
                return;
            }
            // 3D shortcuts
            Key::Char('r') if ctrl && self.mode == Mode::Graph3D => {
                self.graph3d.reset_camera();
                return;
            }
            Key::Char('c') if ctrl && self.mode == Mode::Graph3D => {
                self.graph3d.clear_surfaces();
                return;
            }
            Key::Char('m') if ctrl && self.mode == Mode::Graph3D => {
                self.graph3d.cycle_render_mode();
                return;
            }
            Key::Char('a') if ctrl && self.mode == Mode::Graph3D => {
                self.auto_rotate = !self.auto_rotate;
                return;
            }
            Key::Char('g') if ctrl && self.mode == Mode::Graph3D => {
                self.graph3d.config.show_coord_planes = !self.graph3d.config.show_coord_planes;
                return;
            }
            Key::Char('s') if ctrl && self.mode == Mode::Graph3D => {
                self.settings3d_panel_open = !self.settings3d_panel_open;
                return;
            }
            // 3D camera presets
            Key::Char('1') if ctrl && self.mode == Mode::Graph3D => {
                self.graph3d.camera.apply_preset(CameraPreset::Front);
                return;
            }
            Key::Char('3') if ctrl && self.mode == Mode::Graph3D => {
                self.graph3d.camera.apply_preset(CameraPreset::Side);
                return;
            }
            Key::Char('7') if ctrl && self.mode == Mode::Graph3D => {
                self.graph3d.camera.apply_preset(CameraPreset::Top);
                return;
            }
            Key::Char('5') if ctrl && self.mode == Mode::Graph3D => {
                self.graph3d.camera.apply_preset(CameraPreset::Isometric);
                return;
            }
            _ => {}
        }

        if self.mode == Mode::Calculator {
            self.handle_calculator_input(key, modifiers);
            return;
        }

        // Viewport panel keys (graph mode only)
        if self.viewport_panel_open && self.mode == Mode::Graph {
            match key {
                Key::Tab => {
                    self.commit_viewport_edit();
                    self.viewport_edit_field = (self.viewport_edit_field + 1) % 4;
                    self.load_viewport_edit_buffer();
                    return;
                }
                Key::Return => {
                    self.commit_viewport_edit();
                    self.viewport_panel_open = false;
                    self.invalidate_caches();
                    return;
                }
                Key::Escape => {
                    self.viewport_panel_open = false;
                    return;
                }
                Key::Backspace => {
                    self.viewport_edit_buffer.pop();
                    return;
                }
                Key::Char(c @ ('0'..='9' | '.' | '-')) => {
                    self.viewport_edit_buffer.push(*c);
                    return;
                }
                _ => {}
            }
        }

        // Color picker keys (graph mode only)
        if self.color_picker_open && self.mode == Mode::Graph {
            if matches!(key, Key::Escape) {
                self.color_picker_open = false;
                return;
            }
        }

        // 3D settings panel keys
        if self.settings3d_panel_open && self.mode == Mode::Graph3D {
            match key {
                Key::Escape => {
                    self.settings3d_panel_open = false;
                    return;
                }
                Key::Char(']') => {
                    self.graph3d.config.grid_lines = (self.graph3d.config.grid_lines + 5).min(100);
                    return;
                }
                Key::Char('[') => {
                    self.graph3d.config.grid_lines = self.graph3d.config.grid_lines.saturating_sub(5).max(10);
                    return;
                }
                Key::Char('>') => {
                    self.graph3d.config.surface_alpha = (self.graph3d.config.surface_alpha + 0.05).min(1.0);
                    return;
                }
                Key::Char('<') => {
                    self.graph3d.config.surface_alpha = (self.graph3d.config.surface_alpha - 0.05).max(0.1);
                    return;
                }
                Key::Char('c') => {
                    self.graph3d.config.colormap = self.graph3d.config.colormap.next();
                    return;
                }
                Key::Char('m') => {
                    self.graph3d.cycle_render_mode();
                    return;
                }
                _ => {}
            }
        }

        // Table view keys (graph mode only)
        if self.table_visible && self.mode == Mode::Graph {
            match key {
                Key::Up => {
                    if self.table_scroll_offset > 0 {
                        self.table_scroll_offset -= 1;
                    }
                    return;
                }
                Key::Down => {
                    if self.table_scroll_offset + 20 < self.table_data.len() {
                        self.table_scroll_offset += 1;
                    }
                    return;
                }
                Key::Char('e') if ctrl => {
                    self.export_table_to_clipboard();
                    return;
                }
                Key::Char('[') => {
                    self.table_step = (self.table_step / 2.0).max(0.001);
                    self.regenerate_table();
                    return;
                }
                Key::Char(']') => {
                    self.table_step = (self.table_step * 2.0).min(100.0);
                    self.regenerate_table();
                    return;
                }
                _ => {}
            }
        }

        // Plain text editor for graph/3D modes
        match key {
            Key::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.input.remove(self.cursor);
                }
            }
            Key::Delete => {
                if self.cursor < self.input.len() {
                    self.input.remove(self.cursor);
                }
            }
            Key::Left => {
                if ctrl {
                    while self.cursor > 0 && self.input.chars().nth(self.cursor - 1) == Some(' ') {
                        self.cursor -= 1;
                    }
                    while self.cursor > 0 && self.input.chars().nth(self.cursor - 1) != Some(' ') {
                        self.cursor -= 1;
                    }
                } else if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            Key::Right => {
                if ctrl {
                    let len = self.input.len();
                    while self.cursor < len && self.input.chars().nth(self.cursor) != Some(' ') {
                        self.cursor += 1;
                    }
                    while self.cursor < len && self.input.chars().nth(self.cursor) == Some(' ') {
                        self.cursor += 1;
                    }
                } else if self.cursor < self.input.len() {
                    self.cursor += 1;
                }
            }
            Key::Home => {
                self.cursor = 0;
            }
            Key::End => {
                self.cursor = self.input.len();
            }
            Key::Up => {
                if !self.history.is_empty() {
                    match self.history_index {
                        None => {
                            self.history_index = Some(self.history.len() - 1);
                        }
                        Some(idx) if idx > 0 => {
                            self.history_index = Some(idx - 1);
                        }
                        _ => {}
                    }
                    if let Some(idx) = self.history_index {
                        self.input = self.history[idx].input.clone();
                        self.cursor = self.input.len();
                    }
                }
            }
            Key::Down => {
                if let Some(idx) = self.history_index {
                    if idx + 1 < self.history.len() {
                        self.history_index = Some(idx + 1);
                        self.input = self.history[idx + 1].input.clone();
                        self.cursor = self.input.len();
                    } else {
                        self.history_index = None;
                        self.input.clear();
                        self.cursor = 0;
                    }
                }
            }
            Key::Char(c) => {
                self.input.insert(self.cursor, *c);
                self.cursor += 1;
                self.history_index = None;
            }
            Key::Space => {
                self.input.insert(self.cursor, ' ');
                self.cursor += 1;
                self.history_index = None;
            }
            _ => {}
        }
    }

    fn handle_calculator_input(&mut self, key: &Key, modifiers: Modifiers) {
        if !modifiers.ctrl && !modifiers.alt && !modifiers.super_key {
            match key {
                // Plain Up/Down should recall history when input is blank or while
                // actively browsing recalled entries.
                Key::Up if self.is_blank_math_input() || self.calc_history_index.is_some() => {
                    self.recall_calculator_history(true);
                    return;
                }
                Key::Down if self.is_blank_math_input() || self.calc_history_index.is_some() => {
                    self.recall_calculator_history(false);
                    return;
                }
                _ => {}
            }
        }

        if modifiers.ctrl {
            match key {
                Key::Up => {
                    self.recall_calculator_history(true);
                    return;
                }
                Key::Down => {
                    self.recall_calculator_history(false);
                    return;
                }
                _ => {}
            }
        }

        let result = match key {
            Key::Left => self.math_input.handle_key(SpecialKey::Left),
            Key::Right => self.math_input.handle_key(SpecialKey::Right),
            Key::Up => self.math_input.handle_key(SpecialKey::Up),
            Key::Down => self.math_input.handle_key(SpecialKey::Down),
            Key::Tab => {
                if modifiers.shift {
                    self.math_input.handle_key(SpecialKey::ShiftTab)
                } else {
                    self.math_input.handle_key(SpecialKey::Tab)
                }
            }
            Key::Backspace => self.math_input.handle_key(SpecialKey::Backspace),
            Key::Delete => self.math_input.handle_key(SpecialKey::Delete),
            Key::Home => self.math_input.handle_key(SpecialKey::Home),
            Key::End => self.math_input.handle_key(SpecialKey::End),
            // Keep AltGr-generated printable chars (notably '\') usable in calculator mode.
            Key::Char(c) if !modifiers.ctrl && !modifiers.super_key => {
                self.math_input.handle_char(*c)
            }
            Key::Space if !modifiers.ctrl && !modifiers.super_key => {
                self.math_input.handle_char(' ')
            }
            _ => MathInputResult::Ignored,
        };

        if !matches!(result, MathInputResult::Ignored) {
            self.history_index = None;
            self.calc_history_index = None;
        }
    }

    fn run_math_command(&mut self, cmd: &str) -> bool {
        let mut changed = false;
        if !matches!(self.math_input.handle_char('\\'), MathInputResult::Ignored) {
            changed = true;
        }
        for ch in cmd.chars() {
            if !matches!(self.math_input.handle_char(ch), MathInputResult::Ignored) {
                changed = true;
            }
        }
        if !matches!(self.math_input.handle_char(' '), MathInputResult::Ignored) {
            changed = true;
        }
        changed
    }

    fn insert_math_text(&mut self, text: &str) -> bool {
        let mut changed = false;
        for ch in text.chars() {
            if !matches!(self.math_input.handle_char(ch), MathInputResult::Ignored) {
                changed = true;
            }
        }
        changed
    }

    fn handle_calculator_button_action(&mut self, action: CalcButtonAction) {
        match action {
            CalcButtonAction::ToggleExtended => {
                self.calc_buttons_extended = !self.calc_buttons_extended;
            }
            CalcButtonAction::Evaluate => {
                self.evaluate();
            }
            CalcButtonAction::Clear => {
                self.math_input.clear();
                self.history_index = None;
                self.calc_history_index = None;
            }
            CalcButtonAction::Backspace => {
                if !matches!(
                    self.math_input.handle_key(SpecialKey::Backspace),
                    MathInputResult::Ignored
                ) {
                    self.history_index = None;
                    self.calc_history_index = None;
                }
            }
            CalcButtonAction::Delete => {
                if !matches!(
                    self.math_input.handle_key(SpecialKey::Delete),
                    MathInputResult::Ignored
                ) {
                    self.history_index = None;
                    self.calc_history_index = None;
                }
            }
            CalcButtonAction::MoveLeft => {
                if !matches!(
                    self.math_input.handle_key(SpecialKey::Left),
                    MathInputResult::Ignored
                ) {
                    self.history_index = None;
                    self.calc_history_index = None;
                }
            }
            CalcButtonAction::MoveRight => {
                if !matches!(
                    self.math_input.handle_key(SpecialKey::Right),
                    MathInputResult::Ignored
                ) {
                    self.history_index = None;
                    self.calc_history_index = None;
                }
            }
            CalcButtonAction::Tab => {
                if !matches!(
                    self.math_input.handle_key(SpecialKey::Tab),
                    MathInputResult::Ignored
                ) {
                    self.history_index = None;
                    self.calc_history_index = None;
                }
            }
            CalcButtonAction::InsertText(text) => {
                if self.insert_math_text(text) {
                    self.history_index = None;
                    self.calc_history_index = None;
                }
            }
            CalcButtonAction::Command(cmd) => {
                if self.run_math_command(cmd) {
                    self.history_index = None;
                    self.calc_history_index = None;
                }
            }
        }
    }

    fn is_blank_math_input(&self) -> bool {
        match self.math_input.mathbox() {
            MathBox::Row(items) => {
                items.is_empty() || items.iter().all(|item| matches!(item, MathBox::Slot))
            }
            MathBox::Slot => true,
            _ => false,
        }
    }

    fn editable_math_input(mathbox: MathBox) -> MathInput {
        let root = match mathbox {
            MathBox::Row(mut items) => {
                if items.is_empty() || !matches!(items.last(), Some(MathBox::Slot)) {
                    items.push(MathBox::Slot);
                }
                MathBox::Row(items)
            }
            other => MathBox::Row(vec![other, MathBox::Slot]),
        };

        let mut input = MathInput::from_mathbox(root);
        if let MathBox::Row(items) = &input.root {
            if !items.is_empty() {
                input.cursor.enter(items.len() - 1);
            }
        }
        input
    }

    fn recall_calculator_history(&mut self, older: bool) {
        if self.calc_history.is_empty() {
            return;
        }

        let len = self.calc_history.len();
        let new_index = match (self.calc_history_index, older) {
            (None, true) => Some(len - 1),
            (None, false) => None,
            (Some(idx), true) if idx > 0 => Some(idx - 1),
            (Some(idx), true) => Some(idx),
            (Some(idx), false) if idx + 1 < len => Some(idx + 1),
            (Some(_), false) => None,
        };

        if let Some(idx) = new_index {
            self.calc_history_index = Some(idx);
            self.math_input = Self::editable_math_input(self.calc_history[idx].clone());
            self.history_index = None;
        } else {
            self.calc_history_index = None;
            self.math_input.clear();
        }
    }

    fn evaluate_calculator_input(&mut self) {
        let expr = match to_expr(self.math_input.mathbox()) {
            Ok(expr) => expr,
            Err(ConvertError::EmptySlot) => {
                self.history.push(HistoryEntry {
                    input: "<structured input>".to_string(),
                    result: String::new(),
                    error: Some(
                        "Expression is incomplete: fill all empty boxes before evaluating."
                            .to_string(),
                    ),
                });
                self.history_index = None;
                self.calc_history_index = None;
                return;
            }
            Err(err) => {
                self.history.push(HistoryEntry {
                    input: "<structured input>".to_string(),
                    result: String::new(),
                    error: Some(format!("Input error: {err}")),
                });
                self.history_index = None;
                self.calc_history_index = None;
                return;
            }
        };

        let input = expr.to_string();
        let (result, error) = match self.evaluator.eval(&expr) {
            Ok(val) => (val.to_string(), None),
            Err(e) => (String::new(), Some(e.to_string())),
        };

        self.history.push(HistoryEntry {
            input,
            result,
            error,
        });
        self.calc_history.push(from_expr(&expr));
        self.math_input.clear();
        self.history_index = None;
        self.calc_history_index = None;
    }

    fn evaluate(&mut self) {
        if self.mode == Mode::Calculator {
            self.evaluate_calculator_input();
            return;
        }

        if self.input.is_empty() {
            return;
        }

        let input = self.input.clone();

        // In graph mode, add functions to the graph
        if self.mode == Mode::Graph {
            // Check for implicit curve (equation with both x and y)
            // Format: "x^2 + y^2 = 1" becomes F(x,y) = x^2 + y^2 - 1 = 0
            if let Some(eq_pos) = input.find('=') {
                let lhs = input[..eq_pos].trim();
                let rhs = input[eq_pos + 1..].trim();

                // Check if this is "r = f(theta)" (polar), "y = f(x)" (explicit), or implicit
                if lhs == "r" {
                    match parser::parse(rhs) {
                        Ok(expr) => {
                            self.graph.add_polar(expr, (0.0, std::f64::consts::TAU));
                            self.history.push(HistoryEntry {
                                input: input.clone(),
                                result: format!(
                                    "Added polar curve {}",
                                    self.graph.functions.len()
                                ),
                                error: None,
                            });
                            self.invalidate_caches();
                        }
                        Err(e) => {
                            self.history.push(HistoryEntry {
                                input: input.clone(),
                                result: String::new(),
                                error: Some(e.to_string()),
                            });
                        }
                    }
                } else if lhs == "y" {
                    // Explicit function y = f(x)
                    match parser::parse(rhs) {
                        Ok(expr) => {
                            self.graph.add_explicit(expr);
                            self.history.push(HistoryEntry {
                                input: input.clone(),
                                result: format!("Added function {}", self.graph.functions.len()),
                                error: None,
                            });
                            self.invalidate_caches();
                        }
                        Err(e) => {
                            self.history.push(HistoryEntry {
                                input: input.clone(),
                                result: String::new(),
                                error: Some(e.to_string()),
                            });
                        }
                    }
                } else {
                    // Implicit curve: lhs = rhs -> lhs - rhs = 0
                    let implicit_expr = format!("({}) - ({})", lhs, rhs);
                    match parser::parse(&implicit_expr) {
                        Ok(expr) => {
                            self.graph.add_implicit(expr);
                            self.history.push(HistoryEntry {
                                input: input.clone(),
                                result: format!(
                                    "Added implicit curve {}",
                                    self.graph.functions.len()
                                ),
                                error: None,
                            });
                            self.invalidate_caches();
                        }
                        Err(e) => {
                            self.history.push(HistoryEntry {
                                input: input.clone(),
                                result: String::new(),
                                error: Some(e.to_string()),
                            });
                        }
                    }
                }
            } else if input.starts_with("piecewise(") || input.starts_with("pw(") {
                self.parse_piecewise(&input);
            } else if input.contains(',') && (input.contains('t') || input.starts_with('(')) {
                // Parametric curve: (x(t), y(t)) or x(t), y(t)
                self.parse_parametric(&input);
            } else {
                // No equals sign - treat as explicit y = expr
                match parser::parse(input.trim()) {
                    Ok(expr) => {
                        self.graph.add_explicit(expr);
                        self.history.push(HistoryEntry {
                            input: input.clone(),
                            result: format!("Added function {}", self.graph.functions.len()),
                            error: None,
                        });
                        self.invalidate_caches();
                    }
                    Err(e) => {
                        self.history.push(HistoryEntry {
                            input: input.clone(),
                            result: String::new(),
                            error: Some(e.to_string()),
                        });
                    }
                }
            }
        } else if self.mode == Mode::Graph3D {
            // Check for spherical surface: "sphere: expr" or "sph: expr"
            let sph_prefix = input.strip_prefix("sphere:")
                .or_else(|| input.strip_prefix("sph:"));
            if let Some(sph_expr) = sph_prefix {
                match parser::parse(sph_expr.trim()) {
                    Ok(expr) => {
                        self.graph3d.add_spherical(expr);
                        self.history.push(HistoryEntry {
                            input: input.clone(),
                            result: format!("Added spherical surface {}", self.graph3d.surfaces.len()),
                            error: None,
                        });
                    }
                    Err(e) => {
                        self.history.push(HistoryEntry {
                            input: input.clone(),
                            result: String::new(),
                            error: Some(e.to_string()),
                        });
                    }
                }
                self.input.clear();
                self.cursor = 0;
                self.history_index = None;
                return;
            }

            // Check for cylindrical surface: "cyl: expr" or "cylinder: expr"
            let cyl_prefix = input.strip_prefix("cyl:")
                .or_else(|| input.strip_prefix("cylinder:"));
            if let Some(cyl_expr) = cyl_prefix {
                match parser::parse(cyl_expr.trim()) {
                    Ok(expr) => {
                        self.graph3d.add_cylindrical(expr);
                        self.history.push(HistoryEntry {
                            input: input.clone(),
                            result: format!("Added cylindrical surface {}", self.graph3d.surfaces.len()),
                            error: None,
                        });
                    }
                    Err(e) => {
                        self.history.push(HistoryEntry {
                            input: input.clone(),
                            result: String::new(),
                            error: Some(e.to_string()),
                        });
                    }
                }
                self.input.clear();
                self.cursor = 0;
                self.history_index = None;
                return;
            }

            // Check for level surface: "level: expr = c"
            if let Some(level_str) = input.strip_prefix("level:") {
                let level_str = level_str.trim();
                if let Some(eq_pos) = level_str.find('=') {
                    let expr_str = level_str[..eq_pos].trim();
                    let level_val_str = level_str[eq_pos + 1..].trim();
                    let level_val = level_val_str.parse::<f64>().unwrap_or(0.0);
                    match parser::parse(expr_str) {
                        Ok(expr) => {
                            self.graph3d.add_level_surface(expr, level_val);
                            self.history.push(HistoryEntry {
                                input: input.clone(),
                                result: format!("Added level surface {}", self.graph3d.surfaces.len()),
                                error: None,
                            });
                        }
                        Err(e) => {
                            self.history.push(HistoryEntry {
                                input: input.clone(),
                                result: String::new(),
                                error: Some(e.to_string()),
                            });
                        }
                    }
                } else {
                    self.history.push(HistoryEntry {
                        input: input.clone(),
                        result: String::new(),
                        error: Some("Level surface format: level: f(x,y,z) = c".to_string()),
                    });
                }
                self.input.clear();
                self.cursor = 0;
                self.history_index = None;
                return;
            }

            // Check for parametric surface: (x(u,v), y(u,v), z(u,v))
            if input.contains(',') && (input.contains('u') || input.starts_with('(')) {
                self.parse_parametric_surface(&input);
            } else {
                // Try to parse as explicit surface (3D: z = f(x, y))
                // Support formats: "z = expr", "expr" (implicit z=)
                let expr_str = if let Some(rest) = input
                    .strip_prefix("z=")
                    .or_else(|| input.strip_prefix("z ="))
                {
                    rest.trim()
                } else {
                    input.trim()
                };

                match parser::parse(expr_str) {
                    Ok(expr) => {
                        self.graph3d.add_explicit(expr);
                        self.history.push(HistoryEntry {
                            input: input.clone(),
                            result: format!("Added surface {}", self.graph3d.surfaces.len()),
                            error: None,
                        });
                    }
                    Err(e) => {
                        self.history.push(HistoryEntry {
                            input: input.clone(),
                            result: String::new(),
                            error: Some(e.to_string()),
                        });
                    }
                }
            }
        } else {
            // Calculator mode - evaluate expression
            let (result, error) = match parser::parse(&input) {
                Ok(expr) => match self.evaluator.eval(&expr) {
                    Ok(val) => (val.to_string(), None),
                    Err(e) => (String::new(), Some(e.to_string())),
                },
                Err(e) => (String::new(), Some(e.to_string())),
            };

            let entry = HistoryEntry {
                input,
                result,
                error,
            };
            self.history.push(entry);
        }

        self.input.clear();
        self.cursor = 0;
        self.history_index = None;
    }

    /// Parse and add a parametric curve
    /// Formats: "(sin(t), cos(t))" or "sin(t), cos(t)" or "(sin(t), cos(t), 0, 2*pi)"
    fn parse_parametric(&mut self, input: &str) {
        // Remove outer parentheses if present
        let inner = input
            .trim()
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or(input.trim());

        // Split by comma
        let parts: Vec<&str> = inner.split(',').collect();

        if parts.len() < 2 {
            self.history.push(HistoryEntry {
                input: input.to_string(),
                result: String::new(),
                error: Some(
                    "Parametric curve needs at least two components: x(t), y(t)".to_string(),
                ),
            });
            return;
        }

        // Parse x(t) and y(t)
        let x_result = parser::parse(parts[0].trim());
        let y_result = parser::parse(parts[1].trim());

        // Optional t range (default 0 to 2*pi)
        let (t_min, t_max) = if parts.len() >= 4 {
            let min_result = parser::parse(parts[2].trim()).and_then(|e| {
                let eval = Evaluator::new();
                eval.eval(&e)
                    .ok()
                    .and_then(|v| match v {
                        garcalc_cas::Expr::Integer(n) => Some(n as f64),
                        garcalc_cas::Expr::Float(f) => Some(f),
                        _ => None,
                    })
                    .ok_or(garcalc_cas::CasError::Type("expected number".to_string()))
            });
            let max_result = parser::parse(parts[3].trim()).and_then(|e| {
                let eval = Evaluator::new();
                eval.eval(&e)
                    .ok()
                    .and_then(|v| match v {
                        garcalc_cas::Expr::Integer(n) => Some(n as f64),
                        garcalc_cas::Expr::Float(f) => Some(f),
                        _ => None,
                    })
                    .ok_or(garcalc_cas::CasError::Type("expected number".to_string()))
            });
            match (min_result, max_result) {
                (Ok(min), Ok(max)) => (min, max),
                _ => (0.0, std::f64::consts::TAU),
            }
        } else {
            (0.0, std::f64::consts::TAU)
        };

        match (x_result, y_result) {
            (Ok(x_expr), Ok(y_expr)) => {
                self.graph.add_parametric(x_expr, y_expr, (t_min, t_max));
                self.history.push(HistoryEntry {
                    input: input.to_string(),
                    result: format!("Added parametric curve {}", self.graph.functions.len()),
                    error: None,
                });
                self.invalidate_caches();
            }
            (Err(e), _) | (_, Err(e)) => {
                self.history.push(HistoryEntry {
                    input: input.to_string(),
                    result: String::new(),
                    error: Some(e.to_string()),
                });
            }
        }
    }

    /// Parse and add a parametric surface
    /// Format: "(x(u,v), y(u,v), z(u,v))" or with ranges "(x, y, z, u_min, u_max, v_min, v_max)"
    fn parse_parametric_surface(&mut self, input: &str) {
        let inner = input
            .trim()
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or(input.trim());

        let parts: Vec<&str> = inner.split(',').collect();

        if parts.len() < 3 {
            self.history.push(HistoryEntry {
                input: input.to_string(),
                result: String::new(),
                error: Some(
                    "Parametric surface needs three components: x(u,v), y(u,v), z(u,v)".to_string(),
                ),
            });
            return;
        }

        let x_result = parser::parse(parts[0].trim());
        let y_result = parser::parse(parts[1].trim());
        let z_result = parser::parse(parts[2].trim());

        // Default u,v ranges from 0 to 2*pi
        let (u_min, u_max, v_min, v_max) = if parts.len() >= 7 {
            let parse_num = |s: &str| -> f64 {
                parser::parse(s.trim())
                    .and_then(|e| {
                        let eval = Evaluator::new();
                        eval.eval(&e)
                            .ok()
                            .and_then(|v| match v {
                                garcalc_cas::Expr::Integer(n) => Some(n as f64),
                                garcalc_cas::Expr::Float(f) => Some(f),
                                _ => None,
                            })
                            .ok_or(garcalc_cas::CasError::Type("expected number".to_string()))
                    })
                    .unwrap_or(0.0)
            };
            (
                parse_num(parts[3]),
                parse_num(parts[4]),
                parse_num(parts[5]),
                parse_num(parts[6]),
            )
        } else {
            (0.0, std::f64::consts::TAU, 0.0, std::f64::consts::TAU)
        };

        match (x_result, y_result, z_result) {
            (Ok(x_expr), Ok(y_expr), Ok(z_expr)) => {
                self.graph3d
                    .add_parametric(x_expr, y_expr, z_expr, (u_min, u_max), (v_min, v_max));
                self.history.push(HistoryEntry {
                    input: input.to_string(),
                    result: format!("Added parametric surface {}", self.graph3d.surfaces.len()),
                    error: None,
                });
            }
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                self.history.push(HistoryEntry {
                    input: input.to_string(),
                    result: String::new(),
                    error: Some(e.to_string()),
                });
            }
        }
    }

    /// Parse and add a piecewise function
    fn parse_piecewise(&mut self, input: &str) {
        let inner = input
            .trim()
            .strip_prefix("piecewise(")
            .or_else(|| input.trim().strip_prefix("pw("))
            .and_then(|s| s.strip_suffix(')'));

        let inner = match inner {
            Some(s) => s,
            None => {
                self.history.push(HistoryEntry {
                    input: input.to_string(),
                    result: String::new(),
                    error: Some(
                        "Invalid piecewise syntax. Use: piecewise(expr1, cond1, expr2, cond2, ...)"
                            .to_string(),
                    ),
                });
                return;
            }
        };

        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() < 2 || parts.len() % 2 != 0 {
            self.history.push(HistoryEntry {
                input: input.to_string(),
                result: String::new(),
                error: Some(
                    "Piecewise needs pairs: expr1, cond1, expr2, cond2, ...".to_string(),
                ),
            });
            return;
        }

        let mut pieces = Vec::new();
        for chunk in parts.chunks(2) {
            let expr_str = chunk[0].trim();
            let cond_str = chunk[1].trim();

            let expr = match parser::parse(expr_str) {
                Ok(e) => e,
                Err(e) => {
                    self.history.push(HistoryEntry {
                        input: input.to_string(),
                        result: String::new(),
                        error: Some(format!("Parse error in '{}': {}", expr_str, e)),
                    });
                    return;
                }
            };

            let condition = match Self::parse_condition(cond_str) {
                Some(c) => c,
                None => {
                    self.history.push(HistoryEntry {
                        input: input.to_string(),
                        result: String::new(),
                        error: Some(format!(
                            "Invalid condition: '{}'. Use: x<0, x>=2, else",
                            cond_str
                        )),
                    });
                    return;
                }
            };

            pieces.push(garcalc_graph::PiecewisePiece { expr, condition });
        }

        self.graph.add_piecewise(pieces);
        self.history.push(HistoryEntry {
            input: input.to_string(),
            result: format!("Added piecewise function {}", self.graph.functions.len()),
            error: None,
        });
        self.invalidate_caches();
    }

    fn parse_condition(s: &str) -> Option<garcalc_graph::PiecewiseCondition> {
        use garcalc_graph::PiecewiseCondition;
        let s = s.trim();

        if s == "else" || s == "otherwise" {
            return Some(PiecewiseCondition::Always);
        }

        // Try range conditions like "0 <= x < 3"
        if let Some(cond) = Self::parse_range_condition(s) {
            return Some(cond);
        }

        // Try "x >= val", "x > val", "x <= val", "x < val"
        for (op, ctor) in [
            (
                ">=",
                PiecewiseCondition::GreaterEqual as fn(f64) -> PiecewiseCondition,
            ),
            (
                "<=",
                PiecewiseCondition::LessEqual as fn(f64) -> PiecewiseCondition,
            ),
            (
                ">",
                PiecewiseCondition::GreaterThan as fn(f64) -> PiecewiseCondition,
            ),
            (
                "<",
                PiecewiseCondition::LessThan as fn(f64) -> PiecewiseCondition,
            ),
        ] {
            if let Some(rest) = s
                .strip_prefix("x")
                .and_then(|r| r.trim_start().strip_prefix(op))
            {
                if let Ok(val) = rest.trim().parse::<f64>() {
                    return Some(ctor(val));
                }
            }
        }

        None
    }

    fn parse_range_condition(s: &str) -> Option<garcalc_graph::PiecewiseCondition> {
        use garcalc_graph::PiecewiseCondition;
        let s = s.trim();

        let x_pos = s.find('x')?;
        let left = s[..x_pos].trim();
        let right = s[x_pos + 1..].trim();

        if left.is_empty() || right.is_empty() {
            return None;
        }

        let (left_val, left_inclusive) = if let Some(rest) = left.strip_suffix("<=") {
            (rest.trim().parse::<f64>().ok()?, true)
        } else if let Some(rest) = left.strip_suffix("<") {
            (rest.trim().parse::<f64>().ok()?, false)
        } else {
            return None;
        };

        let (right_val, right_inclusive) = if let Some(rest) = right.strip_prefix("<=") {
            (rest.trim().parse::<f64>().ok()?, true)
        } else if let Some(rest) = right.strip_prefix("<") {
            (rest.trim().parse::<f64>().ok()?, false)
        } else {
            return None;
        };

        if left_inclusive && right_inclusive {
            Some(PiecewiseCondition::BetweenInclusive(left_val, right_val))
        } else {
            Some(PiecewiseCondition::Between(left_val, right_val))
        }
    }

    fn commit_viewport_edit(&mut self) {
        if let Ok(val) = self.viewport_edit_buffer.parse::<f64>() {
            match self.viewport_edit_field {
                0 => self.graph.viewport.x_min = val,
                1 => self.graph.viewport.x_max = val,
                2 => self.graph.viewport.y_min = val,
                3 => self.graph.viewport.y_max = val,
                _ => {}
            }
        }
    }

    fn load_viewport_edit_buffer(&mut self) {
        self.viewport_edit_buffer = match self.viewport_edit_field {
            0 => format!("{:.4}", self.graph.viewport.x_min),
            1 => format!("{:.4}", self.graph.viewport.x_max),
            2 => format!("{:.4}", self.graph.viewport.y_min),
            3 => format!("{:.4}", self.graph.viewport.y_max),
            _ => String::new(),
        };
    }

    fn invalidate_caches(&mut self) {
        if self.zeros_visible {
            self.cached_zeros = self.graph.find_zeros();
            self.cached_intersections = self.graph.find_intersections();
        }
        if self.table_visible {
            self.regenerate_table();
        }
    }

    fn regenerate_table(&mut self) {
        let x_min = self.graph.viewport.x_min;
        let x_max = self.graph.viewport.x_max;
        self.table_data =
            self.graph
                .generate_table(self.table_func_index, x_min, x_max, self.table_step);
        self.table_scroll_offset = 0;
    }

    fn export_table_to_clipboard(&self) {
        let mut csv = String::from("x\tf(x)\n");
        for (x, y) in &self.table_data {
            match y {
                Some(yv) => csv.push_str(&format!("{:.6}\t{:.6}\n", x, yv)),
                None => csv.push_str(&format!("{:.6}\tundefined\n", x)),
            }
        }
        let _ = Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(ref mut stdin) = child.stdin {
                    let _ = stdin.write_all(csv.as_bytes());
                }
                child.wait()
            });
    }

    fn render(&mut self) -> Result<()> {
        let math_input = if self.mode == Mode::Calculator {
            Some(&self.math_input)
        } else {
            None
        };
        self.ui.render(
            &self.input,
            self.cursor,
            self.cursor_visible,
            math_input,
            &self.history,
            self.mode,
            &self.graph,
            &self.graph3d,
            self.help_modal_open,
            self.calc_buttons_extended,
            self.show_function_list,
            self.zeros_visible,
            &self.cached_zeros,
            &self.cached_intersections,
            self.table_visible,
            self.table_scroll_offset,
            self.table_step,
            &self.table_data,
            self.viewport_panel_open,
            self.viewport_edit_field,
            &self.viewport_edit_buffer,
            self.color_picker_open,
            self.color_picker_func_index,
            self.settings3d_panel_open,
            self.auto_rotate,
        )?;
        Ok(())
    }

    fn reset_cursor_blink(&mut self) {
        self.cursor_visible = true;
        self.last_cursor_blink = Instant::now();
    }

    fn tick_cursor_blink(&mut self) -> bool {
        const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(550);

        let now = Instant::now();
        if now.duration_since(self.last_cursor_blink) >= CURSOR_BLINK_INTERVAL {
            self.cursor_visible = !self.cursor_visible;
            self.last_cursor_blink = now;
            return true;
        }

        false
    }
}
