// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Responsive layout: priority tiers, contraction stages, adaptive anatomy.
//!
//! Responsive TUI design is **not** “truncate every label.” Components declare
//! content priority and contraction strategies; primary labels and primary
//! actions outlive decorative and secondary information.
//!
//! # Progression (narrower ⇒ higher stage)
//!
//! 1. [`ContractionStage::Full`] — full anatomy  
//! 2. [`ContractionStage::CompactSpacing`] — tighter density  
//! 3. [`ContractionStage::ShortenSecondary`] — short secondary labels  
//! 4. [`ContractionStage::HideOptionalMeta`] — drop low-priority metadata  
//! 5. [`ContractionStage::CollapseSecondaryActions`] — hide secondary actions  
//! 6. [`ContractionStage::SinglePane`] — one primary pane  
//! 7. [`ContractionStage::DrawerOrOverlay`] — secondary → drawer/overlay  
//! 8. [`ContractionStage::LineMode`] — tiny-terminal / line fallback  
//!
//! Pair with [`crate::style::Density`] for spacing tokens and with
//! [`crate::layout::Workspace`] for multi-pane collapse.

use crate::style::Density;

/// Canonical width samples for responsive test matrices (cells).
pub const WIDTH_LADDER: [u16; 7] = [160, 120, 100, 80, 60, 40, 20];

/// Semantic priority of a content part (survival order under pressure).
///
/// Higher priority survives longer. Drop order under contraction is
/// Decorative → Optional → Important → Essential (last).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[non_exhaustive]
pub enum ContentPriority {
    /// Chrome flourish only — drop first.
    Decorative = 0,
    /// Optional metadata, badges, shortcuts, tertiary columns.
    #[default]
    Optional = 1,
    /// Important secondary labels, key columns, secondary panes.
    Important = 2,
    /// Primary labels and primary actions — survive longest.
    Essential = 3,
}

impl ContentPriority {
    /// Whether this tier is visible at the given contraction stage.
    #[must_use]
    pub const fn visible_at(self, stage: ContractionStage) -> bool {
        match self {
            Self::Essential => true,
            Self::Important => !matches!(stage, ContractionStage::LineMode),
            Self::Optional => matches!(
                stage,
                ContractionStage::Full
                    | ContractionStage::CompactSpacing
                    | ContractionStage::ShortenSecondary
            ),
            Self::Decorative => matches!(
                stage,
                ContractionStage::Full | ContractionStage::CompactSpacing
            ),
        }
    }
}

/// Progressive contraction stage for a viewport width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[non_exhaustive]
pub enum ContractionStage {
    /// Full anatomy (wide terminals).
    #[default]
    Full = 0,
    /// Compact spacing; keep structure.
    CompactSpacing = 1,
    /// Shorten secondary labels; keep optional meta when possible.
    ShortenSecondary = 2,
    /// Hide low-priority metadata / badges / shortcuts.
    HideOptionalMeta = 3,
    /// Collapse secondary actions into overflow menus / omit.
    CollapseSecondaryActions = 4,
    /// Single primary pane; secondary regions collapse.
    SinglePane = 5,
    /// Secondary content as drawer or overlay instead of docked pane.
    DrawerOrOverlay = 6,
    /// Tiny-terminal line-mode / essential-only fallback.
    LineMode = 7,
}

impl ContractionStage {
    /// Global default stage bands by terminal width (cells).
    ///
    /// Surfaces may override via [`SurfaceResponsivePolicy::stage_for_width`].
    #[must_use]
    pub const fn from_width(width: u16) -> Self {
        match width {
            0..=24 => Self::LineMode,
            25..=40 => Self::DrawerOrOverlay,
            41..=60 => Self::SinglePane,
            61..=80 => Self::CollapseSecondaryActions,
            81..=100 => Self::HideOptionalMeta,
            101..=120 => Self::ShortenSecondary,
            121..=159 => Self::CompactSpacing,
            _ => Self::Full,
        }
    }

    /// Suggested design density for this stage.
    #[must_use]
    pub const fn suggested_density(self) -> Density {
        match self {
            Self::Full => Density::Comfortable,
            Self::CompactSpacing | Self::ShortenSecondary => Density::Compact,
            Self::HideOptionalMeta
            | Self::CollapseSecondaryActions
            | Self::SinglePane
            | Self::DrawerOrOverlay
            | Self::LineMode => Density::Dashboard,
        }
    }

    /// Whether secondary labels should be abbreviated.
    #[must_use]
    pub const fn shorten_secondary(self) -> bool {
        (self as u8) >= (Self::ShortenSecondary as u8)
    }

    /// Human-readable stage name for tests/docs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::CompactSpacing => "compact-spacing",
            Self::ShortenSecondary => "shorten-secondary",
            Self::HideOptionalMeta => "hide-optional-meta",
            Self::CollapseSecondaryActions => "collapse-secondary-actions",
            Self::SinglePane => "single-pane",
            Self::DrawerOrOverlay => "drawer-or-overlay",
            Self::LineMode => "line-mode",
        }
    }
}

/// Overflow when content exceeds the remaining budget after contraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum OverflowBehavior {
    /// Grapheme-safe ellipsis at the end (default for labels).
    #[default]
    Ellipsis,
    /// Hard clip without ellipsis (rare; status meters).
    Clip,
    /// Scroll within the allocated region.
    Scroll,
    /// Wrap to additional rows when height allows.
    Wrap,
    /// Hide the entire part (used for optional chrome).
    Hide,
}

