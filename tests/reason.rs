use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::core::refusal::Reason;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::popup::Placement;
use junie_tui::widgets::menu::{ContextMenu, MenuItem};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

fn menu(short: &str) -> ContextMenu {
    ContextMenu::new(
        WidgetId::of("t.menu"),
        vec![
            MenuItem::new("Reconnect").shortcut("Enter").reason(short),
            MenuItem::new("Stop").shortcut("t").disabled(true),
        ],
    )
    .anchor(Rect::new(2, 1, 1, 1), Placement::Below)
}

fn border_row(menu: &mut ContextMenu) -> String {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 12));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        menu.render(Rect::new(0, 0, 60, 12), &mut buf, &mut ctx);
        ctx.flush_overlays(&mut buf);
    }
    let (_, h) = menu.size();
    let bottom = 1 + h;
    (0..60)
        .map(|x| buf[(x, bottom)].symbol().to_owned())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

#[test]
fn a_reason_stays_within_the_status_and_menu_limits() {
    let text = |n: usize| "a".repeat(n);
    assert!(Reason::new(&text(60)).within_limits());
    assert!(!Reason::new(&text(61)).within_limits());
    assert!(Reason::new("x").short(&text(20)).within_limits());
    assert!(!Reason::new("x").short(&text(21)).within_limits());
    assert!(Reason::new(&"界".repeat(30)).within_limits());
    assert!(!Reason::new(&"界".repeat(31)).within_limits());
    assert!(Reason::new("x").short(&"界".repeat(10)).within_limits());
    assert!(!Reason::new("x").short(&"界".repeat(11)).within_limits());
}

#[test]
fn a_reason_is_measured_against_the_room_of_its_surface() {
    let reason = Reason::new("Always off · the host decides").short("no disabled state");
    assert!(reason.text_fits(46));
    assert!(reason.text_fits(29));
    assert!(!reason.text_fits(28));
    assert!(reason.short_fits(17));
    assert!(!reason.short_fits(16));
    assert!(!Reason::new("x").short_fits(80));
}

#[test]
fn the_menu_border_leaves_six_columns_less_than_the_menu_is_wide() {
    let menu = menu("env account");
    assert_eq!(menu.size().0, 23);
    assert_eq!(menu.reason_room(false), 17);
    assert_eq!(menu.reason_room(true), 15);
}

#[test]
fn a_short_reason_shows_whole_in_the_menu_border_up_to_its_room() {
    let fits = border_row(&mut menu("no disabled state"));
    assert!(fits.contains(" no disabled state "), "{fits}");
    let cut = border_row(&mut menu("no disabled state!!"));
    assert!(
        !cut.contains("!!") && cut.contains(" no disabled stat… "),
        "{cut}"
    );
}

fn scrolling_menu(short: &str) -> ContextMenu {
    let mut items = vec![MenuItem::new("Reconnect").shortcut("Enter").reason(short)];
    items.extend((1..31).map(|n| MenuItem::new(format!("Item {n}")).disabled(true)));
    ContextMenu::new(WidgetId::of("t.menu"), items)
        .max_rows(5)
        .anchor(Rect::new(2, 1, 1, 1), Placement::Below)
}

#[test]
fn a_scrolling_menu_keeps_its_marker_clear_of_the_reason() {
    let room = scrolling_menu("x").reason_room(true);
    assert_eq!(room, scrolling_menu("x").reason_room(false) - 2);
    let whole: String = ('a'..='z').take(room).collect();
    let fits = border_row(&mut scrolling_menu(&whole));
    assert!(
        fits.contains(&format!(" {whole} ")) && fits.contains("─▾─╯"),
        "{fits}"
    );
    let long: String = ('a'..='z').take(room + 1).collect();
    let cut = border_row(&mut scrolling_menu(&long));
    let kept: String = whole.chars().take(room - 1).collect();
    assert!(
        cut.contains(&format!(" {kept}… ")) && cut.contains("─▾─╯"),
        "{cut}"
    );
    assert!(!cut.contains(&long), "{cut}");
}

#[test]
fn a_narrow_place_shows_the_short_text_or_the_whole_reason() {
    assert_eq!(
        Reason::new("Always off · the host decides")
            .short("managed")
            .short_or_text(),
        "managed"
    );
    assert_eq!(
        Reason::new("Always off · the host decides").short_or_text(),
        "Always off · the host decides"
    );
}
