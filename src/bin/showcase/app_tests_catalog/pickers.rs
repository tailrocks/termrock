//! Lane `pickers` of the #14 burn-down: this lane's callable scenarios.
//!
//! Each scenario builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. Register one `ScenarioCase` per
//! `ScenarioId` variant from the `lane:pickers` block in `../coverage.rs`. Edit only
//! this file, `../coverage/lane_pickers.rs` and `../coverage_debt/pickers.txt`.

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::{Axis, HitRegistry};
use junie_tui::core::id::{ItemKey, Part, PartRef, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::{Actions as _, KeyBinding};
use junie_tui::core::keymap::{Nav, NavKeys};
use junie_tui::core::list::{Moved, RowSource};
use junie_tui::core::scroll::Scrollable;
use junie_tui::core::scroll::TrackClick;
use junie_tui::core::selection::ClickPolicy;
use junie_tui::core::text::TextBuffer;
use junie_tui::theme::{ColorLevel, CursorStyle, Theme, Tone};
use junie_tui::ui::content::ContentState;
use junie_tui::ui::ctx::{Backdrop, Interaction, RenderCtx};
use junie_tui::ui::measure::Measure;
use junie_tui::ui::popup::{Dismiss, Place, Placement, backdrop_id};
use junie_tui::ui::reveal::Reveal;
use junie_tui::ui::scroll_view::ScrollView;
use junie_tui::widgets::completion::{Completion, CompletionAction, TEXT_INSET};
use junie_tui::widgets::edit::{EditField, LineLook};
use junie_tui::widgets::empty::EmptyState;
use junie_tui::widgets::meter_list::{MeterList, MeterRow};
use junie_tui::widgets::option_editor::{
    Affordance, ChoiceEditor, ChoiceResult, ColumnEditor, EditorConfig, OptionItem, Options,
};
use junie_tui::widgets::option_list::{
    AsItem, Item, OptionList, SelectArrows, WheelMode, query_pointer,
};
use junie_tui::widgets::picker::{
    Picker, PickerAction, PickerItemsError, PickerKeyProfile, PickerStatus,
};
use junie_tui::widgets::progress::{
    BarLayout, METER_LOW_MAX, METER_MEDIUM_MAX, Meter, MeterLevel, MeterTone, MeterVisual,
    ProgressBar, ProgressStatus, SPINNER, StaleStyle, Tally, VALUE_W, render_bar, render_bar_with,
    render_indeterminate, render_indeterminate_with, render_meter, render_spinner, spinner_frame,
};
use junie_tui::widgets::props::{
    Prop, PropsAction, PropsCmd, PropsList, Sheet, measure, measure_with, render as render_props,
    render_in, render_with, width as props_width, width_with,
};
use junie_tui::widgets::select::{Select, SelectAction};
use junie_tui::widgets::table::{Cell, Column, DataTable};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Position, Rect};
use ratatui::style::{Color, Modifier};

use super::{ScenarioCase, key, paint_hits, rows};
use crate::coverage::ScenarioId;

#[rustfmt::skip]
pub const CASES: &[ScenarioCase] = &[
    ScenarioCase { id: ScenarioId::MeterToneLevels, run: meter_tone_levels },
    ScenarioCase { id: ScenarioId::MeterStaleAndVisuals, run: meter_stale_and_visuals },
    ScenarioCase { id: ScenarioId::ProgressBarStatuses, run: progress_bar_statuses },
    ScenarioCase { id: ScenarioId::MeterListColumns, run: meter_list_columns },
    ScenarioCase { id: ScenarioId::CompletionLifecycle, run: completion_lifecycle },
    ScenarioCase { id: ScenarioId::OptionListItems, run: option_list_items },
    ScenarioCase { id: ScenarioId::OptionListNavigation, run: option_list_navigation },
    ScenarioCase { id: ScenarioId::PickerSingleActions, run: picker_single_actions },
    ScenarioCase { id: ScenarioId::PickerMultiSelection, run: picker_multi_selection },
    ScenarioCase { id: ScenarioId::PickerStatesAndChrome, run: picker_states_and_chrome },
    ScenarioCase { id: ScenarioId::ColumnEditorModel, run: column_editor_model },
    ScenarioCase { id: ScenarioId::ChoiceEditorKeys, run: choice_editor_keys },
    ScenarioCase { id: ScenarioId::ChoiceEditorPointerAndRender, run: choice_editor_pointer_and_render },
    ScenarioCase { id: ScenarioId::EditorAffordance, run: editor_affordance },
    ScenarioCase { id: ScenarioId::PropsSheetLayout, run: props_sheet_layout },
    ScenarioCase { id: ScenarioId::OptionListSelectArrows, run: option_list_select_arrows },
    ScenarioCase { id: ScenarioId::OptionListQueryPointer, run: option_list_query_pointer },
    ScenarioCase { id: ScenarioId::PropsListInteraction, run: props_list_interaction },
];

fn theme() -> Theme {
    Theme::for_level(ColorLevel::TrueColor)
}

/// One frame at animation tick `tick`; `draw` receives the screen area.
fn frame(area: Rect, tick: u64, draw: impl FnOnce(Rect, &mut Buffer, &mut RenderCtx)) -> Buffer {
    let theme = theme();
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let interaction = Interaction {
        tick,
        ..Interaction::default()
    };
    let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
    draw(area, &mut buf, &mut ctx);
    buf
}

fn line(buf: &Buffer, y: u16) -> String {
    rows(buf)[usize::from(y)].clone()
}

fn cells(buf: &Buffer, y: u16, x: u16, len: u16) -> String {
    (x..x + len).map(|x| buf[(x, y)].symbol()).collect()
}

fn count(text: &str, glyph: &str) -> usize {
    text.matches(glyph).count()
}

fn meter_line(meter: &Meter, width: u16) -> (Buffer, String) {
    let buf = frame(Rect::new(0, 0, width, 1), 0, |area, buf, ctx| {
        meter.render(area, buf, ctx, ctx.theme.canvas);
    });
    let text = line(&buf, 0);
    (buf, text)
}

fn meter_tone_levels() {
    let t = theme();
    // The two threshold constants split the percentages into three bands.
    assert_eq!(MeterLevel::of(0), MeterLevel::Low);
    assert_eq!(MeterLevel::of(METER_LOW_MAX), MeterLevel::Low);
    assert_eq!(MeterLevel::of(METER_LOW_MAX + 1), MeterLevel::Medium);
    assert_eq!(MeterLevel::of(METER_MEDIUM_MAX), MeterLevel::Medium);
    assert_eq!(MeterLevel::of(METER_MEDIUM_MAX + 1), MeterLevel::High);
    assert_eq!(MeterLevel::with_thresholds(40, 30, 50), MeterLevel::Medium);
    assert_eq!(MeterLevel::with_thresholds(50, 30, 50), MeterLevel::High);
    assert_eq!(MeterLevel::with_thresholds(29, 30, 50), MeterLevel::Low);

    // `MeterTone::level` maps a tone and a percentage to the band it paints.
    assert_eq!(MeterTone::Normal.level(Some(90)), Some(MeterLevel::High));
    assert_eq!(MeterTone::Normal.level(None), None);
    assert_eq!(MeterTone::Stale.level(Some(10)), Some(MeterLevel::Low));
    let custom = MeterTone::Level { warn: 20, crit: 40 };
    assert_eq!(custom.level(Some(30)), Some(MeterLevel::Medium));
    assert_eq!(
        MeterTone::Fixed(MeterLevel::High).level(Some(1)),
        Some(MeterLevel::High)
    );
    assert_eq!(MeterTone::Warning.level(None), Some(MeterLevel::Medium));
    assert_eq!(MeterTone::Exhausted.level(None), Some(MeterLevel::High));
    for silent in [MeterTone::Refreshing, MeterTone::Error, MeterTone::Unknown] {
        assert_eq!(silent.level(Some(50)), None);
    }

    // Each band paints its own colour on the filled run and its own suffix.
    const WARN: &str = " ▲";
    assert_eq!(
        format!(" {}", t.glyphs.warning),
        WARN,
        "the suffix is the theme's warning glyph"
    );
    let cases = [
        (Meter::new(Some(30)), t.info, "  "),
        (Meter::new(Some(70)), t.warning, WARN),
        (Meter::new(Some(95)), t.error, " !"),
        (Meter::new(Some(30)).tone(custom), t.warning, WARN),
        (
            Meter::new(Some(30)).tone(MeterTone::Fixed(MeterLevel::High)),
            t.error,
            " !",
        ),
        (
            Meter::new(Some(30)).tone(MeterTone::Warning),
            t.warning,
            WARN,
        ),
        (
            Meter::new(Some(30)).tone(MeterTone::Exhausted),
            t.error,
            " !",
        ),
    ];
    for (meter, run, suffix) in cases {
        let (buf, text) = meter_line(&meter, 24);
        assert_eq!(buf[(0, 0)].symbol(), t.glyphs.bar_fill, "{meter:?}");
        assert_eq!(buf[(0, 0)].fg, run, "{meter:?} paints {run:?}: {text}");
        assert!(
            text.ends_with(&format!("{}{suffix}", meter.value)),
            "{meter:?}: {text:?}"
        );
    }

    // The default bands start at 60% and 85%: the same fill changes colour
    // there, so a different `METER_LOW_MAX` or `METER_MEDIUM_MAX` repaints it.
    for (pct, run) in [
        (59, t.info),
        (60, t.warning),
        (84, t.warning),
        (85, t.error),
    ] {
        let (buf, text) = meter_line(&Meter::new(Some(pct)), 24);
        assert_eq!(buf[(0, 0)].fg, run, "{pct}% paints {run:?}: {text}");
    }
    assert_eq!((METER_LOW_MAX, METER_MEDIUM_MAX), (59, 84));

    // The meter draws a run proportional to the percentage.
    let (_, half) = meter_line(&Meter::new(Some(50)), 28);
    let (_, full) = meter_line(&Meter::new(Some(100)), 28);
    let (_, empty) = meter_line(&Meter::new(Some(0)), 28);
    assert_eq!(count(&empty, t.glyphs.bar_fill), 0);
    assert_eq!(
        count(&full, t.glyphs.bar_fill),
        21,
        "the track is the width minus value and padding"
    );
    assert_eq!(count(&half, t.glyphs.bar_fill), 11);
    assert_eq!(count(&half, t.glyphs.bar_empty), 11);

    // `value` replaces the default text, `render_meter` is the same paint.
    let custom_value = Meter::new(Some(50)).value("5 of 10");
    assert_eq!(custom_value.value, "5 of 10");
    let (_, text) = meter_line(&custom_value, 26);
    assert!(text.contains("5 of 10"), "{text:?}");
    let via_fn = frame(Rect::new(0, 0, 26, 1), 0, |area, buf, ctx| {
        render_meter(
            area,
            buf,
            ctx,
            50,
            "5 of 10",
            MeterTone::Normal,
            ctx.theme.canvas,
        );
    });
    assert_eq!(line(&via_fn, 0), text);

    // A tone with no run falls back to the bare value.
    let (_, none) = meter_line(&Meter::new(None), 12);
    assert!(
        !none.contains(t.glyphs.bar_fill) && none.trim().is_empty(),
        "{none:?}"
    );
    let (unknown, text) = meter_line(&Meter::new(None).tone(MeterTone::Unknown), 12);
    assert!(text.starts_with(t.glyphs.none), "{text:?}");
    assert_eq!(unknown[(0, 0)].fg, t.text_faint);
    let (err, text) = meter_line(
        &Meter::new(Some(10))
            .value("read failed")
            .tone(MeterTone::Error),
        20,
    );
    assert!(text.starts_with("read failed !"), "{text:?}");
    assert_eq!(err[(0, 0)].fg, t.error);

    // Refreshing shows the spinner frame of the animation tick.
    let refreshing = Meter::new(Some(10)).tone(MeterTone::Refreshing);
    let spun = frame(Rect::new(0, 0, 30, 1), 4, |area, buf, ctx| {
        refreshing.render(area, buf, ctx, ctx.theme.canvas);
    });
    assert!(
        line(&spun, 0).contains(&format!("{} refreshing", t.glyphs.spinner_frame(4))),
        "{}",
        line(&spun, 0)
    );

    // The measured width is the narrowest paint that still shows a track.
    let _ = frame(Rect::new(0, 0, 40, 1), 0, |area, _, ctx| {
        let size = Meter::new(Some(50)).measure(area.as_size(), ctx);
        assert_eq!(size.height, 1);
        assert_eq!(
            size.width,
            3 + 3 + 6,
            "value, spacing and the minimum track"
        );
    });
}

