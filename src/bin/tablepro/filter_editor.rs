use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::{Focus, FocusChange, FocusRing, forward_blur};
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::ui::ctx::{RenderCtx, fill};
use junie_tui::ui::layer::{DismissReason, Layer, LayerCx, LayerOut, LayerSpec};
use junie_tui::ui::popup::Dismiss;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::field_common::{PointerField, route_click, route_drag, route_press};
use junie_tui::widgets::input::{InputAction, TextInput};
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::select::{Select, SelectAction};

use crate::app::ModalResult;
use crate::db::ColType;
use crate::tabs::{Filter, FilterField, FilterOp};

pub const ID: WidgetId = WidgetId::of("filter-editor");

pub type FilterDone = Option<(Option<usize>, Filter)>;

pub struct FilterEditor {
    pub index: Option<usize>,
    pub column: Select,
    pub op: Select,
    pub value: TextInput,
    pub value2: TextInput,
    pub apply: Button,
    pub cancel: Button,
    pub area: Rect,
    columns: Vec<(String, ColType)>,

    pub dismiss: Dismiss,
    done: Option<FilterDone>,
}

enum Step {
    Keep(Outcome),
    Apply,
    Cancel,
}

impl FilterEditor {
    pub fn new(
        index: Option<usize>,
        update: bool,
        columns: Vec<(String, ColType)>,
        col_i: usize,
        op: FilterOp,
        value: &str,
        value2: &str,
    ) -> Self {
        let names: Vec<&str> = columns.iter().map(|c| c.0.as_str()).collect();
        let ops = FilterOp::ordered_for(columns[col_i].1);
        let op_labels: Vec<&str> = ops.iter().map(|o| o.label()).collect();
        let op_i = ops.iter().position(|o| *o == op).unwrap_or(0);
        Self {
            index,
            column: Select::new(ID.sub("col"), "Column", &names, col_i),
            op: Select::new(ID.sub("op"), "Operator", &op_labels, op_i)
                .help("Operators that fit the column type come first"),
            value: TextInput::new(ID.sub("value"), "Value")
                .value(value)
                .placeholder("value")
                .plain_label(),
            value2: TextInput::new(ID.sub("value2"), "and")
                .value(value2)
                .plain_label(),
            apply: Button::primary(
                ID.sub("apply"),
                if update {
                    "Update filter"
                } else {
                    "Add filter"
                },
            ),
            cancel: Button::subtle(ID.sub("cancel"), "Cancel"),
            area: Rect::ZERO,
            columns,
            dismiss: Dismiss::ESCAPE,
            done: None,
        }
    }

    fn column_changed(&mut self, i: usize) {
        let ops = FilterOp::ordered_for(self.columns[i].1);
        let labels: Vec<&str> = ops.iter().map(|o| o.label()).collect();
        self.op = Select::new(self.op.id, "Operator", &labels, 0)
            .help("Operators that fit the column type come first");
    }

    fn chosen_op(&self) -> FilterOp {
        let ops = FilterOp::ordered_for(self.columns[self.column.selected].1);
        ops[self.op.selected.min(ops.len() - 1)]
    }

    fn submit(&mut self, focus: &mut Focus) -> Outcome {
        let op = self.chosen_op();
        if self.value.editing {
            self.value.commit();
        }
        if self.value2.editing {
            self.value2.commit();
        }
        let filter = Filter {
            column: self.columns[self.column.selected].0.clone(),
            op,
            value: self.value.text().trim().to_owned(),
            value2: self.value2.text().trim().to_owned(),
            enabled: true,
        };
        // A rejected filter keeps the editor open with its error on the field
        // at fault, so the view behind it is never replaced.
        let ty = self.columns[self.column.selected].1;
        if let Err(e) = filter.check(ty) {
            let input = match e.field {
                FilterField::Value => &mut self.value,
                FilterField::Value2 => &mut self.value2,
            };
            input.field.error = Some(e.message.into());
            focus.focus(input.id);
            return Outcome::Changed;
        }
        self.done = Some(Some((self.index, filter)));
        Outcome::Changed
    }

