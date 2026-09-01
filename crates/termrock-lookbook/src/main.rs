//! termrock-lookbook: interactive lookbook for shared TUI components.
//!
//! **Architecture Invariant:** T2.
//! Entry point: [`main`] — lookbook binary entry.

mod app;
mod focus;
mod host_frame;
mod svg;

use std::{ffi::OsStr, io, path::PathBuf};

use app::Lookbook;
use svg::{check_svgs, write_story_svgs};
use termrock::{
    input::KeyCode,
    keymap::{KeyBinding, KeyChord, Keymap, Visibility},
    style::RolePalette,
};
use termrock_lookbook::demo::catalog;
use termrock_lookbook::stories::stories;

const USAGE: &str = "usage: termrock-lookbook <terminal|list|inventory|render|render-png|check|frame|export-posters>";

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
    // All component keys are forwarded to the shared demo session.
    Forward,
}

static SIDEBAR_BINDINGS: &[KeyBinding<SidebarAction>] = &[
    KeyBinding::borrowed(
        &[KeyChord::plain(KeyCode::Down), KeyChord::plain(KeyCode::Up)],
        SidebarAction::Navigate,
        Some("navigate"),
        Visibility::Shown,
        Some("↑↓"),
    ),
    KeyBinding::borrowed(
        &[
            KeyChord::plain(KeyCode::Char('j')),
            KeyChord::plain(KeyCode::Char('k')),
        ],
        SidebarAction::Navigate,
        None,
        Visibility::HiddenAlias,
        None,
    ),
    KeyBinding::borrowed(
        &[
            KeyChord::plain(KeyCode::Home),
            KeyChord::plain(KeyCode::End),
        ],
        SidebarAction::GoToEdge,
        Some("first/last"),
        Visibility::Shown,
        Some("Home/End"),
    ),
    KeyBinding::borrowed(
        &[KeyChord::plain(KeyCode::Tab)],
        SidebarAction::FocusPreview,
        Some("focus preview"),
        Visibility::Shown,
        Some("⇥"),
    ),
    KeyBinding::borrowed(
        &[
            KeyChord::plain(KeyCode::Char('q')),
            KeyChord::plain(KeyCode::Esc),
        ],
        SidebarAction::Quit,
        Some("quit"),
        Visibility::Shown,
        Some("q/Esc"),
    ),
];
static SIDEBAR_KEYMAP: Keymap<SidebarAction> = Keymap::from_static(SIDEBAR_BINDINGS);