/// Preferred / minimum / max size along one axis (cells).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SizeBudget {
    /// Preferred size under full anatomy.
    pub preferred: u16,
    /// Minimum usable size before the surface should switch strategy
    /// (drawer, line-mode, or hide).
    pub min_usable: u16,
    /// Soft maximum (0 = unbounded by policy).
    pub max: u16,
}

impl SizeBudget {
    /// Fixed preferred = min usable = max.
    #[must_use]
    pub const fn fixed(cells: u16) -> Self {
        Self {
            preferred: cells,
            min_usable: cells,
            max: cells,
        }
    }

    /// Preferred with a smaller usable floor (unbounded max).
    #[must_use]
    pub const fn range(preferred: u16, min_usable: u16) -> Self {
        Self {
            preferred,
            min_usable,
            max: 0,
        }
    }

    /// Clamp a requested size into this budget against an available span.
    #[must_use]
    pub const fn resolve(self, available: u16) -> u16 {
        let mut size = self.preferred;
        if self.max > 0 && size > self.max {
            size = self.max;
        }
        if size > available {
            size = available;
        }
        if size < self.min_usable && available >= self.min_usable {
            size = if self.min_usable < available {
                self.min_usable
            } else {
                available
            };
        }
        size
    }

    /// Whether `available` is below the minimum usable size.
    #[must_use]
    pub const fn below_min_usable(self, available: u16) -> bool {
        available < self.min_usable
    }
}

/// Which anatomy parts remain visible after contraction.
///
/// Primary labels and primary actions map to `essential` and always remain
/// true while the surface is shown at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AdaptiveAnatomy {
    /// Essential content (primary labels / primary actions).
    pub essential: bool,
    /// Important secondary content (key columns, secondary labels).
    pub important: bool,
    /// Optional metadata (badges, shortcuts, tertiary columns).
    pub optional_meta: bool,
    /// Secondary actions (non-primary buttons, extra toolbars).
    pub secondary_actions: bool,
    /// Use full (unabbreviated) secondary labels.
    pub full_secondary_labels: bool,
    /// Multi-pane / multi-column layout still allowed.
    pub multi_pane: bool,
    /// Present secondary region as drawer or overlay instead of docked.
    pub use_drawer: bool,
    /// Tiny-terminal single-line / essential-only mode.
    pub line_mode: bool,
    /// Suggested density for spacing tokens.
    pub density: Density,
    /// Default overflow for clipped essential labels.
    pub overflow: OverflowBehavior,
}

impl AdaptiveAnatomy {
    /// Anatomy for a global stage (baseline before surface overrides).
    #[must_use]
    pub const fn from_stage(stage: ContractionStage) -> Self {
        match stage {
            ContractionStage::Full => Self {
                essential: true,
                important: true,
                optional_meta: true,
                secondary_actions: true,
                full_secondary_labels: true,
                multi_pane: true,
                use_drawer: false,
                line_mode: false,
                density: Density::Comfortable,
                overflow: OverflowBehavior::Ellipsis,
            },
            ContractionStage::CompactSpacing => Self {
                essential: true,
                important: true,
                optional_meta: true,
                secondary_actions: true,
                full_secondary_labels: true,
                multi_pane: true,
                use_drawer: false,
                line_mode: false,
                density: Density::Compact,
                overflow: OverflowBehavior::Ellipsis,
            },
            ContractionStage::ShortenSecondary => Self {
                essential: true,
                important: true,
                optional_meta: true,
                secondary_actions: true,
                full_secondary_labels: false,
                multi_pane: true,
                use_drawer: false,
                line_mode: false,
                density: Density::Compact,
                overflow: OverflowBehavior::Ellipsis,
            },
            ContractionStage::HideOptionalMeta => Self {
                essential: true,
                important: true,
                optional_meta: false,
                secondary_actions: true,
                full_secondary_labels: false,
                multi_pane: true,
                use_drawer: false,
                line_mode: false,
                density: Density::Dashboard,
                overflow: OverflowBehavior::Ellipsis,
            },
            ContractionStage::CollapseSecondaryActions => Self {
                essential: true,
                important: true,
                optional_meta: false,
                secondary_actions: false,
                full_secondary_labels: false,
                multi_pane: true,
                use_drawer: false,
                line_mode: false,
                density: Density::Dashboard,
                overflow: OverflowBehavior::Ellipsis,
            },
            ContractionStage::SinglePane => Self {
                essential: true,
                important: true,
                optional_meta: false,
                secondary_actions: false,
                full_secondary_labels: false,
                multi_pane: false,
                use_drawer: false,
                line_mode: false,
                density: Density::Dashboard,
                overflow: OverflowBehavior::Ellipsis,
            },
            ContractionStage::DrawerOrOverlay => Self {
                essential: true,
                important: true,
                optional_meta: false,
                secondary_actions: false,
                full_secondary_labels: false,
                multi_pane: false,
                use_drawer: true,
                line_mode: false,
                density: Density::Dashboard,
                overflow: OverflowBehavior::Ellipsis,
            },
            ContractionStage::LineMode => Self {
                essential: true,
                important: false,
                optional_meta: false,
                secondary_actions: false,
                full_secondary_labels: false,
                multi_pane: false,
                use_drawer: false,
                line_mode: true,
                density: Density::Dashboard,
                overflow: OverflowBehavior::Ellipsis,
            },
        }
    }

