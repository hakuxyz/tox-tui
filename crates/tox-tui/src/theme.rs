// color themes: built-ins + custom ones from themes.toml

use anyhow::{Context, Result};
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// full color palette used by the ui
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub name: String,
    // focused border / badge
    pub accent: String,
    // dim border / secondary text
    pub accent_dim: String,
    pub online: String,
    pub offline: String,
    pub warn: String,
    pub error: String,
    pub header_bg: String,
    pub header_fg: String,
    pub selected_bg: String,
    pub selected_fg: String,
    pub log: String,
    pub outgoing: String,
    pub incoming: String,
    pub system: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "default".into(),
            accent: "#5fd7ff".into(),
            accent_dim: "#444b5a".into(),
            online: "#5fd787".into(),
            offline: "#6c7086".into(),
            warn: "#f9e2af".into(),
            error: "#f38ba8".into(),
            header_bg: "#5fd7ff".into(),
            header_fg: "#11111b".into(),
            selected_bg: "#313244".into(),
            selected_fg: "#cdd6f4".into(),
            log: "#6c7086".into(),
            outgoing: "#cdd6f4".into(),
            incoming: "#5fd7ff".into(),
            system: "#a6adc8".into(),
        }
    }
}

impl Theme {
    // parse #rgb / #rrggbb / short names
    pub fn color(&self, hex: &str) -> Color {
        parse_color(hex)
    }

    pub fn accent(&self) -> Color {
        self.color(&self.accent)
    }
    pub fn accent_dim(&self) -> Color {
        self.color(&self.accent_dim)
    }
    pub fn online(&self) -> Color {
        self.color(&self.online)
    }
    pub fn offline(&self) -> Color {
        self.color(&self.offline)
    }
    pub fn warn(&self) -> Color {
        self.color(&self.warn)
    }
    pub fn error(&self) -> Color {
        self.color(&self.error)
    }
    pub fn header_bg(&self) -> Color {
        self.color(&self.header_bg)
    }
    pub fn header_fg(&self) -> Color {
        self.color(&self.header_fg)
    }
    pub fn selected_bg(&self) -> Color {
        self.color(&self.selected_bg)
    }
    pub fn selected_fg(&self) -> Color {
        self.color(&self.selected_fg)
    }
    pub fn log(&self) -> Color {
        self.color(&self.log)
    }
    pub fn outgoing(&self) -> Color {
        self.color(&self.outgoing)
    }
    pub fn incoming(&self) -> Color {
        self.color(&self.incoming)
    }
    pub fn system(&self) -> Color {
        self.color(&self.system)
    }
}

// #rgb, #rrggbb, or a few ansi names
pub fn parse_color(s: &str) -> Color {
    let s = s.trim();
    let hex = s.strip_prefix('#').unwrap_or(s);
    match hex.len() {
        3 if hex.chars().all(|c| c.is_ascii_hexdigit()) => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).unwrap_or(0);
            Color::Rgb(r, g, b)
        }
        6 if hex.chars().all(|c| c.is_ascii_hexdigit()) => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            Color::Rgb(r, g, b)
        }
        _ => match s.to_ascii_lowercase().as_str() {
            "black" => Color::Black,
            "red" => Color::Red,
            "green" => Color::Green,
            "yellow" => Color::Yellow,
            "blue" => Color::Blue,
            "magenta" => Color::Magenta,
            "cyan" => Color::Cyan,
            "gray" | "grey" => Color::Gray,
            "darkgray" | "darkgrey" => Color::DarkGray,
            "white" => Color::White,
            "reset" | "default" => Color::Reset,
            _ => Color::Reset,
        },
    }
}

// built-in themes

fn theme(name: &str, over: Theme) -> Theme {
    Theme {
        name: name.into(),
        ..over
    }
}

