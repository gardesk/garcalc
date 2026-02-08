//! Keyboard input handling for math input
//!
//! Provides structured keyboard navigation through a MathBox tree
//! with support for template insertion via commands.

use crate::mathbox::{MathBox, Cursor, Operator};

/// Result of processing a key event
#[derive(Debug, Clone, PartialEq)]
pub enum InputResult {
    /// Input was consumed, UI should redraw
    Consumed,
    /// Input was not handled
    Ignored,
    /// User pressed Enter to evaluate
    Evaluate,
    /// User requested to close/cancel
    Cancel,
}

/// Math input state with cursor
pub struct MathInput {
    /// Root of the expression tree
    pub root: MathBox,
    /// Current cursor position
    pub cursor: Cursor,
    /// Command mode buffer (active when typing \ commands)
    pub command_buffer: Option<String>,
}

impl Default for MathInput {
    fn default() -> Self {
        Self::new()
    }
}

impl MathInput {
    /// Create a new empty input
    pub fn new() -> Self {
        let mut cursor = Cursor::new();
        cursor.enter(0); // Start inside the first slot
        Self {
            root: MathBox::Row(vec![MathBox::Slot]),
            cursor,
            command_buffer: None,
        }
    }

    /// Create from an existing MathBox
    pub fn from_mathbox(mathbox: MathBox) -> Self {
        Self {
            root: mathbox,
            cursor: Cursor::new(),
            command_buffer: None,
        }
    }

    /// Clear the input
    pub fn clear(&mut self) {
        self.root = MathBox::Row(vec![MathBox::Slot]);
        let mut cursor = Cursor::new();
        cursor.enter(0);
        self.cursor = cursor;
        self.command_buffer = None;
    }

    /// Handle a character input
    pub fn handle_char(&mut self, ch: char) -> InputResult {
        // Handle command mode
        if let Some(ref mut buf) = self.command_buffer {
            match ch {
                ' ' | '\n' => {
                    // Execute command
                    let cmd = buf.clone();
                    self.command_buffer = None;
                    return self.execute_command(&cmd);
                }
                c if c.is_alphanumeric() => {
                    buf.push(c);
                    return InputResult::Consumed;
                }
                _ => {
                    // Cancel command mode
                    self.command_buffer = None;
                    return InputResult::Consumed;
                }
            }
        }

        // Normal input
        match ch {
            '\\' => {
                // Enter command mode
                self.command_buffer = Some(String::new());
                InputResult::Consumed
            }
            '/' => {
                // Insert fraction
                self.insert_template(MathBox::fraction_template());
                InputResult::Consumed
            }
            '^' => {
                // Convert current element to power base
                self.wrap_in_power();
                InputResult::Consumed
            }
            '_' => {
                // Convert current element to subscript base
                self.wrap_in_subscript();
                InputResult::Consumed
            }
            '(' => {
                self.insert_at_cursor(MathBox::Parens(Box::new(MathBox::Slot)));
                self.cursor.enter(0);
                InputResult::Consumed
            }
            ')' => {
                // Try to exit parens
                self.try_exit_container();
                InputResult::Consumed
            }
            '|' => {
                self.insert_at_cursor(MathBox::Abs(Box::new(MathBox::Slot)));
                self.cursor.enter(0);
                InputResult::Consumed
            }
            '+' => {
                self.insert_at_cursor(MathBox::Operator(Operator::Add));
                InputResult::Consumed
            }
            '-' => {
                self.insert_at_cursor(MathBox::Operator(Operator::Sub));
                InputResult::Consumed
            }
            '*' => {
                self.insert_at_cursor(MathBox::Operator(Operator::Mul));
                InputResult::Consumed
            }
            '=' => {
                self.insert_at_cursor(MathBox::Operator(Operator::Eq));
                InputResult::Consumed
            }
            '<' => {
                self.insert_at_cursor(MathBox::Operator(Operator::Lt));
                InputResult::Consumed
            }
            '>' => {
                self.insert_at_cursor(MathBox::Operator(Operator::Gt));
                InputResult::Consumed
            }
            ',' => {
                self.insert_at_cursor(MathBox::Operator(Operator::Comma));
                InputResult::Consumed
            }
            c if c.is_ascii_digit() || c == '.' => {
                self.append_to_number(c);
                InputResult::Consumed
            }
            c if c.is_alphabetic() => {
                self.append_to_symbol(c);
                InputResult::Consumed
            }
            _ => InputResult::Ignored,
        }
    }

