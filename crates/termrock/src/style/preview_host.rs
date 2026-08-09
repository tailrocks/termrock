// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Capability-aware theme projection for previews and lookbook hosts.

use super::{ColorCapability, DesignSystem, DesignTokens, Theme, quantize_theme};

/// Projects a design system to a capability ladder for deterministic previews.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityPreviewHost {
    /// Source tokens.
    pub system: DesignSystem,
    /// Active capability.
    pub capability: ColorCapability,
}

impl CapabilityPreviewHost {
    /// Creates a host at truecolor (design target).
    #[must_use]
    pub fn truecolor(system: DesignSystem) -> Self {
        Self {
            system,
            capability: ColorCapability::Truecolor,
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
}
