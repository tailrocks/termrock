//! A validated input: a refused value never replaces the one the host holds.
//!
//! `cargo run -p termrock --example validated_input`

mod common;

use common::{frame, key};
use ratatui::crossterm::event::KeyCode;
use termrock::core::id::WidgetId;
use termrock::widgets::input::{InputAction, TextInput};

/// The host owns the accepted value. The field only proposes a new one.
struct Profile {
    handle: String,
    field: TextInput,
}

impl Profile {
    fn new(handle: &str) -> Self {
        let field = TextInput::new(WidgetId::of("example.handle"), "Handle")
            .required(true)
            .required_message("Enter a handle")
            .validator(|text| text.contains(' ').then(|| "No spaces".to_owned()))
            .value(handle);
        Self {
            handle: handle.to_owned(),
            field,
        }
    }

    /// Types `text` after the current value, then presses Enter.
    fn submit(&mut self, text: &str) {
        self.field.begin_edit();
        self.field.on_key(&key(KeyCode::End));
        for c in text.chars() {
            self.field.on_key(&key(KeyCode::Char(c)));
        }
        let (_, action) = self.field.on_key(&key(KeyCode::Enter));
        if action == Some(InputAction::Committed) && self.field.error().is_none() {
            self.handle = self.field.text().to_owned();
        }
    }

    fn screen(&mut self) -> String {
        let area = ratatui::layout::Rect::new(0, 0, 32, TextInput::HEIGHT + 1);
        let field = &mut self.field;
        let (screen, ()) = frame(area.width, area.height, |buf, ctx| {
            field.render(area, buf, ctx, ctx.theme.canvas);
        });
        format!("{screen}\nsaved handle: {}", self.handle)
    }
}

fn main() {
    let mut profile = Profile::new("alpha");
    profile.submit(" beta");
    println!("-- refused --\n{}", profile.screen());
    profile.field.set_value("alpha");
    profile.submit("2");
    println!("-- accepted --\n{}", profile.screen());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_invalid_value_is_refused_and_the_prior_value_kept() {
        let mut profile = Profile::new("alpha");
        profile.submit(" beta");
        assert_eq!(profile.field.error(), Some("No spaces"));
        assert_eq!(profile.handle, "alpha", "the refused value was not saved");
        assert!(profile.screen().contains("No spaces"));
    }

    #[test]
    fn a_valid_value_is_saved_and_clears_the_error() {
        let mut profile = Profile::new("alpha");
        profile.submit(" beta");
        profile.field.set_value("alpha");
        profile.submit("2");
        assert_eq!(profile.field.error(), None);
        assert_eq!(profile.handle, "alpha2");
    }

    #[test]
    fn a_required_field_refuses_an_emptied_value() {
        let mut profile = Profile::new("alpha");
        profile.field.begin_edit();
        profile.field.on_key(&key(KeyCode::End));
        for _ in 0..profile.handle.len() {
            profile.field.on_key(&key(KeyCode::Backspace));
        }
        profile.field.on_key(&key(KeyCode::Enter));
        assert_eq!(profile.field.error(), Some("Enter a handle"));
        assert_eq!(profile.handle, "alpha");
    }
}
