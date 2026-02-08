//! Application state and event loop

use anyhow::Result;
use garcalc_cas::Evaluator;
use garcalc_ipc::Mode;
use gartk_core::{InputEvent, Key};
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
            mode,
            input: String::new(),
            cursor: 0,
            history: Vec::new(),
            history_index: None,
            popup_mode: popup,
            should_quit: false,
            has_focus: false,
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
        let (result, error) = match garcalc_cas::parser::parse(&input) {
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

        self.input.clear();
        self.cursor = 0;
        self.history_index = None;
    }

    fn render(&mut self) -> Result<()> {
        self.ui.render(&self.input, self.cursor, &self.history, self.mode)?;
        Ok(())
    }
}