    fn step(&mut self, step: Step, focus: &mut Focus) -> Outcome {
        match step {
            Step::Keep(o) => o,
            Step::Apply => self.submit(focus),
            Step::Cancel => {
                self.done = Some(None);
                Outcome::Changed
            }
        }
    }

    fn key(&mut self, key: &Key, focus: &mut Focus, ring: &FocusRing) -> Step {
        let cur = focus.current();

        if !self.value.editing && !self.value2.editing && !self.column.open && !self.op.open {
            match key.code {
                KeyCode::Tab => {
                    focus.next(ring);
                    return Step::Keep(Outcome::Changed);
                }
                KeyCode::BackTab => {
                    focus.prev(ring);
                    return Step::Keep(Outcome::Changed);
                }
                _ => {}
            }
        }
        if cur == Some(self.column.id) {
            let (o, ev) = self.column.on_key(key);
            if let Some(SelectAction::Changed(i)) = ev {
                self.column_changed(i);
            }
            return Step::Keep(o.or(Outcome::Consumed));
        }
        if cur == Some(self.op.id) {
            return Step::Keep(self.op.on_key(key).0.or(Outcome::Consumed));
        }
        for inp in [&mut self.value, &mut self.value2] {
            if cur == Some(inp.id) {
                let (o, ev) = inp.on_key(key);
                match ev {
                    Some(InputAction::CommittedTab { backward }) => {
                        if backward {
                            focus.prev(ring)
                        } else {
                            focus.next(ring)
                        }
                    }
                    Some(InputAction::Committed) => return Step::Apply,
                    _ => {}
                }
                if !o.consumed() && key.is(KeyCode::Enter) {
                    return Step::Apply;
                }
                return Step::Keep(o.or(Outcome::Consumed));
            }
        }
        if cur == Some(self.apply.id) {
            let (o, act) = self.apply.on_key(key);
            if act {
                return Step::Apply;
            }
            return Step::Keep(o.or(Outcome::Consumed));
        }
        if cur == Some(self.cancel.id) {
            let (o, act) = self.cancel.on_key(key);
            if act {
                return Step::Cancel;
            }
            return Step::Keep(o.or(Outcome::Consumed));
        }
        match key.code {
            KeyCode::Tab => {
                focus.next(ring);
                Step::Keep(Outcome::Changed)
            }
            KeyCode::BackTab => {
                focus.prev(ring);
                Step::Keep(Outcome::Changed)
            }
            _ => Step::Keep(Outcome::Consumed),
        }
    }

    fn pointer_step(&mut self, ev: &PointerEvent, focus: &mut Focus) -> Step {
        let (id, pos) = (ev.id, ev.pos());
        let focusing = ev.is(Phase::Press) || ev.is(Phase::Click);
        if let Some(r) = self.column.route(ev) {
            if focusing {
                focus.focus(self.column.id);
            }
            let (o, act) = r.into_parts();
            if let Some(SelectAction::Changed(i)) = act {
                self.column_changed(i);
            }
            return Step::Keep(o.or(Outcome::Changed));
        }
        if let Some(r) = self.op.route(ev) {
            if focusing {
                focus.focus(self.op.id);
            }
            return Step::Keep(r.outcome().or(Outcome::Changed));
        }
        let fields: &mut [&mut dyn PointerField] = &mut [&mut self.value, &mut self.value2];
        match ev.phase() {
            Some(Phase::Press) => {
                let _ = route_press(fields, id, pos, ev.count());
                Step::Keep(Outcome::Changed)
            }
            Some(Phase::Drag) => {
                Step::Keep(route_drag(fields, id, pos).unwrap_or(Outcome::Consumed))
            }
            Some(Phase::Click) => {
                if let Some(o) = route_click(fields, id, pos, ev.count()) {
                    focus.focus(id);
                    return Step::Keep(o);
                }
                if self.apply.id == id && self.apply.on_click() {
                    return Step::Apply;
                }
                if self.cancel.id == id && self.cancel.on_click() {
                    return Step::Cancel;
                }
                Step::Keep(Outcome::Changed)
            }
            _ => Step::Keep(Outcome::Consumed),
        }
    }

