#![cfg(feature = "demo")]
//! The demo's scripted session player, reached only through `PaneBody`.

use jackin_console::domain::agent::Agent;
use jackin_console::domain::instance::{
    AgentState, DaemonSnapshot, Instance, InstanceId, InstanceStatus,
};
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::fixtures::{Scenario, world_for};
use jackin_console::panes::{PaneBody, PaneKey, PaneKindTag, PaneRequest, PaneSpawn, PasteOutcome};
use jackin_console::sim::SimBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use termrock::core::event::Key;
use termrock::widgets::viewport::CellPos;

const WORKSPACE: &str = "payments-platform";
const AREA: Rect = Rect::new(0, 0, 80, 24);

fn spawn(agent: Option<Agent>, start_ms: i64) -> Box<dyn PaneBody> {
    let request = PaneRequest::new(&InstanceId::simulated("jk-7f3a"), agent, None);
    let mut body = SimBackend::shared().spawn_pane(
        &request,
        PaneSpawn {
            id: 9,
            workspace: WORKSPACE.into(),
            now_ms: start_ms,
        },
    );
    body.sync_size(AREA);
    body
}

fn rows(body: &mut dyn PaneBody) -> usize {
    body.sync_size(AREA);
    body.viewport().scroll().content_len
}

fn press(body: &mut dyn PaneBody, code: KeyCode, now_ms: i64) -> PaneKey {
    body.key(
        &Key {
            code,
            mods: KeyModifiers::NONE,
        },
        now_ms,
    )
}

#[test]
fn a_codex_body_plays_its_script_and_answers_a_prompt() {
    let mut body = spawn(Some(Agent::Codex), 1_000);
    assert_eq!(body.kind(), PaneKindTag::Script);
    assert_eq!(body.state(), AgentState::Working);
    assert_eq!(
        rows(body.as_mut()),
        0,
        "nothing is printed before the first tick"
    );

    for (at, changed, lines) in [(1_000, true, 1), (1_500, true, 3), (3_000, true, 6)] {
        assert_eq!(body.tick(at), changed, "t={at}");
        assert_eq!(rows(body.as_mut()), lines, "t={at}");
        assert_eq!(body.state(), AgentState::Working, "t={at}");
    }
    assert!(body.tick(10_000));
    assert_eq!((rows(body.as_mut()), body.state()), (18, AgentState::Done));
    assert!(
        !body.tick(100_000),
        "a finished script has nothing to print"
    );

    assert_eq!(
        press(body.as_mut(), KeyCode::Char('h'), 1),
        PaneKey::Forwarded
    );
    assert_eq!(
        press(body.as_mut(), KeyCode::Char('i'), 1),
        PaneKey::Forwarded
    );
    assert_eq!(
        body.viewport().caret(),
        Some(CellPos { line: 17, col: 4 }),
        "the caret follows the typed text after the prompt"
    );
    assert_eq!(
        press(body.as_mut(), KeyCode::Enter, 2_000_000),
        PaneKey::Forwarded
    );
    assert_eq!(
        (rows(body.as_mut()), body.state()),
        (19, AgentState::Working)
    );
    assert!(body.tick(3_000_000));
    assert_eq!((rows(body.as_mut()), body.state()), (23, AgentState::Done));
}

#[test]
fn a_claude_body_blocks_and_records_the_files_it_touched() {
    let mut body = spawn(Some(Agent::ClaudeCode), 1_000);
    for at in [1_000, 1_500, 3_000, 10_000] {
        body.tick(at);
    }
    assert_eq!(rows(body.as_mut()), 24);
    assert_eq!(body.state(), AgentState::Working);
    body.tick(100_000);
    assert_eq!(
        (rows(body.as_mut()), body.state()),
        (30, AgentState::Blocked)
    );
    assert_eq!(
        body.touched(),
        ["src/settlement/retry.rs", "src/settlement/mod.rs"]
    );
    press(body.as_mut(), KeyCode::Char('h'), 1);
    press(body.as_mut(), KeyCode::Char('i'), 1);
    press(body.as_mut(), KeyCode::Enter, 200_000);
    assert_eq!(body.state(), AgentState::Idle);
}

#[test]
fn a_shell_body_prompts_with_the_workspace_name() {
    let mut body = spawn(None, 1_000);
    assert_eq!(body.state(), AgentState::Unknown);
    for at in [1_000, 1_500, 3_000, 10_000] {
        body.tick(at);
    }
    assert_eq!(rows(body.as_mut()), 12);
    assert_eq!(body.paste("hi", 10_001), PasteOutcome::Sent);
    assert_eq!(
        body.viewport().caret(),
        Some(CellPos { line: 11, col: 22 }),
        "`payments-platform ❯ ` is 20 columns wide"
    );
    body.clear();
    assert_eq!(rows(body.as_mut()), 1, "clearing leaves the prompt line");
    assert!(body.viewport().follow());
}

