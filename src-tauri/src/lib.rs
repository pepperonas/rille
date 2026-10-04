//! App shell: wires the engine, the controller layer and the library to the frontend.

mod audio_service;
mod commands;
mod controller_service;
mod dto;

use std::sync::Arc;

use tauri::{Emitter, Manager};
use tracing_subscriber::EnvFilter;

use audio_service::{AudioService, UiEvent};
use commands::StateChannel;
use controller_service::{ControllerEvent, ControllerService};

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "rille=info".into()))
        .init();

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
            commands::deck_load_file,
            commands::deck_command,
            commands::mixer_command,
            commands::controller_status,
            commands::controller_set_vinyl,
            commands::midi_monitor,
        ])
        .run(tauri::generate_context!());
    if let Err(err) = result {
        tracing::error!(%err, "rille konnte nicht starten");
        std::process::exit(1);
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
                && ch.send(frame).is_err()
            {
                *slot = None; // webview went away; it will subscribe again
            }
            Ok(())
        }
        UiEvent::DeckLoaded(e) => app.emit("deck-loaded", e),
        UiEvent::DeckLoadFailed(e) => app.emit("deck-load-failed", e),
        UiEvent::DeckEnded(deck) => app.emit("deck-ended", deck),
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
    };
    if let Err(e) = result {
        tracing::warn!(%e, "could not deliver controller event");
    }
}