fn meter_stale_and_visuals() {
    let t = theme();
    let fresh = Meter::new(Some(50));
    assert!(!fresh.is_stale());
    assert!(fresh.clone().stale(true).is_stale());
    assert!(Meter::new(Some(50)).tone(MeterTone::Stale).is_stale());
    for tone in [MeterTone::Refreshing, MeterTone::Error, MeterTone::Unknown] {
        assert!(
            !Meter::new(Some(50)).tone(tone).stale(true).is_stale(),
            "{tone:?} wins over stale"
        );
    }

    // Dashed: a distinct fill glyph and italic text.
    let dashed = fresh.clone().stale(true).stale_style(StaleStyle::Dashed);
    let (buf, text) = meter_line(&dashed, 26);
    assert_eq!(buf[(0, 0)].symbol(), t.glyphs.bar_stale, "{text:?}");
    assert!(text.contains("50%"));
    let value_x = u16::try_from(text.chars().position(|c| c == '5').expect("value")).unwrap();
    assert!(
        buf[(value_x, 0)].modifier.contains(Modifier::ITALIC),
        "a dashed stale value is italic"
    );
    // Faint: the normal fill glyph in the faint colour, no suffix.
    let faint = fresh.clone().stale(true).stale_style(StaleStyle::Faint);
    let (buf, text) = meter_line(&faint, 26);
    assert_eq!(buf[(0, 0)].symbol(), t.glyphs.bar_fill, "{text:?}");
    assert_eq!(buf[(0, 0)].fg, t.text_faint);
    assert_ne!(
        meter_line(&dashed, 26).1,
        text,
        "the two stale styles paint differently"
    );

    // `Line` is the default visual: a thin track of fill glyphs.
    let line_visual = Meter::new(Some(50)).visual(MeterVisual::Line);
    assert_eq!(line_visual.visual, MeterVisual::Line);
    assert_eq!(
        line(&meter_line(&line_visual, 26).0, 0),
        meter_line(&fresh, 26).1
    );
    // Block visual: a filled bar behind the value, not a thin track.
    let block = Meter::new(Some(50)).visual(MeterVisual::Block);
    assert_eq!(block.visual, MeterVisual::Block);
    let (buf, text) = meter_line(&block, 20);
    assert!(
        text.contains("50%") && !text.contains(t.glyphs.bar_fill),
        "{text:?}"
    );
    assert_eq!(
        buf[(0, 0)].fg,
        t.canvas,
        "text on the fill reads against it"
    );
    assert_eq!(buf[(0, 0)].bg, t.info);
    assert_ne!(buf[(17, 0)].bg, buf[(0, 0)].bg, "the unfilled rest differs");
    let (_, narrow) = meter_line(&block, 5);
    assert!(narrow.contains("50%"), "too narrow for a bar: {narrow:?}");
    let _ = frame(Rect::new(0, 0, 40, 1), 0, |area, _, ctx| {
        assert_eq!(block.measure(area.as_size(), ctx).width, 4 + 2);
    });
}

fn progress_bar_statuses() {
    let t = theme();
    // VALUE_W columns stay free for the value after the track.
    let bar = |ratio: f64, status: ProgressStatus, width: u16| {
        let buf = frame(Rect::new(0, 0, width, 1), 0, |area, buf, ctx| {
            render_bar(area, buf, ctx, "", ratio, status, ctx.theme.canvas);
        });
        (buf.clone(), line(&buf, 0))
    };
    let (buf, text) = bar(0.5, ProgressStatus::Active, 40);
    let track = usize::from(40 - VALUE_W);
    assert_eq!(track, 33, "seven columns stay free for the value");
    assert_eq!(
        count(&text, t.glyphs.bar_fill) + count(&text, t.glyphs.bar_empty),
        track
    );
    assert_eq!(buf[(0, 0)].fg, t.tone(Tone::Active));
    assert!(text.contains("50%"), "{text:?}");
    let (_, done) = bar(1.0, ProgressStatus::Done, 40);
    assert!(
        done.contains("100%") && done.trim_end().ends_with(t.glyphs.done),
        "{done:?}"
    );
    let (done_buf, _) = bar(1.0, ProgressStatus::Done, 40);
    assert_eq!(done_buf[(0, 0)].fg, t.success);
    let (err_buf, err) = bar(0.25, ProgressStatus::Error, 40);
    assert!(
        err.trim_end().ends_with('!') && err_buf[(0, 0)].fg == t.error,
        "{err:?}"
    );
    let (paused_buf, paused) = bar(0.25, ProgressStatus::Paused, 40);
    assert!(paused.trim_end().ends_with(t.glyphs.paused), "{paused:?}");
    assert_eq!(paused_buf[(0, 0)].fg, t.text_muted);
    // A ratio outside 0..=1 clamps; a narrow area drops the track.
    let (_, over) = bar(7.0, ProgressStatus::Active, 40);
    assert!(over.contains("100%"), "{over:?}");
    let (_, tiny) = bar(0.5, ProgressStatus::Active, 8);
    assert!(
        tiny.starts_with("50%") && !tiny.contains(t.glyphs.bar_fill),
        "{tiny:?}"
    );

    // `BarLayout`: a fixed label column and the value reservation.
    let layout = BarLayout::default().label_width(8).reserve_value(true);
    assert_eq!(layout.label_width, Some(8));
    assert!(layout.reserve_value);
    let labelled = frame(Rect::new(0, 0, 40, 1), 0, |area, buf, ctx| {
        render_bar_with(
            area,
            buf,
            ctx,
            "disk",
            0.5,
            ProgressStatus::Active,
            ctx.theme.canvas,
            layout,
        );
    });
    let text = line(&labelled, 0);
    assert!(text.starts_with("disk"), "{text:?}");
    assert_eq!(
        count(&text, t.glyphs.bar_fill) + count(&text, t.glyphs.bar_empty),
        usize::from(40 - 8 - 2 - VALUE_W)
    );
    let truncated = BarLayout::default().label_width(3);
    let cut = frame(Rect::new(0, 0, 40, 1), 0, |area, buf, ctx| {
        render_bar_with(
            area,
            buf,
            ctx,
            "disk",
            0.5,
            ProgressStatus::Active,
            ctx.theme.canvas,
            truncated,
        );
    });
    assert!(!line(&cut, 0).starts_with("disk"), "{}", line(&cut, 0));

    // `ProgressBar` renders determinate and indeterminate bars alike.
    let determinate = ProgressBar::new("sync", 0.5)
        .status(ProgressStatus::Paused)
        .layout(layout);
    assert_eq!(determinate.status, ProgressStatus::Paused);
    assert_eq!(determinate.layout, layout);
    let buf = frame(Rect::new(0, 0, 40, 1), 0, |area, buf, ctx| {
        determinate.render(area, buf, ctx, ctx.theme.canvas);
    });
    assert!(line(&buf, 0).starts_with("sync") && line(&buf, 0).contains("50%"));
    let waiting = ProgressBar::indeterminate("sync");
    assert_eq!(waiting.ratio, None);
    let at = |tick| {
        frame(Rect::new(0, 0, 40, 1), tick, |area, buf, ctx| {
            waiting.render(area, buf, ctx, ctx.theme.canvas);
        })
    };
    let accent_at = |buf: &Buffer| (0..40u16).find(|&x| buf[(x, 0)].fg == t.accent);
    let (first, later) = (accent_at(&at(2)), accent_at(&at(9)));
    assert!(
        first.is_some() && later.is_some() && first != later,
        "{first:?} {later:?}"
    );
    // The free functions are the same paint as the builder.
    let free = frame(Rect::new(0, 0, 40, 1), 2, |area, buf, ctx| {
        render_indeterminate(area, buf, ctx, "", ctx.theme.canvas);
    });
    let with = frame(Rect::new(0, 0, 40, 1), 2, |area, buf, ctx| {
        render_indeterminate_with(area, buf, ctx, "", ctx.theme.canvas, BarLayout::default());
    });
    assert_eq!(line(&free, 0), line(&with, 0));
    let unreserved = frame(Rect::new(0, 0, 40, 1), 2, |area, buf, ctx| {
        render_indeterminate_with(
            area,
            buf,
            ctx,
            "",
            ctx.theme.canvas,
            BarLayout::default().reserve_value(false),
        );
    });
    let cells = |buf: &Buffer| (0..40u16).filter(|&x| buf[(x, 0)].symbol() != " ").count();
    assert_eq!(cells(&with), 40 - usize::from(VALUE_W));
    assert_eq!(cells(&unreserved), 40);

    // The spinner cycles through SPINNER and labels its frame.
    assert_eq!(spinner_frame(0), SPINNER[0]);
    assert_eq!(spinner_frame(13), SPINNER[3]);
    assert_eq!((spinner_frame(0), spinner_frame(13)), ("⠋", "⠸"));
    assert_eq!(spinner_frame(10), spinner_frame(0), "the cycle wraps");
    let spun = frame(Rect::new(0, 0, 20, 1), 3, |area, buf, ctx| {
        render_spinner(area, buf, ctx, "loading", ctx.theme.canvas);
    });
    assert_eq!(
        line(&spun, 0).trim_end(),
        format!("{} loading", t.glyphs.spinner_frame(3))
    );

    // `Tally` counts statuses and words the summary.
    let tally = Tally::of([
        ProgressStatus::Done,
        ProgressStatus::Done,
        ProgressStatus::Active,
        ProgressStatus::Paused,
        ProgressStatus::Error,
    ]);
    assert_eq!(
        (
            tally.done,
            tally.total,
            tally.running,
            tally.paused,
            tally.failed
        ),
        (2, 5, 1, 1, 1)
    );
    assert_eq!(
        tally.text(),
        "2 of 5 done · 1 running · 1 paused · 1 failed"
    );
    assert_eq!(Tally::of([ProgressStatus::Done]).text(), "1 of 1 done");
    let drawn = std::cell::Cell::new(0);
    let buf = frame(Rect::new(0, 0, 60, 1), 0, |area, buf, ctx| {
        drawn.set(tally.render(area, buf, ctx, ctx.theme.canvas));
    });
    assert_eq!(line(&buf, 0).trim_end(), tally.text());
    assert_eq!(usize::from(drawn.get()), tally.text().chars().count());
    let failed_x = line(&buf, 0)
        .find("1 failed")
        .map(|b| line(&buf, 0)[..b].chars().count());
    let failed_x = u16::try_from(failed_x.expect("failed clause")).unwrap();
    assert_eq!(
        buf[(failed_x, 0)].fg,
        t.error_fg().fg.unwrap_or(Color::Reset)
    );
    let squeezed = frame(Rect::new(0, 0, 10, 1), 0, |area, buf, ctx| {
        assert!(tally.render(area, buf, ctx, ctx.theme.canvas) <= 10);
    });
    assert!(line(&squeezed, 0).starts_with("2 of 5"));
}