    /// Whether a content priority tier is shown.
    #[must_use]
    pub const fn shows(self, priority: ContentPriority) -> bool {
        match priority {
            ContentPriority::Essential => self.essential,
            ContentPriority::Important => self.important,
            ContentPriority::Optional => self.optional_meta,
            ContentPriority::Decorative => {
                self.optional_meta && self.full_secondary_labels && self.secondary_actions
            }
        }
    }
}

/// Viewport classification from width × height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewportClass {
    /// Available width in cells.
    pub width: u16,
    /// Available height in rows.
    pub height: u16,
    /// Resolved contraction stage.
    pub stage: ContractionStage,
    /// Adaptive anatomy flags.
    pub anatomy: AdaptiveAnatomy,
}

impl ViewportClass {
    /// Classify a terminal (or host) rectangle with global width bands.
    #[must_use]
    pub const fn classify(width: u16, height: u16) -> Self {
        let mut stage = ContractionStage::from_width(width);
        // Very short terminals also force line-mode regardless of width.
        if height > 0 && height <= 5 {
            stage = ContractionStage::LineMode;
        } else if height > 0 && height <= 10 && (stage as u8) < (ContractionStage::SinglePane as u8)
        {
            stage = ContractionStage::SinglePane;
        }
        Self {
            width,
            height,
            stage,
            anatomy: AdaptiveAnatomy::from_stage(stage),
        }
    }

    /// Classify for a named surface (applies surface-specific stage thresholds).
    #[must_use]
    pub const fn for_surface(surface: ResponsiveSurface, width: u16, height: u16) -> Self {
        let policy = surface.policy();
        let mut stage = policy.stage_for_width(width);
        if height > 0 && height <= policy.line_mode_max_height {
            stage = ContractionStage::LineMode;
        }
        let anatomy = policy.refine_anatomy(AdaptiveAnatomy::from_stage(stage), stage);
        Self {
            width,
            height,
            stage,
            anatomy,
        }
    }
}

/// Named TermRock surfaces with responsive contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ResponsiveSurface {
    /// Application chrome: nav + main + status.
    AppShell,
    /// Navigation sidebar / file tree host.
    Sidebar,
    /// Tab strip.
    Tabs,
    /// List-like table (rows of composed anatomy).
    Table,
    /// Hierarchical tree.
    Tree,
    /// Multi-column data table.
    DataTable,
    /// Multi-field form.
    Form,
    /// Modal dialog.
    Dialog,
    /// Command palette / fuzzy picker.
    CommandPalette,
    /// Prompt / chat composer.
    PromptComposer,
    /// Task / agent rail.
    TaskRail,
    /// Permission / approval prompt.
    PermissionPrompt,
    /// Plan review checklist.
    PlanReview,
    /// Diff review.
    DiffReview,
    /// Log / scrollback viewer.
    LogViewer,
    /// Bottom status bar.
    StatusBar,
}

/// Per-surface size budgets and stage thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceResponsivePolicy {
    /// Preferred / min usable width for the surface body.
    pub width: SizeBudget,
    /// Preferred / min usable height.
    pub height: SizeBudget,
    /// Width at or below which stage becomes LineMode.
    pub line_mode_max_width: u16,
    /// Height at or below which stage becomes LineMode.
    pub line_mode_max_height: u16,
    /// Width at or below which drawer/overlay replacement activates.
    pub drawer_max_width: u16,
    /// Width at or below which multi-pane collapses to single-pane.
    pub single_pane_max_width: u16,
    /// Width at or below which secondary actions collapse.
    pub collapse_actions_max_width: u16,
    /// Width at or below which optional metadata hides.
    pub hide_meta_max_width: u16,
    /// Width at or below which secondary labels shorten.
    pub shorten_max_width: u16,
    /// Width at or below which compact spacing applies.
    pub compact_max_width: u16,
    /// Default overflow for essential labels.
    pub overflow: OverflowBehavior,
    /// Whether this surface supports multi-column / multi-pane at full stage.
    pub supports_multi_pane: bool,
    /// Whether secondary can become a drawer under pressure.
    pub supports_drawer: bool,
}

impl SurfaceResponsivePolicy {
    /// Resolve stage from width using this surface's thresholds.
    #[must_use]
    pub const fn stage_for_width(self, width: u16) -> ContractionStage {
        if width <= self.line_mode_max_width {
            ContractionStage::LineMode
        } else if self.supports_drawer
            && self.drawer_max_width > 0
            && width <= self.drawer_max_width
        {
            ContractionStage::DrawerOrOverlay
        } else if self.supports_multi_pane
            && self.single_pane_max_width > 0
            && width <= self.single_pane_max_width
        {
            ContractionStage::SinglePane
        } else if width <= self.collapse_actions_max_width
            || (self.single_pane_max_width > 0 && width <= self.single_pane_max_width)
        {
            // Surfaces without multi-pane treat the single-pane threshold as
            // secondary-action collapse instead of a pane merge.
            ContractionStage::CollapseSecondaryActions
        } else if width <= self.hide_meta_max_width {
            ContractionStage::HideOptionalMeta
        } else if width <= self.shorten_max_width {
            ContractionStage::ShortenSecondary
        } else if width <= self.compact_max_width {
            ContractionStage::CompactSpacing
        } else {
            ContractionStage::Full
        }
    }

