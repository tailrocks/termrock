// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Lookbook stories that copy junie showcase Overview / Settings / Task runner
//! page anatomy from public TermRock APIs. Crops match `verify/junie`.
#![allow(unused_imports)]
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::StatefulWidget,
};
use termrock::{
    registry::PublicUiId,
    style::{ControlState, DesignSystem, Role},
    text::{display_cols, truncate_cols},
    widgets::{
        Button, ButtonState, ButtonVariant, RadioGroup, RadioOption, RadioState, Tab, Tabs,
        TabsState, TextInput, TextInputState, Toggle, ToggleState, ToggleValue, Tree, TreeNode,
        TreeState,
    },
};

use crate::stories::{Story, StoryIdentity};

/// Stories that copy junie Overview / Settings / Task runner page anatomy.
pub fn stories() -> Vec<Story> {
    vec![
        Story::new(
            "overview/tokens",
            "Junie overview tokens",
            StoryIdentity::PublicUi(PublicUiId::KeyValueList),
            "Tokens card from the Overview page: swatch, name, note.",
            42,
            21,
            overview_tokens_story,
        ),
        Story::new(
            "settings/general",
            "Junie project settings general",
            StoryIdentity::PublicUi(PublicUiId::Form),
            "Settings General: tabs, project name, visibility, switches, save.",
            90,
            18,
            settings_general_story,
        ),
        Story::new(
            "taskrunner/targets",
            "Junie task runner targets",
            StoryIdentity::PublicUi(PublicUiId::Tree),
            "Task runner Targets tree: payments-gateway and shared-libs.",
            32,
            18,
            taskrunner_targets_story,
        ),
    ]
}

fn fill_surface(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    frame
        .buffer_mut()
        .set_style(area, system.style(Role::Surface));
}

fn overview_tokens_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    fill_surface(frame, area, system);
    let theme = system.junie_theme();
    let bg = theme.surface;
    let buffer = frame.buffer_mut();
    buffer.set_stringn(
        area.x,
        area.y,
        "Tokens",
        usize::from(area.width),
        theme.secondary().bg(bg),
    );
    if area.height < 3 {
        return;
    }
    let inner = Rect::new(
        area.x,
        area.y.saturating_add(2),
        area.width,
        area.height.saturating_sub(2),
    );
    let info = Color::Rgb(0x87, 0x87, 0xff);
    let tokens: [(&str, Color, &str); 19] = [
        ("canvas", theme.canvas, "#000000"),
        ("surface", theme.surface, "#111111"),
        ("surface.elevated", theme.surface_elevated, "#18181b"),
        ("surface.overlay", theme.surface_overlay, "#27272a"),
        ("field", theme.field, "#1e1e22"),
        ("popover", theme.popover, "#3f3f46"),
        ("border.subtle", theme.border_subtle, "white 15%"),
        ("border.strong", theme.border_strong, "white 30%"),
        ("text.primary", theme.text_primary, "#ffffff"),
        ("text.secondary", theme.text_secondary, "white 70%"),
        ("text.muted", theme.text_muted, "white 50%"),
        ("text.faint", theme.text_faint, "white 30%"),
        ("accent", theme.accent, "#48e054"),
        ("accent.hover", theme.accent_hover, "#3ab343"),
        ("accent.pressed", theme.accent_pressed, "#2b8632"),
        ("accent.bg", theme.accent_bg, "green 20%"),
        ("error", theme.error, "#e44545"),
        ("warning", theme.warning, "#f59e09"),
        ("info", info, "#8787ff"),
    ];
    let two_col = (inner.height as usize) < tokens.len() && inner.width >= 44;
    let per_col = if two_col {
        tokens.len().div_ceil(2)
    } else {
        tokens.len()
    };
    let col_w = if two_col {
        inner.width / 2
    } else {
        inner.width
    };
    for (i, (name, color, note)) in tokens.iter().enumerate() {
        let col = (i / per_col) as u16;
        let y = inner.y + (i % per_col) as u16;
        if y >= inner.bottom() {
            continue;
        }
        let x = inner.x + col * col_w;
        buffer.set_style(Rect::new(x, y, 4.min(col_w), 1), Style::new().bg(*color));
        if col_w > 4 {
            buffer.set_stringn(x + 4, y, "▏", 1, theme.faint().bg(bg));
        }
        if col_w > 6 {
            buffer.set_stringn(
                x + 6,
                y,
                name,
                usize::from(col_w.saturating_sub(7)),
                theme.primary().bg(bg),
            );
        }
        let nw = display_cols(note) as u16;
        if col_w > 30 && nw + 1 < col_w {
            buffer.set_stringn(
                x + col_w.saturating_sub(nw + 1),
                y,
                note,
                usize::from(nw),
                theme.muted().bg(bg),
            );
        }
    }
}