fn meter_list_columns() {
    let t = theme();
    let sample = || {
        MeterList::new(vec![
            MeterRow::new("CPU", Meter::new(Some(50))).detail("4 cores"),
            MeterRow::new("Memory", Meter::new(Some(90))).detail("12 GB"),
            MeterRow::new("Disk", Meter::new(Some(20))),
        ])
    };
    let area = Rect::new(0, 0, 40, 3);
    let draw = |list: &MeterList, area: Rect| {
        let drawn = std::cell::Cell::new(0);
        let buf = frame(Rect::new(0, 0, 40, 3), 0, |_, buf, ctx| {
            drawn.set(list.render(area, buf, ctx, ctx.theme.canvas));
        });
        (buf, drawn.get())
    };

    let list = sample();
    assert_eq!(list.height(), 3);
    assert_eq!(MeterList::default().height(), 0);
    // The label column is the widest label, the meter takes the rest.
    assert_eq!(list.label_column(area), 6);
    assert_eq!(list.meter_column(area), (7, 33));
    let (buf, drawn) = draw(&list, area);
    assert_eq!(drawn, 3);
    assert!(line(&buf, 0).starts_with("CPU"), "{}", line(&buf, 0));
    assert!(line(&buf, 1).starts_with("Memory"));
    assert_eq!(buf[(7, 0)].symbol(), t.glyphs.bar_fill);
    assert!(
        !line(&buf, 0).contains("4 cores"),
        "no room left for the detail"
    );

    // `reserve` leaves cells to the right of the meter for the detail.
    let reserved = sample().reserve(12);
    assert_eq!(reserved.reserve, 12);
    assert_eq!(reserved.meter_column(area), (7, 21));
    let (buf, _) = draw(&reserved, area);
    assert_eq!(cells(&buf, 0, 30, 7), "4 cores");
    assert_eq!(buf[(30, 0)].fg, t.text_muted, "detail is muted by default");
    // `detail_gap` moves the detail, `detail_tone` recolours it.
    let moved = sample()
        .reserve(12)
        .detail_gap(4)
        .detail_tone(Tone::Success);
    assert_eq!((moved.detail_gap, moved.detail_tone), (4, Tone::Success));
    let (buf, _) = draw(&moved, area);
    assert_eq!(cells(&buf, 0, 32, 7), "4 cores");
    assert_eq!(buf[(32, 0)].fg, t.success);

    // `label_width` fixes the column; longer labels truncate.
    let fixed = sample().label_width(4);
    assert_eq!(
        (fixed.label_column(area), fixed.meter_column(area).0),
        (4, 5)
    );
    let (buf, _) = draw(&fixed, area);
    assert!(line(&buf, 1).starts_with("Mem") && !line(&buf, 1).starts_with("Memory"));
    // `label_gap` pushes the meters right.
    let gapped = sample().label_gap(3);
    assert_eq!((gapped.label_gap, gapped.meter_column(area).0), (3, 9));
    // `label_tone` recolours labels; `pad_label` paints the label cell whole.
    let toned = sample().label_tone(Tone::Error).label_width(8);
    assert_eq!(toned.label_tone, Tone::Error);
    let (loose, _) = draw(&toned, area);
    assert_eq!(loose[(0, 0)].fg, t.error);
    assert_ne!(
        loose[(5, 0)].bg,
        t.canvas,
        "unpadded: only the text is painted"
    );
    let padded = toned.clone().pad_label(true);
    assert!(padded.pad_label);
    let (tight, _) = draw(&padded, area);
    assert_eq!(
        tight[(5, 0)].bg,
        t.canvas,
        "padded: the whole column is painted"
    );
    // `meter_width` caps the meter; `min_meter` hides a meter that gets too small.
    let capped = sample().meter_width(10);
    assert_eq!(capped.meter_column(area), (7, 10));
    let starved = sample().reserve(30).min_meter(8);
    assert_eq!(starved.min_meter, 8);
    assert_eq!(starved.meter_column(area), (7, 0));
    let (buf, drawn) = draw(&starved, area);
    assert_eq!(drawn, 3, "labels still draw");
    assert!(!line(&buf, 0).contains(t.glyphs.bar_fill));
    // Rows past the area are not drawn.
    let (_, drawn) = draw(&sample(), Rect::new(0, 0, 40, 2));
    assert_eq!(drawn, 2);
    let (_, drawn) = draw(&sample(), Rect::new(0, 0, 0, 0));
    assert_eq!(drawn, 0);
}

fn ctrl(c: char) -> Key {
    Key {
        code: KeyCode::Char(c),
        mods: KeyModifiers::CONTROL,
    }
}

fn completion_lifecycle() {
    const ID: WidgetId = WidgetId::of("scenario.completion.life");
    let anchor = Rect::new(10, 6, 1, 1);
    let items = || {
        vec![
            Item::new("blocked").reason("no access"),
            Item::new("alpha").detail("fn"),
            Item::new("alphabet").insert("alphabet(").detail("fn"),
            Item::new("alpine"),
            Item::new("alps"),
        ]
    };
    let mut completion = Completion::new(ID)
        .max_rows(3)
        .popup_width(12, 30)
        .placement(Placement::Below)
        .fit(Place::DEFAULT.text_inset(TEXT_INSET))
        .cursor_style(CursorStyle::Fill)
        .dismiss(Dismiss::LIGHT)
        .nav(Completion::NAV);
    assert!(!completion.is_open() && completion.current().is_none());
    assert_eq!(
        completion.on_key(&key(KeyCode::Down)),
        (Outcome::Ignored, None),
        "a closed popup takes no keys"
    );
    completion.open(items(), anchor, 2);
    assert!(completion.is_open());
    assert_eq!(completion.replace_len, 2);
    assert_eq!(
        completion.current().map(|item| item.label.as_str()),
        Some("alpha"),
        "the cursor starts on the first row that can be accepted"
    );

    // Keys: arrows and Ctrl-N/P move, Tab and Enter accept, Esc dismisses.
    assert_eq!(
        completion.on_key(&key(KeyCode::Down)),
        (Outcome::Changed, None)
    );
    assert_eq!(
        completion.current().map(|i| i.label.as_str()),
        Some("alphabet")
    );
    assert_eq!(completion.on_key(&ctrl('n')), (Outcome::Changed, None));
    assert_eq!(
        completion.current().map(|i| i.label.as_str()),
        Some("alpine")
    );
    assert_eq!(completion.on_key(&ctrl('p')), (Outcome::Changed, None));
    assert_eq!(
        completion.on_key(&key(KeyCode::Up)),
        (Outcome::Changed, None)
    );
    assert_eq!(
        completion.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(CompletionAction::Accept(1)))
    );
    assert_eq!(
        completion.on_key(&key(KeyCode::Tab)),
        (Outcome::Changed, Some(CompletionAction::Accept(1)))
    );
    assert_eq!(
        completion.on_key(&key(KeyCode::Char('x'))),
        (Outcome::Ignored, None)
    );

    // `apply` replaces the typed prefix and closes a call with its bracket.
    let mut text = TextBuffer::single("let al");
    assert!(completion.apply(1, &mut text));
    assert_eq!(text.text(), "let alpha");
    let mut text = TextBuffer::single("let al");
    assert!(completion.apply(2, &mut text));
    assert_eq!(text.text(), "let alphabet()");
    assert_eq!(
        text.cursor_offset(),
        "let alphabet(".len(),
        "the caret sits inside the call"
    );
    let mut text = TextBuffer::single("let al");
    assert!(
        !completion.apply(0, &mut text),
        "a refused row inserts nothing"
    );
    assert_eq!(text.text(), "let al");

    // Rendering places the popup under the anchor, inset by `TEXT_INSET`.
    let screen = Rect::new(0, 0, 60, 16);
    let painted = paint_hits(screen, |area, buf, ctx| completion.render(area, buf, ctx));
    let popup = completion.area;
    assert_eq!(popup.x, anchor.x - TEXT_INSET);
    assert!(popup.y > anchor.y, "below the anchor: {popup:?}");
    assert_eq!(popup.height, 3 + 2, "max_rows caps the visible rows");
    assert!(
        (12..=30).contains(&popup.width),
        "popup_width bounds it: {popup:?}"
    );
    assert!(
        popup.width < 24,
        "popup_width lowers the default minimum of 24: {popup:?}"
    );
    let shown = rows(&painted.buf).join("\n");
    assert!(
        shown.contains("alphabet") && shown.contains("fn"),
        "{shown}"
    );
    assert!(
        !shown.contains("alps"),
        "rows past max_rows scroll out of view"
    );
    // Following the cursor scrolls the later rows in.
    for _ in 0..3 {
        completion.on_key(&key(KeyCode::Down));
    }
    let painted = paint_hits(screen, |area, buf, ctx| completion.render(area, buf, ctx));
    assert!(rows(&painted.buf).join("\n").contains("alps"));

    // Rows are hit regions: a click accepts an eligible row only.
    assert_ne!(completion.row_id(1), completion.row_id(2));
    assert_eq!(
        completion.on_click(completion.row_id(0)),
        None,
        "a refused row is not accepted"
    );
    assert_eq!(
        completion.on_click(completion.row_id(2)),
        Some(CompletionAction::Accept(2))
    );
    assert_eq!(
        completion.current().map(|i| i.label.as_str()),
        Some("alphabet")
    );
    // The backdrop click dismisses and closes the popup.
    assert_eq!(
        completion.on_click(backdrop_id(ID)),
        Some(CompletionAction::Dismiss)
    );
    assert!(!completion.is_open());
    // `close` is idempotent and Esc dismisses an open popup.
    completion.close();
    completion.open(items(), anchor, 0);
    assert_eq!(
        completion.on_key(&key(KeyCode::Esc)),
        (Outcome::Changed, Some(CompletionAction::Dismiss))
    );
    assert!(!completion.is_open());

    // A policy that refuses both dismissals keeps the popup open.
    let mut sticky = Completion::new(ID).dismiss(Dismiss::NEVER);
    sticky.open(items(), anchor, 0);
    assert_eq!(sticky.on_key(&key(KeyCode::Esc)), (Outcome::Ignored, None));
    assert_eq!(sticky.on_click(backdrop_id(ID)), None);
    assert!(sticky.is_open());

    // Placement above puts the popup over the anchor.
    let low = Rect::new(10, 12, 1, 1);
    let mut above = Completion::new(ID).placement(Placement::Above);
    above.open(items(), low, 0);
    let _ = paint_hits(screen, |area, buf, ctx| above.render(area, buf, ctx));
    assert!(above.area.bottom() <= low.y, "{:?}", above.area);
    // A disabled highlighted row shows its reason on the popup border.
    let mut blocked = Completion::new(ID);
    blocked.open(vec![Item::new("blocked").reason("no access")], anchor, 0);
    assert!(!blocked.has_eligible());
    assert_eq!(
        blocked.on_key(&key(KeyCode::Enter)),
        (Outcome::Consumed, None),
        "nothing to accept"
    );
    let painted = paint_hits(screen, |area, buf, ctx| blocked.render(area, buf, ctx));
    assert!(rows(&painted.buf).join("\n").contains("no access"));

    // The defaults of a bare `Completion`. `NAV` leaves Home and End to the
    // text field while paging still moves, the popup stops at
    // `OptionList::MAX_ROWS` rows and sits `TEXT_INSET` columns left of the anchor.
    let many: Vec<Item> = (0..12).map(|i| Item::new(format!("item{i:02}"))).collect();
    let top = Rect::new(anchor.x, 1, 1, 1);
    let mut bare = Completion::new(ID);
    assert_eq!(bare.list.nav, Completion::NAV);
    bare.open(many, top, 0);
    assert_eq!(bare.on_key(&key(KeyCode::End)), (Outcome::Ignored, None));
    assert_eq!(bare.on_key(&key(KeyCode::Home)), (Outcome::Ignored, None));
    assert_eq!(bare.list.cursor.index, 0, "Home and End do not move");
    assert_eq!(bare.on_key(&key(KeyCode::PageDown)).0, Outcome::Changed);
    assert!(bare.list.cursor.index > 0, "paging still moves");
    let _ = paint_hits(screen, |area, buf, ctx| bare.render(area, buf, ctx));
    assert_eq!(bare.area.height, OptionList::MAX_ROWS + 2);
    assert_eq!(bare.area.height, 8 + 2);
    assert_eq!(bare.area.x, top.x - TEXT_INSET);
    assert_eq!(bare.area.x, 10 - 4);
}

