use std::panic::{AssertUnwindSafe, catch_unwind};

use ratatui::layout::Rect;
use termrock::core::id::WidgetId;
use termrock::widgets::choice_dialog::ChoiceDialog;
use termrock::widgets::file_browser::{FileBrowser, FileEntry, MemorySource};
use termrock::widgets::form_dialog::{FormDialog, FormField};
use termrock::widgets::info_dialog::InfoDialog;
use termrock::widgets::input::TextInput;
use termrock::widgets::props::Prop;

use crate::common::paint_in;

fn sweep(what: &str, mut render: impl FnMut(Rect)) {
    let mut panics = Vec::new();
    for w in 1..=14u16 {
        for h in 1..=14u16 {
            let screen = Rect::new(0, 0, w, h);
            if catch_unwind(AssertUnwindSafe(|| render(screen))).is_err() {
                panics.push((w, h));
            }
        }
    }
    let widest = panics.iter().map(|&(w, _)| w).max();
    let tallest = panics.iter().map(|&(_, h)| h).max();
    assert!(
        panics.is_empty(),
        "{what} panics on {} screens, widest {widest:?}, tallest {tallest:?}, first {:?}",
        panics.len(),
        panics.first()
    );
}

#[test]
fn a_choice_dialog_survives_a_screen_too_small_for_its_frame() {
    sweep("ChoiceDialog", |screen| {
        let mut d = ChoiceDialog::new(WidgetId::of("t.choice"), "Pick", "Kind", &["one", "two"], 0);
        paint_in(screen, None, |buf, ctx| d.render(screen, buf, ctx));
    });
}

#[test]
fn a_form_dialog_survives_a_screen_too_small_for_its_frame() {
    sweep("FormDialog", |screen| {
        let id = WidgetId::of("t.form");
        let field = FormField::input("name", TextInput::new(id.sub("name"), "Name"));
        let mut d = FormDialog::new(id, "Account", vec![field]);
        paint_in(screen, None, |buf, ctx| d.render(screen, buf, ctx));
    });
}

#[test]
fn an_info_dialog_survives_a_screen_too_small_for_its_frame() {
    sweep("InfoDialog", |screen| {
        let mut d = InfoDialog::new(
            WidgetId::of("t.info"),
            "Info",
            vec![Prop::new("Key", "value")],
        );
        paint_in(screen, None, |buf, ctx| d.render(screen, buf, ctx));
    });
}

#[test]
fn a_file_browser_survives_a_screen_too_small_for_its_frame() {
    sweep("FileBrowser", |screen| {
        let mut d = FileBrowser::new(
            WidgetId::of("t.browser"),
            "Choose",
            MemorySource::new(vec![FileEntry::dir("/src/app")]).home("/src"),
            "/src",
        );
        paint_in(screen, None, |buf, ctx| d.render(screen, buf, ctx));
    });
}

/// A terminal that reports no rows leaves an empty buffer; a framed dialog
/// used to write its title row into it.
#[test]
fn framed_dialogs_survive_an_empty_buffer() {
    for w in [0u16, 7, 40] {
        let screen = Rect::new(0, 0, w, 0);
        let mut choice = ChoiceDialog::new(
            WidgetId::of("t.choice0"),
            "Pick",
            "Kind",
            &["one", "two"],
            0,
        );
        paint_in(screen, None, |buf, ctx| choice.render(screen, buf, ctx));

        let id = WidgetId::of("t.form0");
        let field = FormField::input("name", TextInput::new(id.sub("name"), "Name"));
        let mut form = FormDialog::new(id, "Account", vec![field]);
        paint_in(screen, None, |buf, ctx| form.render(screen, buf, ctx));

        let mut files = FileBrowser::new(
            WidgetId::of("t.files0"),
            "Choose",
            MemorySource::new(vec![FileEntry::dir("/a")]),
            "/a",
        );
        paint_in(screen, None, |buf, ctx| files.render(screen, buf, ctx));
    }
}
