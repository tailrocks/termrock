//! Helpers shared by the TablePro test modules.

use junie_tui::widgets::dialog::{Dialog, DialogBody};

use super::App;

/// The statements listed by the review dialog that is open on top.
pub(crate) fn review_code(app: &App) -> Vec<String> {
    match &app
        .layers
        .top_as::<Dialog>()
        .expect("the review dialog is open")
        .body
    {
        DialogBody::Facts { code, .. } => code.clone(),
        _ => panic!("the dialog lists statements"),
    }
}