fn option_list_items() {
    // Every builder lands in its field, `insertion` falls back to the label.
    let rich = Item::new("Alpha")
        .key(ItemKey::text("a"))
        .glyph("●")
        .detail("first")
        .tag("T")
        .group("G")
        .matched(vec![0, 1])
        .insert("alpha()")
        .value(7);
    assert_eq!(rich.label, "Alpha");
    assert_eq!(rich.key, Some(ItemKey::text("a")));
    assert_eq!((rich.glyph.as_ref(), rich.detail.as_str()), ("●", "first"));
    assert_eq!(rich.tag.as_deref(), Some("T"));
    assert_eq!(rich.group.as_ref(), "G");
    assert_eq!(rich.matched, vec![0, 1]);
    assert_eq!(rich.insertion(), "alpha()");
    assert_eq!(rich.value, 7);
    assert_eq!(Item::new("Beta").insertion(), "Beta");
    let locked = Item::new("Gamma").reason("locked");
    assert!(locked.disabled, "a reason disables the item");
    assert_eq!(locked.reason.as_deref(), Some("locked"));
    assert!(!locked.clone().disabled(false).disabled);
    assert!(Item::new("Delta").disabled(true).disabled);
    let grouped = Item::new("Eps").selection_group(ItemKey::text("g"));
    assert_eq!(grouped.selection_group, Some(ItemKey::text("g")));
    assert_eq!(Item::default().label, "");

    // `AsItem` lets any row type feed the shared row source.
    fn labels<T: AsItem>(rows: &[T]) -> Vec<&str> {
        rows.iter()
            .map(|row| row.as_item().label.as_str())
            .collect()
    }
    let items = vec![rich, Item::new("Beta").disabled(true), locked, grouped];
    assert_eq!(labels(&items), ["Alpha", "Beta", "Gamma", "Eps"]);
    let source = &items[..];
    assert_eq!(RowSource::len(source), 4);
    assert!(source.enabled(0) && !source.enabled(1) && !source.enabled(2));
    assert_eq!(RowSource::label(source, 3), "Eps");
    assert_eq!(RowSource::key(source, 0), ItemKey::text("a"));
    assert_eq!(RowSource::key(source, 1), ItemKey::index(1));

    // The popup list reports the highlighted reason for a disabled row.
    let mut list = OptionList::new(WidgetId::of("scenario.options.status"));
    list.reset(items.len());
    assert_eq!(list.status(&items), None);
    list.select(2, source);
    assert_eq!(list.status(&items), Some("locked"));
    assert_eq!(list.highlighted(), Some(list.cursor.index));
}

fn option_list_navigation() {
    const ID: WidgetId = WidgetId::of("scenario.options.nav");
    assert_eq!(OptionList::MAX_ROWS, 8);
    assert_eq!(OptionList::WIDTH, (12, 40));
    let base = OptionList::new(ID);
    assert_eq!(
        (base.max_rows, base.width),
        (OptionList::MAX_ROWS, OptionList::WIDTH)
    );
    assert_eq!(base.look, CursorStyle::Fill);
    assert_eq!(base.dismiss, Dismiss::LIGHT);
    assert!(base.fade_edges && base.hover_moves_cursor && base.clear_on_leave);

    let mut list = OptionList::new(ID)
        .max_rows(4)
        .width(30, 10)
        .look(CursorStyle::Bar)
        .dismiss(Dismiss::ESCAPE)
        .placement(Placement::Above)
        .fit(Place::FLUSH)
        .nav(NavKeys::VIM)
        .type_ahead(true)
        .type_ahead_timeout(std::time::Duration::from_millis(900))
        .fade_edges(false)
        .hover_moves_cursor(false)
        .clear_on_leave(false)
        .wheel_mode(WheelMode::Cursor)
        .query(EditField::single("q"));
    assert_eq!(
        (list.max_rows, list.width),
        (4, (30, 30)),
        "width keeps max >= min"
    );
    assert_eq!(list.look, CursorStyle::Bar);
    assert_eq!(list.dismiss, Dismiss::ESCAPE);
    assert_eq!((list.placement, list.fit), (Placement::Above, Place::FLUSH));
    assert_eq!(list.nav, NavKeys::VIM);
    assert!(
        list.type_ahead && !list.fade_edges && !list.hover_moves_cursor && !list.clear_on_leave
    );
    assert_eq!(list.wheel, WheelMode::Cursor);
    assert_eq!(list.query.as_ref().map(|q| q.text()), Some("q"));

    // Navigation skips disabled rows and follows the cursor.
    let items: Vec<Item> = [
        "Apple", "Banana", "Cherry", "Damson", "Elder", "Fig", "Grape",
    ]
    .iter()
    .enumerate()
    .map(|(i, name)| Item::new(*name).key(ItemKey::index(i)).disabled(i == 1))
    .collect();
    let src = &items[..];
    list.reset(items.len());
    let area = Rect::new(0, 0, 30, 6);
    let painted = paint_hits(area, |_, buf, ctx| {
        let inner = Rect::new(1, 1, 20, 4);
        list.layout(inner, src, ctx);
        list.paint_rows(
            inner,
            ctx.theme.canvas,
            src,
            buf,
            ctx,
            |i, paint, buf, _| {
                buf.set_string(
                    paint.area.x + 1,
                    paint.area.y,
                    &items[i].label,
                    paint.style(),
                );
            },
        );
    });
    assert_eq!(
        list.rows_area(),
        Rect::new(1, 1, 19, 4),
        "a scrollbar takes one column"
    );
    assert!(rows(&painted.buf)[1].contains("Apple") && rows(&painted.buf)[2].contains("Banana"));
    assert!(list.more_below(items.len()));
    assert_eq!(list.row_at(Position::new(3, 3)), Some(2));
    assert_eq!(list.row_at(Position::new(28, 0)), None);
    assert_eq!(list.key(&key(KeyCode::Down), src), Moved::To(2));
    let mut vim = OptionList::new(ID.sub("vim"))
        .nav(NavKeys::VIM)
        .type_ahead(false);
    vim.reset(items.len());
    assert_eq!(
        vim.key(&key(KeyCode::Char('j')), src),
        Moved::To(2),
        "vim keys step down past the disabled row"
    );
    assert_eq!(vim.key(&key(KeyCode::Char('k')), src), Moved::To(0));
    assert_eq!(
        list.step(-1, src),
        Moved::To(0),
        "stepping up skips the disabled row"
    );
    assert_eq!(list.nav_step(Nav::End, src), Moved::To(6));
    assert_eq!(list.nav_step(Nav::Home, src), Moved::To(0));
    assert_eq!(list.type_char('d', src), Moved::To(3));
    assert_eq!(list.select(5, src), Moved::To(5));
    assert_eq!(list.highlighted(), Some(5));
    assert_eq!(list.select(99, src), Moved::Ignored);
    assert_eq!(list.key(&key(KeyCode::F(5)), src), Moved::Ignored);

    // Marks are tracked by key.
    let a = ItemKey::index(0);
    assert!(!list.is_checked(a));
    list.toggle_checked(a);
    assert!(list.is_checked(a));
    list.toggle_checked(a);
    assert!(!list.is_checked(a));

    // Row hit regions map back to rows and the backdrop is its own region.
    assert_eq!(list.row_id(2), ID.part(OptionList::row_part(2)));
    assert_eq!(list.row_of(list.row_id(2), items.len()), Some(2));
    assert_eq!(list.row_of(ID.sub("other"), items.len()), None);
    let click = painted.event(Phase::Click, ID, Part::ROW, Position::new(3, 3));
    assert_eq!(list.row_of_event(&click), Some(2));
    assert!(!list.on_backdrop(&click));

    // The wheel moves the cursor in `Cursor` mode, the viewport otherwise.
    list.reset(items.len());
    assert!(list.wheel(Axis::Vertical, 2).outcome() == Outcome::Changed);
    assert_eq!(list.cursor.index, 2);
    list.wheel = WheelMode::Viewport;
    list.cursor.index = 0;
    let _ = list.wheel(Axis::Vertical, 1);
    assert_eq!(
        list.cursor.index, 0,
        "the viewport scrolls, the cursor stays"
    );

    // Gaps add blank lines between rows.
    let mut gapped = OptionList::new(ID.sub("gaps"));
    gapped.set_gaps([false, true, false, false]);
    assert_eq!(
        (
            gapped.line_of(0),
            gapped.line_of(1),
            gapped.line_of(2),
            gapped.line_of(3)
        ),
        (0, 1, 3, 4)
    );
    gapped.reset(4);
    let _ = paint_hits(Rect::new(0, 0, 20, 6), |_, _, ctx| {
        gapped.layout(Rect::new(0, 0, 20, 5), &items[..4], ctx);
    });
    let lines: Vec<_> = gapped.visible_lines().collect();
    assert_eq!(
        lines,
        [
            (0, Some(0)),
            (1, Some(1)),
            (2, None),
            (3, Some(2)),
            (4, Some(3))
        ]
    );
}

