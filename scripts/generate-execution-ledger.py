#!/usr/bin/env python3
"""Generate the pre-refactor execution ledger and coverage inventory.

This script writes documentation only. It does not change production code
or test behavior. Run from the repository root.
"""

from __future__ import annotations

import json
import re
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs" / "testing"
REF = "fffe002e936d3621f22f9132144066a1374d5d74"
MODEL = "Grok 4.7"
SIZES = ((120, 40), (80, 24))
SECTIONS = ("playground", "options", "states")
SPECIMEN = (
    "rest",
    "hover",
    "pressed",
    "focused",
    "focused_hover",
    "selected",
    "disabled",
    "busy",
    "error",
    "editing",
    "empty",
    "loading",
    "overflow",
    "narrow",
)
CATALOG_STATES = [s for s in SPECIMEN if s != "editing"]


def binary_baseline_inventory(baseline_root: Path, repository_root: Path) -> list[dict]:
    """List active binary baselines under the root, excluding frozen history."""
    baselines = []
    for path in sorted(baseline_root.rglob("*.tui.pb")):
        relative = path.relative_to(baseline_root)
        if relative.parts[0] == "pre_refactor":
            continue
        test_id = relative.as_posix()[: -len(".tui.pb")]
        baselines.append(
            {
                "path": str(path.relative_to(repository_root)),
                "test_id": test_id,
                "active": True,
            }
        )
    return baselines


# slug, label, page source, group card, static option array
ROUTES = [
    ("overview", "Overview", "overview.rs", "S01", "OVERVIEW"),
    ("chrome", "Chrome", "chrome.rs", "S01", "CHROME"),
    ("statushints", "Status & hints", "status_hints.rs", "S01", "STATUS_HINTS"),
    ("breadcrumbsrails", "Breadcrumbs & rails", "breadcrumbs_rails.rs", "S01", "BREADCRUMBS_RAILS"),
    ("helpkeys", "Help & keys", "help_keys.rs", "S01", "HELP_KEYS"),
    ("quitleave", "Quit & leave", "quit_leave.rs", "S01", "QUIT_LEAVE"),
    ("focusnavigation", "Focus & navigation", "focus_navigation.rs", "S01", "FOCUS_NAV"),
    ("buttons", "Buttons", "buttons.rs", "S02", "BUTTONS"),
    ("inputs", "Inputs", "inputs.rs", "S02", "INPUTS"),
    ("textareas", "Text areas", "textareas.rs", "S02", "TEXTAREAS"),
    ("chipsselects", "Chips & selects", "chips.rs", "S02", "CHIPS"),
    ("forms", "Forms", "forms.rs", "S02", "FORMS"),
    ("checksradios", "Checks & radios", "checks_radios.rs", "S02", "CHECKS"),
    ("toolbars", "Toolbars", "toolbars.rs", "S02", "TOOLBARS"),
    ("basictable", "Basic table", "tables.rs", "S03", "BASIC_TABLE"),
    ("editable", "Editable", "editable.rs", "S03", "EDITABLE"),
    ("datagrid", "Data grid", "grid.rs", "S03", "DATA_GRID"),
    ("selection", "Selection", "selectable.rs", "S03", "SELECTION"),
    ("celleditors", "Cell editors", "celleditors.rs", "S03", "CELL_EDITORS"),
    ("columns", "Columns", "columns.rs", "S03", "COLUMNS"),
    ("lists", "Lists", "lists.rs", "S04", "LISTS"),
    ("trees", "Trees", "trees.rs", "S04", "TREES"),
    ("sidebars", "Sidebars", "sidebars.rs", "S04", "SIDEBARS"),
    ("tabs", "Tabs", "tabs.rs", "S04", "TABS"),
    ("menus", "Menus", "menus.rs", "S04", "MENUS"),
    ("dialogs", "Dialogs", "dialogs.rs", "S05", "DIALOGS"),
    ("pickers", "Pickers", "pickers.rs", "S05", "PICKERS"),
    ("dialogkinds", "Dialog kinds", "dialog_kinds.rs", "S05", "DIALOG_KINDS"),
    ("popupstooltips", "Popups & tooltips", "popups_tooltips.rs", "S05", "POPUPS_TOOLTIPS"),
    ("filebrowser", "File browser", "file_browser.rs", "S05", "FILE_BROWSER"),
    ("progress", "Progress", "progress.rs", "S06", "PROGRESS"),
    ("emptyerror", "Empty & error", "empty_error.rs", "S06", "EMPTY_ERROR"),
    ("meters", "Meters", "meters.rs", "S06", "METERS"),
    ("notices", "Notices", "notices.rs", "S06", "NOTICES"),
    ("panels", "Panels", "panels.rs", "S07", "PANELS"),
    ("scrolling", "Scrolling", "scrolling.rs", "S07", "SCROLLING"),
    ("splitsstacks", "Splits & stacks", "splits_stacks.rs", "S07", "SPLITS_STACKS"),
    ("masterdetail", "Master-detail", "master_detail.rs", "S07", "MASTER_DETAIL"),
    ("panetree", "Pane tree", "pane_tree.rs", "S07", "PANE_TREE"),
    ("codeeditor", "Code editor", "editor.rs", "S08", "CODE_EDITOR"),
    ("diff", "Diff", "diff.rs", "S08", "DIFF"),
    ("terminal", "Terminal", "terminal.rs", "S08", "TERMINAL"),
    ("textutilities", "Text utilities", "text_utilities.rs", "S08", "TEXT_UTILITIES"),
    ("settings", "Settings", "settings.rs", "S09", "SETTINGS"),
    ("taskrunner", "Task runner", "taskrunner.rs", "S09", "TASKRUNNER"),
    ("queryworkbench", "Query workbench", "query_workbench.rs", "S09", "QUERY_WORKBENCH"),
    ("connections", "Connections", "connections.rs", "S09", "CONNECTIONS"),
    ("finder", "Finder", "finder.rs", "S09", "FINDER"),
    ("diskcleanup", "Disk cleanup", "disk_cleanup.rs", "S10", "DISK_CLEANUP"),
    ("activity", "Activity", "activity.rs", "S10", "ACTIVITY"),
    ("workspaces", "Workspaces", "workspaces.rs", "S10", "WORKSPACES"),
    ("accountsusage", "Accounts & usage", "accounts_usage.rs", "S10", "ACCOUNTS_USAGE"),
    ("launchcockpit", "Launch cockpit", "launch_cockpit.rs", "S10", "LAUNCH_COCKPIT"),
    ("readonlyreport", "Read-only report", "readonly_report.rs", "S10", "READONLY_REPORT"),
]

