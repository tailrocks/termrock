// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Capability-aware preview surface lifecycle.
//!
//! TermRock plans placement and fallbacks only. Consumers own terminal protocol
//! emission, fetch/decode, and file I/O. Stale async results are rejected via
//! generation tokens.
//!
//! **Multi-frame contract:** call [`CapabilityPreviewHost::begin_frame`] each
//! frame, then re-register desired surfaces. Placement IDs are stable for a
//! `(kind, resource_id)` key so steady selection does not thrash
//! Delete+Replace every frame.

use ratatui_core::layout::Rect;

use super::{ColorCapability, DesignSystem, RolePalette, quantize_palette};

/// Kind of capability-aware preview surface (media/resource host).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PreviewSurfaceKind {
    /// Design-system / palette swatch surface.
    Theme,
    /// Image protocol surface (placement only; emission is consumer-owned).
    Image,
    /// Resource browser detail preview slot.
    ResourceDetail,
    /// Generic async payload placeholder.
    AsyncPayload,
}

/// Desired presentation after capability × geometry resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PreviewPresentation {
    /// Styled cell / alt-text only (no external protocol).
    CellFallback,
    /// Kitty graphics protocol (consumer emits).
    Kitty,
    /// iTerm2 inline image (consumer emits).
    ITerm2,
    /// Sixel (consumer emits).
    Sixel,
}

/// One preview slot with placement and lifecycle flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewSurface {
    /// Surface kind.
    pub kind: PreviewSurfaceKind,
    /// Allocated rectangle (caller layout).
    pub area: Rect,
    /// True when the last projection is stale vs source.
    pub stale: bool,
    /// True while async content is pending.
    pub pending: bool,
    /// Optional stable resource id.
    pub resource_id: Option<String>,
    /// Generation token for async reject/replace.
    pub generation: u64,
    /// Resolved presentation for this surface.
    pub presentation: PreviewPresentation,
    /// Placement identity for session diffing (stable across frames for same key).
    pub placement_id: u64,
}

/// Typed session commands for consumer-owned terminal I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaSessionCommand {
    /// Place or replace protocol content for a placement.
    Replace {
        /// Placement identity.
        placement_id: u64,
        /// Resource key.
        resource_id: String,
        /// Target cell area.
        area: Rect,
        /// Chosen protocol.
        presentation: PreviewPresentation,
        /// Generation that produced this command.
        generation: u64,
    },
    /// Delete a previously applied placement.
    Delete {
        /// Placement identity.
        placement_id: u64,
    },
    /// Clear all protocol placements.
    Clear,
}

/// Applied session slot (survives `begin_frame`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct AppliedPlacement {
    placement_id: u64,
    kind: PreviewSurfaceKind,
    resource_id: String,
    generation: u64,
    area: Rect,
    presentation: PreviewPresentation,
}

/// Projects a design system to a capability ladder for deterministic previews
/// and tracks async generation so late results cannot clobber newer selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityPreviewHost {
    /// Source tokens.
    pub system: DesignSystem,
    /// Active capability.
    pub capability: ColorCapability,
    /// Registered preview surfaces for the current frame.
    pub surfaces: Vec<PreviewSurface>,
    /// Monotonic generation counter.
    generation: u64,
    /// Next placement id (only minted for new keys).
    next_placement: u64,
    /// Applied placements still live in the consumer session.
    applied: Vec<AppliedPlacement>,
    /// Whether Kitty/iTerm2/Sixel are considered available (caller probe).
    pub kitty: bool,
    /// iTerm2 support.
    pub iterm2: bool,
    /// Sixel support.
    pub sixel: bool,
}

impl CapabilityPreviewHost {
    /// Creates a host at truecolor (design target).
    #[must_use]
    pub fn truecolor(system: DesignSystem) -> Self {
        Self {
            system,
            capability: ColorCapability::Truecolor,
            surfaces: Vec::new(),
            generation: 0,
            next_placement: 1,
            applied: Vec::new(),
            kitty: false,
            iterm2: false,
            sixel: false,
        }
    }

    /// Sets capability projection.
    #[must_use]
    pub const fn capability(mut self, capability: ColorCapability) -> Self {
        self.capability = capability;
        self
    }

    /// Declares protocol support from consumer probe (never probes itself).
    #[must_use]
    pub const fn protocols(mut self, kitty: bool, iterm2: bool, sixel: bool) -> Self {
        self.kitty = kitty;
        self.iterm2 = iterm2;
        self.sixel = sixel;
        self
    }

