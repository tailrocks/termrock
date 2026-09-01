// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Phosphor PNG export for the widget families used by Jackin.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use termrock::style::RolePalette;

use crate::{
    frame::paint_story_buffer,
    stories::{Story, stories},
};

/// The 17 Jackin-used widget families, by exact [`Story::component`] string.
pub const JACKIN_SUBSET_COMPONENTS: [&str; 17] = [
    "ActionBar",
    "Backdrop",
    "ChoiceDialog",
    "DetailTable",
    "Dialog",
    "DiffView",
    "HintBar",
    "List",
    "MessageDialog",
    "Panel",
    "ProgressBar",
    "StatusBar",
    "Tabs",
    "TerminalCellGrid",
    "TextInput",
    "Toast",
    "Viewport",
];

/// Registered stories of the Jackin-used families, sorted by id.
#[must_use]
pub fn subset_stories() -> Vec<Story> {
    let mut subset: Vec<_> = stories()
        .into_iter()
        .filter(|story| JACKIN_SUBSET_COMPONENTS.contains(&story.component()))
        .collect();
    subset.sort_unstable_by_key(|story| story.id);
    subset
}

/// Canonical PNG baseline filename, matching the SVG filename scheme.
#[must_use]
pub fn story_png_filename(story: Story) -> String {
    format!("{}.png", story.id.replace('/', "-"))
}

/// Render one story at registered geometry using the phosphor palette.
#[must_use]
pub fn render_story_png(story: Story) -> Vec<u8> {
    let palette = RolePalette::default();
    let system = crate::design::lookbook_system(palette.clone());
    let buffer = paint_story_buffer(story, &system, None, None);
    termrock_raster::render_png(&buffer, &palette).expect("registered story must rasterize")
}

/// Write all current Jackin-subset baselines into `out_dir`.
pub fn write_story_pngs(out_dir: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)?;
    subset_stories()
        .into_iter()
        .map(|story| {
            let path = out_dir.join(story_png_filename(story));
            fs::write(&path, render_story_png(story))?;
            Ok(path)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::frame::story_by_id;

    #[test]
    fn subset_filter_matches_component_field_not_id_prefix() {
        let subset = subset_stories();
        let components: BTreeSet<_> = subset.iter().map(|story| story.component()).collect();
        assert_eq!(components, JACKIN_SUBSET_COMPONENTS.into_iter().collect());

        let ids: BTreeSet<_> = subset.iter().map(|story| story.id).collect();
        for id in [
            "capability/ascii-glyphs",
            "panel-stack/omission",
            "list/in-app",
            "panel/in-app",
            "status-bar/in-app",
            "tabs/in-app",
            "detail-table/in-app",
            "diff-view/in-app",
            "toast/in-app",
            "progress-bar/in-app",
            "text-input/in-app",
            "dialog/in-app",
        ] {
            assert!(ids.contains(id), "missing subset story {id}");
        }
        for excluded in [
            "Progress",
            "VirtualList",
            "AlertDialog",
            "NavigationList",
            "KeyValueList",
        ] {
            assert!(!subset.iter().any(|story| story.component() == excluded));
        }
    }

    #[test]
    fn png_filename_mirrors_svg_scheme() {
        let story = story_by_id("panel/focused").expect("panel story");
        assert_eq!(story_png_filename(story), "panel-focused.png");
    }

    #[test]
    fn render_story_png_is_reproducible_and_nonempty() {
        let story = story_by_id("panel/focused").expect("panel story");
        let first = render_story_png(story);
        let second = render_story_png(story);
        assert!(!first.is_empty());
        assert_eq!(first, second);
        assert_eq!(&first[..4], &[0x89, b'P', b'N', b'G']);
    }
}
