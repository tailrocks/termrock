//! TablePro discard review, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t02_discard/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, TABLEPRO, find_all, flash_settle, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const ORGANIZATIONS: &str = "organizations";
const EDITED: &str = "DiscardEdit";
const TITLE: &str = "Discard unsaved changes?";
const DROPPED: &str = "1 pending change will be dropped.";
const RELOADED: &str = "reloaded from the server.";
const PENDING: &str = "1 pending · 1 update";
const DISCARDED: &str = "Changes discarded";
const FOCUS: &str = "▎";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t02_discard/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, TABLEPRO);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Filter connections");
    app
}

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = text(screen);
    for item in present {
        assert!(visible.contains(item), "missing {item:?}\n{visible}");
    }
    for item in absent {
        assert!(!visible.contains(item), "forbidden {item:?}\n{visible}");
    }
}

fn click_listed(app: &App, label: &str) {
    let screen = app.wait_text(label);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| hit.x <= 35)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "list hit {label:?}: {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn click_connect(app: &App) {
    let screen = app.settle();
    let hits: Vec<_> = find_all(&screen, "Connect")
        .into_iter()
        .filter(|hit| {
            let row = row_text(&screen, u32::from(hit.y));
            hit.x >= 40
                && row.contains("Edit")
                && row.contains("Delete")
                && !row.contains("Reconnect")
        })
        .collect();
    assert_eq!(hits.len(), 1, "connect button {hits:?}\n{}", text(&screen));
    app.click(hits[0]);
}

fn connect_development(app: &App) {
    click_listed(app, "Development");
    let detail = app.wait_text(HOST);
    see(
        &detail,
        &[HOST, DATABASE, "Development"],
        &["Authentication failed", "Could not reach the host"],
    );
    click_connect(app);
    // Query 1 is drawn during "Loading schema…", before the connected status.
    let work = app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains(CONNECTED)
            && visible.contains(QUERY)
            && !visible.contains("Loading schema")
    });
    see(
        &work,
        &[CONNECTED, "acme_prod › public", QUERY],
        &[
            "Authentication failed",
            "Could not reach the host",
            "Scratch",
            "~/scratch.db",
        ],
    );
}

fn open_organizations(app: &App) {
    app.key("ctrl+o");
    app.wait_text("Open Quickly");
    app.type_text(ORGANIZATIONS);
    app.wait_text(ORGANIZATIONS);
    app.key("enter");
    app.wait_screen("opened organizations", |screen| {
        let visible = text(screen);
        visible.contains("public › organizations") && !visible.contains("Open Quickly")
    });
}

fn is_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    bytes.iter().enumerate().all(|(index, byte)| match index {
        8 | 13 | 18 | 23 => *byte == b'-',
        _ => byte.is_ascii_hexdigit(),
    })
}

fn uuid_at(row: &str) -> Option<usize> {
    let mut start = 0;
    while start + 36 <= row.len() {
        if !row.is_char_boundary(start) {
            start += 1;
            continue;
        }
        if row.is_char_boundary(start + 36) && is_uuid(&row[start..start + 36]) {
            return Some(start);
        }
        start += 1;
    }
    None
}

fn next_field(rest: &str) -> Option<(&str, &str)> {
    let rest = rest.trim_start();
    if rest.is_empty() {
        return None;
    }
    let bytes = rest.as_bytes();
    let mut end = rest.len();
    for index in 0..bytes.len().saturating_sub(1) {
        if bytes[index] == b' ' && bytes[index + 1] == b' ' {
            end = index;
            break;
        }
    }
    let field = rest[..end].trim();
    if field.is_empty() || field.contains('…') {
        return None;
    }
    Some((field, &rest[end..]))
}

#[derive(Clone, Debug)]
struct OrgCells {
    id: String,
    name: String,
    slug: String,
}

