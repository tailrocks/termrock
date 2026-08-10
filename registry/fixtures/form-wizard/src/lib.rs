//! Source-owned FormWizard block (Plan 053).

use termrock::input::KeyEvent;
use termrock::widgets::{FormWizardOutcome, FormWizardState};

/// Create a wizard with N steps.
#[must_use]
pub fn new_wizard(steps: usize) -> FormWizardState {
    let mut state = FormWizardState::new(steps);
    state.set_focused(true);
    state
}

/// Project validity from the app, then route keys.
pub fn handle_key(state: &mut FormWizardState, key: KeyEvent, step_valid: bool) -> FormWizardOutcome {
    state.set_step_valid(step_valid);
    state.set_focused(true);
    state.handle_key(key)
}
