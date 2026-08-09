//! Source-owned FormWizard block (Plan 053).

use termrock::input::KeyEvent;
use termrock::widgets::{FormWizardOutcome, FormWizardState};

/// Create a wizard with N steps.
#[must_use]
pub fn new_wizard(steps: usize) -> FormWizardState {
    FormWizardState::new(steps)
}

/// Project validity from the app, then route keys.
pub fn handle_key(state: &mut FormWizardState, key: KeyEvent, step_valid: bool) -> FormWizardOutcome {
    state.set_step_valid(step_valid);
    state.handle_key(key)
}
