use super::*;
use jackin_console::app::App;
use jackin_console::usage_connections::{
    BoardEdit, BoardMetric, BoardSettings, PRODUCTS, UsageBoard,
};
use junie_tui::core::event::{Input, Key, Mouse, MouseKind};
use junie_tui::runtime::Application;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Position;

struct Flow {
    app: App,
    backend: Arc<Boundary>,
    size: (u16, u16),
}

impl Flow {
    fn new(size: (u16, u16)) -> Self {
        let (mut app, backend) = accounts_with_usage();
        let requests = std::mem::take(&mut *backend.pending.lock().unwrap());
        if let Some((ticket, UsageAction::List)) = requests.last() {
            app.deliver(&Msg::UsageConnection {
                ticket: *ticket,
                update: UsageUpdate::Accounts(vec![]),
            });
        }
        let mut flow = Self { app, backend, size };
        flow.shown();
        flow
    }

    fn shown(&mut self) -> String {
        screen_at(&mut self.app, self.size.0, self.size.1)
    }

    fn key(&mut self, code: KeyCode) {
        Application::handle(
            &mut self.app,
            Input::Key(Key {
                code,
                mods: KeyModifiers::NONE,
            }),
        );
        self.shown();
    }

    fn focus(&mut self, id: WidgetId) {
        for _ in 0..32 {
            if self.app.shell.focus.current() == Some(id) {
                return;
            }
            self.key(KeyCode::Tab);
        }
        panic!("unreachable {id:?}\n{}", self.shown());
    }

    fn action(&mut self, name: &str) {
        self.focus(WidgetId::of("accounts.usage_connection").sub(name));
        self.key(KeyCode::Enter);
    }

    fn board_action(&mut self, name: &str) {
        self.focus(WidgetId::of("usage.connected_board").sub(name));
        self.key(KeyCode::Enter);
    }

    fn click(&mut self, label: &str) {
        let shown = self.shown();
        let (row, line) = shown
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains(label))
            .unwrap_or_else(|| panic!("missing {label}\n{shown}"));
        let col = line[..line.find(label).unwrap()].chars().count() + 1;
        for kind in [MouseKind::Down, MouseKind::Up] {
            Application::handle(
                &mut self.app,
                Input::Mouse(Mouse::new(kind, Position::new(col as u16, row as u16))),
            );
            self.shown();
        }
    }

    fn click_id(&mut self, id: WidgetId) {
        let rect = self
            .app
            .shell
            .hits
            .area_of(id)
            .unwrap_or_else(|| panic!("missing hit {id:?}"));
        let position = Position::new(rect.x + rect.width / 2, rect.y);
        for kind in [MouseKind::Down, MouseKind::Up] {
            Application::handle(&mut self.app, Input::Mouse(Mouse::new(kind, position)));
            self.shown();
        }
    }

    fn choose_mouse(&mut self, index: usize) {
        let id = WidgetId::of("accounts.usage_connection").sub("provider");
        self.click_id(id);
        let names = PRODUCTS.map(|(_, label)| label);
        let select = junie_tui::widgets::select::Select::new(id, "Provider", &names, 0);
        for _ in 0..16 {
            if self
                .app
                .shell
                .hits
                .area_of(select.option_id(index))
                .is_some()
            {
                self.click_id(select.option_id(index));
                return;
            }
            let rect = (0..PRODUCTS.len())
                .find_map(|row| self.app.shell.hits.area_of(select.option_id(row)))
                .expect("popup rows");
            Application::handle(
                &mut self.app,
                Input::Mouse(Mouse::new(
                    MouseKind::WheelDown,
                    Position::new(rect.x + 1, rect.y),
                )),
            );
            self.shown();
        }
        panic!("provider row not reachable by mouse");
    }

    fn choose(&mut self, field: &str, index: usize) {
        self.focus(WidgetId::of("accounts.usage_connection").sub(field));
        self.key(KeyCode::Enter);
        self.key(KeyCode::Home);
        for _ in 0..index {
            self.key(KeyCode::Down);
        }
        self.key(KeyCode::Enter);
    }

    fn next(&self) -> (Ticket, UsageAction) {
        let mut requests = self.backend.pending.lock().unwrap();
        assert!(!requests.is_empty(), "no request");
        requests.remove(0)
    }

    fn deliver(&mut self, ticket: Ticket, update: UsageUpdate) {
        self.app.deliver(&Msg::UsageConnection { ticket, update });
        self.shown();
    }

    fn capture(&mut self, name: &str) {
        let Some(directory) = std::env::var_os("JACKIN_UI_EVIDENCE_DIR") else {
            return;
        };
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let name = format!("{name}-{}x{}", self.size.0, self.size.1);
        std::fs::write(directory.join(format!("{name}.txt")), self.shown()).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(self.size.0, self.size.1)).unwrap();
        terminal
            .draw(|frame| Application::render(&mut self.app, frame))
            .unwrap();
        evidence::save(
            terminal.backend().buffer(),
            &directory.join(format!("{name}.png")),
            &name,
        );
    }
}