    fn draw(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;

        ctx.modal_backdrop(buf, screen);
        let w = 64u16.min(screen.width.saturating_sub(4));
        let h = 15u16.min(screen.height.saturating_sub(2));
        let area = junie_tui::ui::popup::place(
            screen,
            Rect::ZERO,
            w,
            h,
            junie_tui::ui::popup::Placement::Center,
        );
        self.area = area;
        let bg = t.surface_elevated;
        fill(buf, area, ratatui::style::Style::new().bg(bg));
        let block = ratatui::widgets::Block::new()
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(t.border(true).bg(bg));
        ratatui::widgets::Widget::render(block, area, buf);

        ctx.hits.register_surface(ID, area);
        let inner = area.inner(ratatui::layout::Margin::new(3, 1));
        buf.set_string(
            inner.x,
            inner.y,
            if self.index.is_some() {
                "Edit filter"
            } else {
                "Add filter"
            },
            t.title().bg(bg),
        );
        let mut y = inner.y + 2;
        let (l, r) = junie_tui::ui::layout::Split::new(50, 16, 16).horizontal(
            Rect::new(
                inner.x.saturating_sub(1),
                y,
                inner.width + 1,
                Select::HEIGHT,
            ),
            2,
        );
        self.column.render(l, buf, ctx, bg);
        self.op.render(r, buf, ctx, bg);
        y += Select::HEIGHT;
        let op = self.chosen_op();
        if op.needs_value() {
            if op == FilterOp::Between {
                let (l, r) = junie_tui::ui::layout::Split::new(50, 12, 12).horizontal(
                    Rect::new(
                        inner.x.saturating_sub(1),
                        y,
                        inner.width + 1,
                        TextInput::HEIGHT,
                    ),
                    2,
                );
                self.value.render(l, buf, ctx, bg);
                self.value2.render(r, buf, ctx, bg);
            } else {
                self.value.render(
                    Rect::new(
                        inner.x.saturating_sub(1),
                        y,
                        inner.width + 1,
                        TextInput::HEIGHT,
                    ),
                    buf,
                    ctx,
                    bg,
                );
            }
        } else {
            buf.set_string(
                inner.x + 1,
                y + 1,
                "No value needed for this operator",
                t.muted().bg(bg),
            );
        }
        y += TextInput::HEIGHT + 1;
        let preview = Filter {
            column: self.columns[self.column.selected].0.clone(),
            op,
            value: self.value.text().to_owned(),
            value2: self.value2.text().to_owned(),
            enabled: true,
        }
        .to_sql();
        buf.set_string(
            inner.x,
            y,
            junie_tui::ui::reveal::cut(
                Rect::new(inner.x, y, inner.width, 1),
                &format!("WHERE {preview}"),
            ),
            t.secondary().bg(bg),
        );
        let ay = inner.bottom().saturating_sub(1);
        junie_tui::widgets::action_row::ActionRow::new()
            .align(junie_tui::widgets::button::RowAlign::End)
            .gap(1)
            .render(
                Rect::new(inner.x, ay, inner.width, 1),
                buf,
                ctx,
                bg,
                &mut [&mut self.cancel, &mut self.apply],
            );

        ctx.hits.register(self.column.id, self.column.area);
        ctx.hits.register(self.op.id, self.op.area);
        ctx.hits.register(self.value.id, self.value.area);
        ctx.hits.register(self.value2.id, self.value2.area);
        ctx.hits.register(self.cancel.id, self.cancel.area);
        ctx.hits.register(self.apply.id, self.apply.area);
    }
}

