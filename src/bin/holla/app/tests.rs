use super::*;
use crate::screens::finder::FinderPage;
use junie_tui::core::event::Input;

const ID: &str = "docker-cleanup";

fn app() -> App {
    let mut app = App::launch(
        Scenario::DockerCleanup,
        Motion::Full,
        0,
        Theme::junie(),
        false,
    );
    app.world.clock.running = true;
    app
}

fn key(code: KeyCode, mods: KeyModifiers) -> Input {
    Input::Key(Key { code, mods })
}

fn tick(app: &mut App, n: usize) {
    for _ in 0..n {
        app.handle_input(Input::Tick);
    }
}

fn arm(app: &mut App, id: &str) {
    app.world.plan_mut(id).expect("plan").drift = None;
    app.pending_gate = Some(GateTarget::Plan(id.to_owned()));
    app.gate2_confirmed();
}

fn step(app: &App, id: &str, step: &str) -> StepState {
    app.world
        .plan(id)
        .expect("plan")
        .steps
        .iter()
        .find(|s| s.id == step)
        .expect("step")
        .state
}

fn tick_until_running(app: &mut App, id: &str, step_id: &str) {
    for _ in 0..400 {
        if step(app, id, step_id) == StepState::Running {
            return;
        }
        tick(app, 1);
    }
    panic!(
        "{step_id} never started: {:?} tick={} status={}",
        app.world
            .plan(id)
            .map(|p| p.steps.iter().map(|s| s.state).collect::<Vec<_>>()),
        app.world.tick,
        status(app)
    );
}

fn plan_tab(app: &App, id: &str) -> Option<usize> {
    app.tabs
        .iter()
        .position(|t| matches!(&t.kind, TabKind::Plan(p) if p == id))
}