# Route-specific interactions: slug -> list of (suffix, action, expected, forbidden)
ROUTE_CASES = {
    "overview": [
        ("tokens", "Open each token, plane, kind, ladder, modifier, and contrast example.", "The named example is the visible sample.", "A different token's swatch is not shown as the selected example."),
    ],
    "chrome": [
        ("menus", "Open every Chrome menu by pointer and by keyboard and invoke each enabled command.", "The command's status or screen change matches that command.", "A disabled command does not change application state."),
        ("menu-switch", "Move the pointer and the keyboard from one open menu to the next.", "The newly pointed menu is the only open menu.", "The previous menu's command does not run."),
    ],
    "statushints": [
        ("status", "Post success, error, and progress through the page controls.", "Each status text appears, a progress status expires, and an error stays while the page says it is persistent.", "An expired success message is still visible after its lifetime."),
        ("hints", "Exercise hint priority, hidden hints, width, and the too-small option.", "The visible hint set matches the selected priority and width.", "A hidden hint remains visible."),
    ],
    "breadcrumbsrails": [
        ("navigate", "Enter a folder, return with Backspace, and return with a breadcrumb.", "The visible path is the folder after enter and the parent after each return.", "The rail selection changes when the path does not."),
        ("rail", "Collapse and expand the rail.", "The selected path is the same path after expand.", "Collapse drops the selected path."),
    ],
    "helpkeys": [
        ("help", "Open page help and global help separately, scroll help, and dismiss it.", "Each help layer shows its own title, and focus returns to the control that opened it.", "Help keys activate the page underneath."),
    ],
    "quitleave": [
        ("leave", "Exercise clean leave, dirty Stay, Cancel, Discard, and Save, and the running Stop and Stay choices.", "Each choice produces its named result and dirty Save keeps the edited value.", "Cancel or Stay discards the edit."),
    ],
    "focusnavigation": [
        ("ring", "Tab and Shift+Tab across the whole focus ring, then first-click a control.", "Focus visits every reachable control in both directions and wraps.", "A second focus gutter appears."),
    ],
    "buttons": [
        ("press", "Hover, hold, release inside, and release outside each distinct button family and tone, including busy, icon, toggle, and overflow actions.", "Release inside runs the action once. Release outside does not. Held styling differs from rest and hover.", "A disabled button changes value or runs its action."),
    ],
    "inputs": [
        ("edit", "Exercise each entry mode and blur policy, then selection, paste, undo, redo, validation, and secret masking.", "The applied value matches the typed or pasted text, and Escape or blur follows the declared policy.", "A secret value is shown in clear text."),
    ],
    "textareas": [
        ("multiline", "Insert and paste multiple lines, select a line, scroll, commit, cancel, Tab, and try a read-only area.", "Committed text stays and cancelled text reverts. Tab leaves a read-only area without editing it.", "Read-only text changes."),
    ],
    "chipsselects": [
        ("chips", "Add, remove, clear, activate, and toggle chips, including overflow and the close hit target.", "The chip set and the activated chip identity match the action.", "Closing one chip removes a different chip."),
        ("selects", "Choose, type-ahead, scroll, and dismiss each select, including a disabled select and a mixed value.", "The committed choice is the highlighted row. Dismissal keeps the previous choice.", "A disabled select changes its value."),
    ],
    "forms": [
        ("submit", "Submit an invalid form, correct it, submit the valid form, and reset.", "Invalid submit shows the required error and does not apply. Valid submit shows the exact field values. Reset restores the exact defaults.", "Reset keeps the submitted values."),
    ],
    "checksradios": [
        ("choice", "Move mixed, checked, and unchecked values, and change the radio by key and by pointer.", "Only one radio in the group is on. The checkbox hit target includes its label.", "A disabled choice changes."),
    ],
    "toolbars": [
        ("actions", "Run, stop, use shortcuts and toggles, and exercise alignment, wrapping, and overflow.", "Each action happens once and the overflow item is the item that was chosen.", "Stop runs when the demonstration is idle, or Run starts a second job."),
    ],
    "basictable": [
        ("sort", "Sort ascending, descending, and back to the original order. Select a row and activate a row separately.", "The visible row identities are in the expected order after each sort. Activation is not selection.", "The saved order is discarded or a sort status appears without the order changing."),
    ],
    "editable": [
        ("cells", "Edit, validate, commit, and cancel a cell. Enter an out-of-range number and recover. Paste and move across cells.", "Commit stores the typed value. Cancel restores the previous value. An invalid number does not replace the stored value.", "Blur commits when the page's blur policy says cancel."),
    ],
    "datagrid": [
        ("pending", "Insert, duplicate, delete, and restore rows. Refresh and request more rows. Open the SQL preview and the pending review. Cancel, save, and discard.", "Pending counts match the edits. Cancel keeps the edits. Save clears them. Discard restores the original row values.", "A read-only grid accepts a mutation."),
    ],
    "selection": [
        ("policies", "Exercise none, single, and multiple selection, range, toggle, and Select All with a disabled row. Move the cursor without replacing the set. Apply and revert a mixed bulk field.", "The selected identities match the policy. Cursor movement keeps the set. Select All skips disabled rows.", "A disabled row becomes selected."),
    ],
    "celleditors": [
        ("editors", "Open every constrained cell-editor kind. Enter an out-of-set value, type-ahead, cancel, and check popup placement.", "The editor shows only its allowed values. Cancel keeps the cell. Accept stores the chosen value.", "The popup accepts a value outside its set."),
    ],
    "columns": [
        ("seams", "Resize columns inside their limits. Exercise fixed columns, horizontal overflow, and the column menu.", "Only the dragged seam's cells change style. Width stays inside the declared limits.", "A neighbouring rule or padding cell takes the drag colour."),
    ],
    "lists": [
        ("select", "Range-select, toggle, Select All, skip a disabled item, type-ahead, and context-select a selected and an unselected row.", "The selected set matches each gesture. After blur the accent selection remains.", "Context-selecting an already selected row replaces the set."),
    ],
    "trees": [
        ("branches", "Load a lazy branch, toggle it, activate it, expand recursively, and collapse.", "The final children appear after loading. Collapse hides them and expand shows the same nodes.", "Activation of a branch toggles it when the page separates those actions."),
    ],
    "sidebars": [
        ("sidebar", "Collapse the sidebar, hide the rail, read badges, and recover the selection.", "The selected item is the same item after the sidebar returns.", "Collapse selects a different row."),
    ],
    "tabs": [
        ("tabs", "Create, switch, and close tabs. Cancel, save, and discard a dirty tab. Use overflow, middle-click, and digit jumps.", "Each tab keeps its own body. Closing one dirty tab does not close another. The selected accent is neutral under an overlay.", "A reused index activates a different tab."),
    ],
    "menus": [
        ("menus", "Open each menu by keyboard and pointer. Move across separators, disabled rows, type-ahead, release selection, drag selection, placement, and each dismissal policy.", "The chosen row is the row that was released inside. Dismissal keeps the previous value.", "A disabled row runs."),
    ],
    "dialogs": [
        ("dialogs", "Open every dialog variant, including confirm, rename, unsaved, delete, facts, typed-drop, retry, and release notes. Enter invalid and valid prompts and both typed-confirmation stages.", "Confirm resolves the named action. Cancel restores the opener's focus. Background controls do not activate.", "A click reaches a covered control."),
    ],
    "pickers": [
        ("pickers", "Open Quick, Tabs, Level, Commands, and Remote pickers. Filter, clear, change scope, and exercise loading, error, retry, and nonsearchable states.", "The chosen row is the filtered row. Empty filter shows the empty state. Cancel keeps the previous choice.", "An error state accepts a selection as success."),
    ],
    "dialogkinds": [
        ("kinds", "Open Form, Info, Choice, and Help kinds. Exercise each form field and the nested key chooser. Run failure, success, save, cancel, and keep-open.", "The nested chooser is the top focus owner. Save returns the field values. Keep-open leaves the dialog up.", "The form under the chooser receives keys."),
    ],
    "popupstooltips": [
        ("overlays", "Choose and dismiss a popover. Show a delayed tooltip by pointer and by keyboard. Exercise focus-out, outside-click, and pass-through. Move the anchor while the overlay is open.", "The overlay follows the policy under test and the anchor move.", "Pass-through activates a control the policy says to dismiss."),
    ],
    "filebrowser": [
        ("browse", "Enter a directory, return to the parent, toggle hidden entries, filter, switch selection mode, and cancel. Enter an empty, invalid, and valid URL.", "The visible directory is the opened path. Cancel does not change the caller's path.", "Hidden entries stay visible after they are turned off."),
    ],
    "progress": [
        ("progress", "Start, pause, resume, restart, and complete determinate, indeterminate, and aggregate progress.", "The numeric value and the available actions match the phase.", "A completed bar still shows the running action as the only action."),
    ],
    "emptyerror": [
        ("empty", "Activate New, Import, and Retry from the empty or error state. Refuse a read-only mutation. Narrow the actions until they overflow.", "The activated action's result is visible. Read-only refusal shows its message.", "New runs while the state is read-only."),
    ],
    "meters": [
        ("meters", "Start a refresh, observe the busy state and the final values, then refuse refresh while offline. Show warning, stale, and detail tones.", "Final numbers replace the busy state. Offline does not start a refresh.", "Offline refresh changes the values."),
    ],
    "notices": [
        ("notices", "Cycle every connection-notice outcome. Open details and exercise hints, line limits, and truncation marks.", "The visible outcome is the one that was selected. Truncation shows its mark.", "A truncated notice drops the mark and still clips the line."),
    ],
    "panels": [
        ("panels", "Scroll adjacent and nested panels on both axes.", "The scrolled panel's first visible line changes and the neighbour does not.", "Scrolling one panel moves the other panel's content."),
    ],
    "scrolling": [
        ("scroll", "Track-click, hold the track, drag the thumb, wheel, follow the tail, and type-ahead.", "The thumb and the first visible row move together. Type-ahead wins over a wheel binding when the page says so.", "A neighbour pane scrolls."),
    ],
    "splitsstacks": [
        ("splits", "Resize horizontal and vertical splits, hit the minimum, double-click where supported, and zoom and restore.", "The seam stays inside its minimum. Only that seam's cells change style while dragged.", "Zoom destroys the other pane's content."),
    ],
    "masterdetail": [
        ("detail", "Select a master row, open its detail, cross the collapse breakpoint both ways, and open and close the narrow drawer.", "The detail identity is the selected master row after both breakpoint crossings.", "The drawer shows a different row than the selection."),
    ],
    "panetree": [
        ("panes", "Split, focus, resize, close, and zoom a leaf.", "Unaffected leaves keep their text. Focus and selection survive a resize.", "Closing one leaf focuses an unrelated leaf without the documented fallback."),
    ],
    "codeeditor": [
        ("edit", "Enter, edit, commit, and revert. Select by grapheme, copy, paste, undo, and redo. Move find and accept or dismiss completion. Show diagnostics.", "The buffer text and caret match the edit. Completion uses the owner's focus policy.", "A read-only buffer accepts typing."),
    ],
    "diff": [
        ("diff", "Switch files and unified, review, and empty modes. Scroll, select, copy, and narrow the view.", "The visible diff is the selected file and mode. Narrow clipping keeps the selected line identity.", "Empty mode shows the previous file's hunks."),
    ],
    "terminal": [
        ("run", "Run a successful demonstration and a failed one. Select a step, toggle follow, show find marks, and wrap or scroll horizontally.", "Success and failure are different visible outcomes. Follow keeps the latest line in view.", "A failed run is shown as success."),
    ],
    "textutilities": [
        ("text", "Activate and copy property rows. Exercise fit, wrap, case, duration, and reveal.", "The copied or revealed text is the row's text.", "Reveal shows a different row's value."),
    ],
    "settings": [
        ("settings", "Visit every Settings tab, change a field, and exercise dirty Cancel, Save, and Discard. Invite, edit, and remove a member. Add and remove an environment value.", "Reopening shows the saved value. Discard restores the previous value.", "Cancel stores the edit."),
    ],
    "taskrunner": [
        ("pipeline", "Select targets and run the pipeline. Watch stage, log, and progress. Refuse cancellation, then confirm it, then restart.", "The running stage identity changes. Confirmed cancel stops that run. Restart starts one new run.", "An unconfirmed cancel stops the pipeline."),
    ],
    "queryworkbench": [
        ("queries", "Expand the explorer, open a table, and create, switch, and close query tabs. Run, explain, stop, and clear. Show rows, plan, and error.", "Each tab keeps its own SQL and result. The error is the failed statement's error.", "Switching tabs shows another tab's result as this tab's result."),
    ],
    "connections": [
        ("connections", "Create, edit, test, save, cancel, duplicate, and delete a connection. Filter, and show empty, loading, and failure.", "Reopen shows the saved fields. Cancel keeps the previous record. Delete removes only the chosen connection.", "A failed test marks the connection saved when it was not."),
    ],
    "finder": [
        ("finder", "Edit a query, change scope, activate a result, and open Alternatives, Why, preview, and no results.", "The activated result is the selected row. Empty query results do not run the previous row.", "Why describes a different row than the selection."),
    ],
    "diskcleanup": [
        ("cleanup", "Rescan, select targets, dry-run, and pass both confirmation stages with a wrong phrase and the exact phrase.", "The wrong phrase does not remove items. The exact phrase removes only the selected demonstration items.", "Dry run deletes a demonstration item."),
    ],
    "activity": [
        ("activity", "Filter, follow, find, and copy the stream. Stop and restart.", "Stopping and stopped are different visible states. Restart creates a new run rather than rewriting the stopped one.", "Stop reports stopped before the running state ends."),
    ],
    "workspaces": [
        ("workspaces", "Select a workspace and a roster entry. Launch, edit, prewarm, and delete. Collapse the detail and return.", "Post-delete selection follows the documented fallback. The detail identity matches the selected workspace.", "Delete removes a workspace that was not selected."),
    ],
    "accountsusage": [
        ("accounts", "Select an account and read its facts and usage. Copy, refresh, choose the default, and show disabled, offline, and warning.", "The facts belong to the selected account. Offline refresh does not replace the last values with an error value from another account.", "A disabled account becomes the default."),
    ],
    "launchcockpit": [
        ("launch", "Run the stages to success. Show credential failure, retry, build failure, and cancellation.", "The visible stage is the stage that failed or succeeded. Cancellation does not attach a success result.", "Retry skips the failed stage and shows success."),
    ],
    "readonlyreport": [
        ("report", "Refresh the report, refuse refresh while offline, copy one fact and the whole report, and open a related screen and return.", "The related screen is the one named by the fact. Return shows the same report.", "Offline refresh changes the figures."),
    ],
}


def parse_options() -> dict[str, list[dict]]:
    text = (ROOT / "src/bin/showcase/options.rs").read_text()
    groups: dict[str, list[dict]] = defaultdict(list)
    current = None
    # Associate each Opt::new with the static array that contains it.
    array_at = [(m.start(), m.group(1)) for m in re.finditer(r"(?:pub )?static ([A-Z_]+): &\[Opt\]", text)]
    for match in re.finditer(
        r'Opt::new\(\s*"([^"]+)"\s*,\s*"((?:\\.|[^"\\])*)"\s*,\s*(&\[[^\]]*\]|OFF_ON)',
        text,
    ):
        key, label, values_src = match.group(1), match.group(2), match.group(3)
        name = None
        for pos, candidate in array_at:
            if pos < match.start():
                name = candidate
            else:
                break
        window = text[match.start() : match.start() + 500]
        needs = re.findall(r'\("([^"]+)", "([^"]+)"\)', window.split("Opt::new", 1)[0] if False else window)
        # needs() applies to this opt when it appears before the next Opt::new.
        head = window.split("Opt::new(", 1)[0]
        # The needs clause is AFTER this Opt::new and before the next one. `window` starts at this Opt.
        tail = text[match.end() : match.end() + 240]
        tail = tail.split("Opt::new", 1)[0]
        needs = re.findall(r'\("([^"]+)", "([^"]+)"\)', tail)
        effect = "behaviour" if ".behaviour()" in tail else "frame"
        if values_src == "OFF_ON":
            values = ["off", "on"]
        else:
            values = re.findall(r'"([^"]+)"', values_src)
        groups[name].append(
            {
                "key": key,
                "label": label,
                "values": values,
                "effect": effect,
                "needs": needs,
                "array": name,
            }
        )
    return groups


def row(
    case_id: str,
    owner: str,
    application: str,
    kind: str,
    **extra,
) -> dict:
    base = {
        "case_id": case_id,
        "owner_task": owner,
        "application": application,
        "kind": kind,
        "result": "BLOCKED",
        "disposition_reason": "Assigned in the execution ledger. Not executed yet.",
        "reference_commit": REF,
        "execution_commit": None,
        "evidence_paths": [],
        "baseline_paths": [],
    }
    base.update(extra)
    return base


