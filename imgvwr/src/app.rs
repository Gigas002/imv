use std::path::PathBuf;

use tracing::{debug, info, warn};

#[cfg(feature = "decorations")]
use std::path::Path;

use image::DynamicImage;
use libimgvwr::{
    keybinds::{Action, Keysym},
    loader,
    navigator::Navigator,
    renderer,
    viewport::ViewportState,
    wayland::{InputEvent, WaylandContext},
};

use crate::settings::AppSettings;

#[derive(Default)]
struct EventOutcome {
    dirty: bool,
    quit: bool,
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

fn fit_scale(img: &DynamicImage, window: (u32, u32), min_scale: f32, max_scale: f32) -> f32 {
    let sw = window.0 as f32 / img.width() as f32;
    let sh = window.1 as f32 / img.height() as f32;
    sw.min(sh).clamp(min_scale, max_scale)
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

fn on_navigate_prev(
    navigator: &mut Navigator,
    image: &mut DynamicImage,
    viewport: &mut ViewportState,
) -> EventOutcome {
    let path = navigator.prev().to_path_buf();
    let success = navigate_to(path, image, viewport);
    EventOutcome {
        dirty: success,
        navigated: success,
        quit: false,
    }
}

fn on_navigate_next(
    navigator: &mut Navigator,
    image: &mut DynamicImage,
    viewport: &mut ViewportState,
) -> EventOutcome {
    let path = navigator.next().to_path_buf();
    let success = navigate_to(path, image, viewport);
    EventOutcome {
        dirty: success,
        navigated: success,
        quit: false,
    }
}

fn on_rotate_left(viewport: &mut ViewportState) -> EventOutcome {
    viewport.rotate_left();
    debug!(rotation = viewport.rotation, "rotated left");
    EventOutcome {
        dirty: true,
        ..Default::default()
    }
}

fn on_rotate_right(viewport: &mut ViewportState) -> EventOutcome {
    viewport.rotate_right();
    debug!(rotation = viewport.rotation, "rotated right");
    EventOutcome {
        dirty: true,
        ..Default::default()
    }
}

fn on_delete_file(
    navigator: &mut Navigator,
    image: &mut DynamicImage,
    viewport: &mut ViewportState,
) -> EventOutcome {
    let path = navigator.current().to_path_buf();
    match std::fs::remove_file(&path) {
        Ok(()) => {
            info!(path = %path.display(), "deleted file");
            match navigator.remove_current() {
                Some(next) => {
                    let next = next.to_path_buf();
                    let success = navigate_to(next, image, viewport);
                    EventOutcome {
                        dirty: success,
                        navigated: success,
                        quit: false,
                    }
                }
                None => EventOutcome {
                    quit: true,
                    ..Default::default()
                },
            }
        }
        Err(e) => {
            warn!(path = %path.display(), error = %e, "failed to delete file");
            EventOutcome::default()
        }
    }
}

fn on_scroll(delta: f32, settings: &AppSettings, viewport: &mut ViewportState) -> EventOutcome {
    viewport.zoom_by(
        delta * settings.scale_step,
        settings.min_scale,
        settings.max_scale,
    );
    EventOutcome {
        dirty: true,
        ..Default::default()
    }
}

fn on_pointer_motion(dx: f32, dy: f32, viewport: &mut ViewportState) -> EventOutcome {
    viewport.pan(dx, dy);
    EventOutcome {
        dirty: true,
        ..Default::default()
    }
}

fn on_key_action(
    sym: Keysym,
    settings: &AppSettings,
    navigator: &mut Navigator,
    image: &mut DynamicImage,
    viewport: &mut ViewportState,
) -> EventOutcome {
    if sym == settings.key_left {
        on_navigate_prev(navigator, image, viewport)
    } else if sym == settings.key_right {
        on_navigate_next(navigator, image, viewport)
    } else if let Some(action) = settings.keybind_map.lookup(sym) {
        match action {
            Action::Quit => EventOutcome {
                quit: true,
                ..Default::default()
            },
            Action::RotateLeft => on_rotate_left(viewport),
            Action::RotateRight => on_rotate_right(viewport),
            Action::DeleteFile => on_delete_file(navigator, image, viewport),
        }
    } else {
        EventOutcome::default()
    }
}

fn process_event(
    event: InputEvent,
    settings: &AppSettings,
    navigator: &mut Navigator,
    image: &mut DynamicImage,
    viewport: &mut ViewportState,
) -> EventOutcome {
    match event {
        InputEvent::Key(sym) => on_key_action(sym, settings, navigator, image, viewport),
        InputEvent::Scroll(delta) => on_scroll(delta, settings, viewport),
        InputEvent::PointerMotion { dx, dy } => on_pointer_motion(dx, dy, viewport),
        InputEvent::PointerButton { .. } => EventOutcome::default(),
    }
}

pub fn run(settings: AppSettings) -> Result<(), Box<dyn std::error::Error>> {
    if settings.paths.is_empty() {
        return Err("no image paths given".into());
    }

    info!(
        decorations = settings.decorations,
        antialiasing = settings.antialiasing,
        filter = ?settings.filter,
        "imgvwr starting"
    );

    let mut navigator = Navigator::from_path(&settings.paths[0])?;
    let mut image = loader::load(navigator.current())?;
    info!(path = %navigator.current().display(), "loaded first image");

    let mut viewport = ViewportState::default();
    let mut wayland = WaylandContext::connect((800, 600), settings.decorations)?;

    viewport.scale = fit_scale(
        &image,
        wayland.state.window_size,
        settings.min_scale,
        settings.max_scale,
    );

    #[cfg(feature = "decorations")]
    if settings.decorations {
        wayland.set_title(&make_title(navigator.current()));
    }

    loop {
        wayland.dispatch(16)?;

        let events: Vec<InputEvent> = wayland.state.pending_events.drain(..).collect();
        let mut dirty = wayland.state.needs_redraw;
        wayland.state.needs_redraw = false;
        let mut any_navigated = false;

        for event in events {
            let outcome =
                process_event(event, &settings, &mut navigator, &mut image, &mut viewport);
            dirty |= outcome.dirty;
            any_navigated |= outcome.navigated;
            if outcome.quit {
                wayland.state.closed = true;
            }
        }

        if any_navigated {
            viewport.scale = fit_scale(
                &image,
                wayland.state.window_size,
                settings.min_scale,
                settings.max_scale,
            );
            dirty = true;

            #[cfg(feature = "decorations")]
            if settings.decorations {
                wayland.set_title(&make_title(navigator.current()));
            }
        }

        if dirty {
            let (w, h) = wayland.state.window_size;
            let effective_filter = if settings.antialiasing {
                settings.filter
            } else {
                renderer::FilterMethod::Nearest
            };
            let pixels = renderer::render(&image, &viewport, w, h, effective_filter);
            wayland.commit_frame(&pixels, w, h)?;
        }

        if wayland.state.closed {
            break;
        }
    }

    Ok(())
}