    /// Handle a special key
    pub fn handle_key(&mut self, key: SpecialKey) -> InputResult {
        // Cancel command mode on special keys
        if self.command_buffer.is_some() && !matches!(key, SpecialKey::Backspace) {
            self.command_buffer = None;
        }

        match key {
            SpecialKey::Left => {
                self.move_left();
                InputResult::Consumed
            }
            SpecialKey::Right => {
                self.move_right();
                InputResult::Consumed
            }
            SpecialKey::Up => {
                self.move_up();
                InputResult::Consumed
            }
            SpecialKey::Down => {
                self.move_down();
                InputResult::Consumed
            }
            SpecialKey::Tab => {
                self.move_to_next_slot();
                InputResult::Consumed
            }
            SpecialKey::ShiftTab => {
                self.move_to_prev_slot();
                InputResult::Consumed
            }
            SpecialKey::Enter => InputResult::Evaluate,
            SpecialKey::Escape => {
                if self.command_buffer.is_some() {
                    self.command_buffer = None;
                    InputResult::Consumed
                } else {
                    InputResult::Cancel
                }
            }
            SpecialKey::Backspace => {
                if let Some(ref mut buf) = self.command_buffer {
                    buf.pop();
                    if buf.is_empty() {
                        self.command_buffer = None;
                    }
                    InputResult::Consumed
                } else {
                    self.delete_at_cursor();
                    InputResult::Consumed
                }
            }
            SpecialKey::Delete => {
                self.delete_forward();
                InputResult::Consumed
            }
            SpecialKey::Home => {
                self.move_to_start();
                InputResult::Consumed
            }
            SpecialKey::End => {
                self.move_to_end();
                InputResult::Consumed
            }
        }
    }

    /// Execute a command (entered via \ prefix)
    fn execute_command(&mut self, cmd: &str) -> InputResult {
        let template = match cmd.to_lowercase().as_str() {
            "sqrt" => Some(MathBox::sqrt_template()),
            "nthroot" | "root" => Some(MathBox::nthroot_template()),
            "frac" => Some(MathBox::fraction_template()),
            "int" => Some(MathBox::integral_template()),
            "dint" | "defint" => Some(MathBox::definite_integral_template()),
            "ddx" | "diff" | "deriv" => Some(MathBox::derivative_template()),
            "lim" | "limit" => Some(MathBox::limit_template()),
            "sum" => Some(MathBox::sum_template()),
            "prod" | "product" => Some(MathBox::product_template()),
            "abs" => Some(MathBox::Abs(Box::new(MathBox::Slot))),
            "pi" => Some(MathBox::Symbol("π".to_string())),
            "theta" => Some(MathBox::Symbol("θ".to_string())),
            "alpha" => Some(MathBox::Symbol("α".to_string())),
            "beta" => Some(MathBox::Symbol("β".to_string())),
            "gamma" => Some(MathBox::Symbol("γ".to_string())),
            "delta" => Some(MathBox::Symbol("δ".to_string())),
            "epsilon" => Some(MathBox::Symbol("ε".to_string())),
            "lambda" => Some(MathBox::Symbol("λ".to_string())),
            "mu" => Some(MathBox::Symbol("μ".to_string())),
            "sigma" => Some(MathBox::Symbol("σ".to_string())),
            "omega" => Some(MathBox::Symbol("ω".to_string())),
            "inf" | "infinity" => Some(MathBox::Symbol("∞".to_string())),
            "sin" | "cos" | "tan" | "ln" | "log" | "exp" => {
                Some(MathBox::Func {
                    name: cmd.to_lowercase(),
                    args: vec![MathBox::Slot],
                })
            }
            "matrix" => Some(MathBox::matrix_template(2, 2)),
            _ => None,
        };

        if let Some(t) = template {
            self.insert_template(t);
            InputResult::Consumed
        } else {
            InputResult::Ignored
        }
    }

    /// Insert a template at cursor position
    fn insert_template(&mut self, template: MathBox) {
        // Replace current slot or insert at cursor
        if let Some(current) = self.get_current_mut() {
            if current.is_slot() {
                *current = template;
                // Move cursor into first child if it's a container
                if self.get_current().map(|c| c.child_count() > 0).unwrap_or(false) {
                    self.cursor.enter(0);
                }
            } else {
                // Insert after current
                self.insert_after_current(template);
            }
        }
    }

    /// Wrap the current element in a power
    fn wrap_in_power(&mut self) {
        if let Some(current) = self.get_current_mut() {
            if !current.is_slot() {
                let base = std::mem::replace(current, MathBox::Slot);
                *current = MathBox::Power {
                    base: Box::new(base),
                    exp: Box::new(MathBox::Slot),
                };
                // Move to exponent slot
                self.cursor.enter(1);
            } else {
                // Insert power with slot base
                *current = MathBox::Power {
                    base: Box::new(MathBox::Slot),
                    exp: Box::new(MathBox::Slot),
                };
                self.cursor.enter(1);
            }
        }
    }

