// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! OS / terminal light-dark appearance hints for auto theme mapping.

/// Preferred UI polarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Appearance {
    /// Dark surfaces.
    #[default]
    Dark,
    /// Light surfaces.
    Light,
    /// Could not be determined.
    Unknown,
}

/// Mapping from appearance to named theme presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AppearanceThemeMap {
    /// Theme key used for dark appearance.
    pub dark: &'static str,
    /// Theme key used for light appearance.
    pub light: &'static str,
}

impl Default for AppearanceThemeMap {
    fn default() -> Self {
        Self {
            dark: "phosphor",
            light: "slate",
        }
    }
}

impl Appearance {
    /// Best-effort detection without GUI toolkits.
    ///
    /// Order:
    /// 1. `TERMROCK_APPEARANCE` / `GROK_APPEARANCE` / `LC_GROK_APPEARANCE` (`dark`|`light`)
    /// 2. `COLORFGBG` (last field is background index; low = dark, high = light)
    /// 3. macOS `defaults read -g AppleInterfaceStyle` when available
    /// 4. Unknown
    #[must_use]
    pub fn detect() -> Self {
        for key in [
            "TERMROCK_APPEARANCE",
            "GROK_APPEARANCE",
            "LC_GROK_APPEARANCE",
        ] {
            if let Ok(value) = std::env::var(key) {
                match value.to_ascii_lowercase().as_str() {
                    "dark" => return Self::Dark,
                    "light" => return Self::Light,
                    _ => {}
                }
            }
        }
        if let Ok(fgbg) = std::env::var("COLORFGBG")
            && let Some(bg) = fgbg.split(';').next_back()
            && let Ok(index) = bg.parse::<u8>()
        {
            // Common convention: 0–7 dark backgrounds, 8–15 light-ish.
            return if index <= 7 { Self::Dark } else { Self::Light };
        }
        #[cfg(target_os = "macos")]
        {
            if let Some(value) = read_macos_interface_style() {
                return value;
            }
        }
        Self::Unknown
    }

    /// Resolves a preset theme name from an appearance map.
    #[must_use]
    pub const fn theme_key(self, map: AppearanceThemeMap) -> &'static str {
        match self {
            Self::Light => map.light,
            Self::Dark | Self::Unknown => map.dark,
        }
    }
}

/// Builds a theme for the current appearance using standard presets.
#[must_use]
pub fn palette_for_appearance(appearance: Appearance) -> crate::style::RolePalette {
    match appearance {
        Appearance::Light => crate::style::RolePalette::slate(),
        Appearance::Dark | Appearance::Unknown => crate::style::RolePalette::tailrocks_phosphor(),
    }
}

#[cfg(target_os = "macos")]
fn read_macos_interface_style() -> Option<Appearance> {
    let output = std::process::Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .ok()?;
    if !output.status.success() {
        // Missing key usually means Light.
        return Some(Appearance::Light);
    }
    let text = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    if text.contains("dark") {
        Some(Appearance::Dark)
    } else {
        Some(Appearance::Light)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_key_maps_light_and_dark() {
        let map = AppearanceThemeMap::default();
        assert_eq!(Appearance::Light.theme_key(map), "slate");
        assert_eq!(Appearance::Dark.theme_key(map), "phosphor");
        assert_eq!(Appearance::Unknown.theme_key(map), "phosphor");
    }

    #[test]
    fn palette_for_appearance_diverges() {
        let dark = palette_for_appearance(Appearance::Dark);
        let light = palette_for_appearance(Appearance::Light);
        assert_ne!(
            dark.style(crate::style::Role::Canvas),
            light.style(crate::style::Role::Canvas)
        );
    }
}
