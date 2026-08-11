//! termrock-lookbook: interactive lookbook for shared TUI components.
//!
//! **Architecture Invariant:** T2.
//! Entry point: [`main`] — lookbook binary entry.

mod app;
mod focus;
mod frame;
mod host_frame;
mod interactors;
mod json;
mod knobs;
mod stories;
mod svg;

use std::{ffi::OsStr, io, path::PathBuf};

use app::Lookbook;
use json::json_escape;
use stories::stories;
use svg::{check_svgs, write_story_svgs};
use termrock::{
    input::KeyCode,
    keymap::{KeyBinding, KeyChord, Keymap, Visibility, glyph},
    style::RolePalette,
};

const USAGE: &str = "usage: termrock-lookbook <terminal|list|render|check|frame|export-frames>";

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

static PREVIEW_BINDINGS: &[KeyBinding<PreviewAction>] = &[
    KeyBinding::borrowed(
        &[
            KeyChord::plain(KeyCode::Esc),
            KeyChord::plain(KeyCode::Tab),
            KeyChord::plain(KeyCode::BackTab),
        ],
        PreviewAction::BackToList,
        Some("back to list"),
        Visibility::Shown,
        Some("Esc/⇥"),
    ),
    KeyBinding::borrowed(
        &[
            KeyChord::plain(KeyCode::Up),
            KeyChord::plain(KeyCode::Down),
            KeyChord::plain(KeyCode::Left),
            KeyChord::plain(KeyCode::Right),
        ],
        PreviewAction::Forward,
        Some("interact"),
        Visibility::Shown,
        Some(glyph::ALL_ARROWS),
    ),
    KeyBinding::borrowed(
        &[KeyChord::plain(KeyCode::PageDown)],
        PreviewAction::PageDown,
        Some("page"),
        Visibility::Shown,
        Some(glyph::PGUP_PGDN),
    ),
    KeyBinding::borrowed(
        &[KeyChord::plain(KeyCode::PageUp)],
        PreviewAction::PageUp,
        None,
        Visibility::HiddenAlias,
        None,
    ),
    KeyBinding::borrowed(
        &[KeyChord::plain(KeyCode::Char('J'))],
        PreviewAction::MovePreviewDown,
        Some("move preview"),
        Visibility::Shown,
        Some("J/K"),
    ),
    KeyBinding::borrowed(
        &[KeyChord::plain(KeyCode::Char('K'))],
        PreviewAction::MovePreviewUp,
        None,
        Visibility::HiddenAlias,
        None,
    ),
];
static PREVIEW_KEYMAP: Keymap<PreviewAction> = Keymap::from_static(PREVIEW_BINDINGS);

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

    if first == OsStr::new("export-frames") {
        return cmd_export_frames(args);
    }

    Err(USAGE.into())
}

fn cmd_frame(
    mut args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), Box<dyn std::error::Error>> {
    use frame::{PreviewKey, paint_story_after_keys, paint_story_frame, story_by_id};
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
        paint_story_frame(story, &theme, cols, rows)
    } else {
        paint_story_after_keys(story, &theme, cols, rows, &keys)
    };
    println!("{}", serde_json::to_string(&frame)?);
    Ok(())
}

fn public_widget_components_from_api()
-> Result<std::collections::HashSet<String>, Box<dyn std::error::Error>> {
    use std::collections::HashSet;
    use std::fs;
    // Prefer repo public-api SoT when run from workspace root.
    let candidates = [
        PathBuf::from("docs/api/public-api.txt"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/api/public-api.txt"),
    ];
    let mut text = None;
    for path in candidates {
        if let Ok(body) = fs::read_to_string(&path) {
            text = Some(body);
            break;
        }
    }
    let Some(text) = text else {
        return Ok(HashSet::new());
    };
    let mut set = HashSet::new();
    for line in text.lines() {
        // Match `for termrock::widgets::Foo` / `for &termrock::widgets::Foo`.
        if let Some(idx) = line.find("termrock::widgets::") {
            let rest = &line[idx + "termrock::widgets::".len()..];
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                set.insert(name);
            }
        }
    }
    Ok(set)
}

