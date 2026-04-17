use std::path::PathBuf;

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
}

fn navigate_to(path: PathBuf, image: &mut DynamicImage, viewport: &mut ViewportState) -> bool {
    match loader::load(&path) {
        Ok(img) => {
            *image = img;
            viewport.reset();
            true
        }
        Err(e) => {
            eprintln!("imgvwr: {}: {e}", path.display());
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

    match event {
        InputEvent::Key(sym) => {
            if sym == settings.key_left {
                dirty = navigate_to(navigator.prev().to_path_buf(), image, viewport);
            } else if sym == settings.key_right {
                dirty = navigate_to(navigator.next().to_path_buf(), image, viewport);
            } else if let Some(action) = settings.keybind_map.lookup(sym) {
                match action {
                    Action::Quit => quit = true,
                    Action::RotateLeft => {
                        viewport.rotate_left();
                        dirty = true;
                    }
                    Action::RotateRight => {
                        viewport.rotate_right();
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

    EventOutcome { dirty, quit }
}

pub fn run(settings: AppSettings) -> Result<(), Box<dyn std::error::Error>> {
    if settings.paths.is_empty() {
        return Err("no image paths given".into());
    }

    let mut navigator = Navigator::from_path(&settings.paths[0])?;
    let mut image = loader::load(navigator.current())?;
    let mut viewport = ViewportState::default();
    let mut wayland = WaylandContext::connect((800, 600))?;

    loop {
        wayland.dispatch(16)?;

        let events: Vec<InputEvent> = wayland.state.pending_events.drain(..).collect();
        let mut dirty = wayland.state.needs_redraw;
        wayland.state.needs_redraw = false;

        for event in events {
            let outcome =
                process_event(event, &settings, &mut navigator, &mut image, &mut viewport);
            dirty |= outcome.dirty;
            if outcome.quit {
                wayland.state.closed = true;
            }
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