    /// Surface-specific anatomy tweaks after baseline stage mapping.
    #[must_use]
    pub const fn refine_anatomy(
        self,
        mut anatomy: AdaptiveAnatomy,
        _stage: ContractionStage,
    ) -> AdaptiveAnatomy {
        anatomy.overflow = self.overflow;
        if !self.supports_multi_pane {
            anatomy.multi_pane = false;
        }
        if !self.supports_drawer {
            anatomy.use_drawer = false;
        }
        anatomy
    }
}

impl ResponsiveSurface {
    /// All surfaces with a responsive contract (for matrix tests).
    pub const ALL: [Self; 16] = [
        Self::AppShell,
        Self::Sidebar,
        Self::Tabs,
        Self::Table,
        Self::Tree,
        Self::DataTable,
        Self::Form,
        Self::Dialog,
        Self::CommandPalette,
        Self::PromptComposer,
        Self::TaskRail,
        Self::PermissionPrompt,
        Self::PlanReview,
        Self::DiffReview,
        Self::LogViewer,
        Self::StatusBar,
    ];

    /// Builtin responsive policy for this surface.
    #[must_use]
    pub const fn policy(self) -> SurfaceResponsivePolicy {
        match self {
            Self::AppShell => SurfaceResponsivePolicy {
                width: SizeBudget::range(120, 40),
                height: SizeBudget::range(40, 12),
                line_mode_max_width: 24,
                line_mode_max_height: 5,
                drawer_max_width: 48,
                single_pane_max_width: 72,
                collapse_actions_max_width: 90,
                hide_meta_max_width: 100,
                shorten_max_width: 120,
                compact_max_width: 140,
                overflow: OverflowBehavior::Ellipsis,
                supports_multi_pane: true,
                supports_drawer: true,
            },
            Self::Sidebar => SurfaceResponsivePolicy {
                width: SizeBudget {
                    preferred: 28,
                    min_usable: 12,
                    max: 48,
                },
                height: SizeBudget::range(24, 5),
                line_mode_max_width: 10,
                line_mode_max_height: 3,
                drawer_max_width: 40,
                single_pane_max_width: 60,
                collapse_actions_max_width: 72,
                hide_meta_max_width: 80,
                shorten_max_width: 100,
                compact_max_width: 120,
                overflow: OverflowBehavior::Ellipsis,
                supports_multi_pane: false,
                supports_drawer: true,
            },
            Self::Tabs => SurfaceResponsivePolicy {
                width: SizeBudget::range(80, 16),
                height: SizeBudget::fixed(1),
                line_mode_max_width: 12,
                line_mode_max_height: 1,
                drawer_max_width: 0,
                single_pane_max_width: 0,
                collapse_actions_max_width: 40,
                hide_meta_max_width: 48,
                shorten_max_width: 64,
                compact_max_width: 100,
                overflow: OverflowBehavior::Scroll,
                supports_multi_pane: false,
                supports_drawer: false,
            },
            Self::Table | Self::Tree => SurfaceResponsivePolicy {
                width: SizeBudget::range(80, 20),
                height: SizeBudget::range(20, 3),
                line_mode_max_width: 16,
                line_mode_max_height: 3,
                drawer_max_width: 0,
                single_pane_max_width: 0,
                collapse_actions_max_width: 48,
                hide_meta_max_width: 60,
                shorten_max_width: 80,
                compact_max_width: 100,
                overflow: OverflowBehavior::Ellipsis,
                supports_multi_pane: false,
                supports_drawer: false,
            },
            Self::DataTable => SurfaceResponsivePolicy {
                width: SizeBudget::range(100, 24),
                height: SizeBudget::range(16, 4),
                line_mode_max_width: 20,
                line_mode_max_height: 3,
                drawer_max_width: 0,
                single_pane_max_width: 0,
                collapse_actions_max_width: 56,
                hide_meta_max_width: 72,
                shorten_max_width: 90,
                compact_max_width: 120,
                overflow: OverflowBehavior::Scroll,
                supports_multi_pane: false,
                supports_drawer: false,
            },
            Self::Form => SurfaceResponsivePolicy {
                width: SizeBudget::range(72, 24),
                height: SizeBudget::range(20, 6),
                line_mode_max_width: 18,
                line_mode_max_height: 4,
                drawer_max_width: 0,
                single_pane_max_width: 48,
                collapse_actions_max_width: 56,
                hide_meta_max_width: 64,
                shorten_max_width: 80,
                compact_max_width: 100,
                overflow: OverflowBehavior::Wrap,
                supports_multi_pane: true,
                supports_drawer: false,
            },
            Self::Dialog => SurfaceResponsivePolicy {
                width: SizeBudget {
                    preferred: 48,
                    min_usable: 24,
                    max: 80,
                },
                height: SizeBudget {
                    preferred: 12,
                    min_usable: 5,
                    max: 30,
                },
                line_mode_max_width: 20,
                line_mode_max_height: 4,
                drawer_max_width: 36,
                single_pane_max_width: 40,
                collapse_actions_max_width: 48,
                hide_meta_max_width: 56,
                shorten_max_width: 64,
                compact_max_width: 80,
                overflow: OverflowBehavior::Wrap,
                supports_multi_pane: false,
                supports_drawer: true,
            },
            Self::CommandPalette => SurfaceResponsivePolicy {
                width: SizeBudget {
                    preferred: 56,
                    min_usable: 28,
                    max: 80,
                },
                height: SizeBudget {
                    preferred: 16,
                    min_usable: 6,
                    max: 30,
                },
                line_mode_max_width: 22,
                line_mode_max_height: 5,
                drawer_max_width: 40,
                single_pane_max_width: 48,
                collapse_actions_max_width: 56,
                hide_meta_max_width: 64,
                shorten_max_width: 72,
                compact_max_width: 90,
                overflow: OverflowBehavior::Ellipsis,
                supports_multi_pane: false,
                supports_drawer: true,
            },
            Self::PromptComposer => SurfaceResponsivePolicy {
                width: SizeBudget::range(80, 24),
                height: SizeBudget {
                    preferred: 6,
                    min_usable: 2,
                    max: 20,
                },
                line_mode_max_width: 20,
                line_mode_max_height: 2,
                drawer_max_width: 0,
                single_pane_max_width: 0,
                collapse_actions_max_width: 48,
                hide_meta_max_width: 60,
                shorten_max_width: 80,
                compact_max_width: 100,
                overflow: OverflowBehavior::Wrap,
                supports_multi_pane: false,
                supports_drawer: false,
            },
            Self::TaskRail => SurfaceResponsivePolicy {
                width: SizeBudget {
                    preferred: 32,
                    min_usable: 14,
                    max: 48,
                },
                height: SizeBudget::range(24, 5),
                line_mode_max_width: 12,
                line_mode_max_height: 3,
                drawer_max_width: 48,
                single_pane_max_width: 64,
                collapse_actions_max_width: 72,
                hide_meta_max_width: 80,
                shorten_max_width: 100,
                compact_max_width: 120,
                overflow: OverflowBehavior::Ellipsis,
                supports_multi_pane: false,
                supports_drawer: true,
            },
            Self::PermissionPrompt => SurfaceResponsivePolicy {
                width: SizeBudget {
                    preferred: 52,
                    min_usable: 28,
                    max: 72,
                },
                height: SizeBudget {
                    preferred: 10,
                    min_usable: 5,
                    max: 20,
                },
                line_mode_max_width: 22,
                line_mode_max_height: 4,
                drawer_max_width: 36,
                single_pane_max_width: 44,
                collapse_actions_max_width: 52,
                hide_meta_max_width: 60,
                shorten_max_width: 72,
                compact_max_width: 90,
                overflow: OverflowBehavior::Wrap,
                supports_multi_pane: false,
                supports_drawer: true,
            },
            Self::PlanReview => SurfaceResponsivePolicy {
                width: SizeBudget::range(72, 28),
                height: SizeBudget::range(18, 6),
                line_mode_max_width: 22,
                line_mode_max_height: 4,
                drawer_max_width: 40,
                single_pane_max_width: 56,
                collapse_actions_max_width: 64,
                hide_meta_max_width: 80,
                shorten_max_width: 100,
                compact_max_width: 120,
                overflow: OverflowBehavior::Scroll,
                supports_multi_pane: true,
                supports_drawer: true,
            },
            Self::DiffReview => SurfaceResponsivePolicy {
                width: SizeBudget::range(100, 32),
                height: SizeBudget::range(24, 6),
                line_mode_max_width: 24,
                line_mode_max_height: 4,
                drawer_max_width: 0,
                single_pane_max_width: 70,
                collapse_actions_max_width: 80,
                hide_meta_max_width: 90,
                shorten_max_width: 110,
                compact_max_width: 140,
                overflow: OverflowBehavior::Scroll,
                supports_multi_pane: true,
                supports_drawer: false,
            },
            Self::LogViewer => SurfaceResponsivePolicy {
                width: SizeBudget::range(80, 20),
                height: SizeBudget::range(20, 4),
                line_mode_max_width: 16,
                line_mode_max_height: 3,
                drawer_max_width: 0,
                single_pane_max_width: 0,
                collapse_actions_max_width: 40,
                hide_meta_max_width: 56,
                shorten_max_width: 72,
                compact_max_width: 100,
                overflow: OverflowBehavior::Scroll,
                supports_multi_pane: false,
                supports_drawer: false,
            },
            Self::StatusBar => SurfaceResponsivePolicy {
                width: SizeBudget::range(80, 10),
                height: SizeBudget::fixed(1),
                line_mode_max_width: 8,
                line_mode_max_height: 1,
                drawer_max_width: 0,
                single_pane_max_width: 0,
                collapse_actions_max_width: 30,
                hide_meta_max_width: 40,
                shorten_max_width: 56,
                compact_max_width: 80,
                overflow: OverflowBehavior::Hide,
                supports_multi_pane: false,
                supports_drawer: false,
            },
        }
    }

