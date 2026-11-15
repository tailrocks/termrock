<!-- SPDX-License-Identifier: LicenseRef-Proprietary -->

# Required before custom validator (#35)

Defect: `EditField::validate` checked a custom `Validator` only when `required` was unset. `TextInput::required(true).validator(v)` therefore accepted empty text whenever `v` accepted it, and a validator that checks only length or characters let an empty required value through.

Fix, in `src/widgets/edit.rs`: `EditField::validate` runs the required check first. An empty buffer fails with the required message and the validator is not called. Otherwise the validator runs once and its result replaces the previous error. `TextInput::validate` is unchanged and still syncs `required` before it calls the field.

Rules:

- Required means zero bytes. Whitespace-only text is not empty, so the validator decides it. An app that rejects blank text writes that rule in its validator.
- `ValidateWhen` and `ValidateOn` still decide when validation runs. They do not change which rules apply.
- A valid value clears the previous error on the next validation.

Consumers migrated, because each set `required(true)` with a validator that also rejected empty text:

- Jackin: `screens/config.rs` (env key), `screens/roles.rs` (location), `screens/role_editor.rs` (role name, path, entry name) and `screens/editor.rs` (workspace name). Each sets `required_message` to the text its validator showed for empty input, so the visible message does not change. `path_input` sets it only for required fields.
- Showcase: `pages/dialogs.rs` (rename), `pages/selectable.rs` (name and email) keep their previous empty-input text through `required_message`. `pages/settings.rs` (`var_validator`) and `pages/inputs.rs` (`email`) drop their empty branch, which the required check now owns.
- TablePro connection form: `name` already showed `Required` for empty input, the library default, so it needs no change. Its validator keeps the blank-text rule.
- FormDialog: `submit` validates every visible input through `TextInput::validate`, so the new order applies there without a change.

Not changed: library widgets other than `EditField::validate`, layout, colours, focus language and the `visual-baseline` tag.

Tests: `widgets::edit::issue35_tests` (direct field matrix, counts validator calls), `widgets::input::issue35_tests::issue35_input_required_error_recovers` (Enter rejection, correction through AfterError and Live, render), `widgets::input::issue35_tests::issue35_input_commit_mode_validates_only_on_commit` (`ValidateWhen::Commit`: keystrokes do not run the custom rule, the required error appears only on commit, and a corrected commit runs it once), `widgets::input::issue35_tests::issue35_input_cancel_skips_custom_validation` (Esc leaves edit mode without running the custom rule or validating), `widgets::input::issue35_tests::issue35_required_after_error_uses_current_configuration` (message and validator replaced after an error), and `widgets::form_dialog::issue35_tests::issue35_form_save_blocked_by_required_before_custom` (Save blocked, focus returned to the field).

Limits: the cancel regression uses the default `ValidateOn::Change`. Under `ValidateOn::EveryCommit`, `cancel` still validates the restored text, as it did before this change. The Showcase demonstration is the `Owner rule` option on the Inputs page Options tab (`inputs.owner_rule`, `email` by default). Set to `max-length`, the owner field pairs `required(true)` with a length-only validator that accepts empty text. Clearing the field and pressing Enter shows `Required`, and a value over 24 characters shows the validator's message, so the required check runs first and the custom rule runs only on nonempty text. The functional case `s02_inputs::option_owner_rule_required_precedes_length_validator` drives this through the PTY and fails on the validator-first order. The extra option row deliberately changes `sc_inputs_options_80x24` (shorter scrollbar thumb) and `sc_inputs_options_120x40` (the new row), accepted through `scripts/accept-baseline.py` with receipts under `docs/testing/acceptance-receipts/showcase-owner-rule-35-20261115/`.
