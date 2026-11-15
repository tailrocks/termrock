use junie_tui::clock::SimClock;

#[test]
fn sim_clock_reports_relative_age_in_the_largest_unit() {
    let mut clock = SimClock::new(1_000_000, 0);
    clock.advance(90_000);

    assert_eq!(clock.ago(clock.now_secs() - 30), "30 s ago");
    assert_eq!(clock.ago(clock.now_secs() - 120), "2 min ago");
    assert_eq!(clock.ago(clock.now_secs() + 60), "0 s ago");
}