    /// Classify this surface at the given size.
    #[must_use]
    pub const fn classify(self, width: u16, height: u16) -> ViewportClass {
        ViewportClass::for_surface(self, width, height)
    }

    /// Adaptive anatomy at width (height = 24 default for width-only tests).
    #[must_use]
    pub const fn anatomy_for_width(self, width: u16) -> AdaptiveAnatomy {
        self.classify(width, 24).anatomy
    }

    /// Form column count from anatomy (1 or 2).
    #[must_use]
    pub const fn form_columns(self, width: u16) -> u8 {
        if !matches!(self, Self::Form) {
            return 1;
        }
        let a = self.anatomy_for_width(width);
        if a.multi_pane && !a.line_mode {
            2
        } else {
            1
        }
    }

    /// Stable name for docs/tests.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AppShell => "app-shell",
            Self::Sidebar => "sidebar",
            Self::Tabs => "tabs",
            Self::Table => "table",
            Self::Tree => "tree",
            Self::DataTable => "data-table",
            Self::Form => "form",
            Self::Dialog => "dialog",
            Self::CommandPalette => "command-palette",
            Self::PromptComposer => "prompt-composer",
            Self::TaskRail => "task-rail",
            Self::PermissionPrompt => "permission-prompt",
            Self::PlanReview => "plan-review",
            Self::DiffReview => "diff-review",
            Self::LogViewer => "log-viewer",
            Self::StatusBar => "status-bar",
        }
    }
}

