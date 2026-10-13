//! termrock-lookbook: interactive lookbook for shared TUI components.
//!
//! **Architecture Invariant:** T2.
//! Entry point: [`main`] — lookbook binary entry.

mod app;
mod interactors;
mod json;
mod knobs;
mod runner;
mod stories;
mod svg;

use std::{ffi::OsStr, io, path::PathBuf, time::Duration};

use app::Lookbook;
use json::json_escape;
use stories::stories;
use svg::{check_svgs, write_story_svgs};
use termrock::{
    Theme,
    input::KeyCode,
    keymap::{KeyBinding, KeyChord, Keymap, Visibility, glyph},
};

const USAGE: &str = "usage: termrock-lookbook <terminal|list|render|check>";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidebarAction {
    /// Up or Down (or j/k); direction resolved from the chord at the dispatch site.
    Navigate,
    /// Home or End; target (first/last) resolved from the chord at dispatch site.
    GoToEdge,
    FocusPreview,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewAction {
    BackToList,
    MovePreviewDown,
    MovePreviewUp,
    PageDown,
    PageUp,
    // Arrow keys and all other keys are forwarded to the active interactor.
    Forward,
}

static SIDEBAR_KEYMAP: Keymap<SidebarAction> = Keymap::new(&[
    KeyBinding {
        chords: &[KeyChord::plain(KeyCode::Down), KeyChord::plain(KeyCode::Up)],
        action: SidebarAction::Navigate,
        hint: Some("navigate"),
        visibility: Visibility::Shown,
        glyph: Some("↑↓"),
    },
    KeyBinding {
        chords: &[
            KeyChord::plain(KeyCode::Char('j')),
            KeyChord::plain(KeyCode::Char('k')),
        ],
        action: SidebarAction::Navigate,
        hint: None,
        visibility: Visibility::HiddenAlias,
        glyph: None,
    },
    KeyBinding {
        chords: &[
            KeyChord::plain(KeyCode::Home),
            KeyChord::plain(KeyCode::End),
        ],
        action: SidebarAction::GoToEdge,
        hint: Some("first/last"),
        visibility: Visibility::Shown,
        glyph: Some("Home/End"),
    },
    KeyBinding {
        chords: &[KeyChord::plain(KeyCode::Tab)],
        action: SidebarAction::FocusPreview,
        hint: Some("focus preview"),
        visibility: Visibility::Shown,
        glyph: Some("⇥"),
    },
    KeyBinding {
        chords: &[
            KeyChord::plain(KeyCode::Char('q')),
            KeyChord::plain(KeyCode::Esc),
        ],
        action: SidebarAction::Quit,
        hint: Some("quit"),
        visibility: Visibility::Shown,
        glyph: Some("q/Esc"),
    },
]);

static PREVIEW_KEYMAP: Keymap<PreviewAction> = Keymap::new(&[
    KeyBinding {
        chords: &[
            KeyChord::plain(KeyCode::Esc),
            KeyChord::plain(KeyCode::Tab),
            KeyChord::plain(KeyCode::BackTab),
        ],
        action: PreviewAction::BackToList,
        hint: Some("back to list"),
        visibility: Visibility::Shown,
        glyph: Some("Esc/⇥"),
    },
    KeyBinding {
        chords: &[
            KeyChord::plain(KeyCode::Up),
            KeyChord::plain(KeyCode::Down),
            KeyChord::plain(KeyCode::Left),
            KeyChord::plain(KeyCode::Right),
        ],
        action: PreviewAction::Forward,
        hint: Some("interact"),
        visibility: Visibility::Shown,
        glyph: Some(glyph::ALL_ARROWS),
    },
    KeyBinding {
        chords: &[KeyChord::plain(KeyCode::PageDown)],
        action: PreviewAction::PageDown,
        hint: Some("page"),
        visibility: Visibility::Shown,
        glyph: Some(glyph::PGUP_PGDN),
    },
    KeyBinding {
        chords: &[KeyChord::plain(KeyCode::PageUp)],
        action: PreviewAction::PageUp,
        hint: None,
        visibility: Visibility::HiddenAlias,
        glyph: None,
    },
    KeyBinding {
        chords: &[KeyChord::plain(KeyCode::Char('J'))],
        action: PreviewAction::MovePreviewDown,
        hint: Some("move preview"),
        visibility: Visibility::Shown,
        glyph: Some("J/K"),
    },
    KeyBinding {
        chords: &[KeyChord::plain(KeyCode::Char('K'))],
        action: PreviewAction::MovePreviewUp,
        hint: None,
        visibility: Visibility::HiddenAlias,
        glyph: None,
    },
]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Sidebar,
    Preview,
    Knobs,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let Some(first) = args.next() else {
        return Err(USAGE.into());
    };

    if first == OsStr::new("terminal") {
        if args.next().is_some() {
            return Err("usage: termrock-lookbook terminal".into());
        }
        return run_terminal();
    }

    if first == OsStr::new("list") {
        let format = args.next();
        if format.as_deref() == Some(OsStr::new("--format"))
            && args.next().as_deref() == Some(OsStr::new("json"))
            && args.next().is_none()
        {
            let entries = stories()
                .iter()
                .map(|story| {
                    format!(
                        r#"{{"id":"{}","title":"{}","component":"{}"}}"#,
                        json_escape(story.id),
                        json_escape(story.title),
                        json_escape(story.component)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            println!("[{entries}]");
            return Ok(());
        }
        if format.is_none() {
            for story in stories() {
                println!("{}\t{}", story.id, story.title);
            }
            return Ok(());
        }
        return Err("usage: termrock-lookbook list [--format json]".into());
    }

    if first == OsStr::new("render") {
        let usage = "usage: termrock-lookbook render [--theme <phosphor|slate>] --out <dir>";
        let mut out_dir = None;
        let mut theme = None;
        while let Some(flag) = args.next() {
            if flag == OsStr::new("--out") && out_dir.is_none() {
                out_dir = args.next().map(PathBuf::from);
            } else if flag == OsStr::new("--theme") && theme.is_none() {
                theme = match args.next().as_deref() {
                    Some(value) if value == OsStr::new("phosphor") => Some(Theme::default()),
                    Some(value) if value == OsStr::new("slate") => Some(Theme::slate()),
                    _ => return Err(usage.into()),
                };
            } else {
                return Err(usage.into());
            }
        }
        let Some(out_dir) = out_dir else {
            return Err(usage.into());
        };
        return write_svgs(out_dir, &theme.unwrap_or_default());
    }

    if first == OsStr::new("check") {
        if args.next().as_deref() != Some(OsStr::new("--dir")) {
            return Err("usage: termrock-lookbook check --dir <dir>".into());
        }
        let Some(dir) = args.next() else {
            return Err("usage: termrock-lookbook check --dir <dir>".into());
        };
        if args.next().is_some() {
            return Err("usage: termrock-lookbook check --dir <dir>".into());
        }
        return check_svgs(PathBuf::from(dir));
    }

    Err(USAGE.into())
}

fn run_terminal() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = Lookbook::new();
    runner::run(
        &mut app,
        Duration::from_millis(120),
        Lookbook::render,
        Lookbook::update,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests;

fn write_svgs(out_dir: PathBuf, theme: &Theme) -> Result<(), Box<dyn std::error::Error>> {
    for path in write_story_svgs(&out_dir, theme)? {
        let mut stdout = io::stdout().lock();
        drop(io::Write::write_fmt(
            &mut stdout,
            format_args!("{}\n", path.display()),
        ));
    }
    Ok(())
}