impl Layer<ModalResult> for FilterEditor {
    fn spec(&self) -> LayerSpec {
        let initial = if self.value.text().is_empty() {
            self.value.id
        } else {
            self.apply.id
        };
        LayerSpec::modal(self.dismiss).initial_focus(initial)
    }

    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _: &()) {
        self.draw(screen, buf, ctx);
    }

    fn on_key(&mut self, key: &Key, cx: &mut LayerCx<'_>) -> Outcome {
        if key.is(KeyCode::Esc)
            && !self.value.is_dirty()
            && !self.value2.is_dirty()
            && !self.column.open
            && !self.op.open
        {
            return Outcome::Ignored;
        }
        let step = self.key(key, cx.focus, cx.ring);
        self.step(step, cx.focus)
    }

    fn pointer(&mut self, ev: &PointerEvent, cx: &mut LayerCx<'_>) -> Outcome {
        let step = self.pointer_step(ev, cx.focus);
        self.step(step, cx.focus)
    }

    fn on_paste(&mut self, text: &str) -> Outcome {
        if self.value.editing {
            return self.value.on_paste(text);
        }
        Outcome::Consumed
    }

    fn on_blur(&mut self, change: FocusChange) -> Outcome {
        forward_blur(
            change,
            &mut [&mut self.column, &mut self.op, &mut self.value],
        )
    }

    fn hints(&self, _: Option<WidgetId>) -> Vec<Hint> {
        vec![
            hint("Tab", "Next field"),
            hint("Enter", "Apply"),
            hint("Esc", "Cancel"),
        ]
    }

    fn on_dismiss(&mut self, _: DismissReason) {
        self.done = Some(None);
    }

    fn take_result(&mut self) -> Option<LayerOut<ModalResult>> {
        self.done
            .take()
            .map(|d| LayerOut::Close(ModalResult::Filter(d)))
    }
}

#[cfg(test)]
mod issue10_tests {
    use super::*;
    use crate::tabs::TableTab;

    fn editor(col_i: usize, op: FilterOp, value: &str, value2: &str) -> FilterEditor {
        let columns = vec![
            ("notes".to_owned(), ColType::Text),
            ("order_number".to_owned(), ColType::Int),
            ("is_gift".to_owned(), ColType::Bool),
        ];
        FilterEditor::new(None, false, columns, col_i, op, value, value2)
    }

    #[test]
    fn issue10_editor_keeps_an_invalid_regex_on_the_value_field() {
        let mut editor = editor(0, FilterOp::Regex, "(", "");
        let mut focus = Focus::default();
        editor.submit(&mut focus);
        assert!(
            editor.done.is_none(),
            "an invalid filter must not close the editor"
        );
        assert!(Layer::<ModalResult>::take_result(&mut editor).is_none());
        let error = editor.value.field.error.as_deref().expect("value error");
        assert!(error.starts_with("invalid regular expression"), "{error}");
        assert!(editor.value2.field.error.is_none());
        assert_eq!(focus.current(), Some(editor.value.id));
        assert_eq!(editor.value.text(), "(", "the typed text survives");

        editor.value = TextInput::new(ID.sub("value"), "Value").value("^p.id$");
        editor.submit(&mut focus);
        let Some(LayerOut::Close(ModalResult::Filter(Some((None, filter))))) =
            Layer::<ModalResult>::take_result(&mut editor)
        else {
            panic!("a valid regex closes the editor with its filter");
        };
        assert_eq!(
            (filter.op, filter.value.as_str()),
            (FilterOp::Regex, "^p.id$")
        );
    }

    #[test]
    fn issue10_editor_keeps_a_reversed_range_on_the_upper_field() {
        let mut editor = editor(1, FilterOp::Between, "5", "1");
        let mut focus = Focus::default();
        editor.submit(&mut focus);
        assert!(editor.done.is_none());
        assert!(Layer::<ModalResult>::take_result(&mut editor).is_none());
        let error = editor.value2.field.error.as_deref().expect("value2 error");
        assert_eq!(error, "The upper bound is below the lower bound");
        assert!(editor.value.field.error.is_none());
        assert_eq!(focus.current(), Some(editor.value2.id));
        assert_eq!((editor.value.text(), editor.value2.text()), ("5", "1"));
    }

    type Case = (
        &'static str,
        FilterOp,
        &'static str,
        &'static str,
        bool,
        &'static str,
    );