#[test]
fn a_script_body_keeps_the_design_defaults() {
    let mut body = spawn(Some(Agent::Codex), 1_000);
    body.tick(10_000);
    assert_eq!(body.title(), None);
    assert!(body.note(&Vocabulary::openshell().pane).is_none());
    assert!(!body.sends_prefix());
    assert!(body.close_needs_confirm());
    body.focus_changed(false, 5);
    body.focus_changed(true, 6);
    body.redraw();
    assert_eq!((rows(body.as_mut()), body.state()), (18, AgentState::Done));
    assert_eq!(press(body.as_mut(), KeyCode::F(5), 7), PaneKey::Consumed);
    assert_eq!(press(body.as_mut(), KeyCode::Tab, 7), PaneKey::Consumed);
    assert!(body.viewport().keyboard_copy());
}

#[test]
fn the_demo_daemons_hold_script_bodies() {
    let world = world_for(Scenario::CapsuleMulti);
    let daemon = world
        .daemons
        .get(&InstanceId::simulated("jk-7f3a"))
        .expect("the multi-pane daemon");
    let seen: Vec<_> = daemon
        .panes
        .iter()
        .map(|p| {
            (
                p.id,
                p.label.as_str(),
                p.agent,
                p.body.state(),
                p.body.kind(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            (
                1,
                "Claude Code",
                Some(Agent::ClaudeCode),
                AgentState::Blocked,
                PaneKindTag::Script
            ),
            (
                2,
                "Codex",
                Some(Agent::Codex),
                AgentState::Done,
                PaneKindTag::Script
            ),
            (3, "Shell", None, AgentState::Unknown, PaneKindTag::Script),
            (4, "Shell", None, AgentState::Unknown, PaneKindTag::Script),
            (
                5,
                "Amp",
                Some(Agent::Amp),
                AgentState::Blocked,
                PaneKindTag::Script
            ),
        ]
    );
    assert_eq!(
        daemon.touched_files(),
        ["src/settlement/mod.rs", "src/settlement/retry.rs"]
    );
    assert_eq!(
        daemon.pane(1).and_then(|p| p.account.as_deref()),
        Some("acct-claude-work")
    );
    let labels: Vec<_> = daemon
        .tabs
        .iter()
        .map(|t| daemon.tab_label(t, &|_| None))
        .collect();
    assert_eq!(labels, ["Mix (3)", "Shell", "docs"]);
}

#[test]
fn the_sim_backend_proposes_the_instance_agent_and_account_as_the_first_pane() {
    let world = world_for(Scenario::CapsuleMulti);
    let instance: &Instance = world
        .instance(&InstanceId::simulated("jk-7f3a"))
        .expect("the running instance");
    assert_eq!(instance.status, InstanceStatus::Running);
    assert!(matches!(instance.daemon, DaemonSnapshot::Tabs(_)));
    assert_eq!(
        SimBackend::shared().initial_panes(instance),
        [PaneRequest::new(
            &instance.id,
            Some(instance.agent),
            Some("acct-claude-work".into())
        )]
    );
}

#[test]
fn the_new_agent_scripts_reach_a_prompt_and_leave_working_state() {
    for agent in [
        Agent::MuseCode,
        Agent::Antigravity,
        Agent::MinimaxCode,
        Agent::MimoCode,
        Agent::Omp,
    ] {
        let mut body = spawn(Some(agent), 1_000);
        assert_eq!(body.state(), AgentState::Working);
        assert!(body.viewport().caret().is_none(), "{agent:?}");
        assert!(body.tick(1_000));
        assert_eq!(body.state(), AgentState::Done, "{agent:?}");
        let prompt = body.viewport().caret().expect("the Await prompt");
        assert_eq!(
            press(body.as_mut(), KeyCode::Char('h'), 1_001),
            PaneKey::Forwarded
        );
        assert_eq!(
            body.viewport().caret().map(|caret| caret.col),
            Some(prompt.col + 1),
            "{agent:?} accepts input at Await"
        );
        assert_eq!(
            press(body.as_mut(), KeyCode::Enter, 1_002),
            PaneKey::Forwarded
        );
        assert_eq!(body.state(), AgentState::Working, "{agent:?}");
        assert!(rows(body.as_mut()) > 1, "{agent:?}");
    }
}

fn text_of(body: &mut dyn PaneBody) -> String {
    use ratatui::buffer::Buffer;
    use termrock::core::focus::FocusRing;
    use termrock::core::hit::HitRegistry;
    use termrock::theme::{ColorLevel, Theme};
    use termrock::ui::ctx::{Interaction, RenderCtx};

    body.sync_size(AREA);
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(AREA);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    body.viewport()
        .draw(AREA, &mut buf, &mut ctx, theme.canvas, false);
    (0..AREA.height)
        .map(|y| {
            (0..AREA.width)
                .map(|x| buf[(x, y)].symbol().to_owned())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_shell_body_answers_pwd_with_the_path_of_its_workspace() {
    let mut body = spawn(None, 1_000);
    for at in [1_000, 1_500, 3_000, 10_000] {
        body.tick(at);
    }
    for c in "pwd".chars() {
        press(body.as_mut(), KeyCode::Char(c), 10_100);
    }
    press(body.as_mut(), KeyCode::Enter, 10_200);
    body.tick(10_300);

    let shown = text_of(body.as_mut());
    assert!(
        shown.contains("/workspace/payments-platform"),
        "the reply names the workspace of the body:\n{shown}"
    );
}