pub fn builtin(name: &str) -> Option<Theme> {
    let d = Theme::default();
    let t = match name {
        "default" | "dark" => d,
        "light" => theme(
            "light",
            Theme {
                accent: "#005f87".into(),
                accent_dim: "#b2b2b2".into(),
                online: "#008700".into(),
                offline: "#8a8a8a".into(),
                warn: "#af5f00".into(),
                error: "#af0000".into(),
                header_bg: "#005f87".into(),
                header_fg: "#ffffff".into(),
                selected_bg: "#d0d0d0".into(),
                selected_fg: "#000000".into(),
                log: "#8a8a8a".into(),
                outgoing: "#000000".into(),
                incoming: "#005f87".into(),
                system: "#5f5f5f".into(),
                ..d.clone()
            },
        ),
        "nord" => theme(
            "nord",
            Theme {
                accent: "#88c0d0".into(),
                accent_dim: "#4c566a".into(),
                online: "#a3be8c".into(),
                offline: "#4c566a".into(),
                warn: "#ebcb8b".into(),
                error: "#bf616a".into(),
                header_bg: "#88c0d0".into(),
                header_fg: "#2e3440".into(),
                selected_bg: "#434c5e".into(),
                selected_fg: "#eceff4".into(),
                log: "#4c566a".into(),
                outgoing: "#eceff4".into(),
                incoming: "#8fbcbb".into(),
                system: "#d8dee9".into(),
                ..d.clone()
            },
        ),
        "gruvbox" => theme(
            "gruvbox",
            Theme {
                accent: "#83a598".into(),
                accent_dim: "#665c54".into(),
                online: "#b8bb26".into(),
                offline: "#7c6f64".into(),
                warn: "#fabd2f".into(),
                error: "#fb4934".into(),
                header_bg: "#83a598".into(),
                header_fg: "#282828".into(),
                selected_bg: "#504945".into(),
                selected_fg: "#ebdbb2".into(),
                log: "#928374".into(),
                outgoing: "#ebdbb2".into(),
                incoming: "#8ec07c".into(),
                system: "#d5c4a1".into(),
                ..d.clone()
            },
        ),
        "solarized-dark" => theme(
            "solarized-dark",
            Theme {
                accent: "#268bd2".into(),
                accent_dim: "#586e75".into(),
                online: "#859900".into(),
                offline: "#586e75".into(),
                warn: "#b58900".into(),
                error: "#dc322f".into(),
                header_bg: "#268bd2".into(),
                header_fg: "#fdf6e3".into(),
                selected_bg: "#073642".into(),
                selected_fg: "#eee8d5".into(),
                log: "#657b83".into(),
                outgoing: "#eee8d5".into(),
                incoming: "#2aa198".into(),
                system: "#93a1a1".into(),
                ..d.clone()
            },
        ),
        "solarized-light" => theme(
            "solarized-light",
            Theme {
                accent: "#268bd2".into(),
                accent_dim: "#93a1a1".into(),
                online: "#859900".into(),
                offline: "#93a1a1".into(),
                warn: "#b58900".into(),
                error: "#dc322f".into(),
                header_bg: "#268bd2".into(),
                header_fg: "#fdf6e3".into(),
                selected_bg: "#eee8d5".into(),
                selected_fg: "#073642".into(),
                log: "#839496".into(),
                outgoing: "#657b83".into(),
                incoming: "#2aa198".into(),
                system: "#586e75".into(),
                ..d.clone()
            },
        ),
        "dracula" => theme(
            "dracula",
            Theme {
                accent: "#bd93f9".into(),
                accent_dim: "#6272a4".into(),
                online: "#50fa7b".into(),
                offline: "#6272a4".into(),
                warn: "#f1fa8c".into(),
                error: "#ff5555".into(),
                header_bg: "#bd93f9".into(),
                header_fg: "#282a36".into(),
                selected_bg: "#44475a".into(),
                selected_fg: "#f8f8f2".into(),
                log: "#6272a4".into(),
                outgoing: "#f8f8f2".into(),
                incoming: "#8be9fd".into(),
                system: "#bbbbbb".into(),
                ..d.clone()
            },
        ),
        "monochrome" => theme(
            "monochrome",
            Theme {
                accent: "#ffffff".into(),
                accent_dim: "#555555".into(),
                online: "#ffffff".into(),
                offline: "#555555".into(),
                warn: "#cccccc".into(),
                error: "#ffffff".into(),
                header_bg: "#ffffff".into(),
                header_fg: "#000000".into(),
                selected_bg: "#333333".into(),
                selected_fg: "#ffffff".into(),
                log: "#777777".into(),
                outgoing: "#ffffff".into(),
                incoming: "#dddddd".into(),
                system: "#999999".into(),
                ..d.clone()
            },
        ),
        _ => return None,
    };
    Some(t)
}

