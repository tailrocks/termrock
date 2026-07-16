//! Public harness metadata for TermRock component stories.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Catalog metadata for one deterministic component story.
pub struct StoryMetadata {
    /// Stable story identifier used by previews and documentation.
    pub id: &'static str,
    /// Human-readable story title.
    pub title: &'static str,
    /// Public TermRock component exercised by the story.
    pub component: &'static str,
}

/// Generated inventory of every catalog story.
pub const STORIES: &[StoryMetadata] = &[
    StoryMetadata {
        id: "panel/focused",
        title: "Focused panel",
        component: "Panel",
    },
    StoryMetadata {
        id: "action-bar/basic",
        title: "Action bar",
        component: "ActionBar",
    },
    StoryMetadata {
        id: "tabs/status",
        title: "Tabs",
        component: "Tabs",
    },
    StoryMetadata {
        id: "hint-bar/wrapped",
        title: "Hint bar",
        component: "HintBar",
    },
    StoryMetadata {
        id: "list/selection",
        title: "List",
        component: "List",
    },
    StoryMetadata {
        id: "tree/navigation",
        title: "Tree navigation",
        component: "Tree",
    },
    StoryMetadata {
        id: "form/responsive",
        title: "Responsive form",
        component: "Form",
    },
    StoryMetadata {
        id: "split-pane/horizontal",
        title: "Horizontal split pane",
        component: "SplitPane",
    },
    StoryMetadata {
        id: "text-input/filter",
        title: "Filter composition",
        component: "TextInput",
    },
    StoryMetadata {
        id: "detail-table/basic",
        title: "Detail table",
        component: "DetailTable",
    },
    StoryMetadata {
        id: "status-bar/basic",
        title: "Status bar",
        component: "StatusBar",
    },
    StoryMetadata {
        id: "dialog/message",
        title: "Message dialog",
        component: "Dialog",
    },
    StoryMetadata {
        id: "choice-dialog/basic",
        title: "Choice dialog",
        component: "ChoiceDialog",
    },
    StoryMetadata {
        id: "message-dialog/details",
        title: "Detailed message dialog",
        component: "MessageDialog",
    },
    StoryMetadata {
        id: "diff/basic",
        title: "Diff view",
        component: "DiffView",
    },
    StoryMetadata {
        id: "toast/success",
        title: "Toast",
        component: "Toast",
    },
    StoryMetadata {
        id: "backdrop/basic",
        title: "Backdrop",
        component: "Backdrop",
    },
    StoryMetadata {
        id: "viewport/both-axes",
        title: "Scrollable viewport",
        component: "Viewport",
    },
];

#[must_use]
/// Returns the generated catalog story inventory.
#[must_use]
pub fn stories() -> &'static [StoryMetadata] {
    STORIES
}