fn entry(provider: &str, label: &str) -> UsageEntry {
    UsageEntry {
        id: format!("fixture-{provider}"),
        provider: provider.into(),
        label: label.into(),
        details: vec!["Session · 65% remaining · resets in 58m".into()],
        resets: vec![],
        hub: None,
        region: None,
        device_login: false,
    }
}

#[test]
fn every_provider_is_choosable_by_keyboard_and_mouse_at_both_sizes() {
    for size in [(140, 40), (80, 24)] {
        for mouse in [false, true] {
            for (index, (provider, label)) in PRODUCTS.into_iter().enumerate() {
                let mut flow = Flow::new(size);
                if mouse {
                    if size.0 < 100 {
                        flow.key(KeyCode::Tab);
                    }
                    flow.click("Add usage account…");
                } else {
                    flow.key(KeyCode::Char('n'));
                }
                if mouse {
                    flow.choose_mouse(index);
                } else {
                    flow.choose("provider", index);
                }
                let shown = flow.shown();
                assert!(shown.contains(label), "{shown}");
                assert_eq!(
                    shown.contains("Sign-in method"),
                    matches!(provider, "kimi" | "minimax"),
                    "{provider}\n{shown}"
                );
                assert_eq!(
                    shown.contains("MiMo product / cluster"),
                    provider == "mimo",
                    "{shown}"
                );
                if !mouse {
                    flow.capture(&format!("choose-{provider}"));
                }
                if mouse {
                    flow.click("Continue");
                } else {
                    flow.action("save");
                }
                let (ticket, action) = flow.next();
                assert!(
                    matches!(action, UsageAction::Begin(choice) if choice.provider == provider),
                    "{provider}"
                );
                let (prompt, expected) = match provider {
                    "claude" | "codex" => (
                        UsageUpdate::Browser {
                            url: "https://example.test/sign-in".into(),
                            accepts_paste: true,
                        },
                        "Browser sign-in",
                    ),
                    "grok" | "muse" | "opencode" => (
                        UsageUpdate::Device {
                            url: "https://example.test/device".into(),
                            code: "FIXT-1234".into(),
                        },
                        "Device sign-in",
                    ),
                    "gemini" => (
                        UsageUpdate::Field {
                            label: "Desktop OAuth client ID".into(),
                            optional: false,
                            masked: false,
                        },
                        "Desktop OAuth client ID",
                    ),
                    _ => (
                        UsageUpdate::Key {
                            url: format!("https://example.test/{provider}/keys"),
                        },
                        "API key",
                    ),
                };
                flow.deliver(ticket, prompt);
                let shown = flow.shown();
                assert!(shown.contains(expected), "{shown}");
                if !mouse {
                    flow.capture(&format!("prompt-{provider}"));
                }
                flow.action("back");
                assert!(matches!(flow.next().1, UsageAction::Cancel));
                assert!(flow.shown().contains(label), "Back lost {provider}");
                flow.key(KeyCode::Esc);
                assert!(matches!(flow.next().1, UsageAction::Cancel));
                assert!(!flow.shown().contains("Continue"));
            }
        }
    }
}

