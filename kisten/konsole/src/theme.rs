#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Theme {
    #[default]
    Default,
    Dark,
    Light,
    DarkColorblind,
    LightColorblind,
    DarkAnsi,
    LightAnsi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ThemePalette {
    pub(crate) accent: &'static str,
    pub(crate) selection: &'static str,
    pub(crate) warning: &'static str,
    pub(crate) dim: &'static str,
}

impl Theme {
    const ALL: [Self; 7] = [
        Self::Default,
        Self::Dark,
        Self::Light,
        Self::DarkColorblind,
        Self::LightColorblind,
        Self::DarkAnsi,
        Self::LightAnsi,
    ];

    pub(crate) fn all() -> impl ExactSizeIterator<Item = Self> {
        Self::ALL.into_iter()
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Dark => "dark",
            Self::Light => "light",
            Self::DarkColorblind => "dark-colorblind",
            Self::LightColorblind => "light-colorblind",
            Self::DarkAnsi => "dark-ansi",
            Self::LightAnsi => "light-ansi",
        }
    }

    pub(crate) fn from_stored_name(name: &str) -> Self {
        let name = name.trim();
        Self::all()
            .find(|theme| theme.name().eq_ignore_ascii_case(name))
            .unwrap_or_default()
    }

    pub(crate) const fn palette(self) -> ThemePalette {
        match self {
            Self::Default | Self::Dark => ThemePalette {
                accent: "\x1b[38;5;208m",
                selection: "\x1b[38;5;222m",
                warning: "\x1b[33m",
                dim: "\x1b[2m",
            },
            Self::Light => ThemePalette {
                accent: "\x1b[38;2;0;95;135m",
                selection: "\x1b[38;2;0;75;115m",
                warning: "\x1b[38;2;145;75;0m",
                dim: "\x1b[38;2;95;95;95m",
            },
            Self::DarkColorblind => ThemePalette {
                accent: "\x1b[38;2;86;180;233m",
                selection: "\x1b[38;2;230;159;0m",
                warning: "\x1b[38;2;240;228;66m",
                dim: "\x1b[38;2;158;158;158m",
            },
            Self::LightColorblind => ThemePalette {
                accent: "\x1b[38;2;0;114;178m",
                selection: "\x1b[38;2;213;94;0m",
                warning: "\x1b[38;2;160;110;0m",
                dim: "\x1b[38;2;90;90;90m",
            },
            Self::DarkAnsi => ThemePalette {
                accent: "\x1b[36m",
                selection: "\x1b[96m",
                warning: "\x1b[33m",
                dim: "\x1b[90m",
            },
            Self::LightAnsi => ThemePalette {
                accent: "\x1b[34m",
                selection: "\x1b[94m",
                warning: "\x1b[31m",
                dim: "\x1b[90m",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_theme_names_are_stable() {
        assert_eq!(
            Theme::all().map(Theme::name).collect::<Vec<_>>(),
            vec![
                "default",
                "dark",
                "light",
                "dark-colorblind",
                "light-colorblind",
                "dark-ansi",
                "light-ansi",
            ]
        );
    }

    #[test]
    fn stored_theme_name_is_canonicalized_and_unknown_values_fall_back() {
        assert_eq!(Theme::from_stored_name(" LIGHT "), Theme::Light);
        assert_eq!(Theme::from_stored_name("not-a-theme"), Theme::Default);
        assert_eq!(Theme::from_stored_name(""), Theme::Default);
    }

    #[test]
    fn each_theme_has_a_complete_palette() {
        for theme in Theme::all() {
            let palette = theme.palette();
            assert!(!palette.accent.is_empty(), "{} accent", theme.name());
            assert!(!palette.selection.is_empty(), "{} selection", theme.name());
            assert!(!palette.warning.is_empty(), "{} warning", theme.name());
            assert!(!palette.dim.is_empty(), "{} dim", theme.name());
        }
    }
}
