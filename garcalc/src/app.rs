//! Application state and event loop

use anyhow::Result;
use garcalc_cas::{parser, Evaluator};
use garcalc_graph::{Graph2D, Graph3D};
use garcalc_ipc::Mode;
use gartk_core::{InputEvent, Key, MouseButton};
use gartk_x11::{Connection, EventLoop, EventLoopConfig, Window, WindowConfig};

use crate::config::Config;
use crate::ui::CalculatorUI;

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
    /// Input text
    input: String,
    /// Cursor position
    cursor: usize,
    /// Calculation history
    history: Vec<HistoryEntry>,
    /// History navigation index
    history_index: Option<usize>,
    /// Whether running in popup mode
    popup_mode: bool,
    /// Whether app should quit
    should_quit: bool,
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
            history: Vec::new(),
            history_index: None,
            popup_mode: popup,
            should_quit: false,
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
                    self.handle_key(&key_event.key, key_event.modifiers.ctrl);
                    ev.request_redraw();
                }
                InputEvent::MousePress(mouse_ev) => {
                    let x = mouse_ev.position.x as f64;
                    let y = mouse_ev.position.y as f64;
                    if self.mode == Mode::Graph {
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
                    if (self.mode == Mode::Graph || self.mode == Mode::Graph3D)
                        && mouse_ev.button == Some(MouseButton::Left)
                    {
                        self.drag_start = None;
                    }
                }
                InputEvent::MouseMove(mouse_ev) => {
                    let x = mouse_ev.position.x as f64;
                    let y = mouse_ev.position.y as f64;
                    if self.mode == Mode::Graph {
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
                    if self.mode == Mode::Graph {
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
                }
                InputEvent::FocusOut => {
                    if self.has_focus && self.popup_mode {
                        self.should_quit = true;
                    }
                }
                InputEvent::Idle => {
                    // Handle idle - could animate cursor here
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

    fn handle_key(&mut self, key: &Key, ctrl: bool) {
        match key {
            Key::Escape => {
                if self.popup_mode {
                    self.should_quit = true;
                } else {
                    // Clear input in standalone mode
                    self.input.clear();
                    self.cursor = 0;
                }
            }
            Key::Return => {
                self.evaluate();
            }
            // Mode switching with F-keys
            Key::F1 => {
                self.mode = Mode::Calculator;
            }
            Key::F2 => {
                self.mode = Mode::Graph;
            }
            Key::F3 => {
                self.mode = Mode::Graph3D;
            }
            // Graph-specific keys (2D)
            Key::Char('r') if ctrl && self.mode == Mode::Graph => {
                // Reset viewport
                self.graph.reset_viewport();
            }
            Key::Char('c') if ctrl && self.mode == Mode::Graph => {
                // Clear functions
                self.graph.clear_functions();
            }
            Key::Char('t') if ctrl && self.mode == Mode::Graph => {
                // Toggle trace
                self.graph.trace_enabled = !self.graph.trace_enabled;
            }
            // Graph3D-specific keys
            Key::Char('r') if ctrl && self.mode == Mode::Graph3D => {
                // Reset camera
                self.graph3d.reset_camera();
            }
            Key::Char('c') if ctrl && self.mode == Mode::Graph3D => {
                // Clear surfaces
                self.graph3d.clear_surfaces();
            }
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
                    // Move to previous word
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
                    // Move to next word
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
                // Navigate history
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

    fn evaluate(&mut self) {
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
                                result: format!("Added implicit curve {}", self.graph.functions.len()),
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
                let expr_str = if let Some(rest) = input.strip_prefix("z=").or_else(|| input.strip_prefix("z =")) {
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
        let inner = input.trim().strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or(input.trim());

        // Split by comma
        let parts: Vec<&str> = inner.split(',').collect();

        if parts.len() < 2 {
            self.history.push(HistoryEntry {
                input: input.to_string(),
                result: String::new(),
                error: Some("Parametric curve needs at least two components: x(t), y(t)".to_string()),
            });
            return;
        }

        // Parse x(t) and y(t)
        let x_result = parser::parse(parts[0].trim());
        let y_result = parser::parse(parts[1].trim());

        // Optional t range (default 0 to 2*pi)
        let (t_min, t_max) = if parts.len() >= 4 {
            let min_result = parser::parse(parts[2].trim())
                .and_then(|e| {
                    let eval = Evaluator::new();
                    eval.eval(&e).ok().and_then(|v| match v {
                        garcalc_cas::Expr::Integer(n) => Some(n as f64),
                        garcalc_cas::Expr::Float(f) => Some(f),
                        _ => None,
                    }).ok_or(garcalc_cas::CasError::Type("expected number".to_string()))
                });
            let max_result = parser::parse(parts[3].trim())
                .and_then(|e| {
                    let eval = Evaluator::new();
                    eval.eval(&e).ok().and_then(|v| match v {
                        garcalc_cas::Expr::Integer(n) => Some(n as f64),
                        garcalc_cas::Expr::Float(f) => Some(f),
                        _ => None,
                    }).ok_or(garcalc_cas::CasError::Type("expected number".to_string()))
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
        let inner = input.trim().strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or(input.trim());

        let parts: Vec<&str> = inner.split(',').collect();

        if parts.len() < 3 {
            self.history.push(HistoryEntry {
                input: input.to_string(),
                result: String::new(),
                error: Some("Parametric surface needs three components: x(u,v), y(u,v), z(u,v)".to_string()),
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
                        eval.eval(&e).ok().and_then(|v| match v {
                            garcalc_cas::Expr::Integer(n) => Some(n as f64),
                            garcalc_cas::Expr::Float(f) => Some(f),
                            _ => None,
                        }).ok_or(garcalc_cas::CasError::Type("expected number".to_string()))
                    })
                    .unwrap_or(0.0)
            };
            (parse_num(parts[3]), parse_num(parts[4]), parse_num(parts[5]), parse_num(parts[6]))
        } else {
            (0.0, std::f64::consts::TAU, 0.0, std::f64::consts::TAU)
        };

        match (x_result, y_result, z_result) {
            (Ok(x_expr), Ok(y_expr), Ok(z_expr)) => {
                self.graph3d.add_parametric(x_expr, y_expr, z_expr, (u_min, u_max), (v_min, v_max));
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
        self.ui.render(&self.input, self.cursor, &self.history, self.mode, &self.graph, &self.graph3d)?;
        Ok(())
    }
}
