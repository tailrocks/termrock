//! Lookbook-local multi-line edit-core prototype for Plan 034.

use unicode_segmentation::UnicodeSegmentation;

use termrock::text::display_cols;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Cursor {
    pub(crate) line: usize,
    pub(crate) byte: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Edit {
    Insert(char),
    Newline,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp(usize),
    PageDown(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorBuffer {
    lines: Vec<String>,
    cursor: Cursor,
    goal_column: Option<usize>,
}

impl EditorBuffer {
    pub(crate) fn new(lines: Vec<String>, cursor: Cursor) -> Self {
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        assert!(cursor.line < lines.len(), "cursor line must exist");
        assert!(is_grapheme_boundary(&lines[cursor.line], cursor.byte));
        Self {
            lines,
            cursor,
            goal_column: None,
        }
    }

    pub(crate) fn apply(&mut self, edit: Edit) -> bool {
        let changed = match edit {
            Edit::Insert(character) => self.insert(character),
            Edit::Newline => self.newline(),
            Edit::Backspace => self.backspace(),
            Edit::Delete => self.delete(),
            Edit::Left => self.left(),
            Edit::Right => self.right(),
            Edit::Up => self.vertical(-1),
            Edit::Down => self.vertical(1),
            Edit::Home => {
                let changed = self.cursor.byte != 0;
                self.cursor.byte = 0;
                self.goal_column = None;
                changed
            }
            Edit::End => {
                let changed = self.cursor.byte != self.current_line().len();
                self.cursor.byte = self.current_line().len();
                self.goal_column = None;
                changed
            }
            Edit::PageUp(height) => self.vertical(-page_delta(height)),
            Edit::PageDown(height) => self.vertical(page_delta(height)),
        };
        debug_assert!(self.invariants_hold());
        changed
    }

    fn insert(&mut self, character: char) -> bool {
        if character == '\n' || character == '\r' || character.is_control() {
            return false;
        }
        let insertion = self.cursor.byte;
        self.lines[self.cursor.line].insert(insertion, character);
        self.cursor.byte = boundary_at_or_after(
            &self.lines[self.cursor.line],
            insertion + character.len_utf8(),
        );
        self.goal_column = None;
        true
    }

    fn newline(&mut self) -> bool {
        let suffix = self.lines[self.cursor.line].split_off(self.cursor.byte);
        self.cursor.line += 1;
        self.cursor.byte = 0;
        self.lines.insert(self.cursor.line, suffix);
        self.goal_column = None;
        true
    }

    fn backspace(&mut self) -> bool {
        if let Some(previous) = previous_boundary(self.current_line(), self.cursor.byte) {
            self.lines[self.cursor.line].drain(previous..self.cursor.byte);
            self.cursor.byte = previous;
            self.goal_column = None;
            true
        } else if self.cursor.line > 0 {
            let current = self.lines.remove(self.cursor.line);
            self.cursor.line -= 1;
            let join = self.lines[self.cursor.line].len();
            self.lines[self.cursor.line].push_str(&current);
            self.cursor.byte = boundary_at_or_after(&self.lines[self.cursor.line], join);
            self.goal_column = None;
            true
        } else {
            false
        }
    }

    fn delete(&mut self) -> bool {
        if let Some(next) = next_boundary(self.current_line(), self.cursor.byte) {
            self.lines[self.cursor.line].drain(self.cursor.byte..next);
            self.goal_column = None;
            true
        } else if self.cursor.line + 1 < self.lines.len() {
            let join = self.cursor.byte;
            let next = self.lines.remove(self.cursor.line + 1);
            self.lines[self.cursor.line].push_str(&next);
            self.cursor.byte = boundary_at_or_after(&self.lines[self.cursor.line], join);
            self.goal_column = None;
            true
        } else {
            false
        }
    }

    fn left(&mut self) -> bool {
        if let Some(previous) = previous_boundary(self.current_line(), self.cursor.byte) {
            self.cursor.byte = previous;
            self.goal_column = None;
            true
        } else if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.byte = self.current_line().len();
            self.goal_column = None;
            true
        } else {
            self.goal_column = None;
            false
        }
    }

    fn right(&mut self) -> bool {
        if let Some(next) = next_boundary(self.current_line(), self.cursor.byte) {
            self.cursor.byte = next;
            self.goal_column = None;
            true
        } else if self.cursor.line + 1 < self.lines.len() {
            self.cursor.line += 1;
            self.cursor.byte = 0;
            self.goal_column = None;
            true
        } else {
            self.goal_column = None;
            false
        }
    }

    fn vertical(&mut self, delta: isize) -> bool {
        let before = self.cursor;
        let goal = *self
            .goal_column
            .get_or_insert_with(|| display_cols(&self.lines[self.cursor.line][..self.cursor.byte]));
        self.cursor.line = self
            .cursor
            .line
            .saturating_add_signed(delta)
            .min(self.lines.len() - 1);
        self.cursor.byte = byte_at_display_column(self.current_line(), goal);
        self.cursor != before
    }

    fn current_line(&self) -> &str {
        &self.lines[self.cursor.line]
    }

    fn invariants_hold(&self) -> bool {
        !self.lines.is_empty()
            && self.cursor.line < self.lines.len()
            && is_grapheme_boundary(self.current_line(), self.cursor.byte)
    }
}

fn page_delta(height: usize) -> isize {
    isize::try_from(height.max(1)).unwrap_or(isize::MAX)
}

fn is_grapheme_boundary(line: &str, byte: usize) -> bool {
    byte == line.len() || line.grapheme_indices(true).any(|(index, _)| index == byte)
}

fn previous_boundary(line: &str, byte: usize) -> Option<usize> {
    line[..byte]
        .grapheme_indices(true)
        .next_back()
        .map(|(index, _)| index)
}

fn next_boundary(line: &str, byte: usize) -> Option<usize> {
    line[byte..]
        .graphemes(true)
        .next()
        .map(|grapheme| byte + grapheme.len())
}

fn boundary_at_or_after(line: &str, byte: usize) -> usize {
    line.grapheme_indices(true)
        .map(|(index, _)| index)
        .chain(std::iter::once(line.len()))
        .find(|boundary| *boundary >= byte)
        .unwrap_or(line.len())
}

fn byte_at_display_column(line: &str, goal: usize) -> usize {
    let mut column = 0;
    let mut boundary = 0;
    for (byte, grapheme) in line.grapheme_indices(true) {
        let next_column = column + display_cols(grapheme);
        if next_column > goal {
            break;
        }
        column = next_column;
        boundary = byte + grapheme.len();
    }
    boundary
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Case {
        name: &'static str,
        lines: &'static [&'static str],
        cursor: Cursor,
        goal: Option<usize>,
        edit: Edit,
        expected_lines: &'static [&'static str],
        expected_cursor: Cursor,
        expected_goal: Option<usize>,
        changed: bool,
    }

    #[test]
    fn edit_and_cursor_contract_table() {
        let cases = [
            case(
                "insert ascii",
                &["ac"],
                c(0, 1),
                Edit::Insert('b'),
                &["abc"],
                c(0, 2),
            ),
            case(
                "insert cjk",
                &["ab"],
                c(0, 1),
                Edit::Insert('界'),
                &["a界b"],
                c(0, 4),
            ),
            case(
                "insert emoji",
                &[""],
                c(0, 0),
                Edit::Insert('🧪'),
                &["🧪"],
                c(0, 4),
            ),
            case(
                "insert combining",
                &["e"],
                c(0, 1),
                Edit::Insert('\u{301}'),
                &["e\u{301}"],
                c(0, 3),
            ),
            case(
                "insert base before combining",
                &["\u{301}x"],
                c(0, 0),
                Edit::Insert('e'),
                &["e\u{301}x"],
                c(0, 3),
            ),
            case(
                "insert zwj joins emoji",
                &["👩👩"],
                c(0, 4),
                Edit::Insert('\u{200d}'),
                &["👩\u{200d}👩"],
                c(0, 11),
            ),
            case(
                "newline middle",
                &["abcd"],
                c(0, 2),
                Edit::Newline,
                &["ab", "cd"],
                c(1, 0),
            ),
            case(
                "newline start",
                &["ab"],
                c(0, 0),
                Edit::Newline,
                &["", "ab"],
                c(1, 0),
            ),
            case(
                "newline end",
                &["ab"],
                c(0, 2),
                Edit::Newline,
                &["ab", ""],
                c(1, 0),
            ),
            case(
                "backspace ascii",
                &["abc"],
                c(0, 2),
                Edit::Backspace,
                &["ac"],
                c(0, 1),
            ),
            case(
                "backspace grapheme",
                &["e\u{301}x"],
                c(0, 3),
                Edit::Backspace,
                &["x"],
                c(0, 0),
            ),
            case(
                "backspace joins",
                &["ab", "cd"],
                c(1, 0),
                Edit::Backspace,
                &["abcd"],
                c(0, 2),
            ),
            case(
                "backspace join repairs boundary",
                &["e", "\u{301}x"],
                c(1, 0),
                Edit::Backspace,
                &["e\u{301}x"],
                c(0, 3),
            ),
            unchanged("backspace start", &["ab"], c(0, 0), Edit::Backspace),
            case(
                "delete ascii",
                &["abc"],
                c(0, 1),
                Edit::Delete,
                &["ac"],
                c(0, 1),
            ),
            case(
                "delete emoji",
                &["a🧪b"],
                c(0, 1),
                Edit::Delete,
                &["ab"],
                c(0, 1),
            ),
            case(
                "delete joins",
                &["ab", "cd"],
                c(0, 2),
                Edit::Delete,
                &["abcd"],
                c(0, 2),
            ),
            case(
                "delete join repairs boundary",
                &["e", "\u{301}x"],
                c(0, 1),
                Edit::Delete,
                &["e\u{301}x"],
                c(0, 3),
            ),
            unchanged("delete end", &["ab"], c(0, 2), Edit::Delete),
            case(
                "left grapheme",
                &["e\u{301}x"],
                c(0, 3),
                Edit::Left,
                &["e\u{301}x"],
                c(0, 0),
            ),
            case(
                "left crosses line",
                &["ab", "cd"],
                c(1, 0),
                Edit::Left,
                &["ab", "cd"],
                c(0, 2),
            ),
            case(
                "right emoji",
                &["🧪x"],
                c(0, 0),
                Edit::Right,
                &["🧪x"],
                c(0, 4),
            ),
            case(
                "right crosses line",
                &["ab", "cd"],
                c(0, 2),
                Edit::Right,
                &["ab", "cd"],
                c(1, 0),
            ),
            case("home", &["abc"], c(0, 2), Edit::Home, &["abc"], c(0, 0)),
            case("end", &["abc"], c(0, 1), Edit::End, &["abc"], c(0, 3)),
            case(
                "up same column",
                &["abcd", "wxyz"],
                c(1, 3),
                Edit::Up,
                &["abcd", "wxyz"],
                c(0, 3),
            ),
            case(
                "down same column",
                &["abcd", "wxyz"],
                c(0, 2),
                Edit::Down,
                &["abcd", "wxyz"],
                c(1, 2),
            ),
            case(
                "down short line",
                &["abcd", "x"],
                c(0, 3),
                Edit::Down,
                &["abcd", "x"],
                c(1, 1),
            ),
            goal_case(
                "down restores goal",
                &["abcd", "x", "wxyz"],
                c(1, 1),
                Some(3),
                Edit::Down,
                c(2, 3),
                Some(3),
            ),
            goal_case(
                "wide goal stays before split",
                &["ab", "a界"],
                c(0, 2),
                None,
                Edit::Down,
                c(1, 1),
                Some(2),
            ),
            case(
                "down empty line",
                &["ab", ""],
                c(0, 2),
                Edit::Down,
                &["ab", ""],
                c(1, 0),
            ),
            goal_case(
                "up from empty",
                &["ab", ""],
                c(1, 0),
                None,
                Edit::Up,
                c(0, 0),
                Some(0),
            ),
            goal_case(
                "page down",
                &["a", "b", "c", "d"],
                c(0, 1),
                None,
                Edit::PageDown(2),
                c(2, 1),
                Some(1),
            ),
            goal_case(
                "page up clamps",
                &["a", "b", "c", "d"],
                c(1, 1),
                None,
                Edit::PageUp(4),
                c(0, 1),
                Some(1),
            ),
        ];

        assert!(cases.len() >= 20);
        for test in cases {
            let mut editor = EditorBuffer::new(
                test.lines.iter().map(|line| (*line).to_owned()).collect(),
                test.cursor,
            );
            editor.goal_column = test.goal;
            assert_eq!(editor.apply(test.edit), test.changed, "{}", test.name);
            assert_eq!(
                editor.lines,
                strings(test.expected_lines),
                "{} buffer",
                test.name
            );
            assert_eq!(editor.cursor, test.expected_cursor, "{} cursor", test.name);
            assert_eq!(editor.goal_column, test.expected_goal, "{} goal", test.name);
        }
    }

    fn c(line: usize, byte: usize) -> Cursor {
        Cursor { line, byte }
    }

    fn case(
        name: &'static str,
        lines: &'static [&'static str],
        cursor: Cursor,
        edit: Edit,
        expected_lines: &'static [&'static str],
        expected_cursor: Cursor,
    ) -> Case {
        Case {
            name,
            lines,
            cursor,
            goal: None,
            edit,
            expected_lines,
            expected_cursor,
            expected_goal: if matches!(
                edit,
                Edit::Up | Edit::Down | Edit::PageUp(_) | Edit::PageDown(_)
            ) {
                Some(display_cols(&lines[cursor.line][..cursor.byte]))
            } else {
                None
            },
            changed: true,
        }
    }

    fn unchanged(
        name: &'static str,
        lines: &'static [&'static str],
        cursor: Cursor,
        edit: Edit,
    ) -> Case {
        let mut test = case(name, lines, cursor, edit, lines, cursor);
        test.changed = false;
        test
    }

    fn goal_case(
        name: &'static str,
        lines: &'static [&'static str],
        cursor: Cursor,
        goal: Option<usize>,
        edit: Edit,
        expected_cursor: Cursor,
        expected_goal: Option<usize>,
    ) -> Case {
        Case {
            name,
            lines,
            cursor,
            goal,
            edit,
            expected_lines: lines,
            expected_cursor,
            expected_goal,
            changed: true,
        }
    }

    fn strings(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|line| (*line).to_owned()).collect()
    }
}
