// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Capability-aware theme projection and preview surface lifecycle.

use ratatui_core::layout::Rect;

use super::{ColorCapability, DesignSystem, DesignTokens, Theme, quantize_theme};

/// Kind of capability-aware preview surface (media/resource host).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PreviewSurfaceKind {
    /// Theme / design-system swatch.
    Theme,
    /// Image protocol surface (placement only; emission is consumer-owned).
    Image,
    /// Resource browser detail preview slot.
    ResourceDetail,
    /// Generic async payload placeholder.
    AsyncPayload,
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
}

/// Projects a design system to a capability ladder for deterministic previews.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityPreviewHost {
    /// Source tokens.
    pub system: DesignSystem,
    /// Active capability.
    pub capability: ColorCapability,
    /// Registered preview surfaces for the current frame.
    pub surfaces: Vec<PreviewSurface>,
}

impl CapabilityPreviewHost {
    /// Creates a host at truecolor (design target).
    #[must_use]
    pub fn truecolor(system: DesignSystem) -> Self {
        Self {
            system,
            capability: ColorCapability::Truecolor,
            surfaces: Vec::new(),
        }
    }

    /// Sets capability projection.
    #[must_use]
    pub const fn capability(mut self, capability: ColorCapability) -> Self {
        self.capability = capability;
        self
    }

    /// Theme quantized for the active capability.
    #[must_use]
    pub fn projected_theme(&self) -> Theme {
        quantize_theme(self.system.theme(), self.capability)
    }

    /// Tokens with projected theme.
    #[must_use]
    pub fn projected_tokens(&self) -> DesignTokens {
        let mut tokens = self.system.tokens.clone();
        tokens.theme = self.projected_theme();
        tokens
    }

    /// Clears per-frame surface registrations.
    pub fn begin_frame(&mut self) {
        self.surfaces.clear();
    }

    /// Registers a preview surface for the current frame.
    pub fn register_surface(&mut self, surface: PreviewSurface) {
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
    pub fn place_image(&mut self, area: Rect, resource_id: impl Into<String>, pending: bool) {
        self.surfaces.push(PreviewSurface {
            kind: PreviewSurfaceKind::Image,
            area,
            stale: false,
            pending,
            resource_id: Some(resource_id.into()),
        });
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

    #[test]
    fn monochrome_projection_is_deterministic() {
        let host = CapabilityPreviewHost::truecolor(DesignSystem::phosphor())
            .capability(ColorCapability::Monochrome);
        let a = host.projected_theme();
        let b = host.projected_theme();
        assert_eq!(a, b);
    }

    #[test]
    fn image_placement_and_stale_lifecycle() {
        let mut host = CapabilityPreviewHost::truecolor(DesignSystem::phosphor());
        host.begin_frame();
        host.place_image(Rect::new(0, 0, 10, 5), "img-1", true);
        assert_eq!(host.pending_count(), 1);
        host.mark_stale(PreviewSurfaceKind::Image);
        assert!(host.surfaces[0].stale);
        host.begin_frame();
        assert!(host.surfaces.is_empty());
    }
}