fn alt(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::ALT,
    }
}

fn fruit() -> Vec<Item> {
    vec![
        Item::new("Apple")
            .key(ItemKey::text("apple"))
            .selection_group(ItemKey::text("pome")),
        Item::new("Banana").key(ItemKey::text("banana")),
        Item::new("Pear")
            .key(ItemKey::text("pear"))
            .selection_group(ItemKey::text("pome")),
    ]
}

fn drawn(picker: &mut Picker, screen: Rect) -> String {
    let painted = paint_hits(screen, |area, buf, ctx| picker.draw(area, buf, ctx));
    rows(&painted.buf).join("\n")
}

fn picker_single_actions() {
    const ID: WidgetId = WidgetId::of("scenario.picker.single");
    let screen = Rect::new(0, 0, 90, 24);
    let mut picker = Picker::new(ID, "Pick fruit")
        .placeholder("Find fruit")
        .hints("Enter Choose")
        .empty_text("Nothing here")
        .delete_is_secondary(true)
        .nav(NavKeys::DESKTOP);
    assert_eq!(PickerKeyProfile::current(), PickerKeyProfile::Desktop);
    picker.set_items(fruit()).expect("keyed items");
    let text = drawn(&mut picker, screen);
    assert!(
        text.contains("Find fruit"),
        "the placeholder shows while the query is empty"
    );
    assert!(text.contains("Enter Choose"), "custom hints show: {text}");
    assert_eq!(picker.list().cursor.index, 0);

    // Each key resolves to its own typed action.
    assert_eq!(picker.on_key(&key(KeyCode::Down)), (Outcome::Changed, None));
    assert_eq!(
        picker.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(PickerAction::Chosen(1)))
    );
    assert_eq!(
        picker.on_key(&alt(KeyCode::Enter)),
        (Outcome::Changed, Some(PickerAction::ChosenAlt(1)))
    );
    assert_eq!(
        picker.on_key(&key(KeyCode::Delete)),
        (Outcome::Changed, Some(PickerAction::Secondary(1)))
    );
    assert_eq!(
        picker.on_key(&key(KeyCode::Tab)),
        (Outcome::Changed, Some(PickerAction::NextScope))
    );
    assert_eq!(
        picker.on_key(&key(KeyCode::BackTab)),
        (Outcome::Changed, Some(PickerAction::PrevScope))
    );
    assert_eq!(
        picker.on_key(&key(KeyCode::Backspace)),
        (Outcome::Changed, Some(PickerAction::Back)),
        "backspace on an empty query steps back"
    );
    // Typing edits the query; Esc clears it first, then cancels.
    assert_eq!(
        picker.on_key(&key(KeyCode::Char('b'))),
        (Outcome::Changed, Some(PickerAction::QueryChanged))
    );
    assert_eq!(picker.query(), "b");
    assert!(drawn(&mut picker, screen).contains(" b"));
    assert_eq!(
        picker.on_key(&key(KeyCode::Esc)),
        (Outcome::Changed, Some(PickerAction::QueryChanged))
    );
    assert_eq!(picker.query(), "");
    assert_eq!(
        picker.on_key(&key(KeyCode::Esc)),
        (Outcome::Changed, Some(PickerAction::Cancelled))
    );
    assert_eq!(picker.on_click_outside(), Some(PickerAction::Cancelled));
    assert_eq!(picker.on_paste("kiwi").1, Some(PickerAction::QueryChanged));
    assert_eq!(picker.query(), "kiwi");

    // Nothing matches: the empty text shows and Enter submits the query.
    picker
        .set_visible_keys(vec![])
        .expect("an empty view is valid");
    assert!(drawn(&mut picker, screen).contains("Nothing here"));
    assert_eq!(
        picker.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(PickerAction::Submit))
    );
    // A custom empty state replaces the plain text.
    let mut custom =
        Picker::new(ID, "Pick fruit").empty(EmptyState::new("No fruit").hint("Add some first"));
    custom.set_items(vec![]).expect("empty items");
    let text = drawn(&mut custom, screen);
    assert!(
        text.contains("No fruit") && !text.contains("No matches"),
        "{text}"
    );

    // With Delete reserved for editing the query it is not a secondary action.
    let mut editing = Picker::new(ID, "Pick fruit").delete_is_secondary(false);
    editing.set_items(fruit()).expect("keyed items");
    assert!(!editing.delete_is_secondary);
    let (_, action) = editing.on_key(&key(KeyCode::Delete));
    assert_eq!(action, None);
    // A list without a search field treats Delete as secondary regardless.
    let mut plain = Picker::new(ID, "Pick fruit").delete_is_secondary(false);
    plain.set_searchable(false).expect("a plain list");
    plain.set_items(fruit()).expect("keyed items");
    assert!(!plain.is_searchable());
    assert_eq!(
        plain.on_key(&key(KeyCode::Delete)),
        (Outcome::Changed, Some(PickerAction::Secondary(0)))
    );
    plain.set_searchable(true).expect("the field comes back");
    assert!(plain.is_searchable());
    assert!(
        !Picker::new(ID, "x")
            .searchable(false)
            .expect("builder")
            .is_searchable()
    );
}

fn picker_multi_selection() {
    const ID: WidgetId = WidgetId::of("scenario.picker.multi");
    // Desktop with a search field toggles on Insert, so Space types.
    let mut desktop = Picker::new(ID, "Pick")
        .key_profile(PickerKeyProfile::Desktop)
        .multi_select(true)
        .expect("multi");
    desktop.set_items(fruit()).expect("keyed items");
    assert!(desktop.is_multi());
    assert_eq!(
        desktop.on_key(&key(KeyCode::Insert)),
        (Outcome::Changed, Some(PickerAction::Toggled))
    );
    assert_eq!(desktop.checked(), &[ItemKey::text("apple")]);
    desktop.set_cursor(2);
    assert_eq!(
        desktop.on_key(&key(KeyCode::Insert)),
        (
            Outcome::Consumed,
            Some(PickerAction::SelectionConflict {
                attempted: ItemKey::text("pear"),
                selected: ItemKey::text("apple"),
            })
        ),
        "two items of one selection group cannot both be checked"
    );
    assert_eq!(desktop.checked(), &[ItemKey::text("apple")]);
    assert_eq!(
        desktop.on_key(&key(KeyCode::Enter)),
        (
            Outcome::Changed,
            Some(PickerAction::ChosenMany(vec![ItemKey::text("apple")]))
        )
    );
    assert_eq!(
        desktop.on_key(&key(KeyCode::Char(' '))),
        (Outcome::Changed, Some(PickerAction::QueryChanged)),
        "Space types into the search field"
    );

    // Without a search field Space toggles.
    let mut plain = Picker::new(ID, "Pick").multi_select(true).expect("multi");
    plain.set_searchable(false).expect("no query");
    plain.set_items(fruit()).expect("items");
    plain.set_cursor(1);
    assert_eq!(
        plain.on_key(&key(KeyCode::Char(' '))),
        (Outcome::Changed, Some(PickerAction::Toggled))
    );
    assert_eq!(plain.checked(), &[ItemKey::text("banana")]);
    // Enabling the search field on a multi picker needs stable keys.
    let mut unkeyed = Picker::new(ID, "Pick").multi_select(true).expect("multi");
    unkeyed.set_searchable(false).expect("no query");
    unkeyed
        .set_items(vec![Item::new("loose")])
        .expect("positional keys are fine without a search field");
    assert_eq!(
        unkeyed.set_searchable(true),
        Err(PickerItemsError::MissingStableKey)
    );
    assert!(!unkeyed.is_searchable());

    // The Vim profile moves the toggle to Ctrl-T.
    let mut vim = Picker::new(ID, "Pick")
        .key_profile(PickerKeyProfile::Vim)
        .multi_select(true)
        .expect("multi");
    vim.set_items(fruit()).expect("items");
    let (_, action) = vim.on_key(&key(KeyCode::Insert));
    assert_ne!(
        action,
        Some(PickerAction::Toggled),
        "Insert no longer toggles"
    );
    assert_eq!(
        vim.on_key(&ctrl('t')),
        (Outcome::Changed, Some(PickerAction::Toggled))
    );
    assert_eq!(vim.checked().len(), 1);
    vim.reset_selection();
    assert!(vim.checked().is_empty());
    assert_ne!(PickerKeyProfile::Vim, PickerKeyProfile::Desktop);
}