fn confirm_close_dialog(app: &mut App) {
    assert!(!app.modals.is_empty(), "the close dialog is open");
    app.handle_input(key(KeyCode::Right, KeyModifiers::NONE));
    app.handle_input(key(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.modals.is_empty(), "the dialog resolved");
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}

fn live_steps(app: &App, id: &str) -> usize {
    app.world
        .plan(id)
        .expect("plan")
        .steps
        .iter()
        .filter(|s| {
            s.included
                && matches!(
                    s.state,
                    StepState::Ready | StepState::Waiting | StepState::Running
                )
        })
        .count()
}

#[test]
fn plan_stop_and_close_finalizes_the_running_plan() {
    let mut app = app();
    arm(&mut app, ID);
    tick_until_running(&mut app, ID, "rm");
    let tab = plan_tab(&app, ID).expect("plan tab open");
    app.activate_tab(tab);

    app.handle_input(key(KeyCode::Char('w'), KeyModifiers::CONTROL));
    confirm_close_dialog(&mut app);

    let plan = app.world.plan(ID).expect("plan");
    assert_eq!(plan.phase, PlanPhase::Done);
    assert!(plan.ended_tick.is_some());
    assert_eq!(
        live_steps(&app, ID),
        0,
        "nothing stays running in a Done plan"
    );
    assert_eq!(step(&app, ID, "rm"), StepState::Cancelled);
    assert!(plan_tab(&app, ID).is_none(), "the tab closed");
    assert!(
        status(&app).starts_with("Clean Docker completely stopped · "),
        "{}",
        status(&app)
    );
    assert!(status(&app).contains("cancelled"), "{}", status(&app));

    let snapshot = app.world.plan(ID).cloned();
    tick(&mut app, 120);
    assert_eq!(
        app.world.plan(ID).cloned(),
        snapshot,
        "ticks change nothing"
    );
}

#[test]
fn plan_stop_and_close_uses_captured_plan_id() {
    let mut app = app();
    let mut other = app.world.plan(ID).cloned().expect("plan");
    other.id = "other-plan".into();
    other.title = "Other plan".into();
    app.world.plans.push(other);
    arm(&mut app, ID);
    arm(&mut app, "other-plan");
    tick_until_running(&mut app, ID, "stop");
    tick_until_running(&mut app, "other-plan", "stop");

    let first = plan_tab(&app, ID).expect("first tab");
    app.activate_tab(first);
    app.handle_input(key(KeyCode::Char('w'), KeyModifiers::CONTROL));
    let second = plan_tab(&app, "other-plan").expect("second tab");
    app.activate_tab(second);
    app.shell
        .focus
        .set(Some(WidgetId::of("close-tab").sub("ok")));
    app.handle_input(key(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.modals.is_empty(), "the dialog resolved");

    assert_eq!(app.world.plan(ID).expect("plan").phase, PlanPhase::Done);
    assert!(plan_tab(&app, ID).is_none());
    let other = app.world.plan("other-plan").expect("other plan");
    assert_eq!(other.phase, PlanPhase::Running, "the other plan is intact");
    assert_eq!(step(&app, "other-plan", "stop"), StepState::Running);
    assert!(plan_tab(&app, "other-plan").is_some());
}

#[test]
fn plan_stop_dialog_dismissal_keeps_the_plan_running() {
    let mut app = app();
    arm(&mut app, ID);
    tick_until_running(&mut app, ID, "rm");
    let tab = plan_tab(&app, ID).expect("plan tab open");
    app.activate_tab(tab);

    app.handle_input(key(KeyCode::Char('w'), KeyModifiers::CONTROL));
    assert!(!app.modals.is_empty());
    app.handle_input(key(KeyCode::Enter, KeyModifiers::NONE));

    assert!(app.modals.is_empty());
    assert_eq!(app.world.plan(ID).expect("plan").phase, PlanPhase::Running);
    assert_eq!(step(&app, ID, "rm"), StepState::Running);
    assert!(plan_tab(&app, ID).is_some());
}

#[test]
fn disk_page_stops_animating_on_the_tick_that_publishes_the_cache() {
    let mut app = App::launch(
        Scenario::DiskCleanup,
        Motion::Full,
        0,
        Theme::junie(),
        false,
    );
    app.world.clock.running = true;
    app.world.disk.scan_ticks = 1;
    app.world.fs.dir("/work/big", 1);
    for i in 0..4096 {
        app.world.fs.file(&format!("/work/big/f{i:05}"), 4096, 1);
    }
    app.world.start_scan("/work/big");
    app.push_page(Page::Disk {
        path: "/work/big".into(),
    });

    tick(&mut app, 1);
    assert!(app.top().expect("page").animating(&app.world));
    assert_eq!(
        app.world.persisted.sizes, None,
        "cache waits for the last entry"
    );

    tick(&mut app, 1);
    assert!(app.world.scan.as_ref().expect("scan").completed());
    assert!(app.world.persisted.sizes.is_some());
    assert!(!app.top().expect("page").animating(&app.world));
}

const FINAL_NAME: &str = "zz-final-hit.txt";
const FINAL_QUERY: &str = "zz-final-hit";

fn files_app() -> App {
    let mut app = App::launch(
        Scenario::ParityFiles,
        Motion::Full,
        0,
        Theme::junie(),
        false,
    );
    app.world.clock.running = true;
    app
}

fn set_index_source(app: &mut App, done_at: u64, failure: Option<&str>) {
    let index = app
        .world
        .sources
        .iter_mut()
        .find(|s| s.name == "index")
        .expect("index source");
    index.done_at = done_at;
    index.failure = failure.map(str::to_owned);
}

fn add_final_file(app: &mut App) -> String {
    let path = format!("{}/{FINAL_NAME}", app.world.location.home);
    app.world.fs.text(&path, "final\n", 1);
    path
}

fn open_find(app: &mut App) {
    app.activate_tab(0);
    app.push_page(Page::Find);
}

fn type_query(app: &mut App, query: &str) {
    for c in query.chars() {
        app.handle_input(key(KeyCode::Char(c), KeyModifiers::NONE));
    }
}

fn find_page(app: &mut App) -> &mut crate::screens::files::FilesPage {
    let active = app.active();
    app.router
        .top_of_mut(active)
        .and_then(|s| s.as_files())
        .expect("files page on top")
}

fn hits(app: &mut App) -> Vec<String> {
    find_page(app).hit_paths()
}

fn published(app: &mut App) -> Option<(usize, bool)> {
    find_page(app).published_index()
}

fn selected(app: &mut App) -> Option<String> {
    find_page(app).selected_hit()
}

fn install_index(app: &mut App, index: Vec<String>) {
    find_page(app).install_index_for_test(index);
}

fn find_page_is_top(app: &mut App) -> bool {
    find_page(app).is_find_mode()
}

fn call_counts() -> (u64, u64) {
    crate::sim::fs::call_counts()
}

fn advance_world_only(app: &mut App, n: usize) {
    for _ in 0..n {
        drop(app.world.tick());
    }
}

#[test]
fn files_find_publishes_last_entry_on_completion_tick() {
    let mut app = files_app();
    set_index_source(&mut app, 10, None);
    let path = add_final_file(&mut app);
    open_find(&mut app);
    type_query(&mut app, FINAL_QUERY);

    tick(&mut app, 9);
    assert!(
        hits(&mut app).is_empty(),
        "the last entry is not revealed yet"
    );

    tick(&mut app, 1);
    assert_eq!(hits(&mut app), vec![path.clone()]);
    assert_eq!(selected(&mut app), Some(path));
    assert_eq!(published(&mut app).map(|(_, done)| done), Some(true));
}

#[test]
fn files_find_skipped_tick_still_publishes_completion() {
    let mut app = files_app();
    set_index_source(&mut app, 10, None);
    let path = add_final_file(&mut app);
    open_find(&mut app);
    type_query(&mut app, FINAL_QUERY);

    advance_world_only(&mut app, 10);
    tick(&mut app, 1);
    assert_eq!(hits(&mut app), vec![path]);
}

#[test]
fn files_find_terminal_progress_is_published_once() {
    let mut app = files_app();
    set_index_source(&mut app, 10, None);
    let path = add_final_file(&mut app);
    open_find(&mut app);
    type_query(&mut app, FINAL_QUERY);
    tick(&mut app, 12);
    let stamp = published(&mut app);
    let calls = call_counts();
    assert_eq!(hits(&mut app), vec![path.clone()]);

    tick(&mut app, 100);
    assert_eq!(published(&mut app), stamp, "terminal stamp is stable");
    assert_eq!(
        call_counts(),
        calls,
        "no find or index work after publication"
    );
    assert_eq!(hits(&mut app), vec![path]);
}

#[test]
fn files_find_failed_source_preserves_full_index_policy() {
    let mut app = files_app();
    set_index_source(&mut app, 5, Some("index unavailable"));
    let path = add_final_file(&mut app);
    open_find(&mut app);
    type_query(&mut app, FINAL_QUERY);

    tick(&mut app, 4);
    assert!(hits(&mut app).is_empty());

    tick(&mut app, 1);
    assert_eq!(hits(&mut app), vec![path]);
    assert_eq!(
        app.world.failed_sources(),
        vec![("index", "index unavailable")]
    );
}

#[test]
fn files_find_empty_terminal_is_published_once() {
    let mut app = files_app();
    set_index_source(&mut app, 3, None);
    let empty = format!("{}/empty-find", app.world.location.home);
    app.world.fs.dir(&empty, 1);
    app.world.location.home = empty;
    open_find(&mut app);
    type_query(&mut app, "anything");

    tick(&mut app, 3);
    let stamp = published(&mut app);
    assert_eq!(stamp, Some((0, true)));
    assert!(hits(&mut app).is_empty());
    let calls = call_counts();

    tick(&mut app, 100);
    assert_eq!(published(&mut app), stamp, "empty terminal stamp is stable");
    assert_eq!(call_counts(), calls, "empty terminal is not recomputed");
}

#[test]
fn files_find_empty_terminal_is_not_rescanned_on_reentry() {
    let mut app = files_app();
    set_index_source(&mut app, 1, None);
    let empty = format!("{}/empty-find", app.world.location.home);
    app.world.fs.dir(&empty, 1);
    app.world.location.home = empty.clone();
    open_find(&mut app);
    tick(&mut app, 2);
    assert_eq!(call_counts().0, 1, "the home index is built once on open");

    app.push_page(Page::Files {
        path: empty,
        query: None,
    });
    let before_reentry = call_counts().0;
    app.handle_input(key(KeyCode::Esc, KeyModifiers::NONE));
    assert!(find_page_is_top(&mut app), "the find page is back on top");
    assert_eq!(
        call_counts().0,
        before_reentry,
        "re-entry does not rebuild an empty index"
    );
}

#[test]
fn files_find_enter_after_completion_publishes_current_index() {
    let mut app = files_app();
    set_index_source(&mut app, 2, None);
    let path = add_final_file(&mut app);
    advance_world_only(&mut app, 5);

    open_find(&mut app);
    type_query(&mut app, FINAL_QUERY);
    assert_eq!(hits(&mut app), vec![path]);
}

#[test]
fn files_find_equal_length_index_replacement_republishes() {
    let mut app = files_app();
    set_index_source(&mut app, 1, None);
    open_find(&mut app);
    install_index(&mut app, vec!["/t/a-report.txt".into()]);
    type_query(&mut app, "report");
    tick(&mut app, 2);
    assert_eq!(hits(&mut app), vec!["/t/a-report.txt".to_owned()]);

    install_index(&mut app, vec!["/t/b-report.txt".into()]);
    tick(&mut app, 1);
    assert_eq!(hits(&mut app), vec!["/t/b-report.txt".to_owned()]);
}

#[test]
fn files_find_completion_preserves_selected_path() {
    let mut app = files_app();
    set_index_source(&mut app, 10, None);
    let early = format!("{}/aa-dir", app.world.location.home);
    app.world.fs.dir(&early, 1);
    let partial = format!("{early}/zz-final-hit-a.txt");
    app.world.fs.text(&partial, "partial\n", 1);
    let path = add_final_file(&mut app);
    open_find(&mut app);
    type_query(&mut app, FINAL_QUERY);

    tick(&mut app, 9);
    assert_eq!(hits(&mut app), vec![partial.clone()]);
    assert_eq!(selected(&mut app), Some(partial.clone()));

    tick(&mut app, 1);
    assert_eq!(hits(&mut app), vec![path, partial.clone()]);
    assert_eq!(selected(&mut app), Some(partial));
}

fn drawn_text(app: &mut App) -> String {
    let mut term =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).expect("test terminal");
    term.draw(|f| junie_tui::shell::render(app, f))
        .expect("test backend draws");
    let buf = term.backend().buffer();
    let area = buf.area;
    (area.y..area.y + area.height)
        .map(|y| {
            (area.x..area.x + area.width)
                .map(|x| buf[(x, y)].symbol().to_owned())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn files_find_completion_renders_the_same_publication() {
    let mut app = files_app();
    set_index_source(&mut app, 10, None);
    let path = add_final_file(&mut app);
    open_find(&mut app);
    type_query(&mut app, FINAL_QUERY);

    tick(&mut app, 9);
    let before = drawn_text(&mut app);
    assert!(before.contains("· 0 results"), "{before}");
    assert!(before.contains("indexing"), "{before}");

    tick(&mut app, 1);
    let after = drawn_text(&mut app);
    assert!(after.contains("· 1 results"), "{after}");
    assert!(!after.contains("indexing"), "{after}");
    assert_eq!(hits(&mut app), vec![path]);
}

#[test]
fn files_find_enter_publishes_zero_duration_and_absent_index_sources() {
    let mut app = files_app();
    set_index_source(&mut app, 0, None);
    let path = add_final_file(&mut app);
    open_find(&mut app);
    type_query(&mut app, FINAL_QUERY);
    assert_eq!(
        hits(&mut app),
        vec![path.clone()],
        "zero-duration index is done on enter"
    );

    let mut absent = files_app();
    absent.world.sources.retain(|s| s.name != "index");
    let path = add_final_file(&mut absent);
    open_find(&mut absent);
    type_query(&mut absent, FINAL_QUERY);
    assert_eq!(
        hits(&mut absent),
        vec![path],
        "a missing index source is done"
    );
    assert_eq!(published(&mut absent).map(|(_, done)| done), Some(true));
}

fn finder_app() -> App {
    let mut app = App::launch(Scenario::RustDirty, Motion::Full, 0, Theme::junie(), false);
    app.world.clock.running = true;
    app
}

fn finder(app: &mut App) -> &mut FinderPage {
    app.router
        .top_of_mut(0)
        .and_then(|s| s.as_finder())
        .expect("finder on the first tab")
}

fn finder_select_explore(app: &mut App, id: &str) {
    for _ in 0..80 {
        match finder(app).current_id() {
            Some(cur) if cur == id => return,
            Some(cur) if cur.starts_with("explore.") => {
                app.handle_input(key(KeyCode::Right, KeyModifiers::NONE));
            }
            _ => {
                app.handle_input(key(KeyCode::Down, KeyModifiers::NONE));
            }
        }
    }
    panic!("finder never selected {id}");
}

#[test]
fn finder_explore_selection_survives_background_tick() {
    let mut app = finder_app();
    finder_select_explore(&mut app, "explore.git");

    tick(&mut app, 3);

    assert_eq!(
        finder(&mut app).current_id().as_deref(),
        Some("explore.git")
    );
}

#[test]
fn finder_missing_explore_selection_uses_first_selectable() {
    let mut app = finder_app();
    finder_select_explore(&mut app, "explore.git");

    app.handle_input(key(KeyCode::Down, KeyModifiers::CONTROL));
    tick(&mut app, 1);
    let after_scope = finder(&mut app).current_id();
    app.handle_input(key(KeyCode::Home, KeyModifiers::NONE));
    let first = finder(&mut app).current_id();

    assert_ne!(after_scope.as_deref(), Some("explore.git"));
    assert_eq!(after_scope, first);
}

#[test]
fn finder_query_edit_resets_explore_selection() {
    let mut app = finder_app();
    finder_select_explore(&mut app, "explore.git");

    app.handle_input(key(KeyCode::Char('g'), KeyModifiers::NONE));
    tick(&mut app, 1);
    let typed = finder(&mut app).current_id();
    app.handle_input(key(KeyCode::Home, KeyModifiers::NONE));

    assert_eq!(finder(&mut app).current_id(), typed);
}

const LIST_COLS: usize = 78;

fn explore_tiles(ids: &[&str]) -> Vec<crate::domain::catalog::ExploreTile> {
    let all = crate::domain::catalog::default_explore_tiles();
    ids.iter()
        .map(|id| {
            all.iter()
                .find(|t| t.id == *id)
                .unwrap_or_else(|| panic!("no default tile {id}"))
                .clone()
        })
        .collect()
}

fn finder_list_lines(app: &mut App) -> Vec<String> {
    drawn_text(app)
        .lines()
        .map(|l| l.chars().take(LIST_COLS).collect())
        .collect()
}

fn drawn_explore_row(app: &mut App) -> (u16, String) {
    let lines = finder_list_lines(app);
    let heading = lines
        .iter()
        .position(|l| l.trim() == "Explore")
        .expect("Explore heading drawn");
    (heading as u16 + 1, lines[heading + 1].clone())
}

fn drawn_tile(app: &mut App, label: &str) -> (u16, u16) {
    let (y, row) = drawn_explore_row(app);
    let at = row
        .find(label)
        .unwrap_or_else(|| panic!("{label} not drawn: {row}"));
    (row[..at].chars().count() as u16, y)
}

fn chosen_tile(app: &mut App) -> Option<String> {
    let (_, row) = drawn_explore_row(app);
    let at = row.find('\u{203a}')?;
    row[at + '\u{203a}'.len_utf8()..]
        .split_whitespace()
        .next()
        .map(str::to_owned)
}

fn focus_markers(app: &mut App) -> usize {
    finder_list_lines(app)
        .iter()
        .map(|l| l.chars().filter(|c| *c == '\u{203a}').count())
        .sum()
}

fn click_at(app: &mut App, x: u16, y: u16) {
    use junie_tui::core::event::{Mouse, MouseKind};
    let at = ratatui::layout::Position::new(x, y);
    app.handle_input(Input::Mouse(Mouse::new(MouseKind::Down, at)));
    app.handle_input(Input::Mouse(Mouse::new(MouseKind::Up, at)));
}

fn finder_group(app: &mut App) -> Option<&'static str> {
    finder(app).group
}

#[test]
fn finder_explore_pointer_activation_uses_retained_id() {
    let mut app = finder_app();
    app.world.explore_tiles = explore_tiles(&[
        "explore.system",
        "explore.tasks",
        "explore.files",
        "explore.git",
        "explore.disk",
        "explore.services",
    ]);
    tick(&mut app, 1);

    let (x, y) = drawn_tile(&mut app, "Disk");
    click_at(&mut app, x + 1, y);
    assert_eq!(finder_group(&mut app), Some("Disk"));

    app.handle_input(key(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(finder_group(&mut app), None);
    assert_eq!(
        finder(&mut app).current_id().as_deref(),
        Some("explore.disk")
    );
    assert_eq!(chosen_tile(&mut app).as_deref(), Some("Disk"));
    assert_eq!(focus_markers(&mut app), 1);
}

#[test]
fn finder_explore_selection_tracks_reordered_cells() {
    let mut app = finder_app();
    finder_select_explore(&mut app, "explore.git");
    assert_eq!(chosen_tile(&mut app).as_deref(), Some("Git"));

    app.world.explore_tiles = explore_tiles(&[
        "explore.system",
        "explore.disk",
        "explore.files",
        "explore.tasks",
        "explore.git",
    ]);
    tick(&mut app, 1);

    assert_eq!(
        finder(&mut app).current_id().as_deref(),
        Some("explore.git")
    );
    assert_eq!(chosen_tile(&mut app).as_deref(), Some("Git"));
    assert_eq!(focus_markers(&mut app), 1);
    let drawn = drawn_text(&mut app);
    assert!(
        drawn
            .lines()
            .any(|l| l.contains("Git") && l.contains("explore \u{b7} here")),
        "preview follows the moved tile: {drawn}"
    );

    app.handle_input(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(finder_group(&mut app), Some("Git"));
}

/// Explore tiles in `order`, with the Git summary long enough to overflow the
/// preview pane so its scroll offset can move.
fn long_git_explore_tiles(order: &[&str]) -> Vec<crate::domain::catalog::ExploreTile> {
    let words: Vec<String> = (0..600).map(|i| format!("L{i:03}")).collect();
    let summary: &'static str = Box::leak(words.join(" ").into_boxed_str());
    let mut tiles = explore_tiles(order);
    tiles
        .iter_mut()
        .find(|t| t.id == "explore.git")
        .expect("git tile")
        .summary = summary;
    tiles
}

fn preview_top_word(app: &mut App) -> String {
    drawn_text(app)
        .lines()
        .find_map(|line| {
            line.split_whitespace()
                .find(|w| {
                    w.len() == 4 && w.starts_with('L') && w[1..].bytes().all(|b| b.is_ascii_digit())
                })
                .map(str::to_owned)
        })
        .expect("preview word drawn")
}

#[test]
fn finder_explore_reorder_keeps_preview_scroll() {
    let mut app = finder_app();
    app.world.explore_tiles = long_git_explore_tiles(&[
        "explore.system",
        "explore.tasks",
        "explore.files",
        "explore.git",
        "explore.disk",
        "explore.services",
    ]);
    tick(&mut app, 1);
    finder_select_explore(&mut app, "explore.git");
    drawn_text(&mut app);
    click_at(&mut app, 100, 9);
    for _ in 0..3 {
        app.handle_input(key(KeyCode::Down, KeyModifiers::NONE));
    }
    let scrolled = preview_top_word(&mut app);
    assert_ne!(
        scrolled, "L000",
        "preview did not scroll before the rebuild"
    );

    app.world.explore_tiles = long_git_explore_tiles(&[
        "explore.system",
        "explore.disk",
        "explore.files",
        "explore.tasks",
        "explore.git",
        "explore.services",
    ]);
    tick(&mut app, 1);

    assert_eq!(
        finder(&mut app).current_id().as_deref(),
        Some("explore.git")
    );
    assert_eq!(preview_top_word(&mut app), scrolled);
}

fn long_explore_tiles(order: &[&str]) -> Vec<crate::domain::catalog::ExploreTile> {
    let words: Vec<String> = (0..600).map(|i| format!("L{i:03}")).collect();
    let summary: &'static str = Box::leak(words.join(" ").into_boxed_str());
    let mut tiles = explore_tiles(order);
    for tile in &mut tiles {
        tile.summary = summary;
    }
    tiles
}

fn scroll_preview_down(app: &mut App, times: usize) {
    use junie_tui::core::event::{Mouse, MouseKind};
    let at = ratatui::layout::Position::new(100, 9);
    for _ in 0..times {
        app.handle_input(Input::Mouse(Mouse::new(MouseKind::WheelDown, at)));
    }
}

#[test]
fn finder_explore_key_selection_change_resets_preview_scroll() {
    let mut app = finder_app();
    app.world.explore_tiles = long_explore_tiles(&[
        "explore.system",
        "explore.tasks",
        "explore.files",
        "explore.git",
        "explore.disk",
        "explore.services",
    ]);
    tick(&mut app, 1);
    finder_select_explore(&mut app, "explore.git");
    drawn_text(&mut app);
    scroll_preview_down(&mut app, 3);
    assert_ne!(
        preview_top_word(&mut app),
        "L000",
        "preview did not scroll before the key move"
    );

    app.handle_input(key(KeyCode::Right, KeyModifiers::NONE));

    assert_eq!(
        finder(&mut app).current_id().as_deref(),
        Some("explore.disk")
    );
    assert_eq!(preview_top_word(&mut app), "L000");
}

#[test]
fn finder_explore_click_selection_change_resets_preview_scroll() {
    let mut app = finder_app();
    app.world.explore_tiles = long_explore_tiles(&[
        "explore.system",
        "explore.tasks",
        "explore.files",
        "explore.git",
        "explore.disk",
        "explore.services",
    ]);
    tick(&mut app, 1);
    finder_select_explore(&mut app, "explore.git");
    drawn_text(&mut app);
    scroll_preview_down(&mut app, 3);
    assert_ne!(
        preview_top_word(&mut app),
        "L000",
        "preview did not scroll before the click"
    );

    let (x, y) = drawn_tile(&mut app, "Disk");
    click_at(&mut app, x + 1, y);
    assert_eq!(finder_group(&mut app), Some("Disk"));
    app.handle_input(key(KeyCode::Esc, KeyModifiers::NONE));

    assert_eq!(finder_group(&mut app), None);
    assert_eq!(
        finder(&mut app).current_id().as_deref(),
        Some("explore.disk")
    );
    assert_eq!(preview_top_word(&mut app), "L000");
}

#[test]
fn finder_empty_explore_row_is_not_selectable() {
    let mut app = finder_app();
    finder_select_explore(&mut app, "explore.git");

    app.world.explore_tiles = vec![];
    tick(&mut app, 1);
    let after = finder(&mut app).current_id();
    app.handle_input(key(KeyCode::Home, KeyModifiers::NONE));
    let first = finder(&mut app).current_id();
    assert_eq!(after, first);
    assert!(
        first
            .as_deref()
            .is_some_and(|id| !id.starts_with("explore."))
    );
    assert_eq!(focus_markers(&mut app), 1);

    for _ in 0..40 {
        app.handle_input(key(KeyCode::Down, KeyModifiers::NONE));
        let id = finder(&mut app).current_id();
        assert!(
            id.as_deref().is_some_and(|id| !id.starts_with("explore.")),
            "{id:?}"
        );
    }
    assert_eq!(focus_markers(&mut app), 1);
}

#[test]
fn finder_identity_moves_between_item_and_explore() {
    let mut app = finder_app();
    finder_select_explore(&mut app, "explore.git");

    let (cwd, host, now) = (
        app.world.location.cwd.clone(),
        app.world.host.name.clone(),
        app.world.now_secs(),
    );
    app.world
        .memory
        .usage
        .actions
        .push(crate::domain::usage::ActionUse {
            item: "explore.git".into(),
            path: Some(cwd),
            host,
            stamps: vec![now],
        });
    tick(&mut app, 1);

    assert_eq!(
        finder(&mut app).current_id().as_deref(),
        Some("explore.git")
    );
    assert_eq!(chosen_tile(&mut app), None);
    assert_eq!(focus_markers(&mut app), 1);
    let drawn = drawn_text(&mut app);
    assert!(
        drawn
            .lines()
            .any(|l| l.contains("Git") && l.contains("explore \u{b7} here")),
        "preview keeps the same item: {drawn}"
    );

    app.world.memory.usage.actions.clear();
    tick(&mut app, 1);
    assert_eq!(
        finder(&mut app).current_id().as_deref(),
        Some("explore.git")
    );
    assert_eq!(chosen_tile(&mut app).as_deref(), Some("Git"));
    assert_eq!(focus_markers(&mut app), 1);

    app.handle_input(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(finder_group(&mut app), Some("Git"));
}
