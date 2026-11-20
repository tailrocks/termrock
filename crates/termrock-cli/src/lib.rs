use clap::{Parser, ValueEnum};

use termrock_core::motion::Motion;
use termrock_theme::{Charset, ColorLevel, Theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ColorArg {
    #[value(name = "truecolor", alias = "24bit")]
    TrueColor,
    #[value(name = "256")]
    Ansi256,
    #[value(name = "16")]
    Ansi16,
    #[value(name = "none", alias = "mono")]
    Mono,
}

impl From<ColorArg> for ColorLevel {
    fn from(value: ColorArg) -> Self {
        match value {
            ColorArg::TrueColor => Self::TrueColor,
            ColorArg::Ansi256 => Self::Ansi256,
            ColorArg::Ansi16 => Self::Ansi16,
            ColorArg::Mono => Self::Mono,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum GlyphsArg {
    #[value(name = "unicode", alias = "utf8")]
    Unicode,
    #[value(name = "ascii")]
    Ascii,
}

impl From<GlyphsArg> for Charset {
    fn from(value: GlyphsArg) -> Self {
        match value {
            GlyphsArg::Unicode => Self::Unicode,
            GlyphsArg::Ascii => Self::Ascii,
        }
    }
}

/// The options every application accepts.
#[derive(Clone, Debug, Default, PartialEq, Eq, clap::Args)]
pub struct CommonArgs {
    /// Colour level (default: detected from the terminal; NO_COLOR selects none)
    #[arg(short = 'c', long, value_enum, value_name = "LEVEL")]
    pub color: Option<ColorArg>,
    /// Glyphs and copy (default: detected from the terminal; ASCII on the
    /// Linux console, VT terminals and the legacy Windows console;
    /// JUNIE_TUI_GLYPHS=unicode|ascii overrides detection)
    #[arg(long, value_enum, value_name = "SET")]
    pub glyphs: Option<GlyphsArg>,
    /// How much moves (default: full, or reduced when the app's NO_MOTION variable is set)
    #[arg(short = 'm', long, value_enum, value_name = "MOTION")]
    pub motion: Option<Motion>,
    /// With --motion paused, the tick to render
    #[arg(short = 'f', long, default_value_t = 0, value_name = "N")]
    pub frame: u64,
}

impl CommonArgs {
    pub fn level(&self) -> ColorLevel {
        self.color
            .map(ColorLevel::from)
            .unwrap_or_else(ColorLevel::detect)
    }

    pub fn charset(&self) -> Charset {
        self.glyphs
            .map(Charset::from)
            .unwrap_or_else(Charset::detect)
    }

    pub fn theme(&self) -> Theme {
        self.charset().apply(Theme::for_level(self.level()))
    }

    pub fn motion(&self, env_var: &str) -> Motion {
        Motion::from_cli_or_env(self.motion, env_var)
    }
}

pub fn motion_help(env_var: &str) -> String {
    format!("Motion:    explicit --motion wins; otherwise {env_var}=1 selects reduced motion")
}

pub fn parse<C: Parser>(after_help: String) -> C {
    let matches = C::command().after_help(after_help).get_matches();
    C::from_arg_matches(&matches).unwrap_or_else(|error| error.exit())
}