def main() -> None:
    options = parse_options()
    by_array = options
    tasks: list[dict] = []
    rows: list[dict] = []

    def add_task(task: dict) -> None:
        tasks.append(task)

    def add_rows(items: list[dict]) -> None:
        rows.extend(items)

    # Prerequisite tasks.
    prereq = [
        (
            "G00",
            "A",
            [],
            ["docs/testing/tasks/G00.json", "docs/testing/reference-manifest.json"],
            "Record REFERENCE_SHA, tool digests, the missing visual-baseline tag, the Nextest list, and the existing gate results. Do not edit production or test behaviour.",
        ),
        (
            "G01",
            "A",
            ["G00"],
            ["docs/testing/refactor-coverage.json", "docs/testing/known-gaps.md", "docs/testing/execution-ledger.md"],
            "Every implemented screen, section, option, scenario, and specimen state has one owner and one coverage row. NOT_APPLICABLE is unused for an implemented state that has no test.",
        ),
        (
            "G02",
            "B",
            ["G01"],
            ["tests/functional/support.rs", "tests/functional/cases/g02_harness.rs"],
            "stable_screen fails when 700ms of stability is not reached. The fingerprint covers comparator fields. A dead child fails a wait. Unsupported clock paste is not sent. Negative cases use a temporary baseline store.",
        ),
        (
            "G05",
            "B",
            ["G00", "G01", "G02"],
            ["docs/testing/reference-manifest.json", "tests/functional/support.rs"],
            "Ordinary runs read CARGO_BIN_EXE and cannot accept baselines. Reference capture is a separate mode aimed at the unchanged reference binaries. Every new baseline records source, binary digest, fixture, environment, and comparator.",
        ),
        (
            "G03",
            "B",
            ["G05"],
            [
                "tests/functional/showcase.rs",
                "tests/functional/tablepro.rs",
                "tests/functional/holla.rs",
                "tests/functional/jackin.rs",
                "tests/functional/lifecycle.rs",
                "tests/functional/visual.rs",
            ],
            "Each repaired test name matches the actions performed. Keyboard activation starts from a fresh control. Sort compares row identities. The form resets and submits. The dialog cycles focus. TablePro, Holla, and Jackin assertions require the real result, and narrow checkpoints are captured before the terminal grows.",
        ),
        (
            "G04",
            "B",
            ["G01", "G03"],
            ["scripts/check-refactor-coverage.py", "docs/testing/coverage-schema.json"],
            "Controlled fixtures fail when a route, test, checkpoint receipt, or baseline is removed, and when a required result is not PASS. The full repository inventory is not required to pass in G04.",
        ),
    ]
    for task_id, wave, deps, files, acceptance in prereq:
        add_task(
            {
                "task_id": task_id,
                "wave": wave,
                "kind": "prerequisite",
                "model": MODEL,
                "reference_commit": REF,
                "dependencies": deps,
                "owned_files": files,
                "forbidden_shared_edits": ["src/"],
                "source_inputs": [
                    "AGENTS.md",
                    "docs/GOAL.md",
                    "docs/functional-testing.md",
                    "tests/functional/support.rs",
                    "src/catalog.rs",
                ],
                "coverage_rows": [],
                "checkpoints": [],
                "acceptance": acceptance,
                "status": "queued",
                "architecture_refactor": False,
            }
        )

    forbidden = [
        "tests/functional/support.rs",
        "tests/functional/main.rs",
        "tests/functional/cases/mod.rs",
        "docs/testing/refactor-coverage.json",
        "src/",
    ]

    # Shell task.
    shell_rows = []
    for opt in by_array.get("SHELL", []):
        shell_rows.append(
            row(
                f"opt.{opt['key']}",
                "S01.shell",
                "showcase",
                "option",
                route="shell",
                option_values=opt["values"],
                effect=opt["effect"],
                needs=opt["needs"],
                combination_reason="Execute the default and one non-default boundary. Execute a second option only when needs names it.",
            )
        )
    for suffix, action, expected, forbidden_effect in [
        (
            "inspector",
            "Open the Inspector, move through its sections, change an option, and close it.",
            "The Inspector shows the current page and the option change is visible on that page.",
            "Closing the Inspector changes the page selection.",
        ),
        (
            "sidebar-policy",
            "Switch Current, Accordion, and All expansion.",
            "The visible groups match the selected policy.",
            "All leaves a collapsed group expanded.",
        ),
        (
            "shell-override",
            "Change a shell option that the page says affects a control, from the default to one other value.",
            "The control's visible behaviour matches that value.",
            "The page option changes when only the shell option was edited.",
        ),
    ]:
        shell_rows.append(
            row(
                f"sc.shell.{suffix}",
                "S01.shell",
                "showcase",
                "transition",
                action_sequence=action,
                expected_results=expected,
                forbidden_effects=forbidden_effect,
                checkpoint_ids=[f"sc.shell.{suffix}.120x40"],
                terminal_columns=120,
                terminal_rows=40,
            )
        )
    add_rows(shell_rows)
    add_task(
        {
            "task_id": "S01.shell",
            "wave": "C",
            "kind": "leaf",
            "model": MODEL,
            "reference_commit": REF,
            "dependencies": ["G03", "G04", "G05"],
            "owned_files": [
                "tests/functional/cases/s01_shell.rs",
                "docs/testing/cases/S01.shell.json",
                "tests/functional/baselines/pre_refactor/S01.shell/",
            ],
            "forbidden_shared_edits": forbidden,
            "source_inputs": [
                "src/bin/showcase/app.rs",
                "src/bin/showcase/inspector.rs",
                "src/bin/showcase/options.rs",
                "src/shell/",
            ],
            "coverage_rows": [item["case_id"] for item in shell_rows],
            "checkpoints": [f"sc.shell.{name}.120x40" for name in ("inspector", "sidebar-policy", "shell-override")],
            "acceptance": "Shell options, Inspector sections, and sidebar expansion policies show their exact visible result. A shell change does not retarget the page.",
            "status": "queued",
            "architecture_refactor": False,
        }
    )

    route_task_ids = []
    for slug, label, source, card, array in ROUTES:
        task_id = f"{card}.{slug}"
        route_task_ids.append(task_id)
        owned_rows = []
        owned_rows.append(
            row(
                f"sc.{slug}.route",
                task_id,
                "showcase",
                "route",
                route=slug,
                label=label,
                entry_path=f"--page {slug}",
                return_path="sidebar selection returns to Overview and keyboard [ ] can leave and re-enter the route",
            )
        )
        for section in SECTIONS:
            owned_rows.append(
                row(
                    f"sc.{slug}.{section}",
                    task_id,
                    "showcase",
                    "section",
                    route=slug,
                    section_or_overlay=section,
                )
            )
            for cols, rows_n in SIZES:
                checkpoint = f"sc.{slug}.{section}.{cols}x{rows_n}"
                owned_rows.append(
                    row(
                        checkpoint,
                        task_id,
                        "showcase",
                        "section-size",
                        route=slug,
                        section_or_overlay=section,
                        checkpoint_ids=[checkpoint],
                        terminal_columns=cols,
                        terminal_rows=rows_n,
                        color_mode="truecolor",
                        glyph_mode="unicode",
                        motion_mode="paused",
                        client_platform="macos",
                        expected_results=f"The {label} {section} section is the visible section at {cols} by {rows_n}.",
                        forbidden_effects="A different section's caption is the active caption.",
                        action_sequence=f"Launch showcase --page {slug} --motion paused at {cols}x{rows_n} and activate the {section} tab.",
                    )
                )
        for state in SPECIMEN:
            applicability = (
                "Required when this page's StateGrid rows include the state. "
                "If the grid omits it, set NOT_APPLICABLE with the page source line. "
                "Editing is a SpecimenState and is absent from the catalog state list; "
                "that catalog gap does not remove the specimen when the page draws it."
            )
            owned_rows.append(
                row(
                    f"sc.{slug}.specimen.{state}",
                    task_id,
                    "showcase",
                    "state",
                    route=slug,
                    section_or_overlay="states",
                    applicability=applicability,
                    catalog_listed=state in CATALOG_STATES,
                )
            )
        owned_rows.append(
            row(
                f"sc.{slug}.compare-levels",
                task_id,
                "showcase",
                "transition",
                route=slug,
                section_or_overlay="states",
                action_sequence="Turn Compare levels on, scroll the bands, narrow the terminal, then turn Compare levels off.",
                expected_results="Colour bands for the supported levels are visible, and the page state is unchanged after return.",
                forbidden_effects="Leaving Compare levels changes the selected page or an option value.",
                checkpoint_ids=[f"sc.{slug}.compare-levels.120x40", f"sc.{slug}.compare-levels.80x24"],
            )
        )
        for opt in by_array.get(array, []):
            owned_rows.append(
                row(
                    f"opt.{opt['key']}",
                    task_id,
                    "showcase",
                    "option",
                    route=slug,
                    option_values=opt["values"],
                    effect=opt["effect"],
                    needs=opt["needs"],
                    combination_reason=(
                        "Execute the default and one non-default boundary. "
                        "Execute another option in combination only when needs declares a dependency. "
                        "Unrelated options do not form a Cartesian product."
                    ),
                    action_sequence=f"Set {opt['key']} from its default to another listed value through --opt or the Options control.",
                    expected_results=f"The visible {opt['label']} effect matches the selected value.",
                    forbidden_effects="A different option's value changes.",
                )
            )
        for suffix, action, expected, forbidden_effect in ROUTE_CASES[slug]:
            owned_rows.append(
                row(
                    f"sc.{slug}.{suffix}",
                    task_id,
                    "showcase",
                    "transition",
                    route=slug,
                    action_sequence=action,
                    expected_results=expected,
                    forbidden_effects=forbidden_effect,
                    checkpoint_ids=[f"sc.{slug}.{suffix}.before", f"sc.{slug}.{suffix}.after"],
                    terminal_columns=120,
                    terminal_rows=40,
                )
            )
        add_rows(owned_rows)
        rust_name = task_id.lower().replace(".", "_")
        add_task(
            {
                "task_id": task_id,
                "wave": "C",
                "kind": "leaf",
                "model": MODEL,
                "reference_commit": REF,
                "dependencies": ["G03", "G04", "G05"],
                "owned_files": [
                    f"tests/functional/cases/{rust_name}.rs",
                    f"docs/testing/cases/{task_id}.json",
                    f"tests/functional/baselines/pre_refactor/{task_id}/",
                ],
                "forbidden_shared_edits": forbidden,
                "source_inputs": [
                    f"src/bin/showcase/pages/{source}",
                    "src/bin/showcase/options.rs",
                    "src/bin/showcase/pages/template.rs",
                    "src/ui/specimen.rs",
                ],
                "coverage_rows": [item["case_id"] for item in owned_rows],
                "checkpoints": [
                    item["case_id"]
                    for item in owned_rows
                    if item["kind"] in ("section-size", "transition")
                ],
                "acceptance": (
                    f"Open {label} by --page, by keyboard, and by pointer. "
                    f"Visit Playground, Options, and States at 120x40 and 80x24. "
                    f"Change one visible option and assert its effect. "
                    f"Cover the route interactions. Capture the narrow layout before growing the terminal. "
                    f"Each assertion names the exact visible result and the forbidden effect."
                ),
                "status": "queued",
                "architecture_refactor": False,
            }
        )

    consumers = consumer_leaves()
    for task in consumers:
        add_task(task["task"])
        add_rows(task["rows"])

    # Closing tasks.
    for task_id, wave, deps, files, acceptance in [
        (
            "C01",
            "E",
            ["G04", "G05"],
            [".velnor/config.toml", ".config/nextest.toml"],
            "Generated CI selects the full required parity inventory with --locked and --no-tests fail. Generated workflow files are not hand-edited. Branch protection is unverified unless the GitHub setting can be read.",
        ),
        (
            "C02",
            "F",
            ["C01"],
            ["docs/testing/probe-receipts/"],
            "Each mandatory sensitivity probe fails at its intended detector and passes after the temporary defect is removed. No probe baseline is accepted.",
        ),
        (
            "C03",
            "F",
            ["C02"],
            ["docs/testing/refactor-coverage.json", "docs/testing/known-gaps.md", "docs/audits/refactor-test-readiness.md"],
            "An independent reviewer who did not write the reviewed cases accepts the inventory. Untested implemented states stay BLOCKED.",
        ),
        (
            "C04",
            "F",
            ["C03"],
            ["docs/testing/refactor-protection.md", "docs/audits/refactor-test-readiness.md"],
            "The report starts with READY only when every required row is PASS at the recorded HEAD. Otherwise it starts with BLOCKED and lists the gaps and resume steps.",
        ),
    ]:
        add_task(
            {
                "task_id": task_id,
                "wave": wave,
                "kind": "closing",
                "model": MODEL,
                "reference_commit": REF,
                "dependencies": deps,
                "owned_files": files,
                "forbidden_shared_edits": ["src/"],
                "source_inputs": [".velnor/config.toml", "docs/ci.md"],
                "coverage_rows": [],
                "checkpoints": [],
                "acceptance": acceptance,
                "status": "queued",
                "architecture_refactor": False,
            }
        )

    # C01 waits until implementation task names are stable. Record that
    # dependency explicitly now that the leaf ids exist.
    leaf_ids = [task["task_id"] for task in tasks if task["kind"] == "leaf"]
    for task in tasks:
        if task["task_id"] == "C01":
            task["dependencies"] = ["G04", "G05", *leaf_ids]

    ids = [task["task_id"] for task in tasks]
    if len(ids) != len(set(ids)):
        dupes = [item for item, count in Counter(ids).items() if count > 1]
        raise SystemExit(f"duplicate task ids: {dupes}")
    owners = Counter(item["owner_task"] for item in rows)
    missing_owners = sorted(set(owners) - set(ids))
    if missing_owners:
        raise SystemExit(f"rows owned by unknown tasks: {missing_owners}")
    case_ids = [item["case_id"] for item in rows]
    if len(case_ids) != len(set(case_ids)):
        dupes = [item for item, count in Counter(case_ids).items() if count > 1]
        raise SystemExit(f"duplicate case ids: {dupes[:10]}")

    # Existing tests and baselines.
    listed = (ROOT / "docs/testing/g00-nextest-list.txt").read_text().splitlines()
    current_tests = []
    weak = (
        "keyboard_activation",
        "table_header_sort",
        "form_required",
        "dialog_opens",
        "showcase_page_",
        "resize",
        "jackin_manager_stop",
        "jackin_settings",
        "jackin_cockpit_launch_failure",
        "jackin_launch_detach",
    )
    for line in listed:
        name = line.split()[-1]
        classification = "insufficient" if any(token in name for token in weak) else "behavior-and-visual"
        if name.startswith("showcase_page_"):
            classification = "visual-only-initial-view"
        current_tests.append(
            {
                "test_name": name,
                "classification": classification,
                "result": "PASS",
                "execution_commit": REF,
                "note": "Existing suite at REFERENCE_SHA. This PASS does not close a new coverage row.",
            }
        )

    baseline_root = ROOT / "tests/functional/baselines"
    baselines = binary_baseline_inventory(baseline_root, ROOT)

    summary = {
        "reference_sha": REF,
        "execution_task_count": len(tasks),
        "leaf_task_count": len(leaf_ids),
        "coverage_row_count": len(rows),
        "showcase_routes": 54,
        "showcase_sections": 54 * 3,
        "showcase_section_sizes": 54 * 3 * 2,
        "specimen_states_per_route": 14,
        "catalog_states": CATALOG_STATES,
        "specimen_states": list(SPECIMEN),
        "catalog_missing_state": "editing",
        "holla_scenarios": 34,
        "jackin_scenarios": 8,
        "option_rows": sum(1 for item in rows if item["kind"] == "option"),
        "shell_option_rows": sum(1 for item in rows if item["case_id"].startswith("opt.shell.")),
        "denominators": dict(Counter(item["kind"] for item in rows)),
        "order": ids,
        "existing_test_count": len(current_tests),
        "existing_baseline_count": len(baselines),
    }
    if summary["showcase_routes"] != 54 or summary["showcase_sections"] != 162 or summary["showcase_section_sizes"] != 324:
        raise SystemExit(summary)

    for task in tasks:
        if task["task_id"] == "G00":
            task["status"] = "passed_existing_gates_pending_clippy_fmt"
            task["evidence"] = {
                "worktree": "clean except untracked terminal-components-baseline-goal.txt",
                "nextest_list_count": summary["existing_test_count"],
                "visual_baseline_tag": None,
                "binaries_sha256": {
                    "showcase": "59646b3de51a53b516276b869826be2d16d81bb35a3c08d9df012acc7854826f",
                    "tablepro": "aeb2ca26942ba5fb74f817f3a7b5ab3f34b3c4e90e86f0afd8c31d13ea2e0343",
                    "holla": "d0e1196de5b1fc1417904c4da43f75960028b0d29e87209e9b69ef1e91cddf8f",
                    "jackin": "8f06be062879e897e5b3a49ea965e015e3f937452925d0baa41e992bd1b63839",
                },
            }
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "tasks").mkdir(exist_ok=True)
    (OUT / "cases").mkdir(exist_ok=True)
    for task in tasks:
        (OUT / "tasks" / f"{task['task_id']}.json").write_text(json.dumps(task, indent=2) + "\n")
        # Case fragment stub. Workers replace result and evidence.
        fragment = {
            "task_id": task["task_id"],
            "cases": [item for item in rows if item["owner_task"] == task["task_id"]],
        }
        (OUT / "cases" / f"{task['task_id']}.json").write_text(json.dumps(fragment, indent=2) + "\n")

    inventory = {
        "schema": "refactor-coverage.v1",
        "reference_sha": REF,
        "summary": summary,
        "rows": rows,
        "current_tests": current_tests,
        "current_baselines": baselines,
    }
    (OUT / "refactor-coverage.json").write_text(json.dumps(inventory, indent=2) + "\n")
    (OUT / "ledger-order.txt").write_text("\n".join(ids) + "\n")
    write_markdown(ids, tasks, summary, current_tests, baselines)
    print(json.dumps({k: summary[k] for k in ("execution_task_count", "leaf_task_count", "coverage_row_count", "option_rows", "shell_option_rows", "denominators")}, indent=2))