fn picker_states_and_chrome() {
    const ID: WidgetId = WidgetId::of("scenario.picker.chrome");
    let screen = Rect::new(0, 0, 90, 24);
    let mut picker = Picker::new(ID, "Pick");
    assert_eq!(picker.status, PickerStatus::Ready);
    picker.set_items(fruit()).expect("items");
    // Loading shows its label and refuses to choose anything.
    picker.status = PickerStatus::Loading("Scanning fruit".into());
    assert!(drawn(&mut picker, screen).contains("Scanning fruit"));
    assert!(picker.current().is_none());
    assert_eq!(picker.on_key(&key(KeyCode::Enter)).1, None);
    // A rejected item list is reported through the error status.
    let dup = vec![
        Item::new("a").key(ItemKey::text("a")),
        Item::new("b").key(ItemKey::text("a")),
    ];
    let error = picker
        .set_items(dup)
        .expect_err("duplicate keys are refused");
    assert_eq!(error, PickerItemsError::DuplicateKey(ItemKey::text("a")));
    picker.report_item_error(error);
    match &picker.status {
        PickerStatus::Error { message, detail } => {
            assert_eq!(message, "Invalid picker items");
            assert!(
                detail
                    .as_deref()
                    .is_some_and(|d| d.contains("DuplicateKey"))
            );
        }
        other => panic!("expected an error status, got {other:?}"),
    }
    assert!(drawn(&mut picker, screen).contains("Invalid picker items"));
    picker
        .set_items(fruit())
        .expect("good items clear the error");
    assert_eq!(picker.status, PickerStatus::Ready);
    assert!(picker.current().is_some());

    // Anchor, fit, backdrop and cursor style.
    let anchor = Rect::new(20, 3, 10, 1);
    let mut anchored = Picker::new(ID, "Pick")
        .anchor(anchor, Placement::Below)
        .fit(Place::FLUSH)
        .backdrop(Backdrop::FULL)
        .cursor_style(CursorStyle::Bar)
        .max_rows(3);
    anchored.set_items(fruit()).expect("items");
    assert_eq!(anchored.anchor, anchor);
    assert_eq!(anchored.backdrop, Backdrop::FULL);
    assert_eq!(anchored.list().look, CursorStyle::Bar);
    assert_eq!(anchored.list().max_rows, 3);
    let _ = drawn(&mut anchored, screen);
    assert!(
        anchored.area.y >= anchor.bottom(),
        "below its anchor: {:?}",
        anchored.area
    );

    // The default backdrop keeps the bottom row, `Backdrop::FULL` dims it too.
    let bottom = |backdrop: Backdrop| {
        let theme = theme();
        let mut buf = Buffer::empty(screen);
        buf.set_string(
            0,
            23,
            "status",
            ratatui::style::Style::new().fg(Color::Rgb(250, 250, 250)),
        );
        let before = buf[(0, 23)].style();
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        let mut p = Picker::new(ID, "Pick").backdrop(backdrop);
        p.set_items(fruit()).expect("items");
        p.draw(screen, &mut buf, &mut ctx);
        ctx.flush_overlays(&mut buf);
        (before, buf[(0, 23)].style())
    };
    let (before, kept) = bottom(Backdrop::DEFAULT);
    assert_eq!(before, kept, "the default backdrop keeps the last row");
    let (before, dimmed) = bottom(Backdrop::FULL);
    assert_ne!(before, dimmed, "a full backdrop dims the last row");
}

fn region_options() -> Options {
    Options::new([
        OptionItem::new("us")
            .label("United States")
            .tone(Tone::Success),
        OptionItem::new("de")
            .label("Germany")
            .description("EU member"),
        OptionItem::new("cn").label("China").disabled("Not enabled"),
        OptionItem::new("jp").label("Japan"),
    ])
}

fn column_editor_model() {
    let cfg = EditorConfig::default();
    let options = region_options();
    // Options and their items.
    assert_eq!((options.len(), options.is_empty()), (4, false));
    assert!(Options::default().is_empty());
    assert_eq!(options.find("jp"), Some(3));
    assert_eq!(options.find("xx"), None);
    let germany = options.get(1).expect("second option");
    assert_eq!(germany.label, "Germany");
    assert_eq!(germany.description.as_deref(), Some("EU member"));
    assert_eq!(options.get(0).and_then(|o| o.tone), Some(Tone::Success));
    assert!(options.get(2).is_some_and(OptionItem::is_disabled));
    assert_eq!(
        options.get(2).and_then(|o| o.disabled.as_deref()),
        Some("Not enabled")
    );
    assert!(!germany.is_disabled());
    assert!(options.get(4).is_none());
    let plain = Options::values(&["a", "b"]);
    assert_eq!(
        plain.get(1).map(|o| (o.value.as_str(), o.label.as_str())),
        Some(("b", "b"))
    );

    // Editors that carry options are constrained; only Text is free.
    assert!(!ColumnEditor::Text.is_constrained());
    assert_eq!(ColumnEditor::default(), ColumnEditor::Text);
    assert_eq!(ColumnEditor::Text.options(), None);
    for editor in [
        ColumnEditor::Select(options.clone()),
        ColumnEditor::Combo(options.clone()),
        ColumnEditor::Radio(options.clone()),
        ColumnEditor::Checkbox(options.clone()),
        ColumnEditor::Segmented(options.clone()),
        ColumnEditor::Tags(options.clone()),
    ] {
        assert_eq!(editor.options(), Some(&options));
        assert!(editor.is_constrained());
        assert_eq!(
            editor.has_popup(),
            !matches!(editor, ColumnEditor::Checkbox(_)),
            "a checkbox flips in place, the rest open a popup"
        );
    }
    assert!(!ColumnEditor::Text.has_popup());

    // The editor is picked by how many options there are.
    let sized = |n: usize| {
        let values: Vec<String> = (0..n).map(|i| format!("v{i}")).collect();
        let refs: Vec<&str> = values.iter().map(String::as_str).collect();
        ColumnEditor::for_options(Options::values(&refs))
    };
    assert!(matches!(sized(2), ColumnEditor::Checkbox(_)));
    assert!(matches!(sized(3), ColumnEditor::Segmented(_)));
    assert!(matches!(sized(5), ColumnEditor::Segmented(_)));
    assert!(matches!(sized(6), ColumnEditor::Select(_)));
    assert!(matches!(sized(15), ColumnEditor::Select(_)));
    assert!(matches!(sized(16), ColumnEditor::Combo(_)));

    // Checkbox values: the first option is off, the second on.
    let toggle = ColumnEditor::Checkbox(Options::values(&["off", "on"]));
    assert_eq!(toggle.checked("on"), Some(true));
    assert_eq!(toggle.checked("off"), Some(false));
    assert_eq!(toggle.checked("other"), None);
    assert_eq!(ColumnEditor::Text.checked("on"), None);
    assert_eq!(toggle.toggled("on").as_deref(), Some("off"));
    assert_eq!(toggle.toggled("off").as_deref(), Some("on"));
    assert_eq!(
        toggle.toggled("junk").as_deref(),
        Some("off"),
        "unknown starts at the first"
    );
    assert_eq!(ColumnEditor::Text.toggled("on"), None);
    let locked = ColumnEditor::Checkbox(Options::new([
        OptionItem::new("off"),
        OptionItem::new("on").disabled("locked"),
    ]));
    assert_eq!(
        locked.toggled("off"),
        None,
        "a disabled target is never toggled to"
    );

    // Display, validity and tone of a raw cell value.
    let select = ColumnEditor::Select(options.clone());
    assert_eq!(select.display("us", &cfg), "United States");
    assert_eq!(select.display("zz", &cfg), "zz");
    assert_eq!(ColumnEditor::Text.display("raw", &cfg), "raw");
    assert!(!select.out_of_set("jp", &cfg));
    assert!(select.out_of_set("zz", &cfg));
    assert!(
        !select.out_of_set("  ", &cfg),
        "a blank cell is not out of set"
    );
    assert!(!ColumnEditor::Text.out_of_set("anything", &cfg));
    assert_eq!(select.tone("us"), Some(Tone::Success));
    assert_eq!(select.tone("de"), None);
    assert_eq!(ColumnEditor::Text.tone("us"), None);
    let tags = ColumnEditor::Tags(options.clone());
    assert_eq!(tags.display("us, de", &cfg), "United States, Germany");
    assert!(tags.out_of_set("us, zz", &cfg));
    assert!(!tags.out_of_set("us, de", &cfg));
    assert_eq!(tags.tone("us"), None, "tags carry no single tone");
    let semi = EditorConfig {
        tag_separator: "; ",
        ..EditorConfig::default()
    };
    assert_eq!(tags.display("us;de", &semi), "United States; Germany");
}

fn editor_for(kind: &str, raw: &str, cfg: &EditorConfig) -> ChoiceEditor {
    let options = region_options();
    let editor = match kind {
        "select" => ColumnEditor::Select(options),
        "combo" => ColumnEditor::Combo(options),
        "radio" => ColumnEditor::Radio(options),
        "tags" => ColumnEditor::Tags(options),
        "segmented" => ColumnEditor::Segmented(options),
        other => panic!("unknown kind {other}"),
    };
    ChoiceEditor::open(WidgetId::of("scenario.choice"), &editor, raw, cfg)
        .expect("a constrained editor opens")
}

