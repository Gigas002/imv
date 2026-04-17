use std::path::PathBuf;

use tracing::{debug, info, warn};

#[cfg(feature = "decorations")]
use std::path::Path;

use image::DynamicImage;
use libimgvwr::{
    keybinds::Action,
    loader,
    navigator::Navigator,
    renderer,
    viewport::ViewportState,
    wayland::{InputEvent, WaylandContext},
};

use crate::settings::AppSettings;

struct EventOutcome {
    dirty: bool,
    quit: bool,
    #[cfg_attr(not(feature = "decorations"), allow(dead_code))]
    navigated: bool,
}

#[cfg(feature = "decorations")]
fn make_title(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("imgvwr");
    format!("{name} — imgvwr")
}

fn navigate_to(path: PathBuf, image: &mut DynamicImage, viewport: &mut ViewportState) -> bool {
    match loader::load(&path) {
        Ok(img) => {
            *image = img;
            viewport.reset();
            info!(path = %path.display(), "navigated to image");
            true
        }
        Err(e) => {
            warn!(path = %path.display(), error = %e, "failed to load image");
            false
        }
    }
}

fn process_event(
    event: InputEvent,
    settings: &AppSettings,
    navigator: &mut Navigator,
    image: &mut DynamicImage,
    viewport: &mut ViewportState,
) -> EventOutcome {
    let mut dirty = false;
    let mut quit = false;
    let mut navigated = false;

    match event {
        InputEvent::Key(sym) => {
            if sym == settings.key_left {
                let success = navigate_to(navigator.prev().to_path_buf(), image, viewport);
                dirty = success;
                navigated = success;
            } else if sym == settings.key_right {
                let success = navigate_to(navigator.next().to_path_buf(), image, viewport);
                dirty = success;
                navigated = success;
            } else if let Some(action) = settings.keybind_map.lookup(sym) {
                match action {
                    Action::Quit => quit = true,
                    Action::RotateLeft => {
                        viewport.rotate_left();
                        debug!(rotation = viewport.rotation, "rotated left");
                        dirty = true;
                    }
                    Action::RotateRight => {
                        viewport.rotate_right();
                        debug!(rotation = viewport.rotation, "rotated right");
                        dirty = true;
                    }
                }
            }
        }
        InputEvent::Scroll(delta) => {
            viewport.zoom_by(
                delta * settings.scale_step,
                settings.min_scale,
                settings.max_scale,
            );
            dirty = true;
        }
        InputEvent::PointerMotion { dx, dy } => {
            viewport.pan(dx, dy);
            dirty = true;
        }
        InputEvent::PointerButton { .. } => {}
    }

    EventOutcome {
        dirty,
        quit,
        navigated,
    }
}

pub fn run(settings: AppSettings) -> Result<(), Box<dyn std::error::Error>> {
    if settings.paths.is_empty() {
        return Err("no image paths given".into());
    }

    info!(
        decorations = settings.decorations,
        filter = ?settings.filter,
        "imgvwr starting"
    );

    let mut navigator = Navigator::from_path(&settings.paths[0])?;
    let mut image = loader::load(navigator.current())?;

    info!(path = %navigator.current().display(), "loaded first image");

    let mut viewport = ViewportState::default();
    let mut wayland = WaylandContext::connect((800, 600), settings.decorations)?;

    #[cfg(feature = "decorations")]
    if settings.decorations {
        wayland.set_title(&make_title(navigator.current()));
    }

    loop {
        wayland.dispatch(16)?;

        let events: Vec<InputEvent> = wayland.state.pending_events.drain(..).collect();
        let mut dirty = wayland.state.needs_redraw;
        wayland.state.needs_redraw = false;
        #[cfg(feature = "decorations")]
        let mut any_navigated = false;

        for event in events {
            let outcome =
                process_event(event, &settings, &mut navigator, &mut image, &mut viewport);
            dirty |= outcome.dirty;
            #[cfg(feature = "decorations")]
            {
                any_navigated |= outcome.navigated;
            }
            if outcome.quit {
                wayland.state.closed = true;
            }
        }

        #[cfg(feature = "decorations")]
        if settings.decorations && any_navigated {
            wayland.set_title(&make_title(navigator.current()));
        }

        if dirty {
            let (w, h) = wayland.state.window_size;
            let pixels = renderer::render(&image, &viewport, w, h, settings.filter);
            wayland.commit_frame(&pixels, w, h)?;
        }

        if wayland.state.closed {
            break;
        }
    }

    Ok(())
}
