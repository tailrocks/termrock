use super::*;
use crate::core::event::Key;
use crate::core::focus::BlurAction;
use crate::core::hit::HitRegistry;
use crate::core::refusal::{Reason, clear_refusal, take_refusal};
use crate::theme::{ColorLevel, Theme};
use crate::ui::ctx::{Interaction, RenderCtx};
use crate::widgets::input::TextInput;
use crate::widgets::select::Select;
use ratatui::crossterm::event::KeyModifiers;

const FORM: WidgetId = WidgetId::of("t.form");
const SCREEN: Rect = Rect::new(0, 0, 100, 40);
const WHY: &str = "Managed by the host";

fn press(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

fn why() -> Reason {
    Reason::new(WHY).short("managed")
}

fn form() -> FormDialog {
    let mut off = Checkbox::new(FORM.sub("off"), "Disabled", false);
    off.disabled = true;
    let mut locked = RadioGroup::new(FORM.sub("locked"), "Locked", &["a", "b"], 0);
    locked.disabled = true;
    FormDialog::new(
        FORM,
        "Settings",
        vec![
            FormField::check("live", Checkbox::new(FORM.sub("live"), "Live", false)),
            FormField::check(
                "fixed",
                Checkbox::new(FORM.sub("fixed"), "Fixed", true).fixed(why()),
            ),
            FormField::check("off", off),
            FormField::check(
                "mixed",
                Checkbox::new(FORM.sub("mixed"), "Mixed", true).mixed(true),
            ),
            FormField::radio(
                "mode",
                RadioGroup::new(FORM.sub("mode"), "Mode", &["a", "b", "c"], 0)
                    .fixed_option(2, why()),
            ),
            FormField::radio("locked", locked),
        ],
    )
}

struct Rig {
    form: FormDialog,
    focus: Focus,
    hits: HitRegistry,
}

impl Rig {
    fn new() -> Self {
        Self::with(form())
    }

    fn with(form: FormDialog) -> Self {
        clear_refusal();
        let mut rig = Self {
            form,
            focus: Focus::default(),
            hits: HitRegistry::default(),
        };
        rig.draw();
        rig
    }

    fn draw(&mut self) {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(SCREEN);
        let mut ring = FocusRing::default();
        self.hits = HitRegistry::default();
        let interaction = Interaction {
            focus: self.focus.current(),
            ..Default::default()
        };
        let mut ctx = RenderCtx::new(&theme, interaction, &mut self.hits, &mut ring);
        self.form.render(SCREEN, &mut buf, &mut ctx);
        ctx.flush_overlays(&mut buf);
    }

    fn click(&mut self, id: WidgetId) -> Outcome {
        self.draw();
        let region = self
            .hits
            .regions()
            .iter()
            .rev()
            .find(|r| r.id == id)
            .expect("the rendered form registers a hit region for the control");
        let pos = Position::new(region.area.x, region.area.y);
        assert_eq!(
            self.hits.resolve(pos).map(|h| h.owner),
            Some(region.owner),
            "the pointer resolves to the control"
        );
        self.form.on_click(id, pos, &mut self.focus)
    }

    fn click_direct(&mut self, id: WidgetId) -> Outcome {
        self.form.on_click(id, Position::new(0, 0), &mut self.focus)
    }

    fn key(&mut self, focus: WidgetId, code: KeyCode) -> Outcome {
        self.focus.focus(focus);
        let ring = FocusRing::default();
        self.form.on_key(&press(code), &mut self.focus, &ring)
    }

    fn checked(&self, name: &str) -> bool {
        match &self.form.field(name).expect("field").kind {
            FormFieldKind::Check(c) => c.checked,
            _ => panic!("not a checkbox"),
        }
    }

    fn selected(&self, name: &str) -> usize {
        match &self.form.field(name).expect("field").kind {
            FormFieldKind::Radio(r) => r.selected,
            _ => panic!("not a radio"),
        }
    }

    fn untouched(&self) {
        assert!(!self.form.dirty, "no value changed, so the form is clean");
        assert!(self.form.events.is_empty(), "no change event");
    }
}

fn check_id(name: &str) -> WidgetId {
    FORM.sub(name)
}

fn option_id(group: &str, i: usize) -> WidgetId {
    RadioGroup::new(FORM.sub(group), "", &["a", "b", "c"], 0).option_id(i)
}

#[test]
fn form_change_refused_checkbox_has_no_event() {
    for name in ["fixed", "off"] {
        let id = check_id(name);
        let before = Rig::new().checked(name);

        let mut rig = Rig::new();
        assert!(rig.click(id).consumed(), "{name}: pointer click is routed");
        assert_eq!(rig.checked(name), before, "{name}: pointer leaves value");
        rig.untouched();

        let mut rig = Rig::new();
        rig.click_direct(id);
        assert_eq!(rig.checked(name), before, "{name}: direct click");
        rig.untouched();

        let mut rig = Rig::new();
        rig.key(id, KeyCode::Char(' '));
        assert_eq!(rig.checked(name), before, "{name}: Space leaves value");
        rig.untouched();
    }

    let mut rig = Rig::new();
    rig.click(check_id("fixed"));
    assert_eq!(
        take_refusal().map(|r| r.text).as_deref(),
        Some(WHY),
        "the fixed reason is still raised for the shell"
    );
    rig.key(check_id("fixed"), KeyCode::Char(' '));
    assert_eq!(take_refusal().map(|r| r.text).as_deref(), Some(WHY));
}

#[test]
fn form_change_same_radio_has_no_event() {
    let mut rig = Rig::new();
    let o = rig.click(option_id("mode", 0));
    assert_eq!(o, Outcome::Changed, "the child's repaint outcome is kept");
    assert_eq!(rig.selected("mode"), 0);
    assert_eq!(
        rig.focus.current(),
        Some(FORM.sub("mode")),
        "the click still focuses the group"
    );
    rig.untouched();

    let mut rig = Rig::new();
    rig.click_direct(option_id("mode", 0));
    assert_eq!(rig.selected("mode"), 0);
    rig.untouched();
}

#[test]
fn form_change_blocked_radio_has_no_event() {
    let mut rig = Rig::new();
    rig.click(option_id("mode", 2));
    assert_eq!(rig.selected("mode"), 0, "fixed option is not selected");
    assert_eq!(take_refusal().map(|r| r.text).as_deref(), Some(WHY));
    rig.untouched();

    let mut rig = Rig::new();
    rig.click_direct(option_id("mode", 2));
    assert_eq!(rig.selected("mode"), 0);
    rig.untouched();

    let mut rig = Rig::new();
    rig.click(option_id("locked", 1));
    assert_eq!(rig.selected("locked"), 0, "disabled group keeps selection");
    rig.untouched();

    let mut rig = Rig::new();
    rig.click_direct(option_id("locked", 1));
    assert_eq!(rig.selected("locked"), 0);
    rig.untouched();
}

#[test]
fn form_change_real_toggle_emits_once() {
    let mut rig = Rig::new();
    rig.click(check_id("live"));
    assert!(rig.checked("live"));
    assert!(rig.form.dirty);
    assert_eq!(rig.form.events, vec![FormAction::Changed("live".into())]);

    let mut rig = Rig::new();
    rig.key(check_id("live"), KeyCode::Char(' '));
    assert!(rig.checked("live"));
    assert!(rig.form.dirty);
    assert_eq!(rig.form.events, vec![FormAction::Changed("live".into())]);

    let mut rig = Rig::new();
    rig.click(option_id("mode", 1));
    assert_eq!(rig.selected("mode"), 1);
    assert!(rig.form.dirty);
    assert_eq!(rig.form.events, vec![FormAction::Changed("mode".into())]);

    let mut rig = Rig::new();
    rig.key(FORM.sub("mode"), KeyCode::Down);
    assert_eq!(rig.selected("mode"), 1);
    assert_eq!(rig.form.events, vec![FormAction::Changed("mode".into())]);

    let mut rig = Rig::new();
    rig.click(check_id("mixed"));
    assert!(rig.checked("mixed"), "a mixed box resolves to checked");
    assert_eq!(
        rig.form.events,
        vec![FormAction::Changed("mixed".into())],
        "leaving the mixed state is a value change"
    );
}

#[test]
fn form_change_dirty_form_refusal_stays_dirty() {
    let mut rig = Rig::new();
    rig.form.dirty = true;
    rig.click(check_id("fixed"));
    rig.click(option_id("mode", 0));
    rig.click(option_id("mode", 2));
    rig.key(check_id("fixed"), KeyCode::Char(' '));
    assert!(rig.form.dirty, "a refusal never cleans a dirty form");
    assert!(rig.form.events.is_empty(), "and adds no semantic event");
}

fn secret() -> WidgetId {
    FORM.sub("secret")
}

fn driver() -> WidgetId {
    FORM.sub("driver")
}

fn source_form() -> FormDialog {
    FormDialog::new(
        FORM,
        "Source",
        vec![
            FormField::input(
                "secret",
                TextInput::new(secret(), "Secret")
                    .value("hunter2")
                    .masked()
                    .reveal_tail(2)
                    .required(true)
                    .required_message("A secret is required")
                    .validator(|s| s.contains("bad").then(|| "No bad secrets".to_owned())),
            ),
            FormField::select(
                "driver",
                Select::new(driver(), "Driver", &["pg", "mysql", "sqlite"], 0),
            ),
            FormField::input("host", TextInput::new(FORM.sub("host"), "Host").value("db")),
        ],
    )
}

impl Rig {
    fn input(&self, name: &str) -> &TextInput {
        match &self.form.field(name).expect("field").kind {
            FormFieldKind::Input(i) => i,
            _ => panic!("not an input"),
        }
    }

    fn input_mut(&mut self, name: &str) -> &mut TextInput {
        match &mut self.form.field_mut(name).expect("field").kind {
            FormFieldKind::Input(i) => i,
            _ => panic!("not an input"),
        }
    }

    fn select(&self) -> &Select {
        match &self.form.field("driver").expect("field").kind {
            FormFieldKind::Select(s) => s,
            _ => panic!("not a select"),
        }
    }

    fn paste(&mut self, text: &str) -> Outcome {
        self.form.on_paste(text, &self.focus)
    }

    fn edit_secret(&mut self) {
        self.focus.focus(secret());
        self.draw();
        self.input_mut("secret").begin_edit();
        let field = &mut self.input_mut("secret").field;
        field.press(0, 1, false);
        field.drag(3);
        assert!(self.input("secret").editing && self.input("secret").field.pressed());
    }

    fn open_driver(&mut self) {
        self.click(driver());
        assert!(self.select().open, "the click opens the popup");
        self.draw();
    }

    fn popup_regions(&self) -> usize {
        let s = self.select();
        self.hits
            .regions()
            .iter()
            .filter(|r| s.contains_id(r.id) && r.id != s.id)
            .count()
    }
}

#[test]
fn form_hidden_input_rejects_paste_and_drag() {
    let mut rig = Rig::with(source_form());
    rig.edit_secret();
    rig.form.set_visible("secret", false);

    assert!(!rig.form.is_editing(), "hiding ends the edit session");
    assert!(
        !rig.input("secret").field.pressed(),
        "and the pointer capture"
    );
    assert_eq!(rig.input("secret").text(), "hunter2", "the text is kept");

    rig.paste("PASTED");
    rig.form.on_press(secret(), Position::new(3, 0), 1);
    rig.form.on_drag(secret(), Position::new(6, 0));
    rig.form.on_release(secret(), Position::new(6, 0));
    let o = rig.click_direct(secret());
    assert!(o.consumed(), "a stale click on a hidden field is swallowed");

    assert_eq!(rig.input("secret").text(), "hunter2");
    assert!(!rig.input("secret").field.buffer.has_selection());
    assert!(!rig.input("secret").editing);
    assert_ne!(
        rig.focus.current(),
        Some(secret()),
        "a click on a hidden field never focuses it"
    );
    rig.untouched();
}

fn type_into_secret(rig: &mut Rig, text: &str) {
    rig.focus.focus(secret());
    rig.draw();
    rig.input_mut("secret").begin_edit();
    for c in text.chars() {
        rig.key(secret(), KeyCode::Char(c));
    }
    assert!(rig.input("secret").is_dirty(), "the edit is uncommitted");
    assert!(rig.form.events.is_empty(), "typing reports no change yet");
}

#[test]
fn form_hiding_a_dirty_input_follows_its_blur_policy() {
    let mut rig = Rig::with(source_form());
    type_into_secret(&mut rig, "x");
    rig.form.set_visible("secret", false);
    assert_eq!(rig.input("secret").text(), "x", "commit keeps the edit");
    assert!(!rig.input("secret").editing);
    assert_eq!(
        rig.form.events,
        vec![FormAction::Changed("secret".into())],
        "and reports it like a blur commit"
    );
    assert!(rig.form.dirty);
    rig.form.set_visible("secret", false);
    assert_eq!(rig.form.events.len(), 1, "hiding twice reports once");

    let mut form = source_form();
    match &mut form.field_mut("secret").expect("field").kind {
        FormFieldKind::Input(i) => i.blur_action = BlurAction::Cancel,
        _ => panic!("not an input"),
    }
    let mut rig = Rig::with(form);
    type_into_secret(&mut rig, "x");
    rig.form.set_visible("secret", false);
    assert_eq!(
        rig.input("secret").text(),
        "hunter2",
        "cancel reverts the edit"
    );
    assert!(!rig.input("secret").editing);
    assert!(
        rig.form.events.is_empty(),
        "a reverted edit reports nothing"
    );
    rig.form.set_visible("secret", true);
    assert_eq!(
        rig.input("secret").text(),
        "hunter2",
        "the original value shows again"
    );
}

#[test]
fn form_hidden_select_closes_and_ignores_release() {
    let mut rig = Rig::with(source_form());
    rig.open_driver();
    assert!(rig.popup_regions() > 0, "the popup rows are hit regions");
    let row = rig.select().option_id(2);

    rig.form.set_visible("driver", false);
    assert!(!rig.select().open, "hiding closes the popup");
    rig.draw();
    assert_eq!(rig.popup_regions(), 0, "no popup hit region remains");
    assert!(
        rig.hits.regions().iter().all(|r| r.id != driver()),
        "the hidden select owns no hit region"
    );

    rig.form.on_press(row, Position::new(0, 0), 1);
    rig.form.on_release(row, Position::new(0, 0));
    let o = rig.click_direct(row);
    assert!(o.consumed());
    assert_eq!(rig.select().selected, 0, "the stale row chose nothing");
    assert!(!rig.select().open);
    rig.untouched();
}

#[test]
fn form_hidden_direct_field_mutation_reconciles() {
    let mut rig = Rig::with(source_form());
    rig.edit_secret();
    rig.form.field_mut("secret").expect("field").visible = false;
    rig.paste("PASTED");
    assert_eq!(rig.input("secret").text(), "hunter2", "paste is refused");
    assert!(
        !rig.input("secret").editing,
        "the next event ended the edit"
    );
    rig.untouched();

    let mut rig = Rig::with(source_form());
    rig.open_driver();
    rig.form.field_mut("driver").expect("field").visible = false;
    assert!(!rig.form.any_open_select(), "a hidden popup is not open");
    rig.draw();
    assert!(!rig.select().open, "the next render closed the popup");
    assert_eq!(rig.popup_regions(), 0);

    let mut rig = Rig::with(source_form());
    rig.edit_secret();
    rig.form.field_mut("secret").expect("field").visible = false;
    rig.draw();
    assert!(!rig.input("secret").editing, "render reconciles an input");
    assert!(!rig.input("secret").field.pressed());
    rig.untouched();
}

#[test]
fn form_hidden_reveal_preserves_value_and_policy() {
    let mut rig = Rig::with(source_form());
    rig.edit_secret();
    rig.input_mut("secret").set_error(Some("stale".into()));
    rig.open_driver();
    rig.form.set_visible("secret", false);
    rig.form.set_visible("driver", false);
    rig.draw();

    let values = rig.form.values();
    assert!(
        values.contains(&("secret".into(), FieldValue::Text("hunter2".into()))),
        "hidden values stay in FormValues"
    );
    assert!(values.contains(&("driver".into(), FieldValue::Choice(0))));

    rig.form.set_visible("secret", true);
    rig.form.set_visible("driver", true);
    rig.draw();
    let i = rig.input("secret");
    assert_eq!(i.text(), "hunter2");
    assert!(i.masked && i.required && i.reveal_tail == 2);
    assert!(!i.editing, "no restored edit session");
    assert!(!rig.select().open, "no restored popup");
    assert_eq!(rig.popup_regions(), 0);

    let i = rig.input_mut("secret");
    i.set_value("a bad one");
    assert!(!i.validate(), "the validator survived hide and reveal");
    assert_eq!(i.error(), Some("No bad secrets"));
    rig.untouched();
}

#[test]
fn form_hidden_focus_falls_back_and_escape_works() {
    let mut rig = Rig::with(source_form());
    rig.edit_secret();
    rig.form.set_visible("secret", false);
    assert!(
        !rig.input("secret").editing && !rig.input("secret").field.pressed(),
        "hiding ended the edit session"
    );
    assert_eq!(
        rig.focus.current(),
        Some(secret()),
        "focus still points at it"
    );

    rig.key(secret(), KeyCode::Esc);
    assert_eq!(
        rig.form.events,
        vec![FormAction::Cancel],
        "an idle form dismisses on Escape"
    );
    assert_eq!(
        rig.focus.current(),
        Some(driver()),
        "focus moved to the next visible field"
    );

    // The revealed field holds no session, so Escape dismisses the form
    // instead of rolling the field back and keeping it open.
    rig.form.events.clear();
    rig.form.set_visible("secret", true);
    rig.draw();
    assert!(!rig.input("secret").editing, "no restored edit session");
    rig.key(secret(), KeyCode::Esc);
    assert_eq!(
        rig.form.events,
        vec![FormAction::Cancel],
        "Escape on the revealed field dismisses the form"
    );

    let mut rig = Rig::with(source_form());
    rig.focus.focus(driver());
    rig.form.set_visible("driver", false);
    rig.form.set_visible("host", false);
    rig.key(driver(), KeyCode::Tab);
    assert!(
        rig.focus.current() != Some(driver()) && rig.focus.current() != Some(FORM.sub("host")),
        "no hidden field keeps focus"
    );
}

#[test]
fn form_hidden_layer_paste_has_one_focused_owner() {
    use crate::ui::layer::Layer;
    use crate::ui::layers::LayerResult;
    type L = dyn Layer<LayerResult, ()>;

    let mut rig = Rig::with(source_form());
    rig.focus.focus(secret());
    rig.draw();
    rig.input_mut("secret").begin_edit();

    let layer: &mut L = &mut rig.form;
    assert!(layer.on_paste("legacy").consumed());
    assert!(
        rig.form.events.is_empty(),
        "focusless paste emits no change"
    );
    assert_eq!(
        rig.input("secret").text(),
        "hunter2",
        "focusless paste is inert"
    );
    rig.untouched();

    rig.focus.focus(FORM.sub("host"));
    rig.paste("elsewhere");
    assert_eq!(rig.input("secret").text(), "hunter2", "focus is elsewhere");

    rig.focus.focus(secret());
    let focus = rig.focus.clone();
    let layer: &mut L = &mut rig.form;
    layer.on_paste_focused("-x", &focus);
    assert!(
        rig.input("secret").text().contains("-x"),
        "the focused input owns it"
    );
    rig.untouched();
}