fn choice_editor_keys() {
    let cfg = EditorConfig::default();
    assert_eq!((cfg.max_rows, cfg.popup_width), (8, (14, 48)));
    assert_eq!(cfg.affordance, Affordance::Focused);
    assert!(cfg.type_ahead && cfg.flag_out_of_set);
    assert_eq!((cfg.tag_separator, cfg.placement), (", ", Placement::Below));
    assert_eq!(cfg.cursor_style, CursorStyle::Fill);
    assert_eq!(
        (cfg.dismiss, cfg.wheel),
        (Dismiss::LIGHT, WheelMode::Viewport)
    );

    // Select: arrows skip the disabled row, Enter and Space commit.
    let mut select = editor_for("select", "us", &cfg);
    assert_eq!((select.cursor, select.current), (0, Some(0)));
    assert_eq!(select.shown(), vec![0, 1, 2, 3]);
    assert_eq!(select.blur_value(&cfg).as_deref(), Some("us"));
    assert_eq!(
        select.on_key(&key(KeyCode::Down), &cfg),
        (Outcome::Changed, None)
    );
    assert_eq!(select.cursor, 1);
    assert_eq!(
        select.on_key(&key(KeyCode::Down), &cfg),
        (Outcome::Changed, None)
    );
    assert_eq!(select.cursor, 3, "the disabled row is skipped");
    assert_eq!(
        select.on_key(&key(KeyCode::Enter), &cfg),
        (Outcome::Changed, Some(ChoiceResult::Commit("jp".into())))
    );
    assert_eq!(
        select.on_key(&key(KeyCode::Char(' ')), &cfg),
        (Outcome::Changed, Some(ChoiceResult::Commit("jp".into())))
    );
    assert_eq!(
        select.on_key(&key(KeyCode::Tab), &cfg),
        (
            Outcome::Changed,
            Some(ChoiceResult::CommitAndTab {
                value: "jp".into(),
                backward: false
            })
        )
    );
    assert_eq!(
        select.on_key(&key(KeyCode::BackTab), &cfg),
        (
            Outcome::Changed,
            Some(ChoiceResult::CommitAndTab {
                value: "jp".into(),
                backward: true
            })
        )
    );
    assert_eq!(
        select.on_key(&key(KeyCode::Esc), &cfg),
        (Outcome::Changed, Some(ChoiceResult::Cancel))
    );
    let stubborn = EditorConfig {
        dismiss: Dismiss::NEVER,
        ..cfg.clone()
    };
    assert_eq!(
        select.on_key(&key(KeyCode::Esc), &stubborn),
        (Outcome::Consumed, None)
    );
    assert_eq!(
        select.on_paste("anything"),
        Outcome::Consumed,
        "only a combo takes paste"
    );
    // Type-ahead jumps to a label; turning it off disables the jump.
    let mut typed = editor_for("select", "us", &cfg);
    assert_eq!(
        typed.on_key(&key(KeyCode::Char('j')), &cfg).0,
        Outcome::Changed
    );
    assert_eq!(typed.cursor, 3);
    let no_ahead = EditorConfig {
        type_ahead: false,
        ..cfg.clone()
    };
    let mut still = editor_for("select", "us", &no_ahead);
    assert_eq!(
        still.on_key(&key(KeyCode::Char('j')), &no_ahead).0,
        Outcome::Consumed
    );
    assert_eq!(still.cursor, 0);
    // A disabled current value is never committed on blur: the cursor falls
    // back to the first enabled option.
    let disabled_now = editor_for("select", "cn", &cfg);
    assert_eq!(disabled_now.current, Some(2));
    assert_eq!(disabled_now.blur_value(&cfg).as_deref(), Some("us"));

    // Combo: typing filters, paste filters, Enter commits the first match.
    let mut combo = editor_for("combo", "", &cfg);
    assert_eq!(
        combo.on_key(&key(KeyCode::Char('g')), &cfg),
        (Outcome::Changed, None)
    );
    assert_eq!(combo.shown(), vec![1], "only Germany contains a g");
    assert_eq!(
        combo.cursor, 1,
        "the cursor lands on the first enabled match"
    );
    assert_eq!(combo.on_paste(" jap "), Outcome::Changed);
    assert_eq!(combo.query.text(), "gjap");
    assert!(combo.shown().is_empty());
    assert_eq!(
        combo.on_key(&key(KeyCode::Enter), &cfg),
        (Outcome::Consumed, None)
    );
    let mut combo = editor_for("combo", "", &cfg);
    combo.on_paste("jap");
    assert_eq!(
        combo.on_key(&key(KeyCode::Enter), &cfg),
        (Outcome::Changed, Some(ChoiceResult::Commit("jp".into())))
    );

    // Tags: Space toggles marks, Enter commits them joined.
    let mut tags = editor_for("tags", "us, jp", &cfg);
    assert!(tags.selection.contains(0) && tags.selection.contains(3));
    assert_eq!(
        tags.on_key(&key(KeyCode::Down), &cfg),
        (Outcome::Changed, None)
    );
    assert_eq!(
        tags.on_key(&key(KeyCode::Char(' ')), &cfg),
        (Outcome::Changed, None)
    );
    assert!(tags.selection.contains(1));
    assert_eq!(
        tags.on_key(&key(KeyCode::Enter), &cfg),
        (
            Outcome::Changed,
            Some(ChoiceResult::Commit("us, de, jp".into()))
        )
    );

    // Segmented moves sideways and ignores the vertical wheel.
    let mut segmented = editor_for("segmented", "us", &cfg);
    assert_eq!(
        segmented.on_key(&key(KeyCode::Right), &cfg),
        (Outcome::Changed, None)
    );
    assert_eq!(segmented.cursor, 1);
    assert_eq!(
        segmented.wheel(Axis::Vertical, 1).outcome(),
        Outcome::Consumed
    );
    // The wheel moves the cursor in `Cursor` mode.
    let wheel_cfg = EditorConfig {
        wheel: WheelMode::Cursor,
        ..cfg.clone()
    };
    let mut radio = editor_for("radio", "us", &wheel_cfg);
    assert_eq!(radio.wheel(Axis::Vertical, 1).outcome(), Outcome::Changed);
    assert_eq!(radio.cursor, 1);
    assert_eq!(
        radio.wheel(Axis::Horizontal, 1).outcome(),
        Outcome::Consumed
    );
}

fn choice_editor_pointer_and_render() {
    let cfg = EditorConfig {
        max_rows: 3,
        popup_width: (20, 30),
        placement: Placement::Below,
        ..EditorConfig::default()
    };
    let screen = Rect::new(0, 0, 60, 16);
    let cell = Rect::new(8, 2, 12, 1);

    let mut select = editor_for("select", "us", &cfg);
    assert_eq!(
        select.option_id(1),
        select
            .id
            .part(junie_tui::widgets::option_list::OptionList::row_part(1))
    );
    let painted = paint_hits(screen, |_, buf, ctx| select.render(cell, buf, ctx, &cfg));
    let text = rows(&painted.buf).join("\n");
    assert!(
        text.contains("United States") && text.contains("Germany"),
        "{text}"
    );
    assert!(
        text.contains("EU member"),
        "descriptions show beside the label: {text}"
    );
    assert!(
        !text.contains("Japan"),
        "max_rows keeps the fourth row out of view: {text}"
    );
    // Disabled rows show their reason instead of the description.
    let mut roomy = editor_for("select", "us", &EditorConfig::default());
    let painted = paint_hits(screen, |_, buf, ctx| {
        roomy.render(cell, buf, ctx, &EditorConfig::default());
    });
    assert!(rows(&painted.buf).join("\n").contains("Not enabled"));

    // Pointer: rows commit, disabled rows and unknown ids are consumed.
    assert_eq!(
        select.on_click(select.option_id(3)),
        (Outcome::Changed, Some(ChoiceResult::Commit("jp".into())))
    );
    assert_eq!(select.cursor, 3);
    assert_eq!(
        select.on_click(select.option_id(2)),
        (Outcome::Consumed, None)
    );
    assert_eq!(
        select.on_click(WidgetId::of("elsewhere")),
        (Outcome::Consumed, None)
    );
    assert_eq!(
        select.on_click(backdrop_id(select.id)),
        (Outcome::Changed, Some(ChoiceResult::Cancel))
    );
    // Tags toggle on click instead of committing.
    let mut tags = editor_for("tags", "us", &cfg);
    assert_eq!(tags.on_click(tags.option_id(3)), (Outcome::Changed, None));
    assert!(tags.selection.contains(3));
    // The segmented strip renders every label on one line.
    let mut strip = editor_for("segmented", "us", &cfg);
    let painted = paint_hits(screen, |_, buf, ctx| strip.render(cell, buf, ctx, &cfg));
    let line = rows(&painted.buf)
        .into_iter()
        .find(|l| l.contains("Germany"))
        .expect("strip row");
    assert!(
        line.contains("United States") && line.contains("Japan"),
        "{line}"
    );
    // A combo draws its search line above the options.
    let mut combo = editor_for("combo", "", &cfg);
    combo.on_paste("ger");
    let painted = paint_hits(screen, |_, buf, ctx| combo.render(cell, buf, ctx, &cfg));
    let text = rows(&painted.buf).join("\n");
    assert!(
        text.contains("ger") && text.contains("Germany") && !text.contains("Japan"),
        "{text}"
    );
}

fn editor_affordance() {
    let t = theme();
    let marks = |affordance: Affordance, move_right: bool| {
        let columns = vec![
            Column::new("Name", Constraint::Length(10)).editable(),
            Column::new("Owner", Constraint::Length(10)).editor(ColumnEditor::Select(
                Options::values(&["ana", "kai", "mira"]),
            )),
        ];
        let rows_data = vec![
            vec![Cell::new("lint"), Cell::new("ana")],
            vec![Cell::new("build"), Cell::new("kai")],
        ];
        let mut table = DataTable::try_new(WidgetId::of("scenario.affordance"), columns, rows_data)
            .expect("rectangular rows")
            .cell_nav(true)
            .editor_config(EditorConfig {
                affordance,
                ..EditorConfig::default()
            });
        if move_right {
            let _ = table.on_key(&key(KeyCode::Right));
        }
        let area = Rect::new(0, 0, 30, 6);
        let mut buf = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let interaction = Interaction {
            focus: Some(table.id),
            ..Interaction::default()
        };
        let mut ctx = RenderCtx::new(&t, interaction, &mut hits, &mut ring);
        table.render(area, &mut buf, &mut ctx, t.canvas);
        ctx.flush_overlays(&mut buf);
        rows(&buf)
            .join("\n")
            .matches(t.glyphs.disclosure_open)
            .count()
    };
    assert_eq!(
        marks(Affordance::Never, true),
        0,
        "never shows the open mark"
    );
    assert_eq!(
        marks(Affordance::Always, false),
        2,
        "always marks every editor cell"
    );
    assert_eq!(
        marks(Affordance::Focused, true),
        1,
        "focused marks only the cursor cell"
    );
    assert_eq!(
        marks(Affordance::Focused, false),
        0,
        "an editor cell elsewhere stays plain"
    );
}

fn sample_props() -> Vec<Prop> {
    vec![
        Prop::new("Name", "alpha"),
        Prop::new("Path", "/usr/local/share/terminal-components/data").copyable(),
        Prop::new("Status", "healthy").tone(Tone::Success),
        Prop::new("Notes", "a long note that should wrap across several lines").wrap(),
    ]
}