fn parse_org(row: &str) -> Option<OrgCells> {
    let start = uuid_at(row)?;
    let id = row[start..start + 36].to_string();
    let (name, after_name) = next_field(&row[start + 36..])?;
    let (slug, _) = next_field(after_name)?;
    Some(OrgCells {
        id,
        name: name.to_string(),
        slug: slug.to_string(),
    })
}

fn org_rows(screen: &TerminalSnapshot) -> Vec<OrgCells> {
    (0..screen.rows())
        .filter_map(|index| parse_org(&row_text(screen, index)))
        .collect()
}

fn remember(screen: &TerminalSnapshot) -> (OrgCells, OrgCells) {
    let rows = org_rows(screen);
    assert!(
        rows.len() >= 2,
        "need two organization rows\n{}",
        text(screen)
    );
    let edited = rows[0].clone();
    let neighbor = rows
        .into_iter()
        .skip(1)
        .find(|row| row.name != edited.name)
        .unwrap_or_else(|| panic!("no untouched neighbor\n{}", text(screen)));
    (edited, neighbor)
}

fn row_with_id(screen: &TerminalSnapshot, id: &str) -> String {
    (0..screen.rows())
        .map(|index| row_text(screen, index))
        .find(|row| row.contains(id))
        .unwrap_or_else(|| panic!("row {id} missing\n{}", text(screen)))
}

fn assert_row(screen: &TerminalSnapshot, org: &OrgCells, name: &str) {
    let row = row_with_id(screen, &org.id);
    assert!(
        row.contains(name) && row.contains(&org.slug) && row.contains(&org.id),
        "row {row:?} wanted {name:?} slug {} id {}",
        org.slug,
        org.id
    );
}