#[test]
fn prompt_modes_mask_credentials_and_cancel_without_accepting_late_replies() {
    for size in [(140, 40), (80, 24)] {
        for (name, update, expected) in [
            (
                "browser-paste",
                UsageUpdate::Browser {
                    url: "https://example.test/oauth".into(),
                    accepts_paste: true,
                },
                "authorization response",
            ),
            (
                "browser-wait",
                UsageUpdate::Browser {
                    url: "https://example.test/oauth".into(),
                    accepts_paste: false,
                },
                "Finish signing in",
            ),
            (
                "device",
                UsageUpdate::Device {
                    url: "https://example.test/device".into(),
                    code: "FIXT-1234".into(),
                },
                "FIXT-1234",
            ),
            (
                "field-optional",
                UsageUpdate::Field {
                    label: "Workspace token".into(),
                    optional: true,
                    masked: true,
                },
                "optional",
            ),
            (
                "workspaces",
                UsageUpdate::Workspaces(vec![
                    "Fixture workspace one".into(),
                    "Fixture workspace two".into(),
                ]),
                "Workspace",
            ),
            (
                "error",
                UsageUpdate::Failed("Fixture connection refused".into()),
                "Fixture connection refused",
            ),
        ] {
            let mut flow = Flow::new(size);
            flow.key(KeyCode::Char('n'));
            flow.action("save");
            let (ticket, _) = flow.next();
            flow.deliver(ticket, update);
            assert!(flow.shown().contains(expected), "{}", flow.shown());
            flow.capture(name);
            flow.key(KeyCode::Esc);
            assert!(matches!(flow.next().1, UsageAction::Cancel));
            flow.deliver(ticket, UsageUpdate::Saved(entry("codex", "Late fixture")));
            assert!(!flow.shown().contains("Late fixture"));
        }
    }
}

fn board(rows: usize) -> UsageBoard {
    UsageBoard {
        priority: false,
        summary: "All Usage".into(),
        notices: vec![],
        attention: vec![],
        empty: false,
        alerts: vec![],
        rows: (0..rows)
            .map(|index| BoardMetric {
                account: Some(format!("fixture-{index}")),
                members: vec![],
                member_preferences: vec![jackin_console::usage_connections::MetricPreference {
                    account: format!("fixture-{index}"),
                    hidden: false,
                    pinned: false,
                }],
                provider: "codex".into(),
                metric: format!("session-{index}"),
                title: format!("Fixture account {index:02} · Session"),
                value: format!("{}% remaining", 65 + index),
                group: "ChatGPT / Codex".into(),
                bar: None,
                severity: junie_tui::theme::Tone::Normal,
                reset: Some("resets in 58m".into()),
                details: vec![format!("Detail marker {index:02}")],
                pinned: false,
            })
            .collect(),
        share: "{}".into(),
        settings: BoardSettings {
            flags: vec![("remaining", "Remaining", true)],
            warning: 35,
            critical: 10,
            hidden: vec![],
            proxy: false,
        },
    }
}

#[test]
fn all_usage_first_and_last_readings_are_visible_and_refresh_preserves_focus() {
    for size in [(140, 40), (80, 24)] {
        let mut flow = Flow::new(size);
        flow.app.go(Go::AllUsage);
        flow.shown();
        let (ticket, action) = flow.next();
        assert!(matches!(
            action,
            UsageAction::Board {
                priority: false,
                edit: BoardEdit::Read
            }
        ));
        flow.deliver(ticket, UsageUpdate::Board(Box::new(board(31))));
        let shown = flow.shown();
        assert!(
            shown.contains("65% remaining") && shown.contains("66% remaining"),
            "{shown}"
        );
        assert!(!shown.lines().any(|line| line.trim() == "Priority"));
        flow.capture("all-usage-first");
        flow.key(KeyCode::End);
        assert!(flow.shown().contains("95% remaining"), "{}", flow.shown());
        flow.capture("all-usage-last");
        flow.focus(WidgetId::of("usage.connected_board").sub("save"));
        let focused = flow.app.shell.focus.current();
        flow.deliver(ticket, UsageUpdate::Board(Box::new(board(31))));
        assert_eq!(flow.app.shell.focus.current(), focused);
        flow.key(KeyCode::Enter);
        assert!(
            flow.shown().contains("Detail marker 30"),
            "{}",
            flow.shown()
        );
        flow.focus(WidgetId::of("usage.connected_board").sub("pin"));
        assert_eq!(flow.shown().matches('▎').count(), 1, "{}", flow.shown());
        flow.capture("last-reading-details");
        flow.board_action("save");
        assert!(flow.shown().contains("95% remaining"), "{}", flow.shown());
        flow.capture("last-reading-back");
        flow.key(KeyCode::Home);
        assert!(flow.shown().contains("65% remaining"), "{}", flow.shown());
        flow.board_action("reload");
        assert!(matches!(
            flow.next().1,
            UsageAction::Board {
                edit: BoardEdit::Refresh(None),
                ..
            }
        ));
    }
}