/// Declared part of a component anatomy with priority and overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnatomyPart {
    /// Stable part name (e.g. `"primary"`, `"badge"`, `"shortcut"`).
    pub name: &'static str,
    /// Survival priority.
    pub priority: ContentPriority,
    /// Preferred cell width for this part (0 = flexible).
    pub preferred_width: u16,
    /// Minimum cells to keep the part meaningful (0 = may fully hide).
    pub min_width: u16,
    /// Overflow when budget is short but priority still shows the part.
    pub overflow: OverflowBehavior,
}

impl AnatomyPart {
    /// Essential primary label.
    #[must_use]
    pub const fn essential(name: &'static str, preferred: u16, min: u16) -> Self {
        Self {
            name,
            priority: ContentPriority::Essential,
            preferred_width: preferred,
            min_width: min,
            overflow: OverflowBehavior::Ellipsis,
        }
    }

    /// Important secondary content.
    #[must_use]
    pub const fn important(name: &'static str, preferred: u16, min: u16) -> Self {
        Self {
            name,
            priority: ContentPriority::Important,
            preferred_width: preferred,
            min_width: min,
            overflow: OverflowBehavior::Ellipsis,
        }
    }

    /// Optional metadata (hides under meta contraction).
    #[must_use]
    pub const fn optional(name: &'static str, preferred: u16) -> Self {
        Self {
            name,
            priority: ContentPriority::Optional,
            preferred_width: preferred,
            min_width: 0,
            overflow: OverflowBehavior::Hide,
        }
    }

    /// Secondary action slot.
    #[must_use]
    pub const fn secondary_action(name: &'static str, preferred: u16) -> Self {
        Self {
            name,
            priority: ContentPriority::Optional,
            preferred_width: preferred,
            min_width: 0,
            overflow: OverflowBehavior::Hide,
        }
    }

    /// Whether this part is shown under the given anatomy.
    #[must_use]
    pub const fn visible(self, anatomy: AdaptiveAnatomy) -> bool {
        match self.priority {
            ContentPriority::Essential => anatomy.essential,
            ContentPriority::Important => anatomy.important,
            ContentPriority::Optional => anatomy.optional_meta || anatomy.secondary_actions,
            ContentPriority::Decorative => anatomy.optional_meta && anatomy.full_secondary_labels,
        }
    }
}

/// Standard composed-row anatomy (list / tree / task rail rows).
#[must_use]
pub fn composed_row_anatomy() -> [AnatomyPart; 5] {
    [
        AnatomyPart::important("leading", 2, 1),
        AnatomyPart::essential("primary", 24, 1),
        AnatomyPart::optional("secondary", 16),
        AnatomyPart::optional("badge", 6),
        AnatomyPart::optional("shortcut", 4),
    ]
}

/// Standard status-bar slot anatomy (left → right priority within bar).
#[must_use]
pub fn status_bar_anatomy() -> [AnatomyPart; 4] {
    [
        AnatomyPart::essential("primary", 20, 4),
        AnatomyPart::important("mode", 10, 3),
        AnatomyPart::optional("meta", 16),
        AnatomyPart::optional("clock", 8),
    ]
}

/// Standard dialog chrome anatomy.
#[must_use]
pub fn dialog_anatomy() -> [AnatomyPart; 4] {
    [
        AnatomyPart::essential("title", 24, 4),
        AnatomyPart::essential("primary_action", 10, 4),
        AnatomyPart::important("body", 40, 8),
        AnatomyPart::optional("secondary_action", 10),
    ]
}

