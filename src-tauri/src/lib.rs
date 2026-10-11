//! App shell: wires the engine, the controller layer and the library to the frontend.

mod audio_service;
mod commands;
mod controller_service;
mod dto;
mod library_service;

use std::sync::Arc;

use tauri::{Emitter, Manager};
use tracing_subscriber::EnvFilter;

use audio_service::{AudioService, UiEvent};
use commands::StateChannel;
use controller_service::{ControllerEvent, ControllerService};
use library_service::{Analysis, LibraryEvent, LibraryService, LibraryUnavailable, worker_count};

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "rille=info".into()))
        .init();

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(StateChannel::default())
        .setup(|app| {
            // Must happen on the main thread (run loop), see the function's docs.
            rille_midi::port::init_on_main_thread();
            let handle = app.handle().clone();
            let notify = Arc::new(move |event: UiEvent| deliver(&handle, event));
            let audio = AudioService::start(notify);
            let handle = app.handle().clone();
            let controller = ControllerService::start(
                audio.clone(),
                Box::new(move |event| deliver_controller(&handle, event)),
            );
            app.manage(audio);
            start_library(app);
            if let Some(controller) = controller {
                app.manage(controller);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::state_subscribe,
            commands::audio_devices,
            commands::audio_status,
            commands::audio_open,
            commands::audio_set_cue,
            commands::deck_load_file,
            commands::deck_load_track,
            commands::library_tracks,
            commands::library_peaks,
            commands::library_set_grid,
            commands::library_progress,
            commands::library_import,
            commands::library_choose_folder,
            commands::deck_command,
            commands::mixer_command,
            commands::controller_status,
            commands::controller_set_vinyl,
            commands::controller_pad_mode,
            commands::midi_monitor,
        ])
        .run(tauri::generate_context!());
    if let Err(err) = result {
        tracing::error!(%err, "rille konnte nicht starten");
        std::process::exit(1);
    }
}

/// Library database in Application Support, peaks in Caches (they can be rebuilt).
fn start_library(app: &tauri::App) {
    let (Ok(data), Ok(cache)) = (app.path().app_data_dir(), app.path().app_cache_dir()) else {
        tracing::error!("no application directories; library disabled");
        app.manage(LibraryUnavailable(
            "Kein Ordner für die Library gefunden (Application Support)".into(),
        ));
        return;
    };
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    let handle = app.handle().clone();
    let notify = Arc::new(move |event: LibraryEvent| {
        let result = match event {
            LibraryEvent::Tracks(tracks) => {
                // A finished analysis brings the grid to a deck that already plays the track.
                if let Some(audio) = handle.try_state::<AudioService>() {
                    for t in tracks.iter().filter(|t| t.analysis == Analysis::Done) {
                        let grid = t
                            .bpm
                            .zip(t.first_beat)
                            .map(|(bpm, first_beat)| rille_core::BeatGrid { bpm, first_beat });
                        audio.update_grid(t.id, grid);
                    }
                }
                handle.emit("library-tracks", tracks)
            }
            LibraryEvent::Progress(p) => handle.emit("library-progress", p),
        };
        if let Err(e) = result {
            tracing::warn!(%e, "could not deliver library event");
        }
    });
    match LibraryService::start(
        &data.join("library.db"),
        cache.join("peaks"),
        worker_count(cores),
        notify,
    ) {
        Ok(library) => {
            app.manage(library);
        }
        Err(e) => {
            tracing::error!(%e, "library unavailable");
            app.manage(LibraryUnavailable(e.to_string()));
        }
    }
}

fn deliver(app: &tauri::AppHandle, event: UiEvent) {
    let result = match event {
        UiEvent::State(frame) => {
            let channel = app.state::<StateChannel>();
            let Ok(mut slot) = channel.0.lock() else {
                return;
            };
            if let Some(ch) = slot.as_ref()
                && ch.send(*frame).is_err()
            {
                *slot = None; // webview went away; it will subscribe again
            }
            Ok(())
        }
        UiEvent::DeckLoaded(e) => app.emit("deck-loaded", e),
        UiEvent::DeckLoadFailed(e) => app.emit("deck-load-failed", e),
        UiEvent::DeckLoadWarning(e) => app.emit("deck-load-warning", e),
        UiEvent::DeckEnded(deck) => app.emit("deck-ended", deck),
        UiEvent::HotCuesChanged { library_id, cues } => {
            if let Some(library) = app.try_state::<LibraryService>() {
                library.save_hot_cues(library_id, &cues);
            }
            Ok(())
        }
        UiEvent::AudioChanged(status) => app.emit("audio-changed", status),
    };
    if let Err(e) = result {
        tracing::warn!(%e, "could not deliver event");
    }
}

fn deliver_controller(app: &tauri::AppHandle, event: ControllerEvent) {
    let result = match event {
        ControllerEvent::Status(status) => app.emit("controller-changed", status),
        ControllerEvent::Monitor(lines) => app.emit("midi-monitor", lines),
        ControllerEvent::LibraryScroll(rows) => app.emit("library-scroll", rows),
        ControllerEvent::LibraryLoad(deck) => app.emit("library-load", deck),
    };
    if let Err(e) = result {
        tracing::warn!(%e, "could not deliver controller event");
    }
}