#[test]
fn inventory_is_visible_loading_failure_and_multiple_provider_labels_are_distinct() {
    for size in [(140, 40), (80, 24)] {
        let mut flow = Flow::new(size);
        flow.app.go(Go::Accounts { select: None });
        let (ticket, _) = flow.next();
        flow.deliver(
            ticket,
            UsageUpdate::Accounts(vec![
                entry("codex", "Fixture work"),
                entry("claude", "Fixture work"),
            ]),
        );
        let shown = flow.shown();
        assert!(
            shown.contains("Fixture work") && shown.contains("Usage accounts"),
            "{shown}"
        );
        flow.capture("accounts-inventory");
        flow.key(KeyCode::Down);
        flow.key(KeyCode::Down);
        flow.key(KeyCode::Enter);
        assert!(flow.shown().contains("65% remaining"), "{}", flow.shown());
        flow.capture("account-details");
        flow.action("refresh");
        assert!(matches!(flow.next().1, UsageAction::Refresh(id) if id == "fixture-codex"));
    }
}

#[path = "evidence.rs"]
mod evidence;

#[test]
fn regional_and_device_choices_are_sent_for_the_selected_provider() {
    for size in [(140, 40), (80, 24)] {
        for provider in ["kimi", "minimax"] {
            for region in [0, 1] {
                for device in [0, 1] {
                    let mut flow = Flow::new(size);
                    flow.key(KeyCode::Char('n'));
                    flow.choose(
                        "provider",
                        PRODUCTS.iter().position(|(id, _)| *id == provider).unwrap(),
                    );
                    flow.choose("region", region);
                    flow.choose("auth", device);
                    flow.capture(&format!("{provider}-region-{region}-method-{device}"));
                    flow.action("save");
                    assert!(
                        matches!(flow.next().1, UsageAction::Begin(choice) if choice.provider == provider && choice.region.as_deref() == Some(if region == 0 { "global" } else { "cn" }) && choice.device_login == Some(device == 1))
                    );
                }
            }
        }
        for (region, expected) in [None, Some("cn"), Some("sgp"), Some("ams")]
            .into_iter()
            .enumerate()
        {
            let mut flow = Flow::new(size);
            flow.key(KeyCode::Char('n'));
            flow.choose("provider", 7);
            flow.choose("mimo_region", region);
            flow.action("save");
            assert!(
                matches!(flow.next().1, UsageAction::Begin(choice) if choice.provider == "mimo" && choice.region.as_deref() == expected)
            );
        }
    }
}

#[test]
fn saved_usage_appears_in_inventory_and_account_details_scroll_and_confirm_safely() {
    use jackin_console::usage_connections::ResetItem;
    for size in [(140, 40), (80, 24)] {
        let mut flow = Flow::new(size);
        flow.key(KeyCode::Char('n'));
        flow.choose("provider", 5);
        flow.action("save");
        let (ticket, _) = flow.next();
        flow.deliver(
            ticket,
            UsageUpdate::Key {
                url: "https://example.test/fixture-keys".into(),
            },
        );
        flow.focus(WidgetId::of("accounts.usage_connection").sub("input"));
        flow.key(KeyCode::Enter);
        for character in "fixture-token-only".chars() {
            flow.key(KeyCode::Char(character));
        }
        flow.key(KeyCode::Enter);
        assert!(!flow.shown().contains("fixture-token-only"));
        flow.capture("masked-api-key");
        flow.action("save");
        assert!(
            matches!(flow.next().1, UsageAction::Submit(key) if key.expose() == "fixture-token-only")
        );
        let mut account = entry("zai", "Fixture saved account");
        account.details = (0..31)
            .map(|index| format!("Reading {index:02}: 65% remaining · resets in 58m"))
            .collect();
        account.resets = vec![ResetItem { index: 0, identity: "fixture-reset".into(), description: "Weekly reset expires 2026-12-31 23:59 UTC and immediately refills the weekly limit for this fixture account".into(), usable: true }];
        flow.deliver(ticket, UsageUpdate::Saved(account.clone()));
        let (inventory, request) = flow.next();
        assert_eq!(request, UsageAction::List);
        flow.deliver(inventory, UsageUpdate::Accounts(vec![account.clone()]));
        assert!(flow.shown().contains("Reading 00"));
        flow.capture("long-account-first");
        for _ in 0..12 {
            flow.key(KeyCode::PageDown);
        }
        assert!(flow.shown().contains("Reading 30"), "{}", flow.shown());
        assert!(flow.shown().contains("PgUp/PgDn"));
        flow.capture("long-account-last");
        flow.action("resets");
        flow.action("save");
        let shown = flow
            .shown()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(shown.contains("2026-12-31 23:59 UTC"), "{shown}");
        flow.capture("reset-confirmation");
        flow.key(KeyCode::Esc);
        assert!(matches!(flow.next().1, UsageAction::Cancel));
        assert!(flow.backend.pending.lock().unwrap().is_empty());
        assert!(flow.shown().contains("Fixture saved account"));
    }
}