    /// Column, operator, typed text, whether the upper field is at fault, and
    /// the start of the message the editor must show.
    const REJECTED: [Case; 8] = [
        (
            "order_number",
            FilterOp::Gt,
            "abc",
            "",
            false,
            "Enter a whole number",
        ),
        (
            "order_number",
            FilterOp::Between,
            "1",
            "x",
            true,
            "Enter a whole number",
        ),
        (
            "order_number",
            FilterOp::Between,
            "5",
            "1",
            true,
            "The upper bound is below",
        ),
        (
            "is_gift",
            FilterOp::Eq,
            "yes",
            "",
            false,
            "Enter true or false",
        ),
        (
            "is_gift",
            FilterOp::In,
            "true, maybe",
            "",
            false,
            "Enter true or false",
        ),
        (
            "notes",
            FilterOp::In,
            "paid,,x",
            "",
            false,
            "List items cannot be empty",
        ),
        (
            "notes",
            FilterOp::Regex,
            "(",
            "",
            false,
            "invalid regular expression",
        ),
        ("notes", FilterOp::Eq, "", "", false, "A value is required"),
    ];

    fn at(columns: &[(String, ColType)], name: &str) -> usize {
        columns.iter().position(|c| c.0 == name).expect(name)
    }

    #[test]
    fn issue10_invalid_regex_and_operand_preserve_view() {
        let cat = crate::db::Catalog::acme_prod();
        let table = cat.find(Some("public"), "orders").expect("orders fixture");
        let mut tab = TableTab::new(WidgetId::of("issue10-view"), &cat, table, false);
        tab.filters.push(Filter {
            column: "status".into(),
            op: FilterOp::Eq,
            value: "paid".into(),
            value2: String::new(),
            enabled: true,
        });
        tab.load(&cat);
        let view = |t: &TableTab| {
            (
                t.filters.clone(),
                t.chips.chips().to_vec(),
                t.grid.rows().to_vec(),
                t.total,
            )
        };
        let before = view(&tab);
        assert!(!before.2.is_empty(), "the applied filter shows rows");

        for (column, op, value, value2, upper, message) in REJECTED {
            let col_i = at(&tab.columns, column);
            let mut editor =
                FilterEditor::new(None, false, tab.columns.clone(), col_i, op, value, value2);
            let mut focus = Focus::default();
            editor.submit(&mut focus);
            assert!(
                editor.done.is_none(),
                "{op:?} {value:?} must not close the editor"
            );
            assert!(Layer::<ModalResult>::take_result(&mut editor).is_none());
            let (at_fault, other) = if upper {
                (&editor.value2, &editor.value)
            } else {
                (&editor.value, &editor.value2)
            };
            let error = at_fault
                .field
                .error
                .as_deref()
                .expect("error on the field at fault");
            assert!(error.starts_with(message), "{op:?} {value:?}: {error}");
            assert!(
                other.field.error.is_none(),
                "{op:?} {value:?}: only one field fails"
            );
            assert_eq!(focus.current(), Some(at_fault.id));
            assert_eq!((editor.value.text(), editor.value2.text()), (value, value2));
            assert_eq!(view(&tab), before, "{op:?} {value:?} left the view alone");
        }
    }

    #[test]
    fn issue10_corrected_operands_close_the_editor_with_the_filter() {
        for (col_i, op, value, value2) in [
            (1, FilterOp::Gt, "3", ""),
            (1, FilterOp::Between, "1", "5"),
            (2, FilterOp::Eq, "TRUE", ""),
            (0, FilterOp::In, "paid, x", ""),
        ] {
            let mut editor = editor(col_i, op, value, value2);
            editor.submit(&mut Focus::default());
            let Some(LayerOut::Close(ModalResult::Filter(Some((None, filter))))) =
                Layer::<ModalResult>::take_result(&mut editor)
            else {
                panic!("{op:?} {value:?} should close the editor");
            };
            assert_eq!((filter.op, filter.value.as_str()), (op, value));
        }
    }
}
