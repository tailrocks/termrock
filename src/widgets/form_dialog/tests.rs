use super::*;
use crate::core::event::Key;
use crate::core::hit::HitRegistry;
use crate::core::refusal::{Reason, clear_refusal, take_refusal};
use crate::theme::{ColorLevel, Theme};
use crate::ui::ctx::{Interaction, RenderCtx};
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
        clear_refusal();
        let mut rig = Self {
            form: form(),
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