pub fn builtin_names() -> Vec<&'static str> {
    vec![
        "default",
        "light",
        "nord",
        "gruvbox",
        "solarized-dark",
        "solarized-light",
        "dracula",
        "monochrome",
    ]
}

// on-disk shape: [themes.<name>] tables + active name
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ThemesFile {
    #[serde(default)]
    pub themes: BTreeMap<String, Theme>,
    // last used theme name
    #[serde(default)]
    pub active: Option<String>,
}

pub struct ThemeRegistry {
    pub customs: BTreeMap<String, Theme>,
    pub active: String,
    path: PathBuf,
}

impl ThemeRegistry {
    pub fn load(data_dir: &Path) -> Result<Self> {
        let path = data_dir.join("themes.toml");
        let mut customs = BTreeMap::new();
        let mut active = "default".to_string();

        if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?;
            match toml::from_str::<ThemesFile>(&raw) {
                Ok(file) => {
                    customs = file.themes;
                    if let Some(a) = file.active {
                        active = a;
                    }
                }
                Err(e) => {
                    log::warn!("themes.toml parse error: {e}");
                }
            }
        } else {
            // Seed an example custom theme so users see the format.
            let example = ThemesFile {
                themes: BTreeMap::from([(
                    "example".into(),
                    Theme {
                        name: "example".into(),
                        accent: "#ff79c6".into(),
                        ..Theme::default()
                    },
                )]),
                active: None,
            };
            if let Ok(raw) = toml::to_string_pretty(&example) {
                let _ = std::fs::write(&path, raw);
            }
        }

        // Resolve active: custom wins over builtin only if the name matches.
        if !customs.contains_key(&active) && builtin(&active).is_none() {
            active = "default".into();
        }

        Ok(Self {
            customs,
            active,
            path,
        })
    }

    pub fn get(&self, name: &str) -> Option<Theme> {
        if let Some(t) = self.customs.get(name) {
            let mut t = t.clone();
            t.name = name.to_string();
            return Some(t);
        }
        builtin(name)
    }

    pub fn all_names(&self) -> Vec<String> {
        let mut v: Vec<String> = builtin_names().iter().map(|s| s.to_string()).collect();
        v.extend(self.customs.keys().cloned());
        v
    }

    // switch theme and save themes.toml
    pub fn set_active(&mut self, name: &str) -> Option<Theme> {
        let t = self.get(name)?;
        self.active = name.to_string();
        self.save();
        Some(t)
    }

    pub fn save(&self) {
        let file = ThemesFile {
            themes: self.customs.clone(),
            active: Some(self.active.clone()),
        };
        if let Ok(raw) = toml::to_string_pretty(&file) {
            let tmp = self.path.with_extension("toml.tmp");
            if std::fs::write(&tmp, raw).is_ok() {
                let _ = std::fs::rename(&tmp, &self.path);
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_colors() {
        assert_eq!(parse_color("#ff0000"), Color::Rgb(255, 0, 0));
        assert_eq!(parse_color("00ff00"), Color::Rgb(0, 255, 0));
        assert_eq!(parse_color("#fff"), Color::Rgb(255, 255, 255));
        assert_eq!(parse_color("red"), Color::Red);
    }

    #[test]
    fn all_builtins_resolve() {
        for name in builtin_names() {
            let t = builtin(name).expect(name);
            assert_eq!(t.name, name);
            // Colors must at least parse without panic.
            let _ = t.accent();
            let _ = t.online();
        }
    }

    #[test]
    fn unknown_theme_is_none() {
        assert!(builtin("does-not-exist").is_none());
    }

    #[test]
    fn registry_roundtrip() {
        let dir = std::env::temp_dir().join(format!("tox-theme-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut reg = ThemeRegistry::load(&dir).unwrap();
        assert!(reg.get("default").is_some());
        assert!(reg.path().exists(), "example themes.toml seeded");

        // Register a custom theme and switch to it.
        reg.customs.insert(
            "mine".into(),
            Theme {
                name: "mine".into(),
                accent: "#123456".into(),
                ..Theme::default()
            },
        );
        assert!(reg.set_active("mine").is_some());

        // Reload from disk.
        let reg2 = ThemeRegistry::load(&dir).unwrap();
        assert_eq!(reg2.active, "mine");
        let t = reg2.get("mine").unwrap();
        assert_eq!(t.accent(), Color::Rgb(0x12, 0x34, 0x56));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