static PREVIEW_BINDINGS: &[KeyBinding<PreviewAction>] = &[KeyBinding::borrowed(
    &[KeyChord::plain(KeyCode::Esc)],
    PreviewAction::BackToList,
    Some("back to list"),
    Visibility::Shown,
    Some("Esc"),
)];
static PREVIEW_KEYMAP: Keymap<PreviewAction> = Keymap::from_static(PREVIEW_BINDINGS);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let Some(first) = args.next() else {
        return Err(USAGE.into());
    };

    if first == OsStr::new("terminal") {
        let usage = "usage: termrock-lookbook terminal [--story <id>]";
        let mut story = None;
        while let Some(flag) = args.next() {
            if flag == OsStr::new("--story") && story.is_none() {
                let Some(value) = args.next().and_then(|value| value.into_string().ok()) else {
                    return Err(usage.into());
                };
                story = Some(value);
            } else {
                return Err(usage.into());
            }
        }
        return run_terminal(story.as_deref());
    }

    if first == OsStr::new("list") {
        let format = args.next();
        if format.as_deref() == Some(OsStr::new("--format"))
            && args.next().as_deref() == Some(OsStr::new("json"))
            && args.next().is_none()
        {
            println!("{}", serde_json::to_string(&catalog())?);
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

    if first == OsStr::new("inventory") {
        if args.next().as_deref() == Some(OsStr::new("--format"))
            && args.next().as_deref() == Some(OsStr::new("json"))
            && args.next().is_none()
        {
            println!(
                "{}",
                serde_json::to_string(&termrock_lookbook::demo::inventory_catalog()?)?
            );
            return Ok(());
        }
        return Err("usage: termrock-lookbook inventory --format json".into());
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
                    Some(value) if value == OsStr::new("phosphor") => Some(RolePalette::default()),
                    Some(value) if value == OsStr::new("slate") => Some(RolePalette::slate()),
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

    if first == OsStr::new("render-png") {
        let usage = "usage: termrock-lookbook render-png --out <dir>";
        let mut out_dir = None;
        while let Some(flag) = args.next() {
            if flag == OsStr::new("--out") && out_dir.is_none() {
                out_dir = args.next().map(PathBuf::from);
            } else {
                return Err(usage.into());
            }
        }
        let Some(out_dir) = out_dir else {
            return Err(usage.into());
        };
        for path in termrock_lookbook::png::write_story_pngs(out_dir)? {
            println!("{}", path.display());
        }
        return Ok(());
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

    if first == OsStr::new("frame") {
        return cmd_frame(args);
    }

    if first == OsStr::new("export-posters") {
        return cmd_export_posters(args);
    }

    Err(USAGE.into())
}

fn cmd_frame(
    mut args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), Box<dyn std::error::Error>> {
    use termrock_lookbook::frame::{
        PreviewKey, paint_story_after_keys, paint_story_frame, story_by_id,
    };
    let usage = "usage: termrock-lookbook frame --story <id> [--cols N] [--rows N] [--keys k1,k2]";
    let mut story_id = None;
    let mut cols = None;
    let mut rows = None;
    let mut keys_raw = None;
    while let Some(flag) = args.next() {
        if flag == OsStr::new("--story") {
            story_id = args.next().and_then(|s| s.into_string().ok());
        } else if flag == OsStr::new("--cols") {
            cols = args
                .next()
                .and_then(|s| s.into_string().ok())
                .and_then(|s| s.parse().ok());
        } else if flag == OsStr::new("--rows") {
            rows = args
                .next()
                .and_then(|s| s.into_string().ok())
                .and_then(|s| s.parse().ok());
        } else if flag == OsStr::new("--keys") {
            keys_raw = args.next().and_then(|s| s.into_string().ok());
        } else {
            return Err(usage.into());
        }
    }
    let Some(story_id) = story_id else {
        return Err(usage.into());
    };
    let story = story_by_id(&story_id).ok_or_else(|| format!("unknown story: {story_id}"))?;
    let theme = RolePalette::default();
    let system = termrock_lookbook::design::lookbook_system(theme);
    let keys: Vec<PreviewKey> = keys_raw
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|k| PreviewKey {
            key: k.trim().into(),
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        })
        .collect();
    let frame = if keys.is_empty() {
        paint_story_frame(story, &system, cols, rows)
    } else {
        paint_story_after_keys(story, &system, cols, rows, &keys)
    };
    println!("{}", serde_json::to_string(&frame)?);
    Ok(())
}

fn cmd_export_posters(
    mut args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use termrock_lookbook::frame::{paint_story_frame, story_by_id};
    let usage =
        "usage: termrock-lookbook export-posters --out <dir> --story <id> [--story <id> ...]";
    let mut out_dir = None;
    let mut only: Vec<String> = Vec::new();
    while let Some(flag) = args.next() {
        if flag == OsStr::new("--out") {
            out_dir = args.next().map(PathBuf::from);
        } else if flag == OsStr::new("--story") {
            if let Some(s) = args.next().and_then(|s| s.into_string().ok()) {
                only.push(s);
            }
        } else {
            return Err(usage.into());
        }
    }
    let Some(out_dir) = out_dir else {
        return Err(usage.into());
    };
    if only.is_empty() {
        return Err(usage.into());
    }
    fs::create_dir_all(&out_dir)?;
    let theme = RolePalette::default();
    let system = termrock_lookbook::design::lookbook_system(theme);
    only.sort();
    only.dedup();
    let total = only.len();
    for (index, id) in only.into_iter().enumerate() {
        let story = story_by_id(&id).ok_or_else(|| format!("unknown story: {id}"))?;
        let slug = id.replace('/', "-");
        let poster = paint_story_frame(story, &system, Some(story.width), Some(story.height));
        let path = out_dir.join(format!("{slug}.json"));
        fs::write(&path, serde_json::to_string(&poster)?)?;
        eprintln!("[{}/{}] {}", index + 1, total, path.display());
        println!("{}", path.display());
    }
    Ok(())
}

fn run_terminal(story_id: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = if let Some(story_id) = story_id {
        Lookbook::for_story(Some(story_id))?
    } else {
        Lookbook::new()
    };
    termrock::runtime::run(
        &mut app,
        termrock::runtime::RunOptions::default(),
        Lookbook::render_at,
        Lookbook::update_at,
        Lookbook::next_deadline,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests;

fn write_svgs(out_dir: PathBuf, theme: &RolePalette) -> Result<(), Box<dyn std::error::Error>> {
    for path in write_story_svgs(&out_dir, theme)? {
        let mut stdout = io::stdout().lock();
        drop(io::Write::write_fmt(
            &mut stdout,
            format_args!("{}\n", path.display()),
        ));
    }
    Ok(())
}
