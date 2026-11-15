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

Tests: `widgets::edit::issue35_tests` (direct field matrix, counts validator calls), `widgets::input::issue35_tests::issue35_input_required_error_recovers` (Enter rejection, correction through AfterError, render), and `widgets::form_dialog::issue35_tests::issue35_form_save_blocked_by_required_before_custom` (Save blocked, focus returned to the field).

Limits: the TextInput regression covers `AfterError` and `Live` with `ValidateOn::EveryCommit`. `ValidateWhen::Commit` is not exercised by a regression test. The showcase combined example is the existing `owner` field on the inputs page, which pairs `required(true)` with the email validator. Its empty state appears after the user clears the value.