#[test]
fn inventory_loading_and_failure_do_not_invent_a_zero_usage_count() {
    for size in [(140, 40), (80, 24)] {
        let (app, backend) = accounts_with_usage();
        let requests = std::mem::take(&mut *backend.pending.lock().unwrap());
        let ticket = requests.last().unwrap().0;
        let mut flow = Flow { app, backend, size };
        assert!(flow.shown().contains("Usage loading"), "{}", flow.shown());
        flow.capture("accounts-loading");
        flow.deliver(
            ticket,
            UsageUpdate::Failed("Fixture inventory unavailable".into()),
        );
        assert!(
            flow.shown().contains("Usage unavailable"),
            "{}",
            flow.shown()
        );
        flow.capture("accounts-unavailable");
        flow.key(KeyCode::Down);
        flow.key(KeyCode::Enter);
        let (ticket, action) = flow.next();
        assert_eq!(action, UsageAction::List);
        flow.deliver(
            ticket,
            UsageUpdate::Accounts(vec![entry("codex", "Recovered fixture")]),
        );
        flow.key(KeyCode::Esc);
        assert!(matches!(flow.next().1, UsageAction::Cancel));
        assert!(flow.shown().contains("Recovered fixture"));
    }
}

#[test]
fn gemini_fields_optional_skips_browser_failure_and_workspace_selection_are_interactive() {
    for size in [(140, 40), (80, 24)] {
        let mut flow = Flow::new(size);
        flow.key(KeyCode::Char('n'));
        flow.choose("provider", 8);
        flow.action("save");
        let (ticket, _) = flow.next();
        flow.deliver(
            ticket,
            UsageUpdate::Field {
                label: "Desktop OAuth client ID".into(),
                optional: false,
                masked: false,
            },
        );
        flow.focus(WidgetId::of("accounts.usage_connection").sub("input"));
        flow.key(KeyCode::Enter);
        for character in "fixture-client-id".chars() {
            flow.key(KeyCode::Char(character));
        }
        flow.key(KeyCode::Enter);
        flow.action("save");
        assert!(
            matches!(flow.next().1, UsageAction::Submit(value) if value.expose() == "fixture-client-id")
        );
        for (label, masked) in [("Client secret", true), ("Google Cloud project", false)] {
            flow.deliver(
                ticket,
                UsageUpdate::Field {
                    label: label.into(),
                    optional: true,
                    masked,
                },
            );
            flow.capture(if masked {
                "gemini-optional-secret"
            } else {
                "gemini-optional-project"
            });
            flow.action("save");
            assert!(matches!(flow.next().1, UsageAction::Skip));
        }
        flow.deliver(
            ticket,
            UsageUpdate::Browser {
                url: "https://example.test/gemini-oauth".into(),
                accepts_paste: false,
            },
        );
        flow.action("open");
        assert!(matches!(flow.next().1, UsageAction::OpenBrowser));
        flow.deliver(ticket, UsageUpdate::OpenFailed);
        assert!(flow.shown().contains("Could not open the browser"));
        flow.capture("gemini-browser-fallback");
        flow.key(KeyCode::Esc);
        assert!(matches!(flow.next().1, UsageAction::Cancel));

        flow.key(KeyCode::Char('n'));
        flow.action("save");
        let (ticket, _) = flow.next();
        flow.deliver(
            ticket,
            UsageUpdate::Workspaces(vec![
                "Fixture workspace one".into(),
                "Fixture workspace two".into(),
            ]),
        );
        flow.choose("workspace", 1);
        flow.action("save");
        assert!(matches!(flow.next().1, UsageAction::ChooseWorkspace(1)));
    }
}