fn settings_general_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    let theme = system.junie_theme();
    // Tabs sit on canvas; the General card starts two rows down on surface.
    frame
        .buffer_mut()
        .set_style(area, system.style(Role::Canvas));
    if area.height > 2 {
        frame.buffer_mut().set_style(
            Rect::new(
                area.x,
                area.y.saturating_add(2),
                area.width,
                area.height.saturating_sub(2),
            ),
            system.style(Role::Surface),
        );
    }
    let bg = theme.surface;
    let tabs = [
        Tab::new("general", "General"),
        Tab::new("members", "Members"),
        Tab::new("env", "Environment"),
    ];
    let mut tabs_state = TabsState::new().with_selected("general");
    Tabs::new(&tabs, system).show_close(false).paint(
        Rect::new(area.x, area.y, area.width, 2.min(area.height)),
        frame.buffer_mut(),
        &mut tabs_state,
    );
    if area.height > 1 {
        // junie: gutter cell `─`, eight `━` under General, `─` for the rest.
        let y = area.y + 1;
        let buffer = frame.buffer_mut();
        let faint = theme.border(false).bg(theme.canvas);
        let strong = theme.accent_fg().bg(theme.canvas);
        for x in area.x..area.right() {
            let rel = x.saturating_sub(area.x);
            let (ch, st) = if (1..9).contains(&rel) {
                (system.glyphs.rule_strong(), strong)
            } else {
                (system.glyphs.rule(), faint)
            };
            buffer.set_stringn(x, y, ch, 1, st);
        }
    }
    if area.height < 4 {
        return;
    }
    frame.buffer_mut().set_stringn(
        area.x.saturating_add(2),
        area.y.saturating_add(3),
        "General",
        usize::from(area.width.saturating_sub(2)),
        theme.secondary().bg(bg),
    );
    let right_x = if area.width >= 80 {
        area.x.saturating_add(49)
    } else {
        area.x.saturating_add(31)
    };
    if area.height > 5 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(4),
            area.y.saturating_add(5),
            "Project name ",
            13,
            theme.secondary().bg(bg),
        );
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(17),
            area.y.saturating_add(5),
            "*",
            1,
            theme.accent_fg().bg(bg),
        );
        if right_x < area.right() {
            frame.buffer_mut().set_stringn(
                right_x.saturating_add(2),
                area.y.saturating_add(5),
                "Visibility",
                10,
                theme.secondary().bg(bg),
            );
        }
    }
    if area.height > 6 {
        let mut name = TextInputState::new("payments-gateway");
        name.set_editing(false);
        let field_w = right_x
            .saturating_sub(area.x.saturating_add(6))
            .min(area.right().saturating_sub(area.x.saturating_add(2)));
        let _ = TextInput::new("", system).paint(
            Rect::new(
                area.x.saturating_add(2),
                area.y.saturating_add(6),
                field_w.max(1),
                1,
            ),
            frame.buffer_mut(),
            &mut name,
        );
    }
    if area.height > 8 {
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(4),
            area.y.saturating_add(8),
            "Description",
            11,
            theme.secondary().bg(bg),
        );
    }
    if right_x < area.right() && area.height > 9 {
        let options = [
            RadioOption::new("private", "Private"),
            RadioOption::new("internal", "Internal"),
            RadioOption::new("public", "Public"),
        ];
        let mut radios = RadioState::new(Some("private"));
        radios.set_surface_focused(false);
        let radio_h = 3.min(area.height.saturating_sub(6));
        let _ = RadioGroup::new(&options, system).paint(
            Rect::new(
                right_x,
                area.y.saturating_add(6),
                area.right().saturating_sub(right_x),
                radio_h,
            ),
            frame.buffer_mut(),
            &mut radios,
        );
    }
    if area.height > 11 {
        let recipe = system.input_recipe(ControlState::Default, false, false);
        let desc_w = right_x
            .saturating_sub(area.x.saturating_add(6))
            .min(area.width.saturating_sub(2));
        let copy = "Handles checkout, invoicing and refunds for the storefront.";
        for i in 0..3u16 {
            let y = area.y.saturating_add(9).saturating_add(i);
            if y >= area.bottom() {
                break;
            }
            let row = Rect::new(area.x.saturating_add(2), y, desc_w.max(1), 1);
            frame.buffer_mut().set_style(row, recipe.fill);
            if let Some((glyph, style)) = recipe.prompt {
                frame
                    .buffer_mut()
                    .set_stringn(row.x, row.y, glyph, 1, style);
            }
            if i == 0 && row.width > 2 {
                let budget = usize::from(right_x.saturating_sub(area.x.saturating_add(10)).max(1));
                let ellipsis = system.glyphs.ellipsis();
                let text = truncate_cols(copy, budget, ellipsis);
                let tx = row.x.saturating_add(2);
                frame
                    .buffer_mut()
                    .set_stringn(tx, row.y, text.as_ref(), budget, recipe.value);
                if text.ends_with(ellipsis) {
                    let ex = tx
                        .saturating_add(display_cols(text.as_ref()) as u16)
                        .saturating_sub(display_cols(ellipsis) as u16);
                    frame.buffer_mut().set_stringn(
                        ex,
                        row.y,
                        ellipsis,
                        display_cols(ellipsis),
                        theme.muted().bg(recipe.fill.bg.unwrap_or(bg)),
                    );
                }
            }
        }
    }
    if right_x < area.right() && area.height > 11 {
        let mut off = ToggleState::new();
        let _ = Toggle::new("Auto-merge approved PRs", system).paint(
            Rect::new(
                right_x,
                area.y.saturating_add(10),
                area.right().saturating_sub(right_x),
                1,
            ),
            frame.buffer_mut(),
            &mut off,
        );
        let mut on = ToggleState::with_value(ToggleValue::Pressed);
        let _ = Toggle::new("Protect main branch", system).paint(
            Rect::new(
                right_x,
                area.y.saturating_add(11),
                area.right().saturating_sub(right_x),
                1,
            ),
            frame.buffer_mut(),
            &mut on,
        );
    }
    if area.height > 1 {
        let y = area.bottom().saturating_sub(1);
        let mut save = ButtonState::new();
        let btn = Button::new("Save changes", system)
            .variant(ButtonVariant::Primary)
            .container(bg);
        let w = btn.preferred_width().min(area.width);
        let _ = btn.paint(
            Rect::new(area.x.saturating_add(2), y, w, 1),
            frame.buffer_mut(),
            &mut save,
        );
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(18),
            y,
            "No changes",
            10,
            theme.faint().bg(bg),
        );
    }
}