fn props_sheet_layout() {
    let t = theme();
    let props = sample_props();
    let last = &props[3];
    assert!(last.wrap && !last.copyable && props[1].copyable);
    assert_eq!(props[2].tone, Tone::Success);
    assert_eq!(Prop::new("Plain", "v").tone, Tone::Normal);

    // The sheet decides how wide the label column may grow and when a value
    // drops under its label.
    let sheet = Sheet::default();
    assert_eq!((sheet.label_max_share, sheet.value_min), (40, 12));
    let wide = measure(&props, 80);
    let narrow = measure(&props, 30);
    assert!(
        narrow > wide,
        "a narrow sheet needs more rows: {narrow} vs {wide}"
    );
    assert_eq!(measure_with(&props, 30, sheet), narrow);
    let stacked = Sheet {
        label_max_share: 10,
        value_min: 78,
    };
    assert!(
        measure_with(&props, 80, stacked) > wide,
        "a large value minimum stacks values under their labels"
    );
    let natural = props_width(&props);
    assert!(
        natural >= 8 + 12,
        "label column plus the value minimum: {natural}"
    );
    assert_eq!(width_with(&props, sheet), natural);
    assert!(
        width_with(
            &props,
            Sheet {
                value_min: 80,
                ..sheet
            }
        ) > natural
    );
    assert_eq!(props_width(&[]), 0);
    assert_eq!(measure(&[], 40), 0);

    // `render` paints label and value cells and reports the rows used.
    let area = Rect::new(0, 0, 60, 8);
    let used = std::cell::Cell::new(0);
    let buf = frame(area, 0, |area, buf, ctx| {
        used.set(render_props(area, buf, ctx.theme, &props, ctx.theme.canvas));
    });
    assert_eq!(used.get(), measure(&props, 60));
    let text = rows(&buf);
    assert!(
        text[0].starts_with("Name") && text[0].contains("alpha"),
        "{}",
        text[0]
    );
    assert_eq!(buf[(0, 0)].fg, t.text_muted, "labels are muted");
    let status_x = text[2].find("healthy").expect("status value");
    assert_eq!(
        buf[(u16::try_from(status_x).unwrap(), 2)].fg,
        t.success,
        "values take their tone"
    );
    let squeezed = std::cell::Cell::new(0);
    let _ = frame(Rect::new(0, 0, 60, 2), 0, |area, buf, ctx| {
        squeezed.set(render_with(
            area,
            buf,
            ctx.theme,
            &props,
            ctx.theme.canvas,
            stacked,
        ));
    });
    assert_eq!(squeezed.get(), 2, "rows past the area are not drawn");

    // `render_in` scrolls a sheet that is taller than its area.
    let mut view = ScrollView::new(WidgetId::of("scenario.props.view"));
    let height = std::cell::Cell::new(0);
    let buf = frame(Rect::new(0, 0, 30, 3), 0, |area, buf, ctx| {
        height.set(render_in(
            &mut view,
            area,
            buf,
            ctx,
            &props,
            ctx.theme.canvas,
        ));
    });
    assert_eq!(height.get(), 3, "the view is clamped to its area");
    assert!(rows(&buf)[0].starts_with("Name"));
}

fn props_list_interaction() {
    const ID: WidgetId = WidgetId::of("scenario.props.list");
    let area = Rect::new(0, 0, 50, 6);
    let focused = |list: &mut PropsList, tick: u64| {
        let theme = theme();
        let mut buf = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let interaction = Interaction {
            focus: Some(ID),
            tick,
            ..Interaction::default()
        };
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        list.render(area, &mut buf, &mut ctx, theme.canvas);
        ctx.flush_overlays(&mut buf);
        buf
    };
    let mut list = PropsList::new(ID, sample_props())
        .nav(NavKeys::VIM)
        .click(ClickPolicy::NAVIGATION)
        .scrollbar_gap(2)
        .sheet(Sheet {
            label_max_share: 30,
            value_min: 10,
        })
        .reveal(Reveal::OFF.hover(true))
        .track_click(TrackClick::Jump)
        .skip_when_empty(true);
    assert_eq!(list.scrollbar_gap, 2);
    assert_eq!(list.sheet.label_max_share, 30);
    assert_eq!(list.core.nav, NavKeys::VIM);
    assert_eq!(list.core.click, ClickPolicy::NAVIGATION);
    assert_eq!(list.core.scroll.track_click, Some(TrackClick::Jump));
    assert_eq!(
        list.row_id(1),
        ID.part(junie_tui::core::id::PartRef::item(
            Part::ROW,
            ItemKey::index(1)
        ))
    );

    let buf = focused(&mut list, 0);
    let text = rows(&buf);
    assert!(
        text[0].contains("Name") && text[0].contains("alpha"),
        "{}",
        text[0]
    );

    // Keys: navigation, activation, copy.
    assert_eq!(
        list.on_key(&key(KeyCode::Char('j'))),
        (Outcome::Changed, None),
        "vim keys navigate"
    );
    assert_eq!(list.core.cursor, 1);
    assert_eq!(
        list.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(PropsAction::Activate(1)))
    );
    assert_eq!(
        list.on_key(&key(KeyCode::Char('y'))),
        (Outcome::Changed, Some(PropsAction::Copy(1)))
    );
    assert!(
        focused(&mut list, 0)
            .content
            .iter()
            .any(|c| c.symbol() == "y")
    );
    assert_eq!(PropsCmd::Copy.label(), "Copy");
    assert_eq!(
        PropsCmd::default_keymap(),
        vec![(KeyBinding::char('y'), PropsCmd::Copy)]
    );
    list.core.cursor = 0;
    assert_eq!(
        list.on_key(&key(KeyCode::Char('y'))),
        (Outcome::Consumed, None),
        "a prop that is not copyable refuses the copy key"
    );
    let mut custom =
        PropsList::new(ID, sample_props()).keymap(vec![(KeyBinding::char('c'), PropsCmd::Copy)]);
    custom.core.cursor = 1;
    assert_eq!(
        custom.on_key(&key(KeyCode::Char('c'))),
        (Outcome::Changed, Some(PropsAction::Copy(1)))
    );
    assert_eq!(
        custom.on_key(&key(KeyCode::Char('y'))),
        (Outcome::Ignored, None),
        "the default key is remapped away"
    );

    // Clicks follow the click policy.
    assert_eq!(
        list.on_click(2),
        (Outcome::Changed, Some(PropsAction::Activate(2)))
    );
    assert_eq!(list.core.cursor, 2);
    assert_eq!(list.on_click(99), (Outcome::Consumed, None));
    let mut data = PropsList::new(ID, sample_props()).click(ClickPolicy::DATA);
    assert_eq!(
        data.on_click(1),
        (Outcome::Changed, None),
        "data lists need a double click"
    );
    assert_eq!(
        data.on_click_count(1, 2),
        (Outcome::Changed, Some(PropsAction::Activate(1)))
    );

    // Replacing the props clamps the cursor.
    list.core.cursor = 3;
    list.set_props(vec![Prop::new("Only", "one")]);
    assert_eq!(list.core.cursor, 0);
    assert_eq!(list.props.len(), 1);

    // Loading and empty states replace the rows.
    let mut loading =
        PropsList::new(ID, sample_props()).content(ContentState::loading("Reading properties"));
    assert!(
        rows(&focused(&mut loading, 0))
            .join("\n")
            .contains("Reading properties")
    );
    let mut empty =
        PropsList::new(ID, vec![]).empty(EmptyState::new("No properties").hint("Select an item"));
    let text = rows(&focused(&mut empty, 0)).join("\n");
    assert!(
        text.contains("No properties") && text.contains("Select an item"),
        "{text}"
    );
    assert_eq!(empty.on_key(&key(KeyCode::Down)), (Outcome::Ignored, None));
    // An empty list is skipped by Tab unless it asks to stay a focus stop.
    let stops = |skip: bool| {
        let mut list = PropsList::new(ID, vec![]).skip_when_empty(skip);
        let theme = theme();
        let mut buf = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        {
            let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
            list.render(area, &mut buf, &mut ctx, theme.canvas);
        }
        ring.len()
    };
    assert_eq!((stops(true), stops(false)), (0, 1));
}

fn option_list_select_arrows() {
    const ID: WidgetId = WidgetId::of("scenario.options.arrows");
    // `Step` moves the value with the arrows, `Open` opens the popup instead.
    let mut stepping = Select::new(ID, "Region", &["a", "b", "c"], 0).arrows(SelectArrows::Step);
    assert_eq!(stepping.arrows, SelectArrows::Step);
    assert_eq!(
        stepping.on_key(&key(KeyCode::Down)),
        (Outcome::Changed, Some(SelectAction::Changed(1)))
    );
    assert_eq!(stepping.value(), "b");
    assert!(!stepping.open);
    let mut opening = Select::new(ID, "Region", &["a", "b", "c"], 0).arrows(SelectArrows::Open);
    assert_eq!(
        opening.on_key(&key(KeyCode::Down)),
        (Outcome::Changed, None)
    );
    assert!(opening.open, "the arrow opened the popup");
    assert_eq!(opening.value(), "a");
    assert_ne!(SelectArrows::Step, SelectArrows::Open);
}

fn option_list_query_pointer() {
    const ID: WidgetId = WidgetId::of("scenario.options.query");
    let mut field = EditField::single("hello world");
    let line = Rect::new(4, 2, 30, 1);
    let painted = paint_hits(Rect::new(0, 0, 40, 6), |_, buf, ctx| {
        let t = ctx.theme;
        let style = ratatui::style::Style::new().fg(t.text_primary);
        let look = LineLook {
            text: style,
            selection: t.selection(),
            ellipsis: style,
            ellipsis_glyph: t.glyphs.ellipsis,
            underline: None,
            follow_caret: true,
        };
        field.draw_line(buf, line, &look);
        ctx.register_part(ID, PartRef::of(Part::QUERY), line);
        ctx.register_part(ID, OptionList::row_part(0), Rect::new(4, 4, 30, 1));
    });
    let press = painted.event(
        Phase::Press,
        ID,
        Part::QUERY,
        Position::new(line.x + 5, line.y),
    );
    let response = query_pointer(&mut field, &press).expect("a query press is handled");
    assert_eq!(response.outcome(), Outcome::Changed);
    let click = painted.event(
        Phase::Click,
        ID,
        Part::QUERY,
        Position::new(line.x + 5, line.y),
    );
    assert!(query_pointer(&mut field, &click).is_some());
    assert_eq!(
        field.buffer.cursor_offset(),
        5,
        "the click placed the caret"
    );
    let drag = painted.event(
        Phase::Drag,
        ID,
        Part::QUERY,
        Position::new(line.x + 8, line.y),
    );
    assert_eq!(
        query_pointer(&mut field, &drag).map(|r| r.outcome()),
        Some(Outcome::Changed)
    );
    assert_eq!(
        field.buffer.selection().map(|r| r.end.max(r.start)),
        Some(8),
        "the drag selects to the new offset"
    );
    let release = painted.event(
        Phase::Release,
        ID,
        Part::QUERY,
        Position::new(line.x + 8, line.y),
    );
    assert_eq!(
        query_pointer(&mut field, &release).map(|r| r.outcome()),
        Some(Outcome::Ignored),
        "other phases are swallowed without a change"
    );
    // A click on a list row is not a query event.
    let row_click = painted.event(Phase::Click, ID, Part::ROW, Position::new(5, 4));
    assert!(query_pointer(&mut field, &row_click).is_none());
}