def consumer_leaves() -> list[dict]:
    """Bounded consumer, cross-application, and their coverage rows."""
    specs = []

    def add(task_id, application, sources, acceptance, cases, deps=None, scenarios=None):
        rust_name = task_id.lower().replace(".", "_")
        row_items = [
            row(
                f"{task_id}.route",
                task_id,
                application,
                "route",
                route=task_id,
                entry_path="real binary arguments or keyboard path recorded by the leaf",
                return_path="the leaf returns to the screen that opened the surface, or exits through the tested guard",
            )
        ]
        for suffix, action, expected, forbidden_effect in cases:
            row_items.append(
                row(
                    f"{application[:2].lower()}.{task_id}.{suffix}" if False else f"{task_id}.{suffix}",
                    task_id,
                    application,
                    "transition",
                    scenario=scenarios,
                    action_sequence=action,
                    expected_results=expected,
                    forbidden_effects=forbidden_effect,
                    checkpoint_ids=[f"{task_id}.{suffix}.120x40", f"{task_id}.{suffix}.narrow"],
                    terminal_columns=120,
                    terminal_rows=40,
                    color_mode="truecolor",
                    motion_mode="reduced",
                    client_platform="macos",
                )
            )
        if scenarios:
            for scenario in scenarios:
                row_items.append(
                    row(
                        f"{task_id}.scenario.{scenario}",
                        task_id,
                        application,
                        "scenario",
                        scenario=scenario,
                        action_sequence=f"Launch the application with scenario {scenario} and exercise the surface owned by {task_id}.",
                        expected_results=f"The visible fixture data belongs to {scenario}.",
                        forbidden_effects="The journey asserts a different scenario's identities.",
                    )
                )
        specs.append(
            {
                "rows": row_items,
                "task": {
                    "task_id": task_id,
                    "wave": "C" if task_id[0] in "THJ" else "D",
                    "kind": "leaf",
                    "model": MODEL,
                    "reference_commit": REF,
                    "dependencies": deps or ["G03", "G04", "G05"],
                    "owned_files": [
                        f"tests/functional/cases/{rust_name}.rs",
                        f"docs/testing/cases/{task_id}.json",
                        f"tests/functional/baselines/pre_refactor/{task_id}/",
                    ],
                    "forbidden_shared_edits": [
                        "tests/functional/support.rs",
                        "tests/functional/main.rs",
                        "tests/functional/cases/mod.rs",
                        "src/",
                    ],
                    "source_inputs": sources,
                    "coverage_rows": [item["case_id"] for item in row_items],
                    "checkpoints": [item["checkpoint_ids"][0] for item in row_items if item.get("checkpoint_ids")],
                    "acceptance": acceptance,
                    "status": "queued",
                    "architecture_refactor": False,
                },
            }
        )

    # TablePro
    add(
        "T01.connections",
        "tablepro",
        ["apps/tablepro/src/connections.rs", "apps/tablepro/src/app.rs", "apps/tablepro/src/model.rs"],
        "Tree selection, filtering, empty search, and connection status stay on the selected connection.",
        [
            ("select", "Select each fixture connection and filter until one and then zero remain.", "Details show the selected name and host. Empty search shows the empty state.", "Details keep the previous connection after the selection moves."),
            ("status", "Open idle, connecting, authentication-failure, and unreachable-host fixtures, then recover with another connection.", "Each status text is the fixture's status. Recovery opens the other connection's workbench.", "Recovery shows the failed connection as connected."),
        ],
    )
    add(
        "T01.basic",
        "tablepro",
        ["apps/tablepro/src/connections.rs", "apps/tablepro/src/model.rs"],
        "Basic fields reject blank names and invalid ports and preserve the original record when validation fails.",
        [
            ("fields", "Create a connection. Enter a blank name, a whitespace name, an invalid port, and the implemented valid boundaries. Save.", "The saved record shows the exact valid name, host, port, engine, environment, group, and safe mode.", "An invalid Save replaces the original connection."),
            ("reopen", "Reopen the saved connection.", "Every basic field matches the saved value.", "Reopen shows a default that was not saved."),
        ],
    )
    add(
        "T01.advanced",
        "tablepro",
        ["apps/tablepro/src/connections.rs"],
        "Advanced fields, including the masked password, round-trip through save and reopen.",
        [
            ("secrets", "Fill host, database, user, password, SSL, SSH, and startup. Paste a synthetic secret.", "The password cells are masked. Reopen shows the non-secret fields exactly and the password still masked.", "The secret appears in the terminal text or the saved evidence."),
        ],
    )
    add(
        "T01.test",
        "tablepro",
        ["apps/tablepro/src/connections.rs", "apps/tablepro/src/db.rs"],
        "Test, Save, Save & connect, and Cancel are separate results.",
        [
            ("actions", "Run Test, Save, Save & connect, and Cancel on distinct edits.", "Test does not save. Save reopens with the edit. Save & connect opens the workbench. Cancel keeps the previous record.", "Cancel writes the edit."),
        ],
    )
    add(
        "T01.leave",
        "tablepro",
        ["apps/tablepro/src/connections.rs", "apps/tablepro/src/app.rs"],
        "Every dirty leave path can Stay, Discard, or Save, and invalid Save from the dialog preserves the original.",
        [
            ("guards", "From a dirty form, try close, New, Edit another, and Quit. Choose Stay, Discard, and Save. Also attempt invalid Save from the dialog.", "Stay and a failed validation keep the draft and the original record. Discard restores the original. Save stores the draft.", "A leave path drops the draft without a choice."),
        ],
    )
    add(
        "T02.explorer",
        "tablepro",
        ["apps/tablepro/src/workbench.rs"],
        "Schema expansion, filtering, preview reuse, and permanent open keep object identity.",
        [
            ("tree", "Expand a schema and open a table preview, then pin it.", "The preview title is the chosen object. Pinning does not replace another pinned tab.", "A filter result opens a different object than the selected row."),
        ],
    )
    add(
        "T02.structure",
        "tablepro",
        ["apps/tablepro/src/workbench.rs", "apps/tablepro/src/tabs.rs"],
        "Each Structure tab shows that object's schema values or its empty state.",
        [
            ("tabs", "Open Columns, Indexes, Foreign keys, Constraints, Triggers, and DDL.", "Each tab's heading and cell values belong to the opened object.", "An empty tab shows another tab's rows."),
        ],
    )
    add(
        "T02.data",
        "tablepro",
        ["apps/tablepro/src/workbench.rs", "apps/tablepro/src/tabs.rs"],
        "Editable tables, read-only views, and tables without a primary key follow their visible rules.",
        [
            ("kinds", "Open an editable table, a read-only view, and a table without a primary key.", "The read-only view rejects mutation with the current reason. The editable table accepts a valid edit.", "The view stores an edit."),
        ],
    )
    add(
        "T02.editors",
        "tablepro",
        ["apps/tablepro/src/workbench.rs"],
        "Each implemented data-type editor accepts a valid value and rejects an invalid one, including NULL, insert, duplicate, and delete.",
        [
            ("types", "Edit one value in each implemented type family. Enter an invalid value and NULL where the cell allows it. Insert, duplicate, and delete a row.", "Pending counts match the operations. An invalid value does not become the stored cell.", "Delete removes a row that was not selected."),
        ],
    )
    add(
        "T02.filters",
        "tablepro",
        ["apps/tablepro/src/filter_editor.rs"],
        "Every implemented filter operator and the zero, one, and two-value forms change the visible row set.",
        [
            ("filters", "Add, edit, remove, and reset chips. Exercise match-all, match-any, no rows, and filter-on-cell.", "The visible row identities match the filter. Reset restores the unfiltered identities. Selection stays on the same identity when it remains.", "No-rows shows the previous rows."),
        ],
    )
    add(
        "T02.commit",
        "tablepro",
        ["apps/tablepro/src/app.rs"],
        "Cancel keeps pending changes. Confirm clears them and adds the expected History entry. The simulator's success-only commit is a recorded scope limit.",
        [
            ("commit", "Open the SQL preview, cancel commit, then confirm commit.", "Cancel leaves the pending counts. Confirm clears them and History contains the previewed statement.", "Confirm commits a different statement than the preview."),
        ],
    )
    add(
        "T02.discard",
        "tablepro",
        ["apps/tablepro/src/app.rs"],
        "Discard restores the exact prior row values.",
        [
            ("discard", "Edit a cell, open discard review, cancel, then confirm.", "Cancel keeps the edit. Confirm restores the original cell.", "Discard changes a row that was not pending."),
        ],
    )
    add(
        "T02.viewers",
        "tablepro",
        ["apps/tablepro/src/workbench.rs", "apps/tablepro/src/context.rs"],
        "Cell viewers, row menus, foreign-key follow, refresh, and additional-row fetch stay on the selected cell.",
        [
            ("viewer", "Open a cell viewer, select and copy its text, follow a foreign key, refresh, and fetch more rows.", "The viewer text is the cell text. Follow opens the referenced row.", "Copy captures a different cell."),
        ],
    )
    add(
        "T03.query",
        "tablepro",
        ["apps/tablepro/src/sql.rs", "apps/tablepro/src/app.rs"],
        "Safe mode, statement scope, and batch classification match the visible decision.",
        [
            ("classify", "Run SELECT, a scoped write, an unscoped write, a destructive statement, a mixed batch, EXPLAIN, and EXPLAIN ANALYZE under each implemented safe mode. Include comments, a quoted semicolon, empty SQL, and a syntax error.", "The safety decision is the highest applicable decision. Denied execution adds no result and no History entry.", "A denied statement produces a result grid."),
        ],
    )
    add(
        "T03.completion",
        "tablepro",
        ["apps/tablepro/src/tabs.rs"],
        "Completion opens, selects, accepts, and dismisses without changing uncommitted SQL on dismiss.",
        [
            ("complete", "Open completion, move the selection, accept one item, then dismiss another.", "Accepted text is the selected item. Dismiss leaves the SQL unchanged.", "Dismiss inserts the highlighted item."),
        ],
    )
    add(
        "T03.running",
        "tablepro",
        ["apps/tablepro/src/app.rs"],
        "A running query refuses a duplicate run and can be cancelled before it replaces the result.",
        [
            ("cancel", "Start a query, observe running feedback, refuse a second run, and cancel.", "The cancelled result stays cancelled. A later completion does not replace it.", "Duplicate run starts a second result."),
        ],
    )
    add(
        "T03.confirm",
        "tablepro",
        ["apps/tablepro/src/app.rs"],
        "Ordinary and typed confirmation accept only the exact arming input, once.",
        [
            ("gates", "Cancel an ordinary confirmation and a typed confirmation. Enter a wrong, partial, and exact phrase.", "Wrong and partial text do not execute. Exact text executes once.", "Cancel leaves a new History entry."),
        ],
    )
    add(
        "T03.rows",
        "tablepro",
        ["apps/tablepro/src/tabs.rs"],
        "Empty, normal, and large row results show their counts and refuse editing.",
        [
            ("rows", "Run a query with no rows, a query with known rows, and a large result. Try to edit the result. Select, sort, copy, pin, switch, and close it.", "The grid values are the fixture values. Sort changes row order. Editing is refused.", "Sort changes the status text without the row order."),
        ],
    )
    add(
        "T03.affected",
        "tablepro",
        ["apps/tablepro/src/tabs.rs", "apps/tablepro/src/sql.rs"],
        "A write shows the affected-row body rather than a row grid.",
        [
            ("affected", "Confirm one scoped write.", "The affected body shows the simulated count and History records that statement.", "The body shows rows from the previous SELECT."),
        ],
    )
    add(
        "T03.error",
        "tablepro",
        ["apps/tablepro/src/sql.rs", "apps/tablepro/src/app.rs"],
        "A syntax or execution error is visible, and correcting it removes that error.",
        [
            ("recover", "Run a statement that fails, then correct it and run it again.", "The error text is the failure. The recovered result is the corrected statement's result, and the old error is gone.", "Recovery still shows the old error."),
        ],
    )
    add(
        "T03.plan",
        "tablepro",
        ["apps/tablepro/src/tabs.rs"],
        "Plan tree, raw plan, and analysis output are distinct bodies.",
        [
            ("plan", "Explain a statement, switch to the raw plan, and run EXPLAIN ANALYZE where the mode allows it.", "Each body contains that view's text.", "The raw view still shows only the tree."),
        ],
    )
    add(
        "T03.cancelled",
        "tablepro",
        ["apps/tablepro/src/app.rs"],
        "A cancelled statement stays cancelled through the rest of the batch.",
        [
            ("boundary", "Cancel before completion and, where the UI allows it, between statements.", "The cancelled body remains the result. A later statement does not overwrite it.", "The batch continues into a success body after cancel."),
        ],
    )
    add(
        "T04.history",
        "tablepro",
        ["apps/tablepro/src/tabs.rs", "apps/tablepro/src/app.rs"],
        "History search, scope, details, copy, open, and rerun use the selected entry's SQL.",
        [
            ("rerun", "Search, switch connection scope and all-scope, enable failed-only, open details, copy, and rerun the selected entry. Also search a missing phrase.", "The rerun result is that entry's statement. Empty search shows no rows.", "Rerun executes the previously active editor text instead of the selected entry."),
        ],
    )
    add(
        "T04.quickly",
        "tablepro",
        ["apps/tablepro/src/app.rs"],
        "Open Quickly reaches tables, History, and query tabs, and cancel or no-match changes nothing.",
        [
            ("switcher", "Open Quickly for a table, a History entry, and a query tab. Then show no-match and cancel.", "The opened object is the chosen row. Cancel leaves the active tab unchanged.", "No-match activates the first row."),
        ],
    )
    add(
        "T04.tabs",
        "tablepro",
        ["apps/tablepro/src/workbench.rs", "apps/tablepro/src/app.rs"],
        "Closing first, last, inactive, active, and only tabs keeps the remaining titles and SQL.",
        [
            ("close", "Close tabs by pointer, keyboard, and middle-click where supported.", "The remaining tab titles and SQL values are the ones that were not closed. Active fallback is the documented neighbour.", "A reused index shows a different tab's SQL."),
        ],
    )
    add(
        "T04.preview",
        "tablepro",
        ["apps/tablepro/src/workbench.rs"],
        "Preview reuse does not replace a pinned or dirty tab.",
        [
            ("preview", "Preview an object, pin a tab, dirty another tab, and preview again.", "The pinned and dirty tabs keep their identities and SQL.", "Preview replaces the dirty tab."),
        ],
    )
    add(
        "T04.close",
        "tablepro",
        ["apps/tablepro/src/app.rs"],
        "Dirty close offers Cancel, Discard, and Save, and a modal blocks a second close.",
        [
            ("dirty", "Close a dirty tab and choose Cancel, then Discard, then Save on separate tabs. While confirmation is open, try to close the neighbour.", "Cancel keeps the SQL. The modal refuses the neighbour close.", "Cancel closes the tab."),
        ],
    )
    add(
        "T05.menus",
        "tablepro",
        ["apps/tablepro/src/pages.rs", "apps/tablepro/src/app.rs"],
        "Each shell menu command is the command that was highlighted.",
        [
            ("menus", "Open every implemented shell menu and invoke an enabled command. Invoke a disabled command.", "The enabled command's result is visible. The disabled command does not change state.", "The disabled command runs."),
        ],
    )
    add(
        "T05.help",
        "tablepro",
        ["apps/tablepro/src/app.rs"],
        "Help shows its own title and returns focus to the opener.",
        [
            ("help", "Open Help, scroll it, press a page key, and close it.", "The Help title is visible while it is open. Focus returns to the opener.", "A page key activates the workbench while Help is open."),
        ],
    )
    add(
        "T05.context",
        "tablepro",
        ["apps/tablepro/src/context.rs"],
        "Context-menu actions apply to the selected cell or row.",
        [
            ("context", "Open a row context menu and a cell context menu and run one action in each.", "The action's object is the selected object.", "The action applies to the previously selected row."),
        ],
    )
    add(
        "T05.viewers",
        "tablepro",
        ["apps/tablepro/src/workbench.rs"],
        "A viewer owns selection, copy, and close.",
        [
            ("viewer", "Open a viewer, select text, copy, and close.", "The copy payload is the viewer text. Close returns focus to the cell.", "Close leaves the viewer title visible."),
        ],
    )
    add(
        "T05.maximize",
        "tablepro",
        ["apps/tablepro/src/workbench.rs"],
        "Hiding the explorer and maximizing a pane restore the same layout.",
        [
            ("layout", "Hide and show the explorer. Maximize and restore the supported panes. Drag the implemented seams.", "The restored layout shows the same tabs and SQL. Only the dragged seam changes style.", "Maximize closes a tab."),
        ],
    )
    add(
        "T05.disconnect",
        "tablepro",
        ["apps/tablepro/src/app.rs"],
        "Clean disconnect works. Dirty disconnect can be cancelled and keeps drafts.",
        [
            ("disconnect", "Disconnect with a clean session, then with dirty SQL or pending table edits. Cancel, then confirm on a later attempt.", "Cancel keeps the connection, the SQL, and the pending values.", "Cancel returns to the connection list."),
        ],
    )
    add(
        "T05.quit",
        "tablepro",
        ["apps/tablepro/src/app.rs"],
        "Quit uses the same guard as disconnect and restores the terminal only after the confirmed exit.",
        [
            ("quit", "Quit clean, then quit dirty and cancel, then confirm.", "Cancel preserves drafts. The confirmed exit is code 0 without a signal.", "The harness kill is treated as the application exit."),
        ],
    )

    # Holla. Scenarios are assigned to the leaf that first exercises them.
    add(
        "H01.finder",
        "holla",
        ["apps/holla/src/screens/finder.rs", "apps/holla/src/domain/ranking.rs", "apps/holla/src/app.rs"],
        "Discovery stages and query edits keep the selected identity and do not run a previous result on an empty list.",
        [
            ("discovery", "Show pending, ready, failed, and unavailable discovery. Type, paste, backspace, and clear the query.", "The selected identity stays while order changes. Empty results have no runnable row.", "Enter on an empty result runs the previous result."),
        ],
        scenarios=["first-use", "rust-dirty", "monorepo-root", "parity-discovery"],
    )
    add(
        "H01.scope",
        "holla",
        ["apps/holla/src/screens/finder.rs", "apps/holla/src/domain/usage.rs"],
        "Here, project, and host scope, groups, pin, hide, and ranking reset change the visible set only.",
        [
            ("scope", "Switch Here, project, and host. Pin, hide, restore, and reset ranking.", "The action row's command, directory, and host match the selected item after reranking.", "Hide removes a different item."),
        ],
        scenarios=["monorepo-child", "remote-host"],
    )
    add(
        "H01.alternatives",
        "holla",
        ["apps/holla/src/screens/finder.rs"],
        "Alternatives lists alternatives for the selected item.",
        [
            ("alternatives", "Open Alternatives for the selected row.", "Every alternative names that row's action.", "Alternatives shows another row's action."),
        ],
    )
    add(
        "H01.why",
        "holla",
        ["apps/holla/src/screens/finder.rs", "apps/holla/src/domain/ranking.rs"],
        "Why explains the selected item.",
        [
            ("why", "Open Why for the selected row and close it.", "The explanation names the selected item. Focus returns to that row.", "Why names a different item."),
        ],
    )
    add(
        "H01.memory",
        "holla",
        ["apps/holla/src/domain/usage.rs", "apps/holla/src/app.rs"],
        "Enabled and disabled history, learned queries, malformed memory, and save failure stay inside the simulated contract.",
        [
            ("memory", "Run a search with history enabled and disabled. Load malformed memory and a save failure when the fixture provides them.", "Save failure keeps the current search text.", "A memory failure clears the query."),
        ],
        scenarios=["parity-history"],
    )
    add(
        "H02.trust",
        "holla",
        ["apps/holla/src/screens/review.rs", "apps/holla/src/domain/digest.rs"],
        "Trust cancel and confirm apply to the reviewed path, directory, and digest.",
        [
            ("trust", "Cancel trust, then confirm trust for the same action.", "Cancel starts no activity. Confirm records trust for that path and digest.", "Confirm trusts a different directory."),
        ],
        scenarios=["monorepo-child"],
    )
    add(
        "H02.args",
        "holla",
        ["apps/holla/src/screens/review.rs", "apps/holla/src/domain/scripts.rs"],
        "Required, choice, and secret arguments reject invalid input and do not reveal the secret.",
        [
            ("args", "Fill each implemented argument type, enter invalid input, correct it, and cancel after an edit.", "The displayed command and directory match the form. The secret is masked.", "Cancel starts the activity."),
        ],
        scenarios=["parity-task-sources"],
    )
    add(
        "H02.config",
        "holla",
        ["apps/holla/src/domain/custom.rs", "apps/holla/src/screens/review.rs"],
        "Malformed, duplicate, and unavailable custom actions do not start an activity.",
        [
            ("config", "Load a malformed action, a duplicate action, and an unavailable action.", "The error names that action. No activity starts.", "The unavailable action starts."),
        ],
        scenarios=["parity-custom-actions", "hard-cases"],
    )
    add(
        "H02.reload",
        "holla",
        ["apps/holla/src/domain/custom.rs", "apps/holla/src/app.rs"],
        "Changed configuration, revocation, and reload failure return to the same action.",
        [
            ("reload", "Change trust, revoke it, and reload. Use the fixture's reload failure.", "Failure leaves the current action selected and starts nothing.", "Reload runs the action."),
        ],
    )
    add(
        "H02.validation",
        "holla",
        ["apps/holla/src/screens/review.rs"],
        "Literal shell syntax stays input text, and the action after an error is the same action.",
        [
            ("literal", "Enter literal shell metacharacters where the argument field allows them, then return after an error.", "The field shows the literal text. The owning action is unchanged.", "The metacharacters are executed as a different command."),
        ],
    )
    add(
        "H03.output",
        "holla",
        ["apps/holla/src/screens/activity.rs", "apps/holla/src/domain/activity.rs", "apps/holla/src/sim/world.rs"],
        "Queued, running, completed, failed, and stopped activities keep their own output.",
        [
            ("states", "Reach each implemented activity state, including spawn failure, nonzero exit, and restart. Filter, find, select, copy, and follow output.", "The visible status and exit status belong to that activity. Another activity's output does not change.", "Restart rewrites the failed run's identity."),
        ],
        scenarios=["activities-multi", "launch-failure", "parity-executor"],
    )
    add(
        "H03.input",
        "holla",
        ["apps/holla/src/screens/activity.rs", "apps/holla/src/domain/exec.rs"],
        "Public and secret prompts accept input only when the process can, and Ctrl+C goes to the task while it owns input.",
        [
            ("prompts", "Answer a public prompt and a secret prompt. Try input when it is refused. Press Ctrl+C and then Escape.", "The secret is masked. Escape returns keys to the shell. Refused input does not appear as a delivered line.", "The secret is echoed."),
        ],
        scenarios=["parity-task-input"],
    )
    add(
        "H03.picker",
        "holla",
        ["apps/holla/src/screens/activity.rs"],
        "The activity picker acts on the selected activity.",
        [
            ("picker", "Open the picker and choose one activity.", "The opened output is that activity's output.", "The picker opens the first activity regardless of the highlight."),
        ],
    )
    add(
        "H03.attach",
        "holla",
        ["apps/holla/src/screens/activity.rs", "apps/holla/src/domain/activity.rs"],
        "Detach and attach do not stop the task. Stop and escalation affect only that task.",
        [
            ("attach", "Detach, observe the monitor, attach, request stop, and escalate a resistant process.", "The task is still running after detach. Stop changes that task only.", "Detach stops the task."),
        ],
    )
    add(
        "H03.guards",
        "holla",
        ["apps/holla/src/app.rs", "apps/holla/src/screens/activity.rs"],
        "Running-tab close and quit offer Stay or Stop. Finder q stays query text. Confirmed exit restores the terminal.",
        [
            ("quit", "Close a completed tab. Close a running tab and choose Stay, then Stop and quit. Type q in the Finder query.", "Stay leaves the task running. q is query text. The confirmed exit is code 0.", "Stay stops the task."),
        ],
    )
    add(
        "H04.actions",
        "holla",
        ["apps/holla/src/domain/catalog.rs", "apps/holla/src/domain/outcomes.rs", "apps/holla/src/sim/world.rs"],
        "Each action family shows its command, directory, host, and confirmation, and the follow-up screen shows the simulated effect.",
        [
            ("families", "Run one action in each of Git, Cargo, Docker, Compose, Homebrew services, Gradle, IDEA, and upgrade managers, including batch members.", "The follow-up state matches that action. A batch member keeps its identity.", "The command preview belongs to a different action than the one confirmed."),
        ],
        scenarios=[
            "parity-git-current",
            "parity-git-batch",
            "parity-cargo",
            "parity-docker",
            "parity-brew-services",
            "parity-gradle",
            "parity-idea",
            "parity-upgrade-managers",
        ],
    )
    add(
        "H04.snapshots-git",
        "holla",
        ["apps/holla/src/screens/snapshot.rs"],
        "Git snapshot families show their own empty, unavailable, or populated body and their follow-up action.",
        [
            ("git", "Open git, git-diff, git-stash, and git-all.", "Each body is that family. The follow-up action names the same repository.", "git-diff shows git-stash entries."),
        ],
    )
    add(
        "H04.snapshots-host",
        "holla",
        ["apps/holla/src/screens/snapshot.rs"],
        "Host snapshot families keep their kind in the title and body.",
        [
            ("host", "Open system, processes, port, disk-history, and ssh.", "The title kind matches the opened snapshot.", "One family shows another family's rows."),
        ],
    )
    add(
        "H04.snapshots-docker",
        "holla",
        ["apps/holla/src/screens/snapshot.rs"],
        "Docker snapshot families include the unavailable state.",
        [
            ("docker", "Open docker-ps, docker-df, compose-ps, and docker-unavailable.", "Unavailable does not list containers as running.", "Unavailable offers a follow-up that claims a container started."),
        ],
        scenarios=["docker-cleanup"],
    )
    add(
        "H04.snapshots-services",
        "holla",
        ["apps/holla/src/screens/snapshot.rs"],
        "Service snapshot families show their own rows.",
        [
            ("services", "Open pg, pg-blocking, mise, mise-outdated, service, and brew-services.", "Each body names its family.", "A service row appears in the mise snapshot."),
        ],
        scenarios=["parity-platforms", "parity-platforms-linux"],
    )
    add(
        "H04.plan",
        "holla",
        ["apps/holla/src/screens/plan.rs", "apps/holla/src/domain/plan.rs"],
        "Plan selection, both confirmation stages, drift, start, failure, and final tally use the reviewed steps.",
        [
            ("plan", "Remove an optional step, undo, restore, and reach no runnable steps. Cancel both confirmation stages. Enter the exact phrase and start. Show drift and re-review. Run success and failure. Cancel remaining work.", "The started plan is the reviewed plan. A cancelled confirmation does not start it. Completed steps do not run twice.", "A wrong phrase starts the plan."),
        ],
        scenarios=["upgrade-plan"],
    )
    add(
        "H05.files",
        "holla",
        ["apps/holla/src/screens/files.rs", "apps/holla/src/sim/fs.rs"],
        "Directory entry and parent or root boundaries keep the path identity.",
        [
            ("nav", "Enter a directory and return to the parent and the root boundary.", "The visible path is that directory and then its parent.", "Parent jumps to an unrelated directory."),
        ],
        scenarios=["parity-files", "parity-browser"],
    )
    add(
        "H05.find",
        "holla",
        ["apps/holla/src/screens/files.rs"],
        "Find queries, empty results, and opening the selected path stay in the file pane.",
        [
            ("find", "Search, clear to empty results, select a hit, and open it.", "The opened directory is the selected hit.", "Empty results open the previous path."),
        ],
    )
    add(
        "H05.previews",
        "holla",
        ["apps/holla/src/screens/files.rs", "apps/holla/src/sim/fs.rs"],
        "Each fixture preview type is distinct, and the file list does not move when the preview scrolls.",
        [
            ("preview", "Show text, binary, missing, unreadable, and symlink previews when the fixture has them. Toggle hidden files. Find, select, and copy inside a large preview.", "Hidden-file toggle keeps a valid selection. Preview scrolling leaves the file list in place.", "Copy returns a different file's text."),
        ],
    )
    add(
        "H05.scan",
        "holla",
        ["apps/holla/src/screens/disk.rs", "apps/holla/src/sim/world.rs"],
        "Scan start, progress, partial data, completion, cancellation, cache, and partial errors are distinct.",
        [
            ("scan", "Start a scan, observe progress and partial data, cancel one scan, and complete another. Change the scan root.", "Cancellation does not show a completed total. A stale result is labelled when the root changes.", "A cancelled scan shows the completed byte total as final."),
        ],
        scenarios=["parity-disk-scan", "disk-cleanup"],
    )
    add(
        "H05.disk",
        "holla",
        ["apps/holla/src/screens/disk.rs"],
        "Tree sorting, selection totals, Overview, and TopFiles return to the same node.",
        [
            ("tree", "Expand, sort, multi-select, and read the selected-byte total. Open Overview and TopFiles and return.", "The returned node is the node that was open. The neighbour pane is unchanged.", "Sort changes the total without changing the selected set."),
        ],
        scenarios=["parity-disk-navigation", "parity-insights"],
    )
    add(
        "H06.cleanup",
        "holla",
        ["apps/holla/src/screens/cleanup.rs", "apps/holla/src/domain/cleanup.rs"],
        "Only eligible, unprotected items can be selected for a dry run, Trash, or permanent delete.",
        [
            ("select", "Detect categories, try disabled and protected items, and select eligible items in each mode.", "Protected items stay. The mode label matches the selected mode.", "A protected descendant is included."),
        ],
        scenarios=["parity-delete-safety"],
    )
    add(
        "H06.gate",
        "holla",
        ["apps/holla/src/screens/review.rs", "apps/holla/src/domain/cleanup.rs"],
        "Each confirmation stage can be cancelled. Partial, wrong, and stale plans do not delete.",
        [
            ("confirm", "Cancel each stage. Enter partial, wrong, and exact text. Change the plan and try the old confirmation. Refuse a second cleanup and a failed Gradle stop.", "Exact text on the current plan is the only path that deletes. A failed prerequisite deletes nothing.", "The old phrase deletes after the plan changes."),
        ],
        scenarios=["parity-gradle", "parity-idea"],
    )
    add(
        "H06.report",
        "holla",
        ["apps/holla/src/screens/snapshot.rs", "apps/holla/src/domain/cleanup.rs"],
        "Per-item success, failure, and partial results and the freed-byte total match the simulation.",
        [
            ("results", "Complete a cleanup with mixed item outcomes and open the running and final reports.", "The total and each item status match the simulation.", "A failed item is counted as freed."),
        ],
        scenarios=["parity-cleanup-results"],
    )
    add(
        "H06.log",
        "holla",
        ["apps/holla/src/domain/cleanup.rs", "apps/holla/src/app.rs"],
        "Log failure is visible and recovery does not claim a missing log entry.",
        [
            ("log", "Produce an operation log and the fixture's log failure.", "The failure text is the log failure. Recovery does not invent entries.", "Log failure is shown as a successful delete."),
        ],
    )
    add(
        "H06.quit",
        "holla",
        ["apps/holla/src/app.rs", "apps/holla/src/screens/cleanup.rs"],
        "Quit during cleanup waits for the current report and log contract.",
        [
            ("quit", "Quit while cleanup runs, including when another activity is running.", "The exit path shows the guard. Confirmed exit waits for the report contract.", "Quit drops the report and returns success immediately."),
        ],
        scenarios=["parity-platforms", "parity-platforms-linux"],
    )

    # Jackin
    add("J01.status", "jackin", ["src/bin/jackin/screens/manager.rs", "src/bin/jackin/domain/instance.rs"],
        "Every fixture instance status is visible on its own row.",
        [("status", "Show each implemented instance status and select that instance.", "The selected row's status text is that instance's status.", "Selection shows another instance's status.")],
        scenarios=["first-use", "returning", "launch-running"])
    add("J01.refresh", "jackin", ["src/bin/jackin/screens/manager.rs", "src/bin/jackin/sim/world.rs"],
        "Refresh failure keeps the last good rows and does not apply a stale result to another target.",
        [("refresh", "Start refresh, show success and failure, and change selection while a result is pending.", "Failure leaves the last good rows. A stale result does not change the other target.", "Failure clears the list.")])
    add("J01.stop", "jackin", ["src/bin/jackin/screens/manager.rs", "src/bin/jackin/domain/instance.rs"],
        "Stop cancellation keeps the selected running instance. Confirmed stop changes that instance and the running count.",
        [("stop", "Select a running instance, open Stop, cancel, then confirm on the same instance.", "Cancel keeps it running. Confirm stops that instance and the running count drops by one.", "The stop dialog is absent from the stop checkpoint.")])
    add("J01.purge", "jackin", ["src/bin/jackin/screens/manager.rs"],
        "Purge accepts only the exact instance token.",
        [("purge", "Enter a wrong token and then the exact token.", "The wrong token does not purge. The exact token purges that instance only.", "A partial token purges.")])
    add("J01.delete", "jackin", ["src/bin/jackin/screens/manager.rs", "src/bin/jackin/domain/workspace.rs"],
        "Workspace deletion follows the current instance-reference behaviour.",
        [("delete", "Delete a workspace that the fixture links to an instance and one that does not.", "The visible result matches the current reference rule for each case.", "A different workspace disappears.")])
    add("J01.prewarm", "jackin", ["src/bin/jackin/screens/manager.rs", "src/bin/jackin/sim/world.rs"],
        "Prewarm runs once and a duplicate request is refused.",
        [("prewarm", "Prewarm an instance and request prewarm again.", "The second request is refused and does not add a second record.", "The duplicate starts another instance.")])
    add("J01.prelude", "jackin", ["src/bin/jackin/screens/prelude.rs", "src/bin/jackin/domain/workspace.rs"],
        "Prelude cancel leaves no workspace. Finish reaches the Editor values and save uses the J02 contract.",
        [("steps", "Start from a host directory and from Git. Visit Source, Destination, Edit, Workdir, and Name. Go Back and Cancel at each boundary. Enter an invalid and a duplicate name. Finish one valid prelude.", "Cancel adds no workspace. The Editor shows the pending values. Save then reopen shows those values.", "Invalid name continues.")],
        deps=["G03", "G04", "G05", "J02.save"])
    add("J02.general", "jackin", ["src/bin/jackin/screens/editor.rs", "src/bin/jackin/domain/workspace.rs"],
        "General name, directory, and policy edits stay pending until save.",
        [("general", "Change the name, working directory, and one policy. Reopen after save.", "The reopened values are the saved values.", "A cancelled edit is saved.")],
        scenarios=["returning"])
    add("J02.mounts", "jackin", ["src/bin/jackin/screens/editor.rs", "src/bin/jackin/screens/config.rs"],
        "Mount create, edit, and remove validate destination conflicts before any pending row changes.",
        [("mounts", "Add, edit, and remove a mount. Enter a destination conflict and an invalid path.", "The invalid form does not partially apply. The saved mount matches the form.", "A conflict is stored.")])
    add("J02.roles", "jackin", ["src/bin/jackin/screens/editor.rs", "src/bin/jackin/sim/world.rs"],
        "Role load, trust, failure, and retry stay on this workspace.",
        [("roles", "Load roles, trust one, show the failure fixture, and retry.", "Retry replaces the failure with the loaded roles. The workspace id is unchanged.", "Failure clears saved roles.")])
    add("J02.environments", "jackin", ["src/bin/jackin/screens/config.rs", "src/bin/jackin/screens/editor.rs"],
        "Environment keys, literal values, and secret references do not expose secret material.",
        [("env", "Add, edit, remove, and scope an environment value. Enter a duplicate key and a secret reference.", "The duplicate is rejected. The reference is masked.", "The secret value is visible.")])
    add("J02.accounts", "jackin", ["src/bin/jackin/screens/editor.rs", "src/bin/jackin/domain/workspace.rs"],
        "Account bindings, defaults, and fallback survive save and reopen.",
        [("bindings", "Change a binding and the default override, save, and reopen.", "The reopened binding is the saved binding.", "Fallback shows a different account.")])
    add("J02.save", "jackin", ["src/bin/jackin/screens/editor.rs", "src/bin/jackin/sim/changes.rs", "src/bin/jackin/sim/world.rs"],
        "Preview cancel, direct save, leave Save, Stay, Discard, and the one-shot save failure keep workspace identity.",
        [("save", "Cancel a preview, Stay, Discard, fail one save, retry, and reopen.", "Failure keeps the pending edit and the previous saved values. Retry then reopen shows the new values.", "Failure reports the edit as saved.")],
        scenarios=["hard-cases"])
    add("J03.general", "jackin", ["src/bin/jackin/screens/settings.rs"],
        "A real General edit sets the dirty marker and the change count.",
        [("edit", "Change one General value.", "The dirty marker and the count include that change.", "The settings screen is still the unchanged manager.")],
        scenarios=["returning"])
    add("J03.mounts", "jackin", ["src/bin/jackin/screens/settings.rs", "src/bin/jackin/screens/config.rs"],
        "Global mounts use the shared form and are distinct from workspace mounts.",
        [("mounts", "Add a global mount and compare its scope with a workspace mount.", "The saved scope is global.", "The workspace mount list gains the global mount.")],
        deps=["G03", "G04", "G05", "J02.mounts"])
    add("J03.environments", "jackin", ["src/bin/jackin/screens/settings.rs", "src/bin/jackin/screens/config.rs"],
        "Global environment values use the shared form and keep their scope.",
        [("env", "Add a global environment value and a secret reference.", "Reopen shows the global key and a masked reference.", "The value appears in a workspace environment.")],
        deps=["G03", "G04", "G05", "J02.environments"])
    add("J03.agents", "jackin", ["src/bin/jackin/screens/settings.rs"],
        "Agent authorization changes are part of the dirty count.",
        [("agents", "Change an agent authorization.", "The dirty count includes it. Save then reopen shows it.", "The change applies before save.")])
    add("J03.trust", "jackin", ["src/bin/jackin/screens/settings.rs"],
        "Trust changes survive save and are restored by Discard.",
        [("trust", "Change a trust value, Discard on one attempt, and Save on another.", "Discard restores the old value. Save reopens with the new value.", "Discard saves the change.")])
    add("J03.leave", "jackin", ["src/bin/jackin/screens/settings.rs", "src/bin/jackin/sim/changes.rs"],
        "Clean leave has no guard. Dirty Stay, Discard, Save, preview cancel, failure, and retry each do what they are named.",
        [("leave", "Leave clean. Then dirty the form and choose Stay, Discard, preview cancel, Save, a failing save, and retry.", "Each result matches its name. Focus returns to the opener. Reopen shows the saved or restored values.", "The leave baseline is the manager without an edit.")],
        scenarios=["hard-cases"])
    add("J04.accounts", "jackin", ["src/bin/jackin/screens/accounts.rs", "src/bin/jackin/sim/provider.rs"],
        "Account sources, provider validation, enable, disable, default, and removal keep identity and fallback.",
        [("accounts", "Exercise 1Password, local-folder, and API-key sources and the fixture's missing, locked, denied, revoked, invalid, and wrong-provider material. Enable, disable, choose default, and remove.", "Removal updates workspace-binding fallback. Captures show only mask, reference, fingerprint, or tail.", "A raw secret is visible.")],
        scenarios=["first-use", "accounts-mixed", "returning", "hard-cases"])
    add("J04.form", "jackin", ["src/bin/jackin/screens/accounts.rs"],
        "The account form rejects duplicate source, duplicate identity, a missing name, and an overlong name, and cancel restores the opened values.",
        [("form", "Enter each invalid case, then a valid edit, cancel, and reopen.", "Cancel reopens the previous values. A valid save reopens the new values.", "An invalid form saves a partial account.")])
    add("J04.chooser", "jackin", ["src/bin/jackin/screens/modals.rs", "src/bin/jackin/sim/onepassword.rs"],
        "The credential chooser can move through account, vault, item, and field, and a locked session retries.",
        [("chooser", "Open the chooser, go Back and Cancel, show an inaccessible vault and an unsupported field, and retry a locked session.", "The returned reference is the chosen field. Cancel returns no new reference.", "An inaccessible vault returns a field.")])
    add("J04.usage", "jackin", ["src/bin/jackin/screens/usage.rs", "src/bin/jackin/domain/usage.rs"],
        "Usage overview, provider, and account views keep last-good values after a failed refresh.",
        [("usage", "Open each usage view. Show quota, missing data, mixed health, freshness, and stale values. Start refresh, refuse a duplicate, succeed, and fail.", "Failure keeps the last good numbers. Return opens the same account.", "Failed refresh replaces the numbers with another account's quota.")])
    add("J05.intro", "jackin", ["src/bin/jackin/app.rs", "src/bin/jackin/rain.rs"],
        "First entry, joining, duplicate entry, and unknown discovery follow the current intro rules, including skip, guard, and abort.",
        [("intro", "Enter through each implemented intro path and exercise skip, guard, and abort.", "Abort does not open the cockpit as a successful join. Skip reaches the documented next route.", "Skip bypasses a guard the source still enforces.")],
        scenarios=["first-use", "returning"])
    add("J05.launch", "jackin", ["src/bin/jackin/screens/cockpit.rs", "src/bin/jackin/sim/launch.rs"],
        "Clean, FailNetwork, CredentialsLocked, and BlockedSidecar plans reach their real stages. The 11 stage identities are mapped.",
        [("stages", "Run each launch plan to its meaningful stage boundary with the controls the binary already supports.", "The visible stage name is that plan's stage. A failed launch does not attach a capsule and leaves one instance record.", "The failure checkpoint is still resolving credentials.")],
        scenarios=["launch-running", "launch-failure", "hard-cases"])
    add("J05.logs", "jackin", ["src/bin/jackin/screens/cockpit.rs"],
        "Build logs can be opened before and after output. Copy owns Ctrl+C before abort.",
        [("logs", "Open logs before output and after output. Select text and press Ctrl+C, then press Ctrl+C with no selection.", "The selection copies. The unselected Ctrl+C aborts. Ctrl+Q Cancel stays, and Confirm aborts.", "Ctrl+C aborts while a selection exists.")])
    add("J05.failure", "jackin", ["src/bin/jackin/screens/cockpit.rs", "src/bin/jackin/sim/launch.rs"],
        "The actual failed stage is visible before acknowledgement, and retry or fallback follows the source.",
        [("failure", "Reach the failed stage, acknowledge it, and use locked-credential retry and the supported one-launch fallback.", "The failure text is the failed stage. No capsule is attached.", "Acknowledgement is available before the failure is visible.")])
    add("J05.handoff", "jackin", ["src/bin/jackin/app.rs", "src/bin/jackin/rain.rs"],
        "Handoff and its skip reach the capsule without a second instance.",
        [("handoff", "Complete handoff and skip handoff on separate launches.", "The capsule instance is the launched instance.", "Skip creates another instance.")])
    add("J05.outro", "jackin", ["src/bin/jackin/app.rs", "src/bin/jackin/rain.rs"],
        "The last instance shows Outro. A non-last exit does not.",
        [("outro", "Exit the last instance and exit when another instance remains.", "Outro appears only for the last instance. The terminal is restored.", "A non-last exit shows Outro.")],
        scenarios=["outro-last"])
    add("J06.panes", "jackin", ["src/bin/jackin/screens/capsule.rs", "src/bin/jackin/sim/pty.rs"],
        "Only the focused pane receives typing, paste, and prompt replies.",
        [("input", "Type and paste into each supported pane kind and answer a simulated prompt.", "The other panes' text is unchanged.", "Input appears in an unfocused pane.")],
        scenarios=["capsule-multi"])
    add("J06.tabs", "jackin", ["src/bin/jackin/screens/capsule.rs"],
        "Tabs, splits, zoom, rename, and close keep unaffected pane identity.",
        [("layout", "Create a tab, a shell tab, and an agent tab. Split both ways, move focus, zoom, drag a seam, and resize by key. Refuse a minimum-size violation. Rename and close, including cancel, and close the last pane and last tab.", "The layout stays valid. Unaffected panes keep their output. Cancelled close leaves the pane.", "Minimum-size refusal overlaps panes.")])
    add("J06.prefix", "jackin", ["src/bin/jackin/screens/capsule.rs"],
        "Prefix entry, timeout, repeat, and cancellation do not leak the command into the pane.",
        [("prefix", "Enter the prefix, let it time out, repeat it, and cancel it. Also use the palette and a menu while a pane owns input.", "The command runs only after a completed prefix. Cancellation types nothing into the pane.", "A timed-out prefix still runs the command.")])
    add("J06.reconnect", "jackin", ["src/bin/jackin/screens/capsule.rs", "src/bin/jackin/app.rs"],
        "Detach and reconnect return to the same instance, and the checkpoint is taken after reconnect.",
        [("reconnect", "Detach and reconnect.", "The capsule instance id is the same id. The checkpoint shows the reconnected capsule.", "The baseline is taken before launch.")],
        scenarios=["returning"])
    add("J06.takeover", "jackin", ["src/bin/jackin/screens/capsule.rs", "src/bin/jackin/sim/world.rs"],
        "Takeover, a stale takeover for another instance, reconnect, and leave stay on the intended instance.",
        [("takeover", "Take over the current instance, attempt a stale takeover aimed at another instance, reconnect, and leave.", "The stale attempt does not attach the other instance.", "Stale takeover switches instances.")])
    add("J06.inspect", "jackin", ["src/bin/jackin/screens/inspect.rs"],
        "Inspector selection, detail, diff, and copy refer to the selected change.",
        [("inspect", "Select a change, open its detail and diff, and copy where the result is visible.", "The diff belongs to the selected change.", "Copy returns a different change's text.")])
    add("J06.exit", "jackin", ["src/bin/jackin/screens/capsule.rs", "src/bin/jackin/app.rs"],
        "Clean exit and every dirty choice, including Inspect and the second Discard confirmation, preserve or drop changes as named.",
        [("exit", "Exit clean. On a dirty capsule choose Keep, Discard and confirm the second prompt, cancel Discard, and Inspect then return.", "Keep retains the changes. The second confirm discards them. Cancel keeps them. Inspect returns to the same choice.", "The first Discard confirmation discards without the second prompt.")],
        scenarios=["outro-last", "hard-cases"])
    add("J06.export", "jackin", ["src/bin/jackin/screens/capsule.rs"],
        "Export validates the path, cancel exports nothing, and success reports the simulated path only.",
        [("export", "Enter an invalid path, cancel a valid path, and confirm a valid path.", "Invalid is rejected. Cancel writes no success report. Confirm reports that path.", "The test claims a host file was written.")])

    # Cross-application
    add("X01.cli", "cross", ["src/cli.rs", "src/bin/showcase/main.rs", "apps/tablepro/src/main.rs", "apps/holla/src/main.rs", "src/bin/jackin/main.rs"],
        "Help and invalid CLI values return the current exit status and error text. Short commands do not use the Ctrl+Q helper.",
        [("cli", "Run --help and an unknown page, scenario, connection, option key, and option value for each binary. Repeat --opt and observe precedence.", "The exit status and the error name the bad argument. A valid option is accepted.", "An invalid option starts the full-screen UI and is treated as success.")])
    add("X01.env", "cross", ["src/cli.rs", "src/theme.rs", "src/motion.rs"],
        "Explicit colour, glyph, and motion values beat the environment. NO_COLOR and each NO_MOTION variable are honoured.",
        [("env", "Launch with each colour and glyph value, with NO_COLOR, and with each application NO_MOTION variable.", "The rendered level matches the winning value.", "NO_COLOR still uses truecolor accents.")])
    add("X01.input", "cross", ["src/core/keyboard.rs", "src/core/event.rs", "src/shell/app.rs"],
        "Keyboard fallback, modified keys, bracketed paste, and modal paste ownership match the current routing.",
        [("input", "Send a modified key, a bracketed paste, multiline paste, control-like text, and Unicode. Paste while a modal is open.", "The modal receives the paste. The page underneath does not.", "Control-like paste text is executed as a shortcut.")])
    add("X01.clipboard", "cross", ["src/shell/clipboard.rs", "src/core/keyboard.rs"],
        "Selection copy precedes quit where the app implements it. OSC 52 or a paste round trip is asserted only when the app emits it.",
        [("copy", "Select text and press the copy chord, then press the quit chord with a selection and without one.", "A selection copies. Quit runs only when the app does not consume the key as copy.", "An unsupported clipboard path is reported as a copied payload.")])
    add("X02.startup", "cross", ["src/runtime.rs", "src/shell/app.rs"],
        "The first input after startup and a bounded burst leave the process alive with the expected final screen.",
        [("start", "Send the first key after startup. Send a burst of keys, pointer motion, wheel, and resize.", "The final semantic result is the result of the last intended action. The process is alive.", "The process exits during the burst.")],
        deps=["G03", "G04", "G05"])
    add("X02.resize", "cross", ["src/core/shape.rs", "src/shell/app.rs", "apps/tablepro/src/app.rs", "apps/holla/src/app.rs", "src/bin/jackin/app.rs"],
        "Narrow and too-small states are captured before the terminal grows. Open layers survive.",
        [("resize", "Shrink to 71x20, 72x19, and 72x20 and to each source breakpoint below, at, and above. Do this with a form, a picker, a destructive gate, and running output open. Then grow.", "The narrow checkpoint is the narrow size. After growth the same draft, selection, and focus remain.", "The narrow baseline is captured after the terminal returns to 120x40.")],
        deps=["G03", "G04", "G05", "T05.quit", "H03.guards", "J06.exit"])
    add("X02.suspend", "cross", ["src/runtime.rs"],
        "Suspend and resume are tested only if the pinned PTY API supports them. Otherwise the row records the limitation.",
        [("suspend", "Suspend and resume on this host when the harness can deliver the signal.", "The screen and modes return. If the API cannot suspend, the row stays BLOCKED with the API evidence.", "A killed process is reported as resume.")])
    add("X02.exit", "cross", ["src/shell/app.rs", "src/runtime.rs"],
        "Clean, dirty, running, attached, and launch exits report the real code and restore the observed modes.",
        [("exit", "Exit each applicable application through its clean path and through one guarded path.", "The code is 0 and the signal is empty after the user confirms. Alternate screen, paste, and mouse modes are restored.", "Drop's cleanup kill counts as success.")],
        deps=["G03", "G04", "G05", "T05.quit", "H03.guards", "J05.failure", "J06.exit"])
    add("X03.color", "cross", ["src/theme.rs", "src/ui/ctx.rs"],
        "Truecolor, 256, 16, none, and NO_COLOR each show the shared treatment, and contrast defects are recorded rather than fixed.",
        [("matrix", "Capture one shared control treatment in all five colour configurations for each application shell.", "Rest, hover, pressed, disabled, and selected are distinguishable at every level, including NO_COLOR modifiers.", "A colour change that keeps the text is accepted as equal.")],
        deps=["G03", "G04", "G05", "S02.buttons", "S01.shell", "T05.menus", "H01.finder", "J01.status"])
    add("X03.focus", "cross", ["src/theme.rs", "src/ui/ctx.rs", "src/ui/specimen.rs"],
        "Exactly one live focus gutter exists. Covered selection is neutral. Owner cursors use fill.",
        [("focus", "Focus a control, open an overlay over a selected list, and show an owner-controlled cursor.", "One gutter is visible in the top layer. The covered selection is neutral. The owner cursor is a fill without a second gutter.", "The page under the overlay keeps its gutter.")],
        deps=["G03", "G04", "G05", "S04.lists", "S05.dialogs"])
    add("X03.chrome", "cross", ["src/widgets/scrollbar.rs", "src/widgets/splitter.rs", "src/theme.rs"],
        "Scrollbar and seam states change only their own cells.",
        [("chrome", "Show a scrollbar at rest, hover, and drag, including monochrome modifiers. Show a seam at rest, hover, and drag.", "The thumb glyph stays the same. Only the seam cells change weight and colour.", "Padding cells take the accent.")],
        deps=["G03", "G04", "G05", "S07.scrolling", "S07.splitsstacks"])
    add("X03.glyphs", "cross", ["src/theme.rs", "src/cli.rs"],
        "Unicode and ASCII modes render the same component family with their own glyphs.",
        [("glyphs", "Open one glyph-dependent family in Unicode and ASCII, including combining text, wide text, and emoji where the fixture has them.", "ASCII mode does not draw the Unicode gutter or seam.", "ASCII mode still draws ┃.")],
        deps=["G03", "G04", "G05", "S02.buttons"])
    add("X03.motion", "cross", ["src/motion.rs", "src/bin/jackin/rain.rs", "src/bin/showcase/app.rs"],
        "Full, reduced, and paused motion are distinct where the app implements them. A transient that cannot be frozen stays BLOCKED.",
        [("motion", "Capture paused and reduced frames with the existing frame controls. Capture a Jackin phase with the clock hook. Do not add a Showcase clock hook.", "Paused frames are stable. A Showcase transient that cannot be frozen is BLOCKED, not accepted as a changing baseline.", "A changing frame is stored as the expected image.")],
        deps=["G03", "G04", "G05", "J05.intro", "J05.handoff", "J05.outro", "S06.progress"])
    owner = {
        ("holla", "first-use"): "H01.finder",
        ("holla", "rust-dirty"): "H01.finder",
        ("holla", "monorepo-root"): "H01.finder",
        ("holla", "monorepo-child"): "H01.scope",
        ("holla", "remote-host"): "H01.scope",
        ("holla", "parity-discovery"): "H01.finder",
        ("holla", "parity-history"): "H01.memory",
        ("holla", "parity-task-sources"): "H02.args",
        ("holla", "parity-custom-actions"): "H02.config",
        ("holla", "hard-cases"): "H02.config",
        ("holla", "activities-multi"): "H03.output",
        ("holla", "launch-failure"): "H03.output",
        ("holla", "parity-executor"): "H03.output",
        ("holla", "parity-task-input"): "H03.input",
        ("holla", "parity-git-current"): "H04.actions",
        ("holla", "parity-git-batch"): "H04.actions",
        ("holla", "parity-cargo"): "H04.actions",
        ("holla", "parity-docker"): "H04.actions",
        ("holla", "parity-brew-services"): "H04.actions",
        ("holla", "parity-upgrade-managers"): "H04.actions",
        ("holla", "docker-cleanup"): "H04.snapshots-docker",
        ("holla", "parity-platforms"): "H04.snapshots-services",
        ("holla", "parity-platforms-linux"): "H04.snapshots-services",
        ("holla", "upgrade-plan"): "H04.plan",
        ("holla", "parity-files"): "H05.files",
        ("holla", "parity-browser"): "H05.files",
        ("holla", "parity-disk-scan"): "H05.scan",
        ("holla", "disk-cleanup"): "H05.scan",
        ("holla", "parity-disk-navigation"): "H05.disk",
        ("holla", "parity-insights"): "H05.disk",
        ("holla", "parity-delete-safety"): "H06.cleanup",
        ("holla", "parity-gradle"): "H06.gate",
        ("holla", "parity-idea"): "H06.gate",
        ("holla", "parity-cleanup-results"): "H06.report",
        ("jackin", "first-use"): "J05.intro",
        ("jackin", "returning"): "J02.general",
        ("jackin", "accounts-mixed"): "J04.accounts",
        ("jackin", "launch-running"): "J05.launch",
        ("jackin", "launch-failure"): "J05.launch",
        ("jackin", "capsule-multi"): "J06.panes",
        ("jackin", "outro-last"): "J05.outro",
        ("jackin", "hard-cases"): "J02.save",
    }
    for spec in specs:
        kept = []
        for item in spec["rows"]:
            if item["kind"] != "scenario" or owner.get((item["application"], item["scenario"])) == item["owner_task"]:
                kept.append(item)
        spec["rows"] = kept
        spec["task"]["coverage_rows"] = [item["case_id"] for item in kept]
        spec["task"]["checkpoints"] = [
            item["checkpoint_ids"][0] for item in kept if item.get("checkpoint_ids")
        ]
    missing = sorted(set(owner) - {(item["application"], item["scenario"]) for spec in specs for item in spec["rows"] if item["kind"] == "scenario"})
    if missing:
        raise SystemExit(f"scenario owners produced no row: {missing}")
    return specs