    /// Theme quantized for the active capability.
    #[must_use]
    pub fn projected_theme(&self) -> RolePalette {
        quantize_palette(self.system.palette(), self.capability)
    }

    /// Tokens with projected theme.
    #[must_use]
    pub fn projected_tokens(&self) -> DesignSystem {
        let mut tokens = self.system.clone();
        tokens.palette = self.projected_theme();
        tokens
    }

    /// Bumps generation (call when selection/content identity changes).
    pub fn bump_generation(&mut self) -> u64 {
        self.generation = self.generation.saturating_add(1);
        self.generation
    }

    /// Current generation token.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Clears per-frame surface registrations (keeps applied session state).
    pub fn begin_frame(&mut self) {
        self.surfaces.clear();
    }

    /// Resolves presentation for image-like content under current protocols.
    #[must_use]
    pub fn resolve_presentation(&self, kind: PreviewSurfaceKind) -> PreviewPresentation {
        match kind {
            PreviewSurfaceKind::Image | PreviewSurfaceKind::ResourceDetail => {
                if self.kitty {
                    PreviewPresentation::Kitty
                } else if self.iterm2 {
                    PreviewPresentation::ITerm2
                } else if self.sixel {
                    PreviewPresentation::Sixel
                } else {
                    PreviewPresentation::CellFallback
                }
            }
            PreviewSurfaceKind::Theme | PreviewSurfaceKind::AsyncPayload => {
                PreviewPresentation::CellFallback
            }
        }
    }

    /// Stable placement id for `(kind, resource_id)` — reuses applied/current-frame.
    fn placement_id_for(&mut self, kind: PreviewSurfaceKind, resource_id: &str) -> u64 {
        if let Some(applied) = self
            .applied
            .iter()
            .find(|a| a.kind == kind && a.resource_id == resource_id)
        {
            return applied.placement_id;
        }
        if let Some(surface) = self.surfaces.iter().find(|s| {
            s.kind == kind && s.resource_id.as_deref() == Some(resource_id) && s.placement_id != 0
        }) {
            return surface.placement_id;
        }
        let id = self.next_placement;
        self.next_placement = self.next_placement.saturating_add(1);
        id
    }

    /// Registers a preview surface for the current frame.
    pub fn register_surface(&mut self, mut surface: PreviewSurface) {
        if surface.generation == 0 {
            surface.generation = self.generation;
        }
        if surface.presentation == PreviewPresentation::CellFallback
            && matches!(
                surface.kind,
                PreviewSurfaceKind::Image | PreviewSurfaceKind::ResourceDetail
            )
        {
            surface.presentation = self.resolve_presentation(surface.kind);
        }
        if surface.placement_id == 0 {
            if let Some(resource_id) = surface.resource_id.as_deref() {
                surface.placement_id = self.placement_id_for(surface.kind, resource_id);
            } else {
                surface.placement_id = self.next_placement;
                self.next_placement = self.next_placement.saturating_add(1);
            }
        }
        self.surfaces.push(surface);
    }

    /// Marks all surfaces of `kind` stale (e.g. after resource change).
    pub fn mark_stale(&mut self, kind: PreviewSurfaceKind) {
        for surface in &mut self.surfaces {
            if surface.kind == kind {
                surface.stale = true;
            }
        }
    }

    /// Places an image surface; does not emit OSC protocols (consumer-owned).
    ///
    /// Placement id is stable for the same `resource_id` across frames so
    /// every-frame re-registration does not thrash session commands.
    pub fn place_image(&mut self, area: Rect, resource_id: impl Into<String>, pending: bool) {
        let resource_id = resource_id.into();
        let kind = PreviewSurfaceKind::Image;
        let placement_id = self.placement_id_for(kind, &resource_id);
        self.surfaces.push(PreviewSurface {
            kind,
            area,
            stale: false,
            pending,
            resource_id: Some(resource_id),
            generation: self.generation,
            presentation: self.resolve_presentation(kind),
            placement_id,
        });
    }

    /// Places a resource-browser preview for the selected resource.
    ///
    /// Placement id is stable for the same `resource_id` across frames.
    pub fn place_resource_preview(
        &mut self,
        area: Rect,
        resource_id: impl Into<String>,
        pending: bool,
    ) {
        let resource_id = resource_id.into();
        let kind = PreviewSurfaceKind::ResourceDetail;
        let placement_id = self.placement_id_for(kind, &resource_id);
        self.surfaces.push(PreviewSurface {
            kind,
            area,
            stale: false,
            pending,
            resource_id: Some(resource_id),
            generation: self.generation,
            presentation: self.resolve_presentation(kind),
            placement_id,
        });
    }

