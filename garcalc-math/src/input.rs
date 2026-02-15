//! Keyboard input handling for math input
//!
//! Provides structured keyboard navigation through a MathBox tree
//! with support for template insertion via commands.

use crate::mathbox::{Cursor, MathBox, Operator};

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
    fn char_count(s: &str) -> usize {
        s.chars().count()
    }

    fn byte_index_at_char(s: &str, char_idx: usize) -> usize {
        let target = char_idx.min(Self::char_count(s));
        if target == 0 {
            return 0;
        }
        s.char_indices()
            .nth(target)
            .map(|(idx, _)| idx)
            .unwrap_or_else(|| s.len())
    }

    fn remove_char_at(s: &mut String, char_idx: usize) -> bool {
        let len = Self::char_count(s);
        if char_idx >= len {
            return false;
        }
        let start = Self::byte_index_at_char(s, char_idx);
        let end = Self::byte_index_at_char(s, char_idx + 1);
        s.drain(start..end);
        true
    }

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
            '\\' | ':' => {
                // Enter command mode
                self.command_buffer = Some(String::new());
                InputResult::Consumed
            }
            '/' => {
                self.maybe_promote_out_of_script();
                // Insert fraction
                self.insert_template(MathBox::fraction_template());
                InputResult::Consumed
            }
            '^' => {
                // Convert current element to power base
                self.wrap_in_power();
                InputResult::Consumed
            }
            '!' => {
                // Apply factorial to current element
                self.wrap_in_factorial();
                InputResult::Consumed
            }
            '_' => {
                // Convert current element to subscript base
                self.wrap_in_subscript();
                InputResult::Consumed
            }
            '(' => {
                if !self.wrap_current_symbol_as_function_call() {
                    self.insert_at_cursor(MathBox::Parens(Box::new(MathBox::Slot)));
                    self.cursor.enter(0);
                }
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
                self.maybe_promote_out_of_script();
                self.insert_at_cursor(MathBox::Operator(Operator::Add));
                InputResult::Consumed
            }
            '-' => {
                self.maybe_promote_out_of_script();
                self.insert_at_cursor(MathBox::Operator(Operator::Sub));
                InputResult::Consumed
            }
            '*' => {
                self.maybe_promote_out_of_script();
                self.insert_at_cursor(MathBox::Operator(Operator::Mul));
                InputResult::Consumed
            }
            '=' => {
                self.maybe_promote_out_of_script();
                self.insert_at_cursor(MathBox::Operator(Operator::Eq));
                InputResult::Consumed
            }
            '<' => {
                self.maybe_promote_out_of_script();
                self.insert_at_cursor(MathBox::Operator(Operator::Lt));
                InputResult::Consumed
            }
            '>' => {
                self.maybe_promote_out_of_script();
                self.insert_at_cursor(MathBox::Operator(Operator::Gt));
                InputResult::Consumed
            }
            ',' => {
                self.maybe_promote_out_of_script();
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
        if self.command_buffer.is_some() {
            match key {
                SpecialKey::Enter => {
                    if let Some(cmd) = self.command_buffer.take() {
                        if cmd.is_empty() {
                            return InputResult::Consumed;
                        }
                        return self.execute_command(&cmd);
                    }
                    return InputResult::Ignored;
                }
                SpecialKey::Escape => {
                    self.command_buffer = None;
                    return InputResult::Consumed;
                }
                SpecialKey::Backspace => {
                    if let Some(ref mut buf) = self.command_buffer {
                        buf.pop();
                        if buf.is_empty() {
                            self.command_buffer = None;
                        }
                    }
                    return InputResult::Consumed;
                }
                _ => {
                    // For navigation/editing keys, leave command mode and continue handling the key.
                    self.command_buffer = None;
                }
            }
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
            SpecialKey::Escape => InputResult::Cancel,
            SpecialKey::Backspace => {
                self.delete_at_cursor();
                InputResult::Consumed
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
            "solve" => Some(MathBox::Func {
                name: "solve".to_string(),
                args: vec![MathBox::Slot, MathBox::Symbol("x".to_string())],
            }),
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
            "sin" | "cos" | "tan" | "ln" | "log" | "exp" => Some(MathBox::Func {
                name: cmd.to_lowercase(),
                args: vec![MathBox::Slot],
            }),
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
                if self
                    .get_current()
                    .map(|c| c.child_count() > 0)
                    .unwrap_or(false)
                {
                    self.focus_first_slot_in_current_subtree();
                }
            } else {
                // Insert after current
                if self.insert_after_current(template)
                    && self
                        .get_current()
                        .map(|c| c.child_count() > 0)
                        .unwrap_or(false)
                {
                    self.focus_first_slot_in_current_subtree();
                }
            }
        }
    }

    /// Wrap the current element in a power
    fn wrap_in_power(&mut self) {
        let current_path = self.cursor.path.clone();
        let row_offset = self.cursor.offset;
        let mut row_target = None;
        {
            if let Some(MathBox::Row(items)) =
                Self::get_node_mut_at_path(&mut self.root, &current_path)
            {
                let idx = row_offset.min(items.len());
                let prev_is_operator =
                    idx > 0 && matches!(items.get(idx - 1), Some(MathBox::Operator(_)));
                if idx > 0 && !prev_is_operator {
                    let base = std::mem::replace(&mut items[idx - 1], MathBox::Slot);
                    items[idx - 1] = MathBox::Power {
                        base: Box::new(base),
                        exp: Box::new(MathBox::Slot),
                    };
                    row_target = Some(idx - 1);
                } else {
                    items.insert(
                        idx,
                        MathBox::Power {
                            base: Box::new(MathBox::Slot),
                            exp: Box::new(MathBox::Slot),
                        },
                    );
                    row_target = Some(idx);
                }
            }
        }
        if let Some(item_idx) = row_target {
            self.cursor.path = current_path;
            self.cursor.enter(item_idx);
            self.cursor.enter(1);
            return;
        }

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
        let current_path = self.cursor.path.clone();
        let row_offset = self.cursor.offset;
        let mut row_target = None;
        {
            if let Some(MathBox::Row(items)) =
                Self::get_node_mut_at_path(&mut self.root, &current_path)
            {
                let idx = row_offset.min(items.len());
                let prev_is_operator =
                    idx > 0 && matches!(items.get(idx - 1), Some(MathBox::Operator(_)));
                if idx > 0 && !prev_is_operator {
                    let base = std::mem::replace(&mut items[idx - 1], MathBox::Slot);
                    items[idx - 1] = MathBox::Subscript {
                        base: Box::new(base),
                        sub: Box::new(MathBox::Slot),
                    };
                    row_target = Some(idx - 1);
                } else {
                    items.insert(
                        idx,
                        MathBox::Subscript {
                            base: Box::new(MathBox::Slot),
                            sub: Box::new(MathBox::Slot),
                        },
                    );
                    row_target = Some(idx);
                }
            }
        }
        if let Some(item_idx) = row_target {
            self.cursor.path = current_path;
            self.cursor.enter(item_idx);
            self.cursor.enter(1);
            return;
        }

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

    /// Wrap the current element in factorial
    fn wrap_in_factorial(&mut self) {
        let current_path = self.cursor.path.clone();
        let row_offset = self.cursor.offset;
        enum RowFactorialTarget {
            Wrapped(usize),
            Inserted(usize),
        }
        let mut row_target = None;
        {
            if let Some(MathBox::Row(items)) =
                Self::get_node_mut_at_path(&mut self.root, &current_path)
            {
                let idx = row_offset.min(items.len());
                let prev_is_operator =
                    idx > 0 && matches!(items.get(idx - 1), Some(MathBox::Operator(_)));
                if idx > 0 && !prev_is_operator {
                    let arg = std::mem::replace(&mut items[idx - 1], MathBox::Slot);
                    items[idx - 1] = MathBox::Func {
                        name: "factorial".to_string(),
                        args: vec![arg],
                    };
                    row_target = Some(RowFactorialTarget::Wrapped(idx - 1));
                } else {
                    items.insert(
                        idx,
                        MathBox::Func {
                            name: "factorial".to_string(),
                            args: vec![MathBox::Slot],
                        },
                    );
                    row_target = Some(RowFactorialTarget::Inserted(idx));
                }
            }
        }
        if let Some(target) = row_target {
            self.cursor.path = current_path;
            match target {
                RowFactorialTarget::Wrapped(item_idx) => {
                    self.cursor.enter(item_idx);
                    self.cursor.offset = 0;
                }
                RowFactorialTarget::Inserted(item_idx) => {
                    self.cursor.enter(item_idx);
                    self.cursor.enter(0);
                }
            }
            return;
        }

        if let Some(current) = self.get_current_mut() {
            if !current.is_slot() {
                let arg = std::mem::replace(current, MathBox::Slot);
                *current = MathBox::Func {
                    name: "factorial".to_string(),
                    args: vec![arg],
                };
                self.cursor.offset = 0;
            } else {
                *current = MathBox::Func {
                    name: "factorial".to_string(),
                    args: vec![MathBox::Slot],
                };
                self.cursor.enter(0);
            }
        }
    }

    /// Append a digit to the current number
    fn append_to_number(&mut self, ch: char) {
        let offset = self.cursor.offset;
        let mut new_offset = None;
        if let Some(current) = self.get_current_mut() {
            match current {
                MathBox::Number(s) => {
                    let insert_at = Self::byte_index_at_char(s, offset);
                    s.insert(insert_at, ch);
                    new_offset = Some(offset + 1);
                }
                MathBox::Slot => {
                    *current = MathBox::Number(ch.to_string());
                    new_offset = Some(1);
                }
                _ => {
                    if self.insert_after_current(MathBox::Number(ch.to_string())) {
                        new_offset = Some(1);
                    }
                }
            }
        }
        if let Some(new_offset) = new_offset {
            self.cursor.offset = new_offset;
        }
    }

    /// Append a character to the current symbol
    fn append_to_symbol(&mut self, ch: char) {
        let offset = self.cursor.offset;
        let mut new_offset = None;
        if let Some(current) = self.get_current_mut() {
            match current {
                MathBox::Symbol(s) => {
                    let insert_at = Self::byte_index_at_char(s, offset);
                    s.insert(insert_at, ch);
                    new_offset = Some(offset + 1);
                }
                MathBox::Slot => {
                    *current = MathBox::Symbol(ch.to_string());
                    new_offset = Some(1);
                }
                _ => {
                    if self.insert_after_current(MathBox::Symbol(ch.to_string())) {
                        new_offset = Some(1);
                    }
                }
            }
        }
        if let Some(new_offset) = new_offset {
            self.cursor.offset = new_offset;
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
    fn insert_after_current(&mut self, element: MathBox) -> bool {
        let current_path = self.cursor.path.clone();

        // Case 1: cursor is on a Row node; insert at the row offset.
        if let Some(MathBox::Row(items)) = Self::get_node_mut_at_path(&mut self.root, &current_path)
        {
            let insert_idx = self.cursor.offset.min(items.len());
            items.insert(insert_idx, element);
            self.cursor.path = current_path;
            self.cursor.enter(insert_idx);
            return true;
        }

        // Case 2: parent is a Row; insert as next sibling.
        if let Some((&idx, parent_path)) = current_path.split_last() {
            if let Some(MathBox::Row(items)) =
                Self::get_node_mut_at_path(&mut self.root, parent_path)
            {
                let insert_idx = (idx + 1).min(items.len());
                items.insert(insert_idx, element);
                self.cursor.path = parent_path.to_vec();
                self.cursor.enter(insert_idx);
                return true;
            }
        }

        // Case 3: no row context; wrap current node into a Row and append.
        if let Some(current) = Self::get_node_mut_at_path(&mut self.root, &current_path) {
            let old = std::mem::replace(current, MathBox::Slot);
            *current = MathBox::Row(vec![old, element]);
            self.cursor.path = current_path;
            self.cursor.enter(1);
            return true;
        }

        false
    }

    fn wrap_current_symbol_as_function_call(&mut self) -> bool {
        let path = self.cursor.path.clone();
        let symbol_name = match Self::get_node_at_path(&self.root, &path) {
            Some(MathBox::Symbol(name)) => name.clone(),
            _ => return false,
        };

        let normalized = symbol_name.to_ascii_lowercase();
        if !Self::is_known_function_name(&normalized) {
            return false;
        }

        if let Some(node) = Self::get_node_mut_at_path(&mut self.root, &path) {
            *node = MathBox::Func {
                name: normalized,
                args: vec![MathBox::Slot],
            };
            self.cursor.enter(0);
            self.cursor.offset = 0;
            true
        } else {
            false
        }
    }

    fn is_known_function_name(name: &str) -> bool {
        matches!(
            name,
            "sin"
                | "cos"
                | "tan"
                | "cot"
                | "sec"
                | "csc"
                | "asin"
                | "acos"
                | "atan"
                | "sinh"
                | "cosh"
                | "tanh"
                | "asinh"
                | "acosh"
                | "atanh"
                | "ln"
                | "log"
                | "log10"
                | "log2"
                | "exp"
                | "sqrt"
                | "cbrt"
                | "abs"
                | "floor"
                | "ceil"
                | "round"
                | "trunc"
                | "sign"
                | "gamma"
                | "factorial"
                | "diff"
                | "derivative"
                | "integrate"
                | "integral"
                | "limit"
                | "lim"
                | "solve"
                | "sum"
                | "product"
                | "prod"
                | "simplify"
                | "expand"
                | "factor"
                | "substitute"
                | "subs"
                | "min"
                | "max"
                | "gcd"
                | "lcm"
                | "det"
                | "determinant"
                | "inv"
                | "inverse"
                | "transpose"
                | "trace"
                | "matmul"
                | "identity"
        )
    }

    /// Try to exit current container (parens, etc.)
    fn try_exit_container(&mut self) {
        if !self.cursor.is_at_root() {
            self.cursor.exit();
        }
    }

    fn cursor_is_at_end_of_current(&self) -> bool {
        match self.get_current() {
            Some(MathBox::Number(s)) | Some(MathBox::Symbol(s)) => {
                self.cursor.offset >= Self::char_count(s)
            }
            Some(MathBox::Row(items)) => self.cursor.offset >= items.len(),
            Some(_) => true,
            None => false,
        }
    }

    /// Promote cursor out of exponent/subscript when typing operators at script end.
    fn maybe_promote_out_of_script(&mut self) -> bool {
        if !self.cursor_is_at_end_of_current() {
            return false;
        }

        let Some((&child_idx, parent_path)) = self.cursor.path.split_last() else {
            return false;
        };
        if child_idx != 1 {
            return false;
        }

        let is_script = matches!(
            Self::get_node_at_path(&self.root, parent_path),
            Some(MathBox::Power { .. }) | Some(MathBox::Subscript { .. })
        );
        if !is_script {
            return false;
        }

        self.cursor.path = parent_path.to_vec();
        self.cursor.offset = 0;
        true
    }

    /// Delete at cursor
    fn delete_at_cursor(&mut self) {
        let path = self.cursor.path.clone();
        let offset = self.cursor.offset;

        match self.get_current() {
            Some(MathBox::Number(_)) => {
                if offset > 0 {
                    let mut became_slot = false;
                    let mut removed = false;
                    if let Some(MathBox::Number(s)) =
                        Self::get_node_mut_at_path(&mut self.root, &path)
                    {
                        removed = Self::remove_char_at(s, offset - 1);
                        if s.is_empty() {
                            became_slot = true;
                        }
                    }
                    if became_slot {
                        if let Some(current) = Self::get_node_mut_at_path(&mut self.root, &path) {
                            *current = MathBox::Slot;
                        }
                        self.cursor.offset = 0;
                    } else if removed {
                        self.cursor.offset = offset - 1;
                    }
                } else {
                    self.delete_previous_from_path(&path);
                }
            }
            Some(MathBox::Symbol(_)) => {
                if offset > 0 {
                    let mut became_slot = false;
                    let mut removed = false;
                    if let Some(MathBox::Symbol(s)) =
                        Self::get_node_mut_at_path(&mut self.root, &path)
                    {
                        removed = Self::remove_char_at(s, offset - 1);
                        if s.is_empty() {
                            became_slot = true;
                        }
                    }
                    if became_slot {
                        if let Some(current) = Self::get_node_mut_at_path(&mut self.root, &path) {
                            *current = MathBox::Slot;
                        }
                        self.cursor.offset = 0;
                    } else if removed {
                        self.cursor.offset = offset - 1;
                    }
                } else {
                    self.delete_previous_from_path(&path);
                }
            }
            Some(MathBox::Slot) => {
                if !self.delete_current_slot_in_row(&path) {
                    self.delete_previous_from_path(&path);
                }
            }
            Some(_) => {
                if !path.is_empty() {
                    self.replace_current_with_slot(&path);
                }
            }
            None => {}
        }
    }

    /// Delete forward
    fn delete_forward(&mut self) {
        let path = self.cursor.path.clone();
        let offset = self.cursor.offset;

        match self.get_current() {
            Some(MathBox::Number(s)) => {
                let len = Self::char_count(s);
                if offset < len {
                    let mut became_slot = false;
                    if let Some(MathBox::Number(cur)) =
                        Self::get_node_mut_at_path(&mut self.root, &path)
                    {
                        Self::remove_char_at(cur, offset);
                        if cur.is_empty() {
                            became_slot = true;
                        }
                    }
                    if became_slot {
                        if let Some(current) = Self::get_node_mut_at_path(&mut self.root, &path) {
                            *current = MathBox::Slot;
                        }
                        self.cursor.offset = 0;
                    }
                } else {
                    self.delete_next_from_path(&path);
                }
            }
            Some(MathBox::Symbol(s)) => {
                let len = Self::char_count(s);
                if offset < len {
                    let mut became_slot = false;
                    if let Some(MathBox::Symbol(cur)) =
                        Self::get_node_mut_at_path(&mut self.root, &path)
                    {
                        Self::remove_char_at(cur, offset);
                        if cur.is_empty() {
                            became_slot = true;
                        }
                    }
                    if became_slot {
                        if let Some(current) = Self::get_node_mut_at_path(&mut self.root, &path) {
                            *current = MathBox::Slot;
                        }
                        self.cursor.offset = 0;
                    }
                } else {
                    self.delete_next_from_path(&path);
                }
            }
            Some(MathBox::Slot) => {
                self.delete_next_from_path(&path);
            }
            Some(_) => {
                if !path.is_empty() {
                    self.replace_current_with_slot(&path);
                }
            }
            None => {}
        }
    }

    /// Move cursor left
    fn move_left(&mut self) {
        let row_prev_idx = match self.get_current() {
            Some(MathBox::Row(items)) if self.cursor.offset > 0 => {
                Some(self.cursor.offset.min(items.len()) - 1)
            }
            _ => None,
        };
        if let Some(prev_idx) = row_prev_idx {
            self.cursor.enter(prev_idx);
            self.move_to_end_of_current();
            return;
        }

        if let Some(current) = self.get_current() {
            if matches!(current, MathBox::Number(_) | MathBox::Symbol(_)) && self.cursor.offset > 0
            {
                self.cursor.offset -= 1;
                return;
            }
        }

        let mut path = self.cursor.path.clone();
        while let Some(idx) = path.pop() {
            if idx > 0 {
                path.push(idx - 1);
                self.cursor.path = path;
                self.cursor.offset = 0;
                self.move_to_end_of_current();
                return;
            }
        }
    }

    /// Move cursor right
    fn move_right(&mut self) {
        let row_next_idx = match self.get_current() {
            Some(MathBox::Row(items)) if self.cursor.offset < items.len() => {
                Some(self.cursor.offset)
            }
            _ => None,
        };
        if let Some(next_idx) = row_next_idx {
            self.cursor.enter(next_idx);
            return;
        }

        if let Some(current) = self.get_current() {
            match current {
                MathBox::Number(s) | MathBox::Symbol(s)
                    if self.cursor.offset < Self::char_count(s) =>
                {
                    self.cursor.offset += 1;
                    return;
                }
                // Row uses cursor.offset as an insertion index, so at row boundaries
                // we should climb out/advance rather than re-enter row child 0.
                MathBox::Row(_) => {}
                _ if current.child_count() > 0 => {
                    self.cursor.enter(0);
                    return;
                }
                _ => {}
            }
        }

        let original_path = self.cursor.path.clone();
        let mut path = original_path.clone();
        while let Some(idx) = path.pop() {
            if let Some(parent) = Self::get_node_at_path(&self.root, &path) {
                if idx + 1 < parent.child_count() {
                    path.push(idx + 1);
                    self.cursor.path = path;
                    self.cursor.offset = 0;
                    return;
                }
            }
        }

        if let Some((row_path, insert_offset)) = self.row_insertion_after_path(&original_path) {
            self.cursor.path = row_path;
            self.cursor.offset = insert_offset;
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
        let slots = self.collect_slot_paths();
        if slots.is_empty() {
            return;
        }

        let current_path = self.cursor.path.clone();
        if let Some(idx) = slots.iter().position(|p| p == &current_path) {
            self.cursor.path = slots[(idx + 1) % slots.len()].clone();
            self.cursor.offset = 0;
            return;
        }

        let ordered_paths = self.collect_node_paths();
        let target =
            if let Some(current_idx) = ordered_paths.iter().position(|p| p == &current_path) {
                slots
                    .iter()
                    .find(|slot| {
                        ordered_paths
                            .iter()
                            .position(|p| p == *slot)
                            .is_some_and(|slot_idx| slot_idx > current_idx)
                    })
                    .cloned()
                    .unwrap_or_else(|| slots[0].clone())
            } else {
                slots[0].clone()
            };

        self.cursor.path = target;
        self.cursor.offset = 0;
    }

    /// Move to previous slot (Shift+Tab)
    fn move_to_prev_slot(&mut self) {
        let slots = self.collect_slot_paths();
        if slots.is_empty() {
            return;
        }

        let current_path = self.cursor.path.clone();
        if let Some(idx) = slots.iter().position(|p| p == &current_path) {
            self.cursor.path = if idx == 0 {
                slots[slots.len() - 1].clone()
            } else {
                slots[idx - 1].clone()
            };
            self.cursor.offset = 0;
            return;
        }

        let ordered_paths = self.collect_node_paths();
        let target =
            if let Some(current_idx) = ordered_paths.iter().position(|p| p == &current_path) {
                slots
                    .iter()
                    .rev()
                    .find(|slot| {
                        ordered_paths
                            .iter()
                            .position(|p| p == *slot)
                            .is_some_and(|slot_idx| slot_idx < current_idx)
                    })
                    .cloned()
                    .unwrap_or_else(|| slots[slots.len() - 1].clone())
            } else {
                slots[slots.len() - 1].clone()
            };

        self.cursor.path = target;
        self.cursor.offset = 0;
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
                    self.cursor.offset = Self::char_count(s);
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
        Self::get_node_at_path(&self.root, &self.cursor.path)
    }

    /// Get mutable reference to current element
    fn get_current_mut(&mut self) -> Option<&mut MathBox> {
        Self::get_node_mut_at_path(&mut self.root, &self.cursor.path)
    }

    /// Get the root MathBox
    pub fn mathbox(&self) -> &MathBox {
        &self.root
    }

    /// Get the cursor path for rendering
    pub fn cursor_path(&self) -> &[usize] {
        &self.cursor.path
    }

    /// Get character offset within the currently focused token
    pub fn cursor_offset(&self) -> usize {
        self.cursor.offset
    }

    fn get_node_at_path<'a>(root: &'a MathBox, path: &[usize]) -> Option<&'a MathBox> {
        let mut current = root;
        for &idx in path {
            current = current.child(idx)?;
        }
        Some(current)
    }

    fn get_node_mut_at_path<'a>(root: &'a mut MathBox, path: &[usize]) -> Option<&'a mut MathBox> {
        let mut current = root;
        for &idx in path {
            current = current.child_mut(idx)?;
        }
        Some(current)
    }

    fn collect_slot_paths(&self) -> Vec<Vec<usize>> {
        fn walk(node: &MathBox, path: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
            if matches!(node, MathBox::Slot) {
                out.push(path.clone());
                return;
            }
            for i in MathInput::ordered_child_indices(node) {
                if let Some(child) = node.child(i) {
                    path.push(i);
                    walk(child, path, out);
                    path.pop();
                }
            }
        }

        let mut out = Vec::new();
        let mut path = Vec::new();
        walk(&self.root, &mut path, &mut out);
        out
    }

    fn collect_node_paths(&self) -> Vec<Vec<usize>> {
        fn walk(node: &MathBox, path: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
            out.push(path.clone());
            for i in MathInput::ordered_child_indices(node) {
                if let Some(child) = node.child(i) {
                    path.push(i);
                    walk(child, path, out);
                    path.pop();
                }
            }
        }

        let mut out = Vec::new();
        let mut path = Vec::new();
        walk(&self.root, &mut path, &mut out);
        out
    }

    fn ordered_child_indices(node: &MathBox) -> Vec<usize> {
        match node {
            MathBox::Power { .. } => vec![0, 1],
            MathBox::Derivative { .. } => vec![0, 1],
            MathBox::Sum { .. } | MathBox::Product { .. } => vec![1, 2, 3, 0],
            MathBox::Integral { lower, upper, .. } => {
                let has_lower = lower.is_some();
                let has_upper = upper.is_some();
                let mut indices = Vec::new();
                if has_upper {
                    indices.push(if has_lower { 1 } else { 0 });
                }
                if has_lower {
                    indices.push(0);
                }
                let body_idx = usize::from(has_lower) + usize::from(has_upper);
                indices.push(body_idx);
                indices.push(body_idx + 1); // variable slot/symbol
                indices
            }
            _ => (0..node.child_count()).collect(),
        }
    }

    fn focus_first_slot_in_current_subtree(&mut self) {
        let base = self.cursor.path.clone();
        let slots = self.collect_slot_paths();
        if let Some(path) = slots
            .into_iter()
            .find(|p| p.len() > base.len() && p.starts_with(&base))
        {
            self.cursor.path = path;
            self.cursor.offset = 0;
        }
    }

    fn previous_sibling_path(&self, path: &[usize]) -> Option<Vec<usize>> {
        let mut cursor = path.to_vec();
        while let Some(idx) = cursor.pop() {
            if idx > 0 {
                cursor.push(idx - 1);
                return Some(cursor);
            }
        }
        None
    }

    fn next_sibling_path(&self, path: &[usize]) -> Option<Vec<usize>> {
        let mut cursor = path.to_vec();
        while let Some(idx) = cursor.pop() {
            if let Some(parent) = Self::get_node_at_path(&self.root, &cursor) {
                if idx + 1 < parent.child_count() {
                    cursor.push(idx + 1);
                    return Some(cursor);
                }
            }
        }
        None
    }

    fn row_insertion_after_path(&self, path: &[usize]) -> Option<(Vec<usize>, usize)> {
        let mut cursor = path.to_vec();
        while let Some(idx) = cursor.pop() {
            if let Some(MathBox::Row(items)) = Self::get_node_at_path(&self.root, &cursor) {
                return Some((cursor.clone(), (idx + 1).min(items.len())));
            }
        }
        None
    }

    fn replace_current_with_slot(&mut self, path: &[usize]) {
        if let Some(current) = Self::get_node_mut_at_path(&mut self.root, path) {
            *current = MathBox::Slot;
            self.cursor.path = path.to_vec();
            self.cursor.offset = 0;
        }
    }

    fn is_effectively_empty(node: &MathBox) -> bool {
        match node {
            MathBox::Slot => true,
            MathBox::Number(s) | MathBox::Symbol(s) => s.is_empty(),
            MathBox::Operator(_) => false,
            MathBox::Fraction { num, den } => {
                Self::is_effectively_empty(num) && Self::is_effectively_empty(den)
            }
            MathBox::Power { base, exp } => {
                Self::is_effectively_empty(base) && Self::is_effectively_empty(exp)
            }
            MathBox::Subscript { base, sub } => {
                Self::is_effectively_empty(base) && Self::is_effectively_empty(sub)
            }
            MathBox::Root { index, radicand } => {
                let index_empty = index
                    .as_deref()
                    .map(Self::is_effectively_empty)
                    .unwrap_or(true);
                index_empty && Self::is_effectively_empty(radicand)
            }
            MathBox::Func { args, .. } => args.iter().all(Self::is_effectively_empty),
            MathBox::Abs(inner) | MathBox::Parens(inner) => Self::is_effectively_empty(inner),
            MathBox::Integral {
                lower,
                upper,
                body,
                var,
            } => {
                let lower_empty = lower
                    .as_deref()
                    .map(Self::is_effectively_empty)
                    .unwrap_or(true);
                let upper_empty = upper
                    .as_deref()
                    .map(Self::is_effectively_empty)
                    .unwrap_or(true);
                lower_empty
                    && upper_empty
                    && Self::is_effectively_empty(body)
                    && Self::is_effectively_empty(var)
            }
            MathBox::Derivative { var, body, .. } => {
                Self::is_effectively_empty(var) && Self::is_effectively_empty(body)
            }
            MathBox::Limit { var, to, body, .. } => {
                Self::is_effectively_empty(var)
                    && Self::is_effectively_empty(to)
                    && Self::is_effectively_empty(body)
            }
            MathBox::Sum {
                var,
                lower,
                upper,
                body,
            }
            | MathBox::Product {
                var,
                lower,
                upper,
                body,
            } => {
                Self::is_effectively_empty(var)
                    && Self::is_effectively_empty(lower)
                    && Self::is_effectively_empty(upper)
                    && Self::is_effectively_empty(body)
            }
            MathBox::Matrix { rows } => rows.iter().flatten().all(Self::is_effectively_empty),
            MathBox::Row(items) => items.iter().all(Self::is_effectively_empty),
        }
    }

    fn collapse_parent_if_empty(&mut self, current_path: &[usize]) -> bool {
        let Some((_, parent_path)) = current_path.split_last() else {
            return false;
        };
        if parent_path.is_empty() {
            return false;
        }

        let should_collapse = Self::get_node_at_path(&self.root, parent_path)
            .map(Self::is_effectively_empty)
            .unwrap_or(false);
        if should_collapse {
            self.replace_current_with_slot(parent_path);
            return true;
        }
        false
    }

    fn remove_sibling_from_parent_row(
        &mut self,
        parent_path: &[usize],
        remove_idx: usize,
        cursor_idx: usize,
    ) -> bool {
        if let Some(MathBox::Row(items)) = Self::get_node_mut_at_path(&mut self.root, parent_path) {
            if remove_idx >= items.len() {
                return false;
            }

            items.remove(remove_idx);
            if items.is_empty() {
                items.push(MathBox::Slot);
            }

            let new_idx = cursor_idx.min(items.len().saturating_sub(1));
            self.cursor.path = parent_path.to_vec();
            self.cursor.enter(new_idx);
            self.cursor.offset = 0;
            return true;
        }
        false
    }

    fn delete_current_slot_in_row(&mut self, path: &[usize]) -> bool {
        let Some((&idx, parent_path)) = path.split_last() else {
            return false;
        };
        self.remove_sibling_from_parent_row(parent_path, idx, idx.saturating_sub(1))
    }

    fn delete_previous_from_path(&mut self, current_path: &[usize]) {
        if let Some(prev_path) = self.previous_sibling_path(current_path) {
            if let (Some((&cur_idx, cur_parent)), Some((&prev_idx, prev_parent))) =
                (current_path.split_last(), prev_path.split_last())
            {
                if cur_parent == prev_parent
                    && prev_idx + 1 == cur_idx
                    && self.remove_sibling_from_parent_row(
                        cur_parent,
                        prev_idx,
                        cur_idx.saturating_sub(1),
                    )
                {
                    return;
                }
            }

            if let Some(node) = Self::get_node_mut_at_path(&mut self.root, &prev_path) {
                match node {
                    MathBox::Number(s) if !s.is_empty() => {
                        let last = Self::char_count(s).saturating_sub(1);
                        Self::remove_char_at(s, last);
                        if s.is_empty() {
                            *node = MathBox::Slot;
                            self.cursor.offset = 0;
                        } else {
                            self.cursor.offset = Self::char_count(s);
                        }
                        self.cursor.path = prev_path;
                    }
                    MathBox::Symbol(s) if !s.is_empty() => {
                        let last = Self::char_count(s).saturating_sub(1);
                        Self::remove_char_at(s, last);
                        if s.is_empty() {
                            *node = MathBox::Slot;
                            self.cursor.offset = 0;
                        } else {
                            self.cursor.offset = Self::char_count(s);
                        }
                        self.cursor.path = prev_path;
                    }
                    _ => {
                        *node = MathBox::Slot;
                        self.cursor.path = prev_path;
                        self.cursor.offset = 0;
                    }
                }
            }
            return;
        }

        let _ = self.collapse_parent_if_empty(current_path);
    }

    fn delete_next_from_path(&mut self, current_path: &[usize]) {
        if let Some(next_path) = self.next_sibling_path(current_path) {
            if let (Some((&cur_idx, cur_parent)), Some((&next_idx, next_parent))) =
                (current_path.split_last(), next_path.split_last())
            {
                if cur_parent == next_parent
                    && cur_idx + 1 == next_idx
                    && self.remove_sibling_from_parent_row(cur_parent, next_idx, cur_idx)
                {
                    return;
                }
            }

            if let Some(node) = Self::get_node_mut_at_path(&mut self.root, &next_path) {
                match node {
                    MathBox::Number(s) if !s.is_empty() => {
                        Self::remove_char_at(s, 0);
                        if s.is_empty() {
                            *node = MathBox::Slot;
                        }
                    }
                    MathBox::Symbol(s) if !s.is_empty() => {
                        Self::remove_char_at(s, 0);
                        if s.is_empty() {
                            *node = MathBox::Slot;
                        }
                    }
                    _ => {
                        *node = MathBox::Slot;
                    }
                }
            }
            return;
        }

        let _ = self.collapse_parent_if_empty(current_path);
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
    use crate::convert::to_expr;

    fn run_command(input: &mut MathInput, cmd: &str) {
        input.handle_char('\\');
        for ch in cmd.chars() {
            input.handle_char(ch);
        }
        input.handle_char(' ');
    }

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
    fn test_power_input() {
        let mut input = MathInput::new();
        input.handle_char('2');
        input.handle_char('^');
        input.handle_char('2');

        if let MathBox::Row(items) = &input.root {
            if let MathBox::Power { base, exp } = &items[0] {
                assert!(matches!(base.as_ref(), MathBox::Number(s) if s == "2"));
                assert!(matches!(exp.as_ref(), MathBox::Number(s) if s == "2"));
            } else {
                panic!("Expected Power");
            }
        }
    }

    #[test]
    fn test_operator_after_exponent_promotes_outside_power() {
        let mut input = MathInput::new();
        input.handle_char('x');
        input.handle_char('^');
        input.handle_char('2');
        input.handle_char('+');
        input.handle_char('3');

        if let MathBox::Row(items) = &input.root {
            assert_eq!(items.len(), 3);
            if let MathBox::Power { base, exp } = &items[0] {
                assert!(matches!(base.as_ref(), MathBox::Symbol(s) if s == "x"));
                assert!(matches!(exp.as_ref(), MathBox::Number(s) if s == "2"));
            } else {
                panic!("Expected Power");
            }
            assert!(matches!(&items[1], MathBox::Operator(Operator::Add)));
            assert!(matches!(&items[2], MathBox::Number(s) if s == "3"));
        } else {
            panic!("Expected Row");
        }
    }

    #[test]
    fn test_right_from_exponent_end_moves_to_row_insertion_point() {
        let mut input = MathInput::new();
        input.handle_char('x');
        input.handle_char('^');
        input.handle_char('2');

        input.handle_key(SpecialKey::Right);
        assert_eq!(input.cursor_path(), &[]);
        assert_eq!(input.cursor_offset(), 1);

        input.handle_char('+');
        input.handle_char('3');

        if let MathBox::Row(items) = &input.root {
            assert_eq!(items.len(), 3);
            assert!(matches!(&items[1], MathBox::Operator(Operator::Add)));
            assert!(matches!(&items[2], MathBox::Number(s) if s == "3"));
        } else {
            panic!("Expected Row");
        }
    }

    #[test]
    fn test_right_from_parenthesized_exponent_can_exit_to_outer_row() {
        let mut input = MathInput::new();
        input.handle_char('2');
        input.handle_char('^');
        input.handle_char('(');
        input.handle_char('x');
        input.handle_char('+');
        input.handle_char('3');

        // Right #1: from number end to inner-row insertion at end.
        input.handle_key(SpecialKey::Right);
        assert_eq!(input.cursor_path(), &[0, 1, 0]);
        assert_eq!(input.cursor_offset(), 3);

        // Right #2: climb out of exponent context to outer row insertion.
        input.handle_key(SpecialKey::Right);
        assert_eq!(input.cursor_path(), &[]);
        assert_eq!(input.cursor_offset(), 1);

        input.handle_char('+');
        input.handle_char('4');

        if let MathBox::Row(items) = &input.root {
            assert_eq!(items.len(), 3);
            assert!(matches!(&items[1], MathBox::Operator(Operator::Add)));
            assert!(matches!(&items[2], MathBox::Number(s) if s == "4"));
        } else {
            panic!("Expected Row");
        }
    }

    #[test]
    fn test_subscript_after_exponent_at_row_insertion_wraps_power() {
        let mut input = MathInput::new();
        input.handle_char('2');
        input.handle_char('^');
        input.handle_char('3');

        input.handle_key(SpecialKey::Right);
        assert_eq!(input.cursor_path(), &[]);
        assert_eq!(input.cursor_offset(), 1);

        input.handle_char('_');
        input.handle_char('4');

        if let MathBox::Row(items) = &input.root {
            assert_eq!(items.len(), 1);
            if let MathBox::Subscript { base, sub } = &items[0] {
                if let MathBox::Power {
                    base: power_base,
                    exp,
                } = base.as_ref()
                {
                    assert!(matches!(power_base.as_ref(), MathBox::Number(s) if s == "2"));
                    assert!(matches!(exp.as_ref(), MathBox::Number(s) if s == "3"));
                } else {
                    panic!("Expected Power base for subscript");
                }
                assert!(matches!(sub.as_ref(), MathBox::Number(s) if s == "4"));
            } else {
                panic!("Expected Subscript");
            }
        } else {
            panic!("Expected Row");
        }
    }

    #[test]
    fn test_operator_after_subscript_promotes_outside_subscript() {
        let mut input = MathInput::new();
        input.handle_char('x');
        input.handle_char('_');
        input.handle_char('1');
        input.handle_char('+');
        input.handle_char('2');

        if let MathBox::Row(items) = &input.root {
            assert_eq!(items.len(), 3);
            if let MathBox::Subscript { base, sub } = &items[0] {
                assert!(matches!(base.as_ref(), MathBox::Symbol(s) if s == "x"));
                assert!(matches!(sub.as_ref(), MathBox::Number(s) if s == "1"));
            } else {
                panic!("Expected Subscript");
            }
            assert!(matches!(&items[1], MathBox::Operator(Operator::Add)));
            assert!(matches!(&items[2], MathBox::Number(s) if s == "2"));
        } else {
            panic!("Expected Row");
        }
    }

    #[test]
    fn test_factorial_input_wraps_current() {
        let mut input = MathInput::new();
        input.handle_char('5');
        input.handle_char('!');

        if let MathBox::Row(items) = &input.root {
            if let MathBox::Func { name, args } = &items[0] {
                assert_eq!(name, "factorial");
                assert_eq!(args.len(), 1);
                assert!(matches!(&args[0], MathBox::Number(s) if s == "5"));
            } else {
                panic!("Expected factorial function");
            }
        } else {
            panic!("Expected Row");
        }
    }

    #[test]
    fn test_factorial_input_on_empty_slot_inserts_template() {
        let mut input = MathInput::new();
        input.handle_char('!');

        if let MathBox::Row(items) = &input.root {
            if let MathBox::Func { name, args } = &items[0] {
                assert_eq!(name, "factorial");
                assert_eq!(args.len(), 1);
                assert!(matches!(&args[0], MathBox::Slot));
            } else {
                panic!("Expected factorial function");
            }
        } else {
            panic!("Expected Row");
        }
        assert_eq!(input.cursor_path(), &[0, 0]);
    }

    #[test]
    fn test_insert_in_middle_of_number() {
        let mut input = MathInput::new();
        input.handle_char('1');
        input.handle_char('2');
        input.handle_char('3');

        input.handle_key(SpecialKey::Left);
        input.handle_key(SpecialKey::Left);
        input.handle_char('4');

        if let MathBox::Row(items) = &input.root {
            assert!(matches!(&items[0], MathBox::Number(s) if s == "1423"));
        }
        assert_eq!(input.cursor_offset(), 2);
    }

    #[test]
    fn test_backspace_uses_cursor_offset() {
        let mut input = MathInput::new();
        input.handle_char('1');
        input.handle_char('2');
        input.handle_char('3');

        input.handle_key(SpecialKey::Left);
        input.handle_key(SpecialKey::Left);
        input.handle_key(SpecialKey::Backspace);

        if let MathBox::Row(items) = &input.root {
            assert!(matches!(&items[0], MathBox::Number(s) if s == "23"));
        }
        assert_eq!(input.cursor_offset(), 0);
    }

    #[test]
    fn test_delete_uses_cursor_offset() {
        let mut input = MathInput::new();
        input.handle_char('1');
        input.handle_char('2');
        input.handle_char('3');

        input.handle_key(SpecialKey::Left);
        input.handle_key(SpecialKey::Left);
        input.handle_key(SpecialKey::Delete);

        if let MathBox::Row(items) = &input.root {
            assert!(matches!(&items[0], MathBox::Number(s) if s == "13"));
        }
        assert_eq!(input.cursor_offset(), 1);
    }

    #[test]
    fn test_row_insertion_after_number() {
        let mut input = MathInput::new();
        input.handle_char('2');
        input.handle_char('+');
        input.handle_char('3');

        if let MathBox::Row(items) = &input.root {
            assert_eq!(items.len(), 3);
            assert!(matches!(&items[0], MathBox::Number(s) if s == "2"));
            assert!(matches!(&items[1], MathBox::Operator(Operator::Add)));
            assert!(matches!(&items[2], MathBox::Number(s) if s == "3"));
        } else {
            panic!("Expected Row");
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

    #[test]
    fn test_command_mode_colon_alias() {
        let mut input = MathInput::new();
        input.handle_char(':');
        assert!(input.command_buffer.is_some());

        input.handle_char('s');
        input.handle_char('u');
        input.handle_char('m');
        input.handle_char(' ');

        if let MathBox::Row(items) = &input.root {
            assert!(matches!(items[0], MathBox::Sum { .. }));
        }
    }

    #[test]
    fn test_command_mode_executes_on_enter() {
        let mut input = MathInput::new();
        input.handle_char(':');
        input.handle_char('f');
        input.handle_char('r');
        input.handle_char('a');
        input.handle_char('c');

        let result = input.handle_key(SpecialKey::Enter);
        assert!(matches!(result, InputResult::Consumed));
        assert!(input.command_buffer.is_none());

        if let MathBox::Row(items) = &input.root {
            assert!(matches!(items[0], MathBox::Fraction { .. }));
        } else {
            panic!("Expected Row");
        }
    }

    #[test]
    fn test_sum_command_template() {
        let mut input = MathInput::new();
        run_command(&mut input, "sum");

        if let MathBox::Row(items) = &input.root {
            if let MathBox::Sum {
                var,
                lower,
                upper,
                body,
            } = &items[0]
            {
                assert!(matches!(var.as_ref(), MathBox::Symbol(s) if s == "i"));
                assert!(matches!(lower.as_ref(), MathBox::Slot));
                assert!(matches!(upper.as_ref(), MathBox::Slot));
                assert!(matches!(body.as_ref(), MathBox::Slot));
            } else {
                panic!("Expected Sum template");
            }
        }
    }

    #[test]
    fn test_diff_command_template() {
        let mut input = MathInput::new();
        run_command(&mut input, "diff");

        if let MathBox::Row(items) = &input.root {
            if let MathBox::Derivative { order, var, body } = &items[0] {
                assert_eq!(*order, 1);
                assert!(matches!(var.as_ref(), MathBox::Slot));
                assert!(matches!(body.as_ref(), MathBox::Slot));
            } else {
                panic!("Expected Derivative template");
            }
        }
    }

    #[test]
    fn test_lim_command_template() {
        let mut input = MathInput::new();
        run_command(&mut input, "lim");

        if let MathBox::Row(items) = &input.root {
            if let MathBox::Limit {
                var,
                to,
                direction,
                body,
            } = &items[0]
            {
                assert!(matches!(var.as_ref(), MathBox::Symbol(s) if s == "x"));
                assert!(matches!(to.as_ref(), MathBox::Slot));
                assert!(direction.is_none());
                assert!(matches!(body.as_ref(), MathBox::Slot));
            } else {
                panic!("Expected Limit template");
            }
        }
    }

    #[test]
    fn test_dint_command_template() {
        let mut input = MathInput::new();
        run_command(&mut input, "dint");

        if let MathBox::Row(items) = &input.root {
            if let MathBox::Integral {
                lower,
                upper,
                body,
                var,
            } = &items[0]
            {
                assert!(matches!(lower.as_deref(), Some(MathBox::Slot)));
                assert!(matches!(upper.as_deref(), Some(MathBox::Slot)));
                assert!(matches!(body.as_ref(), MathBox::Slot));
                assert!(matches!(var.as_ref(), MathBox::Symbol(s) if s == "x"));
            } else {
                panic!("Expected definite Integral template");
            }
        }
    }

    #[test]
    fn test_solve_command_template() {
        let mut input = MathInput::new();
        run_command(&mut input, "solve");

        assert_eq!(input.cursor_path(), &[0, 0]);
        if let MathBox::Row(items) = &input.root {
            if let MathBox::Func { name, args } = &items[0] {
                assert_eq!(name, "solve");
                assert_eq!(args.len(), 2);
                assert!(matches!(&args[0], MathBox::Slot));
                assert!(matches!(&args[1], MathBox::Symbol(s) if s == "x"));
            } else {
                panic!("Expected solve function template");
            }
        } else {
            panic!("Expected Row");
        }
    }

    #[test]
    fn test_tab_cycles_fraction_slots() {
        let mut input = MathInput::new();
        run_command(&mut input, "frac");

        assert_eq!(input.cursor_path(), &[0, 0]);

        input.handle_key(SpecialKey::Tab);
        assert_eq!(input.cursor_path(), &[0, 1]);

        input.handle_key(SpecialKey::Tab);
        assert_eq!(input.cursor_path(), &[0, 0]);

        input.handle_key(SpecialKey::ShiftTab);
        assert_eq!(input.cursor_path(), &[0, 1]);
    }

    #[test]
    fn test_tab_skips_non_slot_children_in_sum() {
        let mut input = MathInput::new();
        run_command(&mut input, "sum");

        // Starts at lower bound slot by policy (non-slot var is skipped).
        assert_eq!(input.cursor_path(), &[0, 1]);

        input.handle_key(SpecialKey::Tab);
        assert_eq!(input.cursor_path(), &[0, 2]);

        input.handle_key(SpecialKey::Tab);
        assert_eq!(input.cursor_path(), &[0, 3]);

        input.handle_key(SpecialKey::Tab);
        assert_eq!(input.cursor_path(), &[0, 1]);
    }

    #[test]
    fn test_nested_tab_order_with_power_in_sum_body() {
        let mut input = MathInput::new();
        run_command(&mut input, "sum");

        input.handle_key(SpecialKey::Tab);
        input.handle_key(SpecialKey::Tab);
        assert_eq!(input.cursor_path(), &[0, 3]);

        input.handle_char('x');
        input.handle_char('^');
        assert_eq!(input.cursor_path(), &[0, 3, 1]);

        // From nested exponent slot, Tab should cycle to first slot in the expression.
        input.handle_key(SpecialKey::Tab);
        assert_eq!(input.cursor_path(), &[0, 1]);

        // Shift+Tab should return to the nested exponent slot.
        input.handle_key(SpecialKey::ShiftTab);
        assert_eq!(input.cursor_path(), &[0, 3, 1]);
    }

    #[test]
    fn test_dint_focus_and_tab_order_upper_lower_body() {
        let mut input = MathInput::new();
        run_command(&mut input, "dint");

        // Policy: upper -> lower -> body -> var (var may be non-slot default).
        assert_eq!(input.cursor_path(), &[0, 1]);

        input.handle_key(SpecialKey::Tab);
        assert_eq!(input.cursor_path(), &[0, 0]);

        input.handle_key(SpecialKey::Tab);
        assert_eq!(input.cursor_path(), &[0, 2]);

        input.handle_key(SpecialKey::Tab);
        assert_eq!(input.cursor_path(), &[0, 1]);
    }

    #[test]
    fn test_dint_sequence_builds_expected_power_integrand() {
        let mut input = MathInput::new();
        run_command(&mut input, "dint");

        // Upper bound slot first
        input.handle_char('1');
        input.handle_key(SpecialKey::Tab);
        // Lower bound slot
        input.handle_char('0');
        input.handle_key(SpecialKey::Tab);
        // Body slot: x^(2+x)
        input.handle_char('x');
        input.handle_char('^');
        input.handle_char('(');
        input.handle_char('2');
        input.handle_char('+');
        input.handle_char('x');
        input.handle_char(')');

        let expr = to_expr(input.mathbox()).expect("structured input should convert");
        let rendered = expr.to_string();
        assert!(rendered.contains("integrate("), "rendered: {rendered}");
        assert!(rendered.contains("x^(2+x)"), "rendered: {rendered}");
        assert!(rendered.contains(", x, 0, 1)"), "rendered: {rendered}");
    }

    #[test]
    fn test_integral_body_typed_function_parses_as_function_call() {
        let mut input = MathInput::new();
        run_command(&mut input, "int");

        input.handle_char('s');
        input.handle_char('i');
        input.handle_char('n');
        input.handle_char('(');
        input.handle_char('x');
        input.handle_char(')');

        let expr = to_expr(input.mathbox()).expect("structured input should convert");
        assert_eq!(expr.to_string(), "integrate(sin(x), x)");
    }

    #[test]
    fn test_diff_focus_and_tab_order_var_body() {
        let mut input = MathInput::new();
        run_command(&mut input, "diff");

        assert_eq!(input.cursor_path(), &[0, 0]);
        input.handle_key(SpecialKey::Tab);
        assert_eq!(input.cursor_path(), &[0, 1]);
        input.handle_key(SpecialKey::Tab);
        assert_eq!(input.cursor_path(), &[0, 0]);
    }

    #[test]
    fn test_close_paren_exits_container() {
        let mut input = MathInput::new();
        input.handle_char('(');
        assert_eq!(input.cursor_path(), &[0, 0]);

        input.handle_char(')');
        assert_eq!(input.cursor_path(), &[0]);
    }

    #[test]
    fn test_open_paren_after_function_name_wraps_as_func() {
        let mut input = MathInput::new();
        input.handle_char('s');
        input.handle_char('i');
        input.handle_char('n');
        input.handle_char('(');

        assert_eq!(input.cursor_path(), &[0, 0]);
        if let MathBox::Row(items) = &input.root {
            if let MathBox::Func { name, args } = &items[0] {
                assert_eq!(name, "sin");
                assert_eq!(args.len(), 1);
                assert!(matches!(&args[0], MathBox::Slot));
            } else {
                panic!("Expected function call");
            }
        } else {
            panic!("Expected row root");
        }
    }

    #[test]
    fn test_open_paren_after_non_function_symbol_keeps_parens() {
        let mut input = MathInput::new();
        input.handle_char('f');
        input.handle_char('(');

        assert_eq!(input.cursor_path(), &[1, 0]);
        if let MathBox::Row(items) = &input.root {
            assert!(matches!(&items[0], MathBox::Symbol(s) if s == "f"));
            assert!(matches!(&items[1], MathBox::Parens(_)));
        } else {
            panic!("Expected row root");
        }
    }

    #[test]
    fn test_open_paren_after_mixed_case_function_normalizes_name() {
        let mut input = MathInput::new();
        input.handle_char('S');
        input.handle_char('i');
        input.handle_char('N');
        input.handle_char('(');

        if let MathBox::Row(items) = &input.root {
            assert!(matches!(&items[0], MathBox::Func { name, .. } if name == "sin"));
        } else {
            panic!("Expected row root");
        }
    }

    #[test]
    fn test_backspace_at_exponent_start_deletes_base() {
        let mut input = MathInput::new();
        input.handle_char('2');
        input.handle_char('^');
        input.handle_char('3');

        input.handle_key(SpecialKey::Left);
        assert_eq!(input.cursor_path(), &[0, 1]);
        assert_eq!(input.cursor_offset(), 0);

        input.handle_key(SpecialKey::Backspace);
        assert_eq!(input.cursor_path(), &[0, 0]);
        assert_eq!(input.cursor_offset(), 0);

        if let MathBox::Row(items) = &input.root {
            if let MathBox::Power { base, exp } = &items[0] {
                assert!(matches!(base.as_ref(), MathBox::Slot));
                assert!(matches!(exp.as_ref(), MathBox::Number(s) if s == "3"));
            } else {
                panic!("Expected Power");
            }
        }
    }

    #[test]
    fn test_delete_at_base_end_deletes_exponent_prefix() {
        let mut input = MathInput::new();
        input.handle_char('2');
        input.handle_char('^');
        input.handle_char('3');
        input.handle_char('4');

        input.handle_key(SpecialKey::Down);
        assert_eq!(input.cursor_path(), &[0, 0]);
        input.handle_key(SpecialKey::Right);
        assert_eq!(input.cursor_offset(), 1);

        input.handle_key(SpecialKey::Delete);
        assert_eq!(input.cursor_path(), &[0, 0]);
        assert_eq!(input.cursor_offset(), 1);

        if let MathBox::Row(items) = &input.root {
            if let MathBox::Power { exp, .. } = &items[0] {
                assert!(matches!(exp.as_ref(), MathBox::Number(s) if s == "4"));
            } else {
                panic!("Expected Power");
            }
        }
    }

    #[test]
    fn test_backspace_from_denominator_slot_targets_numerator() {
        let mut input = MathInput::new();
        run_command(&mut input, "frac");
        input.handle_char('1');

        input.handle_key(SpecialKey::Tab);
        assert_eq!(input.cursor_path(), &[0, 1]);
        assert!(matches!(input.get_current(), Some(MathBox::Slot)));

        input.handle_key(SpecialKey::Backspace);
        assert_eq!(input.cursor_path(), &[0, 0]);

        if let MathBox::Row(items) = &input.root {
            if let MathBox::Fraction { num, den } = &items[0] {
                assert!(matches!(num.as_ref(), MathBox::Slot));
                assert!(matches!(den.as_ref(), MathBox::Slot));
            } else {
                panic!("Expected Fraction");
            }
        }
    }

    #[test]
    fn test_delete_from_numerator_slot_targets_denominator() {
        let mut input = MathInput::new();
        run_command(&mut input, "frac");

        input.handle_key(SpecialKey::Tab);
        input.handle_char('5');
        input.handle_key(SpecialKey::ShiftTab);
        assert_eq!(input.cursor_path(), &[0, 0]);

        input.handle_key(SpecialKey::Delete);
        assert_eq!(input.cursor_path(), &[0, 0]);

        if let MathBox::Row(items) = &input.root {
            if let MathBox::Fraction { num, den } = &items[0] {
                assert!(matches!(num.as_ref(), MathBox::Slot));
                assert!(matches!(den.as_ref(), MathBox::Slot));
            } else {
                panic!("Expected Fraction");
            }
        }
    }

    #[test]
    fn test_backspace_on_container_replaces_with_slot() {
        let mut input = MathInput::new();
        input.handle_char('(');
        input.handle_char('x');
        input.handle_char(')');
        assert_eq!(input.cursor_path(), &[0]);

        input.handle_key(SpecialKey::Backspace);
        assert_eq!(input.cursor_path(), &[0]);

        if let MathBox::Row(items) = &input.root {
            assert!(matches!(&items[0], MathBox::Slot));
        } else {
            panic!("Expected Row");
        }
    }

    #[test]
    fn test_backspace_clears_empty_sum_template() {
        let mut input = MathInput::new();
        run_command(&mut input, "sum");

        assert_eq!(input.cursor_path(), &[0, 1]);
        input.handle_key(SpecialKey::Backspace);
        assert_eq!(input.cursor_path(), &[0, 0]);

        input.handle_key(SpecialKey::Backspace);
        assert_eq!(input.cursor_path(), &[0]);

        if let MathBox::Row(items) = &input.root {
            assert!(matches!(&items[0], MathBox::Slot));
        } else {
            panic!("Expected Row");
        }
    }

    #[test]
    fn test_backspace_on_row_slot_removes_current_slot() {
        let mut input = MathInput::new();
        input.handle_char('2');
        input.handle_char('+');
        input.handle_char('3');

        input.handle_key(SpecialKey::Backspace);
        input.handle_key(SpecialKey::Backspace);

        assert_eq!(input.cursor_path(), &[1]);
        if let MathBox::Row(items) = &input.root {
            assert_eq!(items.len(), 2);
            assert!(matches!(&items[0], MathBox::Number(s) if s == "2"));
            assert!(matches!(&items[1], MathBox::Operator(Operator::Add)));
        } else {
            panic!("Expected Row");
        }
    }
}