    /// Wrap the current element in a subscript
    fn wrap_in_subscript(&mut self) {
        if let Some(current) = self.get_current_mut() {
            if !current.is_slot() {
                let base = std::mem::replace(current, MathBox::Slot);
                *current = MathBox::Subscript {
                    base: Box::new(base),
                    sub: Box::new(MathBox::Slot),
                };
                self.cursor.enter(1);
            } else {
                *current = MathBox::Subscript {
                    base: Box::new(MathBox::Slot),
                    sub: Box::new(MathBox::Slot),
                };
                self.cursor.enter(1);
            }
        }
    }

    /// Append a digit to the current number
    fn append_to_number(&mut self, ch: char) {
        if let Some(current) = self.get_current_mut() {
            match current {
                MathBox::Number(s) => {
                    s.push(ch);
                }
                MathBox::Slot => {
                    *current = MathBox::Number(ch.to_string());
                }
                _ => {
                    // Insert after current
                    self.insert_after_current(MathBox::Number(ch.to_string()));
                }
            }
        }
    }

    /// Append a character to the current symbol
    fn append_to_symbol(&mut self, ch: char) {
        if let Some(current) = self.get_current_mut() {
            match current {
                MathBox::Symbol(s) => {
                    s.push(ch);
                }
                MathBox::Slot => {
                    *current = MathBox::Symbol(ch.to_string());
                }
                _ => {
                    self.insert_after_current(MathBox::Symbol(ch.to_string()));
                }
            }
        }
    }

    /// Insert an element at the cursor position
    fn insert_at_cursor(&mut self, element: MathBox) {
        if let Some(current) = self.get_current_mut() {
            if current.is_slot() {
                *current = element;
            } else {
                self.insert_after_current(element);
            }
        }
    }

    /// Insert an element after the current one (in a Row)
    fn insert_after_current(&mut self, element: MathBox) {
        // This is complex - need to handle row insertion
        // For now, simplified implementation
        if let Some(current) = self.get_current_mut() {
            if matches!(current, MathBox::Row(_)) {
                if let MathBox::Row(items) = current {
                    items.push(element);
                }
            }
        }
    }

    /// Try to exit current container (parens, etc.)
    fn try_exit_container(&mut self) {
        if !self.cursor.is_at_root() {
            self.cursor.exit();
        }
    }

    /// Delete at cursor
    fn delete_at_cursor(&mut self) {
        if let Some(current) = self.get_current_mut() {
            match current {
                MathBox::Number(s) if !s.is_empty() => {
                    s.pop();
                    if s.is_empty() {
                        *current = MathBox::Slot;
                    }
                }
                MathBox::Symbol(s) if !s.is_empty() => {
                    s.pop();
                    if s.is_empty() {
                        *current = MathBox::Slot;
                    }
                }
                _ => {
                    // Replace with slot or exit
                    if !self.cursor.is_at_root() {
                        self.cursor.exit();
                    }
                }
            }
        }
    }

    /// Delete forward
    fn delete_forward(&mut self) {
        // For now, same as backspace
        self.delete_at_cursor();
    }

    /// Move cursor left
    fn move_left(&mut self) {
        if self.cursor.offset > 0 {
            self.cursor.offset -= 1;
        } else if !self.cursor.is_at_root() {
            // Exit current and try to move to previous sibling
            if let Some(idx) = self.cursor.exit() {
                if idx > 0 {
                    self.cursor.enter(idx - 1);
                    // Move to end of new element
                    self.move_to_end_of_current();
                }
            }
        }
    }