    /// Accepts an async load result only if `generation` is still current **and**
    /// at least one matching surface is registered this frame.
    ///
    /// Returns `true` only when a surface was updated; `false` when generation
    /// is stale or no surface matches `resource_id`.
    pub fn complete_async(&mut self, generation: u64, resource_id: &str) -> bool {
        if generation != self.generation {
            return false;
        }
        let mut updated = false;
        for surface in &mut self.surfaces {
            if surface.resource_id.as_deref() == Some(resource_id)
                && surface.generation == generation
            {
                surface.pending = false;
                surface.stale = false;
                updated = true;
            }
        }
        updated
    }

    /// Rejects completing a surface when generation mismatches.
    #[must_use]
    pub fn is_current(&self, generation: u64) -> bool {
        generation == self.generation
    }

    /// Diffs desired surfaces vs applied placements → session commands.
    pub fn session_commands(&mut self) -> Vec<MediaSessionCommand> {
        let mut commands = Vec::new();
        let desired: Vec<_> = self
            .surfaces
            .iter()
            .filter(|s| {
                !s.pending
                    && !s.stale
                    && s.presentation != PreviewPresentation::CellFallback
                    && s.resource_id.is_some()
            })
            .cloned()
            .collect();

        let desired_ids: Vec<u64> = desired.iter().map(|s| s.placement_id).collect();
        let mut still_applied = Vec::new();
        for applied in self.applied.drain(..) {
            if desired_ids.contains(&applied.placement_id) {
                still_applied.push(applied);
            } else {
                commands.push(MediaSessionCommand::Delete {
                    placement_id: applied.placement_id,
                });
            }
        }
        self.applied = still_applied;

        for surface in desired {
            let resource_id = surface.resource_id.clone().unwrap_or_default();
            let already = self.applied.iter().any(|a| {
                a.placement_id == surface.placement_id
                    && a.resource_id == resource_id
                    && a.generation == surface.generation
                    && a.area == surface.area
                    && a.presentation == surface.presentation
                    && a.kind == surface.kind
            });
            if already {
                continue;
            }
            self.applied
                .retain(|a| a.placement_id != surface.placement_id);
            self.applied.push(AppliedPlacement {
                placement_id: surface.placement_id,
                kind: surface.kind,
                resource_id: resource_id.clone(),
                generation: surface.generation,
                area: surface.area,
                presentation: surface.presentation,
            });
            commands.push(MediaSessionCommand::Replace {
                placement_id: surface.placement_id,
                resource_id,
                area: surface.area,
                presentation: surface.presentation,
                generation: surface.generation,
            });
        }
        commands
    }

    /// Forces clear of all applied placements.
    pub fn clear_session(&mut self) -> MediaSessionCommand {
        self.applied.clear();
        self.surfaces.clear();
        MediaSessionCommand::Clear
    }

    /// Surfaces currently pending async content.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.surfaces.iter().filter(|s| s.pending).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;

    #[test]
    fn stale_async_is_rejected() {
        let mut host = CapabilityPreviewHost::truecolor(DesignSystem::default());
        let g0 = host.generation();
        host.place_image(Rect::new(0, 0, 10, 5), "a.png", true);
        host.bump_generation();
        assert!(!host.complete_async(g0, "a.png"));
        // Re-place under current generation, then complete.
        host.begin_frame();
        host.place_image(Rect::new(0, 0, 10, 5), "a.png", true);
        assert!(host.is_current(host.generation()));
        assert!(host.complete_async(host.generation(), "a.png"));
        assert_eq!(host.pending_count(), 0);
    }

    #[test]
    fn complete_async_false_when_no_surface() {
        let mut host = CapabilityPreviewHost::truecolor(DesignSystem::default());
        host.bump_generation();
        // Generation current but nothing registered.
        assert!(!host.complete_async(host.generation(), "missing.png"));
        host.place_image(Rect::new(0, 0, 8, 4), "a.png", true);
        host.begin_frame(); // clears surfaces
        assert!(!host.complete_async(host.generation(), "a.png"));
    }