def write_markdown(ids, tasks, summary, current_tests, baselines) -> None:
    lines = [
        "# Execution ledger",
        "",
        "This ledger is the assignment record for the pre-refactor test programme.",
        "It is not a claim that the cases have already passed.",
        "The architecture refactor is out of scope.",
        "",
        f"Reference commit: `{REF}`.",
        f"Execution tasks: {summary['execution_task_count']}.",
        f"Leaf tasks: {summary['leaf_task_count']}.",
        f"Coverage rows: {summary['coverage_row_count']}.",
        "",
        "Recorded order: G00, G01, G02, G05, G03, G04, then one Showcase route per leaf plus S01.shell, then TablePro, Holla, Jackin, and cross-application leaves, then C01, C02, C03, and C04.",
        "",
        "Each leaf has one task id, one case file, one coverage fragment, and one baseline namespace.",
        "Scope cards S01 through S10, T01 through T05, H01 through H06, J01 through J06, and X01 through X03 are not assigned.",
        "Their children are the assigned tasks.",
        "",
        "Showcase section inventory: 54 routes, 162 sections, 324 section-size captures.",
        "SpecimenState has 14 states. The catalog lists 13 and omits `editing`.",
        "Option rows execute a default and one boundary, plus `needs` dependencies. They are not a Cartesian product.",
        "",
        "## Order",
        "",
    ]
    by_id = {task["task_id"]: task for task in tasks}
    for task_id in ids:
        task = by_id[task_id]
        deps = ", ".join(task["dependencies"]) if task["dependencies"] else "none"
        lines.append(f"- `{task_id}` ({task['kind']}, wave {task['wave']}) depends on {deps}.")
    lines.extend(
        [
            "",
            "## Current evidence",
            "",
            f"The Nextest list at the ledger draft contains {len(current_tests)} functional tests.",
            f"The baseline tree contains {len(baselines)} samples.",
            "Those tests are classified in `refactor-coverage.json` and are not counted as PASS for the new rows.",
            "",
            "## Acceptance rule",
            "",
            "A row stays BLOCKED until a PTY case drives the real binary, asserts the semantic result and the forbidden effect, and compares a reference capture.",
            "NOT_APPLICABLE requires source evidence.",
            "An implemented state without a test cannot be closed as NOT_APPLICABLE.",
            "",
        ]
    )
    (OUT / "execution-ledger.md").write_text("\n".join(lines) + "\n")

    gaps = """# Known gaps

These are current-source observations. They are not permission to change product behaviour.

## Catalog and specimen

`SpecimenState` has 14 states, including `editing`.
`src/catalog.rs` lists 13 `state::` ids and omits `editing`.
Pages that draw `editing` still owe a specimen checkpoint.
The catalog entry is not added in this test programme because that is a production change.
The omission stays a characterized inventory gap.

## Clock paste

The shared harness sets `JUNIE_TUI_TEST_CLOCK=1` and Showcase page visits paste `junie-tui:advance-clock:`.
Only Jackin overrides `advance_clock` to consume that paste.
`shell::handle` still delivers the paste as ordinary input when `advance_clock` returns `None`.
The harness must stop sending the paste to applications that do not implement it.
No production clock hook is added to Showcase.

## Existing claims

The keyboard-activation test can accept a result from an earlier click.
The table-sort test discards the row-order value it captured.
The form test does not reset or submit a valid form.
The dialog test does not cycle the modal focus ring.
The Jackin stop, settings, launch-failure, and reconnect baselines do not show the state their names claim.
TablePro `resize_60x20` and Holla `resize_narrow` are 120 by 40 because they were captured after the terminal grew.
Those captures are retired only when a reference binary produces a replacement with a new baseline id.

## Historical visual-baseline

The local repository has no `visual-baseline` tag.
`git ls-remote --tags origin` returned no tags.
The GitHub release `visual-baseline` was not found.
Historical parity is not claimed. The tag is not created.

## Simulated backends

TablePro, Holla, and Jackin simulate their services.
A passing journey does not prove PostgreSQL, Docker, a credential provider, a daemon, or a host export.
"""
    (OUT / "known-gaps.md").write_text(gaps)

    protection = f"""# Refactor protection

This document is the working contract for the test programme.
The readiness report is not READY until C04 records that result at one commit.

Reference commit: `{REF}`.
Leaf count and coverage counts live in [the execution ledger](execution-ledger.md).
The machine-readable inventory is [refactor-coverage.json](refactor-coverage.json).

Ordinary test execution is read-only toward baselines.
Reference acceptance is a separate mode and is not part of the candidate gate.
The architecture refactor has not started.
"""
    (OUT / "refactor-protection.md").write_text(protection)

    manifest = {
        "schema": "reference-manifest.v1",
        "reference_sha": REF,
        "branch": "main",
        "remote_default": "origin/main",
        "design_interaction_polish": REF,
        "main": REF,
        "audited_commit": REF,
        "visual_baseline_tag": None,
        "visual_baseline_release": None,
        "visual_baseline_note": "Remote advertised no tags and no visual-baseline release during this run. The tag was not created.",
        "lockfile_sha256": "582b6cd514d0bd9c90966bdea90028ce24b652a0607d0ef7b8369a1a7532e319",
        "toolchain": {
            "rustc": "1.99.1 (58cf8f9c3 2026-10-13)",
            "cargo": "1.99.1 (5f94df478 2026-08-27)",
            "nextest": "0.9.148",
            "tuiscotti_cli": "0.4.0",
            "tuiscotti_rev": "d8d54155947f6303a41dfd6079b53c0abc16f29a",
            "tuiscotti_tui_test": "0.2.1",
            "mise": "2026.10.25",
            "velnor_actions": "0.1.70",
            "host": "aarch64-apple-darwin",
        },
        "comparator": {
            "name": "tuiscotti FrozenStore state comparison",
            "rev": "d8d54155947f6303a41dfd6079b53c0abc16f29a",
            "fields": [
                "cols",
                "rows",
                "scope",
                "viewport_origin",
                "palette",
                "default_foreground",
                "default_background",
                "default_cursor",
                "cursor.col",
                "cursor.row",
                "cursor.in_capture",
                "cursor.visible",
                "cursor.shape",
                "cursor.color",
                "cursor.blink",
                "title",
                "cells",
            ],
        },
        "environment": {
            "TERM": "xterm-256color",
            "COLORTERM": "truecolor",
            "LANG": "C.UTF-8",
            "LC_ALL": "C.UTF-8",
            "TZ": "UTC",
            "JUNIE_TUI_CLIENT_PLATFORM": "macos",
            "HOLLA_NO_HISTORY": "1",
        },
        "binaries": {
            "showcase": {
                "path": "tests/functional/reference-bins/aarch64-apple-darwin/showcase",
                "worktree": "",
                "sha256": "430460e90a752587d7f1f6d2e37b4d8343076a2787a343048fbd3448b3d54a0b",
                "built_from": REF,
                "platforms": {
                    "aarch64-apple-darwin": {
                        "path": "tests/functional/reference-bins/aarch64-apple-darwin/showcase",
                        "sha256": "430460e90a752587d7f1f6d2e37b4d8343076a2787a343048fbd3448b3d54a0b",
                    },
                    "x86_64-unknown-linux-gnu": {
                        "path": "tests/functional/reference-bins/x86_64-unknown-linux-gnu/showcase",
                        "sha256": "c6cac7998489a8ed6bb427c126e25a3a8bb1027823809beb578c3cec28bebf4a",
                    },
                },
            },
            "tablepro": {
                "path": "tests/functional/reference-bins/aarch64-apple-darwin/tablepro",
                "worktree": "",
                "sha256": "018f598830a0c6ea8912660be87dc16cb40b2c2fcd58f1867dab3ea4eb57a186",
                "built_from": REF,
                "platforms": {
                    "aarch64-apple-darwin": {
                        "path": "tests/functional/reference-bins/aarch64-apple-darwin/tablepro",
                        "sha256": "018f598830a0c6ea8912660be87dc16cb40b2c2fcd58f1867dab3ea4eb57a186",
                    },
                    "x86_64-unknown-linux-gnu": {
                        "path": "tests/functional/reference-bins/x86_64-unknown-linux-gnu/tablepro",
                        "sha256": "02567d634afa9af1a555c654f917c4cb0636d35016897adad5ed03288f6a28f7",
                    },
                },
            },
            "holla": {
                "path": "tests/functional/reference-bins/aarch64-apple-darwin/holla",
                "worktree": "",
                "sha256": "2ea3328773dce7ab202de90711341081aba74f0001d3d05f80ff1812e4bb40d5",
                "built_from": REF,
                "platforms": {
                    "aarch64-apple-darwin": {
                        "path": "tests/functional/reference-bins/aarch64-apple-darwin/holla",
                        "sha256": "2ea3328773dce7ab202de90711341081aba74f0001d3d05f80ff1812e4bb40d5",
                    },
                    "x86_64-unknown-linux-gnu": {
                        "path": "tests/functional/reference-bins/x86_64-unknown-linux-gnu/holla",
                        "sha256": "7a0c8edd7a5f656ecaa861016f3487d1a88f9e4ba77c11c028baf127e53d6f7a",
                    },
                },
            },
            "jackin": {
                "path": "tests/functional/reference-bins/aarch64-apple-darwin/jackin",
                "worktree": "",
                "sha256": "bdb8b27fb925ec10d7511013d0ab82fad4faf5b0c27797aca87eebf2f0802ceb",
                "built_from": REF,
                "platforms": {
                    "aarch64-apple-darwin": {
                        "path": "tests/functional/reference-bins/aarch64-apple-darwin/jackin",
                        "sha256": "bdb8b27fb925ec10d7511013d0ab82fad4faf5b0c27797aca87eebf2f0802ceb",
                    },
                    "x86_64-unknown-linux-gnu": {
                        "path": "tests/functional/reference-bins/x86_64-unknown-linux-gnu/jackin",
                        "sha256": "5fb58a3679191ef0e939d02bed8579519f320fe48d62bac60f78b6f306459f0c",
                    },
                },
            },
        },
        "existing_suite": {
            "status": "PASS",
            "nextest_list_count": summary["existing_test_count"],
            "partitions": [
                {
                    "filter": "test(/holla_/)",
                    "ran": 11,
                    "passed": 11,
                    "skipped": 82,
                    "seconds": 8.259,
                },
                {
                    "filter": "not test(/holla_/)",
                    "ran": 82,
                    "passed": 82,
                    "skipped": 11,
                    "seconds": 56.631,
                },
            ],
            "union": "93 tests, 93 passed, 0 failed. Skips are the other partition.",
        },
    }
    (OUT / "reference-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
