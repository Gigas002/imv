use std::path::PathBuf;

use libimgvwr::{
    keybinds::{KeybindMap, Keysym, keysym_from_str},
    renderer,
};

use crate::{
    cli::Cli,
    config::{Config, FilterMethod, Keybindings},
};

/// Runtime settings derived by merging CLI flags with the loaded config.
///
/// Priority for every field: CLI flag > config file value > built-in default.
/// After [`AppSettings::resolve`] returns, nothing downstream needs `Cli` or `Config`.
pub(crate) struct AppSettings {
    /// Image paths to open, taken directly from positional CLI arguments.
    pub(crate) paths: Vec<PathBuf>,
    /// Whether window title and server-side decorations are enabled (from `[window] decorations`).
    pub(crate) decorations: bool,
    /// Minimum zoom factor (from `[viewer] min_scale`).
    pub(crate) min_scale: f32,
    /// Maximum zoom factor (from `[viewer] max_scale`).
    pub(crate) max_scale: f32,
    /// Zoom step per scroll tick (from `[viewer] scale_step`).
    pub(crate) scale_step: f32,
    /// Resolved scaling filter for the renderer.
    pub(crate) filter: renderer::FilterMethod,
    /// Keysym-to-action map built from `[keybindings]`.
    pub(crate) keybind_map: KeybindMap,
    /// Hardcoded keysym for the left arrow key (previous image).
    pub(crate) key_left: Keysym,
    /// Hardcoded keysym for the right arrow key (next image).
    pub(crate) key_right: Keysym,
}

impl AppSettings {
    pub(crate) fn resolve(cli: &Cli, config: &Config) -> Self {
        let window = config.window.clone().unwrap_or_default();
        let viewer = config.viewer.clone().unwrap_or_default();
        let keybindings = config.keybindings.clone().unwrap_or_default();

        AppSettings {
            paths: cli.paths.clone(),
            decorations: window.decorations.unwrap_or(false),
            min_scale: viewer.min_scale.unwrap_or(0.1),
            max_scale: viewer.max_scale.unwrap_or(100.0),
            scale_step: viewer.scale_step.unwrap_or(0.08),
            filter: to_render_filter(
                viewer
                    .filter_method
                    .as_ref()
                    .unwrap_or(&FilterMethod::Lanczos3),
            ),
            keybind_map: build_keybind_map(&keybindings),
            key_left: keysym_from_str("Left").expect("Left keysym must resolve"),
            key_right: keysym_from_str("Right").expect("Right keysym must resolve"),
        }
    }
}

fn to_render_filter(f: &FilterMethod) -> renderer::FilterMethod {
    match f {
        FilterMethod::Nearest => renderer::FilterMethod::Nearest,
        FilterMethod::Triangle => renderer::FilterMethod::Triangle,
        FilterMethod::CatmullRom => renderer::FilterMethod::CatmullRom,
        FilterMethod::Gaussian => renderer::FilterMethod::Gaussian,
        FilterMethod::Lanczos3 => renderer::FilterMethod::Lanczos3,
    }
}

fn build_keybind_map(keybindings: &Keybindings) -> KeybindMap {
    let quit = resolve_keysym(keybindings.quit.as_deref().unwrap_or("q"), "q");
    let rotate_left = resolve_keysym(
        keybindings.rotate_left.as_deref().unwrap_or("bracketleft"),
        "bracketleft",
    );
    let rotate_right = resolve_keysym(
        keybindings
            .rotate_right
            .as_deref()
            .unwrap_or("bracketright"),
        "bracketright",
    );
    KeybindMap::new(quit, rotate_left, rotate_right)
}

// Try `name`; fall back to `fallback`, which must be a valid XKB keysym name.
fn resolve_keysym(name: &str, fallback: &str) -> Keysym {
    keysym_from_str(name)
        .or_else(|_| keysym_from_str(fallback))
        .expect("fallback keysym must resolve")
}