    #[test]
    fn steady_state_same_resource_emits_no_session_commands() {
        let mut host =
            CapabilityPreviewHost::truecolor(DesignSystem::default()).protocols(true, false, false);
        let area = Rect::new(0, 0, 20, 10);
        host.bump_generation();
        host.begin_frame();
        host.place_image(area, "steady.png", false);
        let cmds1 = host.session_commands();
        assert!(
            matches!(
                cmds1.as_slice(),
                [MediaSessionCommand::Replace {
                    presentation: PreviewPresentation::Kitty,
                    resource_id,
                    ..
                }] if resource_id == "steady.png"
            ),
            "frame1 should place: {cmds1:?}"
        );
        let placement_id = match &cmds1[0] {
            MediaSessionCommand::Replace { placement_id, .. } => *placement_id,
            other => panic!("expected Replace, got {other:?}"),
        };

        // Frame 2: same selection, no generation bump — must not thrash.
        host.begin_frame();
        host.place_image(area, "steady.png", false);
        assert_eq!(host.surfaces[0].placement_id, placement_id);
        let cmds2 = host.session_commands();
        assert!(
            cmds2.is_empty(),
            "frame2 same resource must emit no commands: {cmds2:?}"
        );
    }

    #[test]
    fn session_replace_then_delete_on_selection_change() {
        let mut host =
            CapabilityPreviewHost::truecolor(DesignSystem::default()).protocols(true, false, false);
        host.bump_generation();
        host.begin_frame();
        host.place_image(Rect::new(0, 0, 20, 10), "one.png", false);
        let cmds = host.session_commands();
        assert!(
            matches!(
                cmds.as_slice(),
                [MediaSessionCommand::Replace {
                    presentation: PreviewPresentation::Kitty,
                    ..
                }]
            ),
            "{cmds:?}"
        );
        host.bump_generation();
        host.begin_frame();
        host.place_image(Rect::new(0, 0, 20, 10), "two.png", false);
        let cmds = host.session_commands();
        assert!(
            cmds.iter()
                .any(|c| matches!(c, MediaSessionCommand::Delete { .. })),
            "old placement deleted: {cmds:?}"
        );
        assert!(
            cmds.iter().any(|c| matches!(
                c,
                MediaSessionCommand::Replace {
                    resource_id,
                    ..
                } if resource_id == "two.png"
            )),
            "{cmds:?}"
        );
    }

    #[test]
    fn no_protocol_means_cell_fallback_only() {
        let mut host = CapabilityPreviewHost::truecolor(DesignSystem::default());
        host.place_image(Rect::new(0, 0, 4, 2), "x.png", false);
        assert_eq!(
            host.surfaces[0].presentation,
            PreviewPresentation::CellFallback
        );
        assert!(host.session_commands().is_empty());
    }

    /// Studio media scenario: loading → ready → reselection deletes old placement.
    #[test]
    fn studio_media_scenario_loading_ready_reselect() {
        let mut host =
            CapabilityPreviewHost::truecolor(DesignSystem::default()).protocols(true, false, false);
        host.bump_generation();
        host.begin_frame();
        host.place_image(Rect::new(0, 0, 20, 8), "a.png", true);
        assert_eq!(host.pending_count(), 1);
        let generation = host.generation();
        assert!(host.complete_async(generation, "a.png"));
        assert_eq!(host.pending_count(), 0);
        let cmds = host.session_commands();
        assert!(
            matches!(
                cmds.as_slice(),
                [MediaSessionCommand::Replace {
                    presentation: PreviewPresentation::Kitty,
                    resource_id,
                    ..
                }] if resource_id == "a.png"
            ),
            "{cmds:?}"
        );

        // Reselect B; late A must be rejected.
        let stale = host.generation();
        host.bump_generation();
        host.begin_frame();
        host.place_image(Rect::new(0, 0, 20, 8), "b.png", false);
        assert!(!host.complete_async(stale, "a.png"));
        let cmds = host.session_commands();
        assert!(
            cmds.iter()
                .any(|c| matches!(c, MediaSessionCommand::Delete { .. })),
            "{cmds:?}"
        );
        assert!(
            cmds.iter().any(|c| matches!(
                c,
                MediaSessionCommand::Replace { resource_id, .. } if resource_id == "b.png"
            )),
            "{cmds:?}"
        );
    }

    /// Studio media scenario: clear session yields Clear command.
    #[test]
    fn studio_media_scenario_clear_session() {
        let mut host =
            CapabilityPreviewHost::truecolor(DesignSystem::default()).protocols(false, true, false);
        host.bump_generation();
        host.begin_frame();
        host.place_image(Rect::new(1, 1, 10, 5), "shot.png", false);
        let _ = host.session_commands();
        assert_eq!(host.clear_session(), MediaSessionCommand::Clear);
        assert!(host.surfaces.is_empty());
    }
}
