//! Tauri commands: the only way the frontend talks to Rust.

use std::path::PathBuf;
use std::sync::Mutex;

use rille_core::Command;
use rille_engine::output::{self, OutputRequest};
use tauri::ipc::Channel;
use tauri::{Manager, State};

use crate::audio_service::AudioService;
use crate::controller_service::{ControllerService, ControllerStatus};
use crate::dto::{AudioDevice, AudioStatus, Deck, DeckAction, MixerAction, StateFrame};

/// Where state frames go. Replaced on every subscription (e.g. after a webview reload).
#[derive(Default)]
pub struct StateChannel(pub Mutex<Option<Channel<StateFrame>>>);

#[tauri::command]
pub fn state_subscribe(channel: Channel<StateFrame>, target: State<'_, StateChannel>) {
    if let Ok(mut slot) = target.0.lock() {
        *slot = Some(channel);
    }
}

#[tauri::command]
pub fn audio_devices() -> Vec<AudioDevice> {
    output::list_devices()
        .into_iter()
        .map(AudioDevice::from)
        .collect()
}

#[tauri::command]
pub fn audio_status(audio: State<'_, AudioService>) -> AudioStatus {
    audio.status()
}

#[tauri::command]
pub fn audio_open(
    device_id: Option<String>,
    buffer_frames: u32,
    audio: State<'_, AudioService>,
) -> Result<AudioStatus, String> {
    let request = OutputRequest {
        device_id,
        buffer_frames,
        sample_rate: audio.sample_rate(),
    };
    audio.open(request).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn deck_load_file(
    deck: Deck,
    path: PathBuf,
    audio: State<'_, AudioService>,
) -> Result<u64, String> {
    audio.load_file(deck.into(), path)
}

#[tauri::command]
pub fn deck_command(deck: Deck, action: DeckAction, audio: State<'_, AudioService>) {
    match action {
        // Unloading also cancels a load that is still decoding.
        DeckAction::Unload => audio.unload(deck.into()),
        _ => audio.send(Command::Deck(deck.into(), action.into())),
    }
}

#[tauri::command]
pub fn mixer_command(action: MixerAction, audio: State<'_, AudioService>, app: tauri::AppHandle) {
    audio.send(Command::Mixer(action.into()));
    // A control moved in the app must be picked up again on the hardware.
    if let (Some(controller), Some((key, value))) =
        (app.try_state::<ControllerService>(), action.takeover())
    {
        controller.software_changed(key, value);
    }
}

#[tauri::command]
pub fn controller_status(controller: State<'_, ControllerService>) -> ControllerStatus {
    controller.status()
}

#[tauri::command]
pub fn controller_set_vinyl(on: bool, controller: State<'_, ControllerService>) {
    controller.set_vinyl_mode(on);
}

/// Start or stop streaming MIDI traffic to the developer view.
#[tauri::command]
pub fn midi_monitor(on: bool, controller: State<'_, ControllerService>) {
    controller.set_monitor(on);
}