#[test]
fn empty_priority_offers_all_usage_and_stale_scope_replies_do_not_steal_it() {
    for size in [(140, 40), (80, 24)] {
        let mut flow = Flow::new(size);
        flow.app.go(Go::Usage { select: None });
        flow.shown();
        let (priority_ticket, action) = flow.next();
        assert!(matches!(
            action,
            UsageAction::Board {
                priority: true,
                edit: BoardEdit::DefaultTab
            }
        ));
        assert!(flow.shown().contains("All Usage"));
        let mut priority = board(0);
        priority.priority = true;
        priority.summary = "No priority readings pinned".into();
        flow.deliver(
            priority_ticket,
            UsageUpdate::Board(Box::new(priority.clone())),
        );
        flow.capture("empty-priority");
        flow.board_action("save");
        let (all_ticket, action) = flow.next();
        assert!(matches!(
            action,
            UsageAction::Board {
                priority: false,
                edit: BoardEdit::Read
            }
        ));
        assert_ne!(priority_ticket, all_ticket);
        flow.deliver(priority_ticket, UsageUpdate::Board(Box::new(priority)));
        assert!(!flow.shown().contains("No priority readings pinned"));
        flow.deliver(all_ticket, UsageUpdate::Board(Box::new(board(3))));
        assert!(flow.shown().contains("65% remaining") && flow.shown().contains("67% remaining"));
    }
}

#[test]
fn reset_redemption_requires_a_usable_item_and_an_explicit_confirmation() {
    use jackin_console::usage_connections::ResetItem;
    for size in [(140, 40), (80, 24)] {
        for usable in [false, true] {
            let mut flow = Flow::new(size);
            flow.key(KeyCode::Char('N'));
            let (ticket, action) = flow.next();
            assert_eq!(action, UsageAction::List);
            let mut account = entry("zai", "Fixture reset account");
            account.resets = vec![ResetItem {
                index: 7,
                identity: "fixture-reset-identity".into(),
                description: "Fixture weekly reset expires 2026-12-31 23:59 UTC".into(),
                usable,
            }];
            flow.deliver(ticket, UsageUpdate::Accounts(vec![account]));
            flow.action("save");
            flow.action("resets");
            flow.action("save");
            assert!(flow.backend.pending.lock().unwrap().is_empty());
            if usable {
                assert!(flow.shown().contains("Confirm redemption"));
                flow.action("save");
                assert!(
                    matches!(flow.next().1, UsageAction::Redeem { account, index: 7, identity } if account == "fixture-zai" && identity == "fixture-reset-identity")
                );
            } else {
                assert!(flow.shown().contains("cannot currently be redeemed"));
                assert!(!flow.shown().contains("Confirm redemption"));
            }
        }
    }
}

#[test]
fn usage_row_refresh_shortcut_never_refreshes_execution_bindings() {
    for size in [(140, 40), (80, 24)] {
        let mut flow = Flow::new(size);
        let execution = flow.app.world.accounts.clone();
        flow.key(KeyCode::Char('N'));
        let (ticket, _) = flow.next();
        flow.deliver(
            ticket,
            UsageUpdate::Accounts(vec![entry("codex", "Fixture usage account")]),
        );
        flow.key(KeyCode::Esc);
        assert!(matches!(flow.next().1, UsageAction::Cancel));
        flow.key(KeyCode::Down);
        flow.key(KeyCode::Down);
        flow.key(KeyCode::Char('r'));
        assert!(matches!(flow.next().1, UsageAction::Refresh(id) if id == "fixture-codex"));
        assert_eq!(flow.app.world.accounts, execution);
    }
}