    /// Move cursor right
    fn move_right(&mut self) {
        if let Some(current) = self.get_current() {
            match current {
                MathBox::Number(s) | MathBox::Symbol(s) if self.cursor.offset < s.len() => {
                    self.cursor.offset += 1;
                }
                _ => {
                    // Try to enter first child or move to next sibling
                    if current.child_count() > 0 && self.cursor.offset == 0 {
                        self.cursor.enter(0);
                    } else if !self.cursor.is_at_root() {
                        if let Some(idx) = self.cursor.exit() {
                            let parent = self.get_current();
                            if let Some(p) = parent {
                                if idx + 1 < p.child_count() {
                                    self.cursor.enter(idx + 1);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Move cursor up (for fractions, powers)
    fn move_up(&mut self) {
        // Check if parent is a fraction and we're in denominator
        if self.cursor.path.len() >= 1 {
            let last_idx = self.cursor.path[self.cursor.path.len() - 1];
            self.cursor.exit();
            if let Some(parent) = self.get_current() {
                match parent {
                    MathBox::Fraction { .. } if last_idx == 1 => {
                        // Move from denominator to numerator
                        self.cursor.enter(0);
                    }
                    MathBox::Power { .. } if last_idx == 0 => {
                        // Move from base to exponent
                        self.cursor.enter(1);
                    }
                    _ => {
                        // Restore position
                        self.cursor.enter(last_idx);
                    }
                }
            }
        }
    }

    /// Move cursor down (for fractions)
    fn move_down(&mut self) {
        if self.cursor.path.len() >= 1 {
            let last_idx = self.cursor.path[self.cursor.path.len() - 1];
            self.cursor.exit();
            if let Some(parent) = self.get_current() {
                match parent {
                    MathBox::Fraction { .. } if last_idx == 0 => {
                        // Move from numerator to denominator
                        self.cursor.enter(1);
                    }
                    MathBox::Power { .. } if last_idx == 1 => {
                        // Move from exponent to base
                        self.cursor.enter(0);
                    }
                    _ => {
                        self.cursor.enter(last_idx);
                    }
                }
            }
        }
    }

    /// Move to next slot (Tab)
    fn move_to_next_slot(&mut self) {
        // Simple: try next sibling, or exit and try next
        if let Some(current) = self.get_current() {
            if current.child_count() > 0 {
                self.cursor.enter(0);
                return;
            }
        }

        if !self.cursor.is_at_root() {
            if let Some(idx) = self.cursor.exit() {
                if let Some(parent) = self.get_current() {
                    if idx + 1 < parent.child_count() {
                        self.cursor.enter(idx + 1);
                    }
                }
            }
        }
    }

    /// Move to previous slot (Shift+Tab)
    fn move_to_prev_slot(&mut self) {
        if !self.cursor.is_at_root() {
            if let Some(idx) = self.cursor.exit() {
                if idx > 0 {
                    self.cursor.enter(idx - 1);
                    self.move_to_end_of_current();
                }
            }
        }
    }

    /// Move cursor to start
    fn move_to_start(&mut self) {
        self.cursor = Cursor::new();
    }

    /// Move cursor to end
    fn move_to_end(&mut self) {
        self.cursor = Cursor::new();
        self.move_to_end_of_current();
    }

    /// Move to end of current element
    fn move_to_end_of_current(&mut self) {
        if let Some(current) = self.get_current() {
            match current {
                MathBox::Number(s) | MathBox::Symbol(s) => {
                    self.cursor.offset = s.len();
                }
                _ if current.child_count() > 0 => {
                    self.cursor.enter(current.child_count() - 1);
                    self.move_to_end_of_current();
                }
                _ => {}
            }
        }
    }

    /// Get the current element at cursor
    fn get_current(&self) -> Option<&MathBox> {
        let mut current = &self.root;
        for &idx in &self.cursor.path {
            current = current.child(idx)?;
        }
        Some(current)
    }

    /// Get mutable reference to current element
    fn get_current_mut(&mut self) -> Option<&mut MathBox> {
        let mut current = &mut self.root;
        for &idx in &self.cursor.path {
            current = current.child_mut(idx)?;
        }
        Some(current)
    }

    /// Get the root MathBox
    pub fn mathbox(&self) -> &MathBox {
        &self.root
    }

    /// Get the cursor path for rendering
    pub fn cursor_path(&self) -> &[usize] {
        &self.cursor.path
    }
}

/// Special keys that can be handled
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
    Left,
    Right,
    Up,
    Down,
    Tab,
    ShiftTab,
    Enter,
    Escape,
    Backspace,
    Delete,
    Home,
    End,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_input() {
        let input = MathInput::new();
        assert!(matches!(input.root, MathBox::Row(_)));
    }

    #[test]
    fn test_number_input() {
        let mut input = MathInput::new();
        input.handle_char('1');
        input.handle_char('2');
        input.handle_char('3');

        // Should have "123" as number
        if let MathBox::Row(items) = &input.root {
            if let MathBox::Number(s) = &items[0] {
                assert_eq!(s, "123");
            } else {
                panic!("Expected Number");
            }
        }
    }

    #[test]
    fn test_fraction_input() {
        let mut input = MathInput::new();
        input.handle_char('/');

        // Should have fraction template
        if let MathBox::Row(items) = &input.root {
            assert!(matches!(items[0], MathBox::Fraction { .. }));
        }
    }

    #[test]
    fn test_command_mode() {
        let mut input = MathInput::new();
        input.handle_char('\\');
        assert!(input.command_buffer.is_some());

        input.handle_char('s');
        input.handle_char('q');
        input.handle_char('r');
        input.handle_char('t');
        input.handle_char(' '); // Execute command

        // Should have sqrt template
        if let MathBox::Row(items) = &input.root {
            assert!(matches!(items[0], MathBox::Root { index: None, .. }));
        }
    }
}