/// Contract anatomy parts to a width budget under the given flags.
///
/// Drops lowest-priority visible parts first. Essential parts with
/// `min_width > 0` survive until only they remain (may ellipsis).
#[must_use]
pub fn contract_parts(
    parts: &[AnatomyPart],
    width: u16,
    anatomy: AdaptiveAnatomy,
) -> Vec<AnatomyPart> {
    let mut visible: Vec<AnatomyPart> = parts
        .iter()
        .copied()
        .filter(|p| {
            // Secondary actions use Optional priority but follow secondary_actions flag.
            if p.name.contains("action") && p.priority != ContentPriority::Essential {
                return anatomy.secondary_actions || p.priority == ContentPriority::Essential;
            }
            match p.priority {
                ContentPriority::Essential => anatomy.essential,
                ContentPriority::Important => anatomy.important,
                ContentPriority::Optional => anatomy.optional_meta,
                ContentPriority::Decorative => {
                    anatomy.optional_meta && anatomy.full_secondary_labels
                }
            }
        })
        .collect();

    let drop_order = [
        ContentPriority::Decorative,
        ContentPriority::Optional,
        ContentPriority::Important,
    ];
    for priority in drop_order {
        while occupied_width(&visible) > width {
            let Some(idx) = visible.iter().rposition(|p| p.priority == priority) else {
                break;
            };
            visible.remove(idx);
        }
    }

    // Allocate cells: essential first, then important, then optional.
    visible.sort_by_key(|p| std::cmp::Reverse(p.priority as u8));
    let mut remaining = width;
    for part in &mut visible {
        let floor = if part.priority == ContentPriority::Essential {
            part.min_width.min(remaining)
        } else {
            0
        };
        let want = part.preferred_width.max(part.min_width);
        let take = want.min(remaining).max(floor);
        part.preferred_width = take;
        if take > 0 {
            remaining = remaining.saturating_sub(take.saturating_add(1));
        }
    }
    visible.retain(|p| p.preferred_width > 0 || p.priority == ContentPriority::Essential);
    // Ensure essential always has at least 1 cell when width allows.
    if width > 0 {
        for part in &mut visible {
            if part.priority == ContentPriority::Essential && part.preferred_width == 0 {
                part.preferred_width = 1.min(width);
            }
        }
    }
    visible
}

fn occupied_width(parts: &[AnatomyPart]) -> u16 {
    if parts.is_empty() {
        return 0;
    }
    let mut w = 0u16;
    for (i, p) in parts.iter().enumerate() {
        w = w.saturating_add(p.preferred_width.max(p.min_width));
        if i > 0 {
            w = w.saturating_add(1);
        }
    }
    w
}

