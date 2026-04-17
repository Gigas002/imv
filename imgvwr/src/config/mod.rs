#[cfg(test)]
mod tests;

use std::{env, error::Error, fs::File, io::Read, path::PathBuf};

use tracing::{debug, info, warn};

use image::imageops::FilterType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub window: Option<Window>,
    pub viewer: Option<Viewer>,
    pub keybindings: Option<Keybindings>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            window: Some(Window::default()),
            viewer: Some(Viewer::default()),
            keybindings: Some(Keybindings::default()),
        }
    }
}

impl Config {
    pub fn load(path: &PathBuf) -> Result<Config, Box<dyn Error>> {
        let mut file = File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        toml::from_str(&content).map_err(|e: toml::de::Error| e.into())
    }

    /// Load and merge all config sources in priority order:
    /// built-in defaults → system → user (XDG or HOME) → `override_path`.
    pub fn load_merged(override_path: Option<&std::path::Path>) -> Config {
        let mut config = Config::default();

        let system = Config::get_system_path();
        if system.exists() {
            match Config::load(&system) {
                Ok(c) => {
                    info!(path = %system.display(), "loaded system config");
                    config = Config::merge(config, c);
                }
                Err(e) => {
                    warn!(path = %system.display(), error = %e, "failed to parse system config")
                }
            }
        }

        let user = Config::get_xdg_path().or_else(|_| Config::get_home_path());
        match user {
            Ok(ref p) if p.exists() => match Config::load(p) {
                Ok(c) => {
                    info!(path = %p.display(), "loaded user config");
                    config = Config::merge(config, c);
                }
                Err(e) => warn!(path = %p.display(), error = %e, "failed to parse user config"),
            },
            Err(e) => warn!(error = %e, "could not resolve user config path"),
            _ => {}
        }

        if let Some(path) = override_path {
            match Config::load(&path.to_path_buf()) {
                Ok(c) => {
                    info!(path = %path.display(), "loaded --config override");
                    config = Config::merge(config, c);
                }
                Err(e) => {
                    warn!(path = %path.display(), error = %e, "failed to parse --config override")
                }
            }
        }

        let w = config.window.as_ref();
        let v = config.viewer.as_ref();
        debug!(
            decorations = w.and_then(|w| w.decorations).unwrap_or(false),
            antialiasing = w.and_then(|w| w.antialiasing).unwrap_or(true),
            filter = ?v.and_then(|v| v.filter_method.as_ref()),
            min_scale = v.and_then(|v| v.min_scale).unwrap_or(0.1),
            max_scale = v.and_then(|v| v.max_scale).unwrap_or(100.0),
            scale_step = v.and_then(|v| v.scale_step).unwrap_or(0.08),
            "effective config"
        );

        config
    }

    pub fn get_xdg_path() -> Result<PathBuf, Box<dyn Error>> {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .ok_or_else(|| "XDG_CONFIG_HOME not set".into())
            .map(|p| p.join("imgvwr").join("config.toml"))
    }

    pub fn get_home_path() -> Result<PathBuf, Box<dyn Error>> {
        env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| "HOME not set".into())
            .map(|p| p.join(".config").join("imgvwr").join("config.toml"))
    }

    pub fn get_system_path() -> PathBuf {
        PathBuf::from("/etc/imgvwr/config.toml")
    }

    pub fn merge(base: Config, overlay: Config) -> Config {
        Config {
            window: merge_section(base.window, overlay.window, |b, o| Window {
                decorations: o.decorations.or(b.decorations),
                antialiasing: o.antialiasing.or(b.antialiasing),
            }),
            viewer: merge_section(base.viewer, overlay.viewer, |b, o| Viewer {
                min_scale: o.min_scale.or(b.min_scale),
                max_scale: o.max_scale.or(b.max_scale),
                scale_step: o.scale_step.or(b.scale_step),
                filter_method: o.filter_method.or(b.filter_method),
            }),
            keybindings: merge_section(base.keybindings, overlay.keybindings, |b, o| Keybindings {
                quit: o.quit.or(b.quit),
                rotate_left: o.rotate_left.or(b.rotate_left),
                rotate_right: o.rotate_right.or(b.rotate_right),
            }),
        }
    }
}

fn merge_section<T, F: FnOnce(T, T) -> T>(base: Option<T>, overlay: Option<T>, f: F) -> Option<T> {
    match (base, overlay) {
        (Some(b), Some(o)) => Some(f(b, o)),
        (b, o) => o.or(b),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Window {
    pub decorations: Option<bool>,
    pub antialiasing: Option<bool>,
}

impl Default for Window {
    fn default() -> Self {
        Window {
            decorations: Some(false),
            antialiasing: Some(true),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Viewer {
    pub min_scale: Option<f32>,
    pub max_scale: Option<f32>,
    pub scale_step: Option<f32>,
    pub filter_method: Option<FilterMethod>,
}

impl Default for Viewer {
    fn default() -> Self {
        Viewer {
            min_scale: Some(0.1),
            max_scale: Some(100.0),
            scale_step: Some(0.08),
            filter_method: Some(FilterMethod::default()),
        }
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterMethod {
    Nearest,
    Triangle,
    CatmullRom,
    Gaussian,
    #[default]
    Lanczos3,
}

impl From<FilterMethod> for FilterType {
    fn from(f: FilterMethod) -> FilterType {
        match f {
            FilterMethod::Nearest => FilterType::Nearest,
            FilterMethod::Triangle => FilterType::Triangle,
            FilterMethod::CatmullRom => FilterType::CatmullRom,
            FilterMethod::Gaussian => FilterType::Gaussian,
            FilterMethod::Lanczos3 => FilterType::Lanczos3,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Keybindings {
    pub quit: Option<String>,
    pub rotate_left: Option<String>,
    pub rotate_right: Option<String>,
}

impl Default for Keybindings {
    fn default() -> Self {
        Keybindings {
            quit: Some("q".to_string()),
            rotate_left: Some("[".to_string()),
            rotate_right: Some("]".to_string()),
        }
    }
}
