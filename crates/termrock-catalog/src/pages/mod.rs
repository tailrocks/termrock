// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Catalog pages. Source prefix first; TermRock extensions after.

pub mod buttons;
pub mod chips;
pub mod dialogs;
pub mod editable;
pub mod editor;
pub mod extras;
pub mod forms;
pub mod grid;
pub mod inputs;
pub mod lists;
pub mod overview;
pub mod panels;
pub mod pickers;
pub mod progress;
pub mod scrolling;
pub mod settings;
pub mod sidebars;
pub mod tablepro;
pub mod tables;
pub mod taskrunner;
pub mod textareas;
pub mod trees;

use crate::catalog::PageId;
use crate::page::Page;

/// Mount the live page for an id.
#[must_use]
pub fn mount(id: PageId) -> Box<dyn Page> {
    match id {
        PageId::OVERVIEW => Box::new(overview::OverviewPage::new()),
        PageId::BUTTONS => Box::new(buttons::ButtonsPage::new()),
        PageId::INPUTS => Box::new(inputs::InputsPage::new()),
        PageId::TEXT_AREAS => Box::new(textareas::TextAreasPage::new()),
        PageId::FORMS => Box::new(forms::FormsPage::new()),
        PageId::LISTS => Box::new(lists::ListsPage::new()),
        PageId::TREES => Box::new(trees::TreesPage::new()),
        PageId::TABLES => Box::new(tables::TablesPage::new()),
        PageId::EDITABLE => Box::new(editable::EditablePage::new()),
        PageId::PANELS => Box::new(panels::PanelsPage::new()),
        PageId::SIDEBARS => Box::new(sidebars::SidebarsPage::new()),
        PageId::DIALOGS => Box::new(dialogs::DialogsPage::new()),
        PageId::PROGRESS => Box::new(progress::ProgressPage::new()),
        PageId::SCROLLING => Box::new(scrolling::ScrollingPage::new()),
        PageId::EDITOR => Box::new(editor::EditorPage::new()),
        PageId::GRID => Box::new(grid::GridPage::new()),
        PageId::CHIPS => Box::new(chips::ChipsPage::new()),
        PageId::PICKERS => Box::new(pickers::PickersPage::new()),
        PageId::SETTINGS => Box::new(settings::SettingsPage::new()),
        PageId::TASK_RUNNER => Box::new(taskrunner::TaskRunnerPage::new()),
        PageId::TABLEPRO => Box::new(tablepro::TableProPage::new()),
        PageId::FEEDBACK => Box::new(extras::ExtrasPage::feedback()),
        PageId::OVERLAYS => Box::new(extras::ExtrasPage::overlays()),
        PageId::CHARTS => Box::new(extras::ExtrasPage::charts()),
        PageId::STRUCTURE => Box::new(extras::ExtrasPage::structure()),
        _ => Box::new(overview::OverviewPage::new()),
    }
}