fn assert_dialog(screen: &TerminalSnapshot, edited: &str) {
    see(
        screen,
        &[TITLE, DROPPED, RELOADED, "Cancel", "Discard", edited],
        &[DISCARDED],
    );
    let visible = text(screen);
    assert!(
        visible.contains(&format!("{FOCUS}Cancel")),
        "Cancel is not focused\n{visible}"
    );
    assert!(
        !visible.contains(&format!("{FOCUS}Discard")),
        "Discard took focus\n{visible}"
    );
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn edit_name(app: &App, org: &OrgCells) {
    app.key("right");
    app.key("enter");
    app.wait_text("Apply");
    // macOS field keys are Emacs, so Select All in the open cell editor is Ctrl+L.
    app.key("ctrl+l");
    app.key("backspace");
    app.type_text(EDITED);
    app.key("enter");
    let committed = app.wait_screen("name edit committed", |screen| {
        let visible = text(screen);
        visible.contains(PENDING) && row_with_id(screen, &org.id).contains(EDITED)
    });
    assert_row(&committed, org, EDITED);
    let row = row_with_id(&committed, &org.id);
    assert!(
        !row.contains(&org.name),
        "edit left the original name on the row: {row}"
    );
    see(
        &committed,
        &[EDITED, &org.slug, PENDING],
        &["Apply", DISCARDED, TITLE],
    );
}

#[test]
fn tablepro_discard_route_returns_from_a_clean_grid() {
    let app = open("t02-discard-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    connect_development(&app);
    open_organizations(&app);
    let table = app.wait_text("public › organizations");
    see(
        &table,
        &["Enter Edit", "public › organizations", "name", "slug"],
        &[PENDING, TITLE, DISCARDED, "1 pending"],
    );
    app.key("ctrl+w");
    let back = app.wait_screen("query tab after closing organizations", |screen| {
        let visible = text(screen);
        visible.contains("Type SQL") && !visible.contains("public › organizations")
    });
    let tabs = (0..back.rows())
        .map(|index| row_text(&back, index))
        .find(|row| row.contains(QUERY) && row.contains('×'))
        .unwrap_or_default();
    assert!(
        !tabs.contains(ORGANIZATIONS),
        "organizations tab stayed: {tabs}\n{}",
        text(&back)
    );
    see(
        &back,
        &[QUERY, "Type SQL"],
        &[
            TITLE,
            DISCARDED,
            "public › organizations",
            "T organizations ×",
        ],
    );
    app.finish();
}

#[test]
fn tablepro_discard_cancel_keeps_the_edit_and_confirm_restores_it() {
    let app = open("t02-discard-review");
    connect_development(&app);
    open_organizations(&app);
    let table = app.wait_text("public › organizations");
    let (org, neighbor) = remember(&table);
    assert_ne!(org.name, neighbor.name);
    see(
        &table,
        &[&org.name, &neighbor.name, &neighbor.slug],
        &[EDITED, PENDING],
    );
    edit_name(&app, &org);
    let pending = app.screen();
    assert_row(&pending, &neighbor, &neighbor.name);

    app.key("U");
    let dialog = app.wait_text(TITLE);
    assert_dialog(&dialog, EDITED);
    assert_row(&dialog, &neighbor, &neighbor.name);
    flash_settle(&app);
    let wide = app.screen();
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    assert_dialog(&wide, EDITED);
    shot(&app, "T02.discard.discard.120x40");

    app.resize(80, 24);
    let narrow = app.wait_screen("discard dialog at 80x24", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && text(screen).contains(TITLE)
            && text(screen).contains(DROPPED)
            && text(screen).contains("Cancel")
            && text(screen).contains("Discard")
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    flash_settle(&app);
    let narrow = app.screen();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, DROPPED, "Cancel", "Discard"],
        &[DISCARDED],
    );
    shot(&app, "T02.discard.discard.narrow");

    app.resize(120, 40);
    // The 120×40 size is reported while the narrow grid is still painted,
    // so the dialog still covers the edited name.
    let restored = app.wait_screen("discard dialog back at 120x40", |screen| {
        if screen.cols() != 120 || screen.rows() != 40 {
            return false;
        }
        let visible = text(screen);
        visible.contains("120×40")
            && visible.contains(TITLE)
            && visible.contains(DROPPED)
            && visible.contains(RELOADED)
            && visible.contains(EDITED)
            && visible.contains(&format!("{FOCUS}Cancel"))
            && !visible.contains(&format!("{FOCUS}Discard"))
            && !visible.contains(DISCARDED)
    });
    assert_dialog(&restored, EDITED);

    app.key("enter");
    let kept = app.wait_screen("cancel keeps the edit", |screen| {
        let visible = text(screen);
        !visible.contains(TITLE)
            && !visible.contains(DISCARDED)
            && visible.contains(EDITED)
            && visible.contains(PENDING)
    });
    assert_row(&kept, &org, EDITED);
    assert_row(&kept, &neighbor, &neighbor.name);
    see(&kept, &[PENDING, &org.slug], &[TITLE, DISCARDED]);

    app.key("U");
    let again = app.wait_text(TITLE);
    assert_dialog(&again, EDITED);
    app.key("right");
    let armed = app.wait_screen("discard button focused", |screen| {
        let visible = text(screen);
        visible.contains(&format!("{FOCUS}Discard")) && !visible.contains(&format!("{FOCUS}Cancel"))
    });
    see(&armed, &[TITLE, "Discard"], &[DISCARDED]);
    app.key("enter");
    let cleared = app.wait_screen("confirm restored the cell", |screen| {
        let visible = text(screen);
        visible.contains(DISCARDED) && !visible.contains(TITLE) && !visible.contains(EDITED)
    });
    assert_row(&cleared, &org, &org.name);
    assert_row(&cleared, &neighbor, &neighbor.name);
    let row = row_with_id(&cleared, &org.id);
    assert!(
        !row.contains(EDITED),
        "confirm left the edited token on the row: {row}"
    );
    see(
        &cleared,
        &[DISCARDED, &org.name, &neighbor.name, &neighbor.slug],
        &[EDITED, PENDING, "1 pending", TITLE],
    );
    app.finish();
}
