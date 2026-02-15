//! Application state and event loop

use anyhow::Result;
use garcalc_cas::{Evaluator, parser};
use garcalc_graph::{Graph2D, Graph3D};
use garcalc_ipc::Mode;
use garcalc_math::input::SpecialKey;
use garcalc_math::{
    ConvertError, InputResult as MathInputResult, MathBox, MathInput, from_expr, to_expr,
};
use gartk_core::{InputEvent, Key, Modifiers, MouseButton};
use gartk_x11::{Connection, EventLoop, EventLoopConfig, Window, WindowConfig};
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
                            // Left click - start drag for pan
                            self.drag_start = Some((x, y));
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
            _ => {}
        }

        // Graph shortcuts
        match key {
            Key::Char('r') if ctrl && self.mode == Mode::Graph => {
                self.graph.reset_viewport();
                return;
            }
            Key::Char('c') if ctrl && self.mode == Mode::Graph => {
                self.graph.clear_functions();
                return;
            }
            Key::Char('t') if ctrl && self.mode == Mode::Graph => {
                self.graph.trace_enabled = !self.graph.trace_enabled;
                return;
            }
            Key::Char('r') if ctrl && self.mode == Mode::Graph3D => {
                self.graph3d.reset_camera();
                return;
            }
            Key::Char('c') if ctrl && self.mode == Mode::Graph3D => {
                self.graph3d.clear_surfaces();
                return;
            }
            _ => {}
        }

        if self.mode == Mode::Calculator {
            self.handle_calculator_input(key, modifiers);
            return;
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

                // Check if this is "y = f(x)" (explicit) or implicit
                if lhs == "y" {
                    // Explicit function y = f(x)
                    match parser::parse(rhs) {
                        Ok(expr) => {
                            self.graph.add_explicit(expr);
                            self.history.push(HistoryEntry {
                                input: input.clone(),
                                result: format!("Added function {}", self.graph.functions.len()),
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