/// Whether primary (essential) content survives for a surface at `width`.
#[must_use]
pub fn essential_survives(surface: ResponsiveSurface, width: u16) -> bool {
    surface.anatomy_for_width(width).essential
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_stage_bands_match_ladder() {
        // Default bands (inclusive upper edges of each arm):
        // Full >159 | Compact 121–159 | Shorten 101–120 | Hide 81–100 |
        // Collapse 61–80 | Single 41–60 | Drawer 25–40 | Line ≤24
        let cases = [
            (160, ContractionStage::Full),
            (140, ContractionStage::CompactSpacing),
            (120, ContractionStage::ShortenSecondary),
            (100, ContractionStage::HideOptionalMeta),
            (80, ContractionStage::CollapseSecondaryActions),
            (60, ContractionStage::SinglePane),
            (40, ContractionStage::DrawerOrOverlay),
            (20, ContractionStage::LineMode),
        ];
        for (width, expected) in cases {
            assert_eq!(
                ContractionStage::from_width(width),
                expected,
                "width {width}"
            );
        }
        // Ladder samples are a subset of the matrix (severity non-decreasing).
        for &width in &WIDTH_LADDER {
            let _ = ContractionStage::from_width(width);
        }
    }

    #[test]
    fn stages_monotonically_increase_as_width_shrinks() {
        let mut prev = ContractionStage::Full;
        for &w in &WIDTH_LADDER {
            let stage = ContractionStage::from_width(w);
            assert!(
                (stage as u8) >= (prev as u8) || w == 160,
                "width {w}: stage {stage:?} regressed from {prev:?}"
            );
            // Allow equal or higher severity as width drops across ladder
            if w < 160 {
                assert!(
                    (stage as u8) >= (prev as u8),
                    "width {w}: expected >= {prev:?}, got {stage:?}"
                );
            }
            prev = stage;
        }
    }

    #[test]
    fn essential_survives_every_ladder_width_on_every_surface() {
        for surface in ResponsiveSurface::ALL {
            for &width in &WIDTH_LADDER {
                assert!(
                    essential_survives(surface, width),
                    "{} at {width} cols lost essential content",
                    surface.as_str()
                );
                let anatomy = surface.anatomy_for_width(width);
                assert!(anatomy.essential);
            }
        }
    }

    #[test]
    fn optional_drops_before_essential_on_composed_row() {
        let parts = composed_row_anatomy();
        for &width in &WIDTH_LADDER {
            let anatomy = AdaptiveAnatomy::from_stage(ContractionStage::from_width(width));
            let contracted = contract_parts(&parts, width, anatomy);
            assert!(
                contracted
                    .iter()
                    .any(|p| p.name == "primary" && p.priority == ContentPriority::Essential),
                "primary missing at width {width}: {contracted:?}"
            );
            if width <= 40 {
                assert!(
                    contracted
                        .iter()
                        .all(|p| p.name != "shortcut" && p.name != "badge"
                            || p.preferred_width == 0),
                    "optional chrome should be gone or empty at {width}: {contracted:?}"
                );
            }
        }
    }

    #[test]
    fn priority_visibility_table() {
        assert!(ContentPriority::Essential.visible_at(ContractionStage::LineMode));
        assert!(!ContentPriority::Important.visible_at(ContractionStage::LineMode));
        assert!(!ContentPriority::Optional.visible_at(ContractionStage::HideOptionalMeta));
        assert!(ContentPriority::Optional.visible_at(ContractionStage::ShortenSecondary));
        assert!(!ContentPriority::Decorative.visible_at(ContractionStage::ShortenSecondary));
    }

    #[test]
    fn app_shell_progresses_to_drawer_then_line() {
        let wide = ResponsiveSurface::AppShell.classify(160, 40);
        assert_eq!(wide.stage, ContractionStage::Full);
        assert!(wide.anatomy.multi_pane);
        assert!(!wide.anatomy.use_drawer);

        let mid = ResponsiveSurface::AppShell.classify(60, 24);
        assert!(
            matches!(
                mid.stage,
                ContractionStage::SinglePane | ContractionStage::DrawerOrOverlay
            ),
            "{:?}",
            mid.stage
        );
        assert!(!mid.anatomy.multi_pane);

        let narrow = ResponsiveSurface::AppShell.classify(40, 24);
        assert!(narrow.anatomy.use_drawer || narrow.stage == ContractionStage::DrawerOrOverlay);

        let tiny = ResponsiveSurface::AppShell.classify(20, 24);
        assert_eq!(tiny.stage, ContractionStage::LineMode);
        assert!(tiny.anatomy.line_mode);
        assert!(tiny.anatomy.essential);
        assert!(!tiny.anatomy.optional_meta);
        assert!(!tiny.anatomy.secondary_actions);
    }

    #[test]
    fn form_columns_collapse_under_single_pane() {
        assert_eq!(ResponsiveSurface::Form.form_columns(160), 2);
        assert_eq!(ResponsiveSurface::Form.form_columns(100), 2);
        // single_pane_max_width = 48 for Form
        assert_eq!(ResponsiveSurface::Form.form_columns(40), 1);
        assert_eq!(ResponsiveSurface::Form.form_columns(20), 1);
    }

    #[test]
    fn secondary_actions_collapse_before_primary() {
        let parts = dialog_anatomy();
        let anatomy = AdaptiveAnatomy::from_stage(ContractionStage::CollapseSecondaryActions);
        let contracted = contract_parts(&parts, 40, anatomy);
        assert!(
            contracted.iter().any(|p| p.name == "primary_action"),
            "{contracted:?}"
        );
        assert!(
            contracted.iter().all(|p| p.name != "secondary_action"),
            "secondary action should be filtered by anatomy: {contracted:?}"
        );
        assert!(contracted.iter().any(|p| p.name == "title"));
    }

    #[test]
    fn status_bar_matrix_keeps_primary() {
        let parts = status_bar_anatomy();
        for &width in &WIDTH_LADDER {
            let anatomy = ResponsiveSurface::StatusBar.anatomy_for_width(width);
            let contracted = contract_parts(&parts, width, anatomy);
            assert!(
                contracted.iter().any(|p| p.name == "primary"),
                "status primary lost at {width}: {contracted:?}"
            );
            if width <= 20 {
                assert!(
                    !anatomy.optional_meta,
                    "status bar still shows optional meta at {width}"
                );
            }
        }
    }

    #[test]
    fn width_ladder_matrix_all_surfaces() {
        // Exhaustive matrix: surface × width → essential + stage ordered.
        for surface in ResponsiveSurface::ALL {
            let mut last_severity = 0u8;
            for &width in &WIDTH_LADDER {
                let class = surface.classify(width, 24);
                assert!(
                    class.anatomy.essential,
                    "{}@{width}: essential false",
                    surface.as_str()
                );
                // Severity non-decreasing as width decreases across ladder.
                let sev = class.stage as u8;
                assert!(
                    sev >= last_severity || width == WIDTH_LADDER[0],
                    "{}: width {width} stage {:?} severity {sev} < previous {last_severity}",
                    surface.as_str(),
                    class.stage
                );
                last_severity = sev;

                // Primary > secondary survival rule
                if !class.anatomy.important {
                    assert!(
                        !class.anatomy.optional_meta,
                        "{}@{width}: optional without important",
                        surface.as_str()
                    );
                }
                if class.anatomy.line_mode {
                    assert!(!class.anatomy.secondary_actions);
                    assert!(!class.anatomy.multi_pane);
                }
            }
        }
    }

    #[test]
    fn short_height_forces_line_mode() {
        let class = ViewportClass::classify(160, 4);
        assert_eq!(class.stage, ContractionStage::LineMode);
    }

    #[test]
    fn size_budget_resolve_respects_available() {
        let b = SizeBudget::range(40, 20);
        assert_eq!(b.resolve(100), 40);
        assert_eq!(b.resolve(30), 30);
        assert!(b.below_min_usable(10));
    }

    #[test]
    fn diff_review_collapses_side_by_side() {
        let wide = ResponsiveSurface::DiffReview.classify(160, 40);
        assert!(wide.anatomy.multi_pane);
        let narrow = ResponsiveSurface::DiffReview.classify(60, 24);
        assert!(
            !narrow.anatomy.multi_pane || narrow.stage >= ContractionStage::SinglePane,
            "{:?}",
            narrow.stage
        );
    }

    #[test]
    fn sidebar_and_task_rail_prefer_drawer_when_narrow() {
        for surface in [ResponsiveSurface::Sidebar, ResponsiveSurface::TaskRail] {
            let class = surface.classify(36, 24);
            assert!(
                class.anatomy.use_drawer || class.stage >= ContractionStage::DrawerOrOverlay,
                "{}: {:?}",
                surface.as_str(),
                class.stage
            );
        }
    }

    #[test]
    fn permission_and_plan_keep_primary_actions() {
        for surface in [
            ResponsiveSurface::PermissionPrompt,
            ResponsiveSurface::PlanReview,
        ] {
            for &w in &WIDTH_LADDER {
                let a = surface.anatomy_for_width(w);
                assert!(a.essential, "{}@{w}", surface.as_str());
            }
        }
    }
}