fn taskrunner_targets_story(frame: &mut Frame<'_>, area: Rect, system: &DesignSystem) {
    fill_surface(frame, area, system);
    let theme = system.junie_theme();
    frame.buffer_mut().set_stringn(
        area.x,
        area.y,
        "Targets",
        usize::from(area.width),
        theme.secondary().bg(theme.surface),
    );
    if area.height < 3 {
        return;
    }
    let nodes = [
        TreeNode::new("pg", Line::from("payments-gateway"), 0)
            .branch()
            .expanded(),
        TreeNode::new("build", Line::from("build"), 1)
            .branch()
            .expanded(),
        TreeNode::new("compile", Line::from("compile"), 2),
        TreeNode::new("lint", Line::from("lint"), 2),
        TreeNode::new("typecheck", Line::from("typecheck"), 2),
        TreeNode::new("test", Line::from("test"), 1)
            .branch()
            .expanded(),
        TreeNode::new("unit", Line::from("unit"), 2),
        TreeNode::new("integration", Line::from("integration"), 2),
        TreeNode::new("e2e", Line::from("e2e"), 2),
        TreeNode::new("deploy", Line::from("deploy"), 1)
            .branch()
            .expanded(),
        TreeNode::new("staging", Line::from("staging"), 2),
        TreeNode::new("production", Line::from("production"), 2),
        TreeNode::new("libs", Line::from("shared-libs"), 0)
            .branch()
            .expanded(),
        TreeNode::new("scompile", Line::from("compile"), 1),
        TreeNode::new("publish", Line::from("publish"), 1),
    ];
    let mut state = TreeState::<&str>::new(None);
    frame.render_stateful_widget(
        &Tree::new(&nodes, system).focused(false),
        Rect::new(
            area.x,
            area.y.saturating_add(2),
            area.width,
            area.height.saturating_sub(2),
        ),
        &mut state,
    );
}