fn cmd_export_frames(
    mut args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), Box<dyn std::error::Error>> {
    use frame::{
        PreviewKey, paint_story_after_keys, paint_story_frame, preferred_step_key,
        resolve_export_tour, story_by_id,
    };
    use std::fs;
    let usage = "usage: termrock-lookbook export-frames --out <dir> [--story id]* | --all-public";
    let mut out_dir = None;
    let mut only: Vec<String> = Vec::new();
    let mut all_public = false;
    while let Some(flag) = args.next() {
        if flag == OsStr::new("--out") {
            out_dir = args.next().map(PathBuf::from);
        } else if flag == OsStr::new("--story") {
            if let Some(s) = args.next().and_then(|s| s.into_string().ok()) {
                only.push(s);
            }
        } else if flag == OsStr::new("--all-public") {
            all_public = true;
        } else {
            return Err(usage.into());
        }
    }
    let Some(out_dir) = out_dir else {
        return Err(usage.into());
    };
    fs::create_dir_all(&out_dir)?;
    let theme = RolePalette::default();
    // Default: every public-widget lookbook story (docs Ghostty SoT). Override with --story.
    let ids: Vec<String> = if !only.is_empty() {
        only
    } else {
        let public = public_widget_components_from_api()?;
        let mut from_catalog: Vec<String> = stories()
            .into_iter()
            .filter(|s| {
                // --all-public: every story whose component is in public-api.
                // Default (no --story): same — docs Ghostty SoT is the full public set.
                public.is_empty() || public.contains(s.component)
            })
            .map(|s| s.id.to_string())
            .collect();
        // Always include composite tour pack id used by handbook.
        if !from_catalog.iter().any(|id| id == "agent-workbench/basic") {
            from_catalog.push("agent-workbench/basic".into());
        }
        from_catalog.sort();
        from_catalog.dedup();
        if from_catalog.is_empty() {
            return Err(
                "export-frames: no stories resolved (pass --story or run from repo root with docs/api/public-api.txt)"
                    .into(),
            );
        }
        let _ = all_public;
        from_catalog
    };
    use frame::{
        CELL_HEIGHT_PX, CELL_WIDTH_PX, RESPONSIVE_STORY_SIZES, pick_size_key,
        story_size_for_css_host,
    };
    // Compact JSON — docs host does not need pretty-print (large multi-story export).
    fn write_frame_json(
        path: PathBuf,
        value: &impl serde::Serialize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::write(path, serde_json::to_string(value)?)?;
        Ok(())
    }
    let total = ids.len();
    for (index, id) in ids.into_iter().enumerate() {
        let story = story_by_id(&id).ok_or_else(|| format!("unknown story: {id}"))?;
        // Fixed composite tours, auto variant tours for static components, or key steps.
        let tour = resolve_export_tour(&id);
        let step_key = if tour.is_some() {
            None
        } else {
            preferred_step_key(story)
        };
        let interactive = tour.is_some() || step_key.is_some();
        let steps = if let Some(ref tour) = tour {
            tour.len() as u32
        } else if step_key.is_some() {
            6
        } else {
            1
        };
        let slug = id.replace('/', "-");
        let pack = out_dir.join(&slug);
        fs::create_dir_all(&pack)?;
        let mut size_keys: Vec<String> = Vec::new();
        for &(sc, sr) in RESPONSIVE_STORY_SIZES {
            let size_key = format!("{sc}x{sr}");
            size_keys.push(size_key.clone());
            let size_dir = pack.join(&size_key);
            fs::create_dir_all(&size_dir)?;
            if let Some(ref tour) = tour {
                for (step, story_id) in tour.iter().enumerate() {
                    let step_story = story_by_id(story_id)
                        .ok_or_else(|| format!("unknown tour story: {story_id}"))?;
                    let f = paint_story_frame(step_story, &theme, Some(sc), Some(sr));
                    // Force interactive flag so hosts enable input for tour packs.
                    let mut f = f;
                    f.interactive = true;
                    // Keep pack id stable; paint title comes from the scene story.
                    f.story_id = id.clone();
                    write_frame_json(size_dir.join(format!("{step}.json")), &f)?;
                }
            } else {
                // Prefer interactor paint for interactive stories so step 0 matches step graph.
                let base = if step_key.is_some() {
                    paint_story_after_keys(story, &theme, Some(sc), Some(sr), &[])
                } else {
                    paint_story_frame(story, &theme, Some(sc), Some(sr))
                };
                write_frame_json(size_dir.join("0.json"), &base)?;
                if let Some(key) = step_key {
                    for step in 1..=5 {
                        let keys: Vec<PreviewKey> = (0..step)
                            .map(|_| PreviewKey {
                                key: key.into(),
                                ctrl: false,
                                alt: false,
                                shift: false,
                                meta: false,
                            })
                            .collect();
                        let f = paint_story_after_keys(story, &theme, Some(sc), Some(sr), &keys);
                        write_frame_json(size_dir.join(format!("{step}.json")), &f)?;
                    }
                }
            }
        }
        // Default root frames = 40x8 (mid preset) for simple hosts
        let default_key = pick_size_key(40, 8);
        let default_dir = pack.join(&default_key);
        for entry in fs::read_dir(&default_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            // Copy only step JSON into pack root (skip nested size dirs).
            if name_str.ends_with(".json") {
                fs::copy(entry.path(), pack.join(&name))?;
            }
        }
        // Prove helpers are on the export path (host CSS → story size → pack key)
        let (want_c, want_r) = story_size_for_css_host(720, 320);
        let _resolved = pick_size_key(want_c, want_r);
        let manifest = serde_json::json!({
            "storyId": id,
            "title": story.title,
            "component": story.component,
            "interactive": interactive,
            "steps": steps,
            "cellWidthPx": CELL_WIDTH_PX,
            "cellHeightPx": CELL_HEIGHT_PX,
            "sizes": size_keys,
            "defaultSize": default_key,
            "padCells": 1,
            "stepKey": step_key,
            "tour": tour.map(|t| t.to_vec()),
        });
        write_frame_json(pack.join("manifest.json"), &manifest)?;
        eprintln!(
            "[{}/{}] {} (interactive={} steps={})",
            index + 1,
            total,
            pack.display(),
            interactive,
            steps
        );
        println!("{}", pack.display());
    }
    Ok(())
}

fn run_terminal() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = Lookbook::new();
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
