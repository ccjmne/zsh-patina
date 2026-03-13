use std::{collections::HashMap, fmt::Formatter, fs, str::FromStr};

use anyhow::{Context, Result};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error, MapAccess, Visitor, value::MapAccessDeserializer},
};
use syntect::{
    highlighting::{
        Color as SyntectColor, FontStyle, ScopeSelector, ScopeSelectors, StyleModifier,
        Theme as SyntectTheme, ThemeItem, ThemeSettings,
    },
    parsing::ScopeStack,
};

use crate::color::Color;

#[derive(Clone, PartialEq, Eq)]
pub enum ThemeSource {
    Simple,
    Patina,
    Lavender,
    File(String),
}

impl Serialize for ThemeSource {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            ThemeSource::Simple => serializer.serialize_str("simple"),
            ThemeSource::Patina => serializer.serialize_str("patina"),
            ThemeSource::Lavender => serializer.serialize_str("lavender"),
            ThemeSource::File(path) => serializer.serialize_str(&format!("file:{path}")),
        }
    }
}

impl<'de> Deserialize<'de> for ThemeSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "simple" => Ok(ThemeSource::Simple),
            "patina" => Ok(ThemeSource::Patina),
            "lavender" => Ok(ThemeSource::Lavender),
            _ if s.starts_with("file:") => Ok(ThemeSource::File(s[5..].to_string())),
            _ => Err(Error::custom(format!("Unsupported theme source: {s}"))),
        }
    }
}

pub struct Style {
    pub foreground: Color,
    pub background: Option<Color>,
    pub bold: bool,
    pub underline: bool,
}

impl TryFrom<&Style> for StyleModifier {
    type Error = anyhow::Error;

    fn try_from(style: &Style) -> Result<Self> {
        let font_style = if style.bold || style.underline {
            let mut fs = FontStyle::empty();
            if style.bold {
                fs.set(FontStyle::BOLD, true);
            }
            if style.underline {
                fs.set(FontStyle::UNDERLINE, true);
            }
            Some(fs)
        } else {
            None
        };
        Ok(Self {
            foreground: Some(style.foreground.into()),
            background: style.background.as_ref().map(Into::into),
            font_style,
        })
    }
}

impl<'de> Deserialize<'de> for Style {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringOrStruct;

        impl<'de> Visitor<'de> for StringOrStruct {
            type Value = Style;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("string or style struct")
            }

            fn visit_str<E>(self, value: &str) -> Result<Style, E>
            where
                E: serde::de::Error,
            {
                Ok(Style {
                    foreground: Color::try_from(value).map_err(E::custom)?,
                    background: None,
                    bold: false,
                    underline: false,
                })
            }

            fn visit_map<M>(self, map: M) -> Result<Style, M::Error>
            where
                M: MapAccess<'de>,
            {
                #[derive(Deserialize)]
                struct Helper {
                    foreground: String,
                    background: Option<String>,
                    #[serde(default)]
                    bold: bool,
                    #[serde(default)]
                    underline: bool,
                }

                let h = Helper::deserialize(MapAccessDeserializer::new(map))?;

                Ok(Style {
                    foreground: Color::try_from(h.foreground.as_str()).map_err(M::Error::custom)?,
                    background: h
                        .background
                        .map(|bg| Color::try_from(bg.as_str()).map_err(M::Error::custom))
                        .transpose()?,
                    bold: h.bold,
                    underline: h.underline,
                })
            }
        }

        deserializer.deserialize_any(StringOrStruct)
    }
}

#[derive(Deserialize)]
pub struct Theme {
    #[serde(flatten)]
    scopes: HashMap<String, Style>,
}

impl Theme {
    /// Load a built-in theme or a custom one from a file
    pub fn load(source: &ThemeSource) -> Result<Self> {
        Ok(match source {
            ThemeSource::Simple => toml::from_slice(include_bytes!("../themes/simple.toml"))
                .context("Unable to load simple theme")?,
            ThemeSource::Patina => toml::from_slice(include_bytes!("../themes/patina.toml"))
                .context("Unable to load default theme")?,
            ThemeSource::Lavender => toml::from_slice(include_bytes!("../themes/lavender.toml"))
                .context("Unable to load lavender theme")?,
            ThemeSource::File(path) => {
                let theme_source = fs::read_to_string(path)
                    .with_context(|| format!("Failed to read theme file `{path}'"))?;
                toml::from_str(&theme_source)
                    .with_context(|| format!("Failed to parse theme file `{path}'"))?
            }
        })
    }

    /// Resolve a scope to a color by looking it up in the theme. If the scope
    /// is not found, its parent scopes are tried until a match is found or
    /// there are no more parent scopes left.
    pub fn resolve<'a>(&'a self, scope: &str) -> Option<&'a Style> {
        let mut s = scope;
        while !s.is_empty() {
            if let Some(c) = self.scopes.get(s) {
                return Some(c);
            }
            s = s.rsplit_once('.')?.0;
        }
        None
    }
}

impl TryFrom<Theme> for SyntectTheme {
    type Error = anyhow::Error;

    fn try_from(theme: Theme) -> Result<Self> {
        Ok(SyntectTheme {
            settings: ThemeSettings {
                foreground: Some(SyntectColor {
                    r: 7,
                    g: 0,
                    b: 0,
                    a: 0,
                }),
                // this will be converted to `None` in the highlighter module:
                background: Some(SyntectColor {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 1,
                }),
                ..Default::default()
            },
            scopes: theme
                .scopes
                .iter()
                .map(|s| {
                    Ok(ThemeItem {
                        scope: ScopeSelectors {
                            selectors: vec![ScopeSelector {
                                path: ScopeStack::from_str(s.0)?,
                                ..Default::default()
                            }],
                        },
                        style: s.1.try_into()?,
                    })
                })
                .collect::<Result<_>>()?,
            ..Default::default()
        })
    }
}
