use super::*;
use jackin_console::usage_connections::HubRow;

const FORM: &str = "accounts.usage_connection";

fn sample_account(hub: Option<&str>) -> UsageEntry {
    UsageEntry {
        id: "second-account".into(),
        label: "Second workspace".into(),
        provider: "kimi".into(),
        details: vec!["Weekly 42%".into()],
        resets: vec![],
        hub: hub.map(str::to_owned),
        region: Some("cn".into()),
        device_login: true,
    }
}

fn inspect(row: UsageEntry) -> (jackin_console::app::App, Arc<Boundary>) {
    let (mut app, backend) = accounts_with_usage();
    press(&mut app, "N");
    let (ticket, action) = backend.pending.lock().unwrap().pop().unwrap();
    assert_eq!(action, UsageAction::List);
    app.deliver(&Msg::UsageConnection {
        ticket,
        update: UsageUpdate::Accounts(vec![row]),
    });
    activate(&mut app, FORM, "save");
    (app, backend)
}

#[test]
fn failed_refresh_retries_the_selected_account_and_back_preserves_the_target() {
    let row = sample_account(None);
    let (mut app, backend) = inspect(row.clone());
    activate(&mut app, FORM, "refresh");
    let (ticket, action) = backend.pending.lock().unwrap().pop().unwrap();
    assert_eq!(action, UsageAction::Refresh(row.id.clone()));
    app.deliver(&Msg::UsageConnection {
        ticket,
        update: UsageUpdate::Failed("Try again later".into()),
    });
    assert!(screen(&mut app).contains("Try again later"));
    activate(&mut app, FORM, "save");
    let (retry, action) = backend
        .pending
        .lock()
        .unwrap()
        .pop()
        .expect("retry request");
    assert_ne!(ticket, retry);
    assert_eq!(action, UsageAction::Refresh(row.id.clone()));
    app.deliver(&Msg::UsageConnection {
        ticket: retry,
        update: UsageUpdate::Failed("Still busy".into()),
    });
    activate(&mut app, FORM, "back");
    assert!(screen(&mut app).contains("Second workspace"));
    activate(&mut app, FORM, "remove");
    assert!(screen(&mut app).contains("Remove the local connection for Second workspace?"));
    activate(&mut app, FORM, "back");
    let shown = screen(&mut app);
    assert!(shown.contains("Weekly 42%"), "{shown}");
    assert!(
        !backend
            .pending
            .lock()
            .unwrap()
            .iter()
            .any(|(_, a)| matches!(a, UsageAction::Remove(_)))
    );
}

#[test]
fn mirror_manages_its_own_hub_and_back_from_removal_retains_its_scope() {
    let (mut app, backend) = inspect(sample_account(Some("hub-b")));
    activate(&mut app, FORM, "hubs");
    let (ticket, action) = backend.pending.lock().unwrap().pop().unwrap();
    assert_eq!(action, UsageAction::Hubs);
    let hubs = ["a", "b"]
        .map(|id| HubRow {
            id: format!("hub-{id}"),
            address: format!("https://{id}.example.test"),
            refused: false,
        })
        .to_vec();
    app.deliver(&Msg::UsageConnection {
        ticket,
        update: UsageUpdate::Hubs(hubs),
    });
    let shown = screen(&mut app);
    assert!(
        shown.contains("https://b.example.test") && shown.contains("Sync accounts"),
        "{shown}"
    );
    activate(&mut app, FORM, "remove_hub");
    let shown = screen(&mut app);
    assert!(
        shown.contains("mirrored accounts") && shown.contains("https://b.example.test"),
        "{shown}"
    );
    activate(&mut app, FORM, "back");
    assert!(screen(&mut app).contains("Sync accounts"));
    activate(&mut app, FORM, "back");
    assert!(screen(&mut app).contains("Second workspace"));
    assert!(
        !backend
            .pending
            .lock()
            .unwrap()
            .iter()
            .any(|(_, a)| matches!(a, UsageAction::RemoveHub(_)))
    );
}

#[test]
fn reconnect_back_preserves_account_region_and_selected_method() {
    let row = sample_account(None);
    let (mut app, backend) = inspect(row.clone());
    activate(&mut app, FORM, "reconnect");
    activate(&mut app, FORM, "save");
    let (ticket, action) = backend.pending.lock().unwrap().pop().unwrap();
    let UsageAction::Begin(first) = action else {
        panic!("expected sign-in")
    };
    assert_eq!(first.replace.as_deref(), Some(row.id.as_str()));
    assert_eq!(first.region.as_deref(), Some("cn"));
    assert_eq!(first.device_login, Some(true));
    app.deliver(&Msg::UsageConnection {
        ticket,
        update: UsageUpdate::Device {
            url: "https://example.test/verify".into(),
            code: "SYNTHETIC".into(),
        },
    });
    activate(&mut app, FORM, "back");
    activate(&mut app, FORM, "save");
    let (_, action) = backend.pending.lock().unwrap().pop().unwrap();
    assert_eq!(action, UsageAction::Begin(first));
}

#[test]
fn empty_hubs_offers_add_without_an_inert_manage_action() {
    let (mut app, backend) = accounts_with_usage();
    press(&mut app, "n");
    activate(&mut app, FORM, "hubs");
    let (ticket, action) = backend.pending.lock().unwrap().pop().unwrap();
    assert_eq!(action, UsageAction::Hubs);
    app.deliver(&Msg::UsageConnection {
        ticket,
        update: UsageUpdate::Hubs(Vec::new()),
    });
    let shown = screen(&mut app);
    assert!(shown.contains("No hubs connected."), "{shown}");
    let save = WidgetId::of(FORM).sub("save");
    for _ in 0..12 {
        assert_ne!(app.shell.focus.current(), Some(save));
        press(&mut app, "tab");
    }
    activate(&mut app, FORM, "add_hub");
    let shown = screen(&mut app);
    assert!(
        shown.contains("Hub URL") && shown.contains("Management key"),
        "{shown}"
    );
}
