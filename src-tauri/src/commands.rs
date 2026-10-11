//! Tauri commands: the only way the frontend talks to Rust.

use std::path::PathBuf;
use std::sync::Mutex;

use rille_core::{BeatGrid, Command};
use rille_engine::output::{self, OutputRequest};
use tauri::ipc::{Channel, Response};
use tauri::{Manager, State};

use crate::audio_service::{AudioService, TrackMeta};
use crate::controller_service::{ControllerService, ControllerStatus, PadMode};
use crate::dto::{AudioDevice, AudioStatus, Deck, DeckAction, MixerAction, StateFrame};
use crate::library_service::{
    LibraryProgress, LibraryService, LibraryTrack, LibraryUnavailable, LoadInfo,
};

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

/// Async: adding the file to the library reads its tags and touches the database, which must
/// not stall the main thread.
#[tauri::command(async)]
pub fn deck_load_file(
    deck: Deck,
    path: PathBuf,
    audio: State<'_, AudioService>,
    app: tauri::AppHandle,
) -> Result<u64, String> {
    // The file joins the library (tags, tempo, waveform); without a library it still plays.
    let meta = app
        .try_state::<LibraryService>()
        .and_then(|lib| lib.ensure(&path))
        .map(meta_of)
        .unwrap_or_else(|| TrackMeta::from_path(&path));
    audio.load_file(deck.into(), path, meta)
}

fn meta_of(info: LoadInfo) -> TrackMeta {
    TrackMeta {
        title: info.title,
        artist: info.artist,
        bpm: info.bpm,
        library_id: Some(info.id),
        grid: info.grid,
        hot_cues: info.hot_cues,
    }
}

fn library(app: &tauri::AppHandle) -> Result<State<'_, LibraryService>, String> {
    app.try_state::<LibraryService>().ok_or_else(|| {
        app.try_state::<LibraryUnavailable>().map_or_else(
            || "Die Library ist nicht verfügbar".to_string(),
            |u| format!("Die Library ist nicht verfügbar: {}", u.0),
        )
    })
}

#[tauri::command]
pub fn deck_load_track(
    deck: Deck,
    id: i64,
    audio: State<'_, AudioService>,
    app: tauri::AppHandle,
) -> Result<u64, String> {
    let library = library(&app)?;
    let row = library
        .row(id)
        .ok_or_else(|| "Dieser Track ist nicht mehr in der Library".to_string())?;
    library.prioritize(&row);
    let info = library.load_info(&row);
    let path = info.path.clone();
    audio.load_file(deck.into(), path, meta_of(info))
}

/// Waveform peaks of a library track, binary ("RLPK" v1, see `analysis/peaks.rs`).
#[tauri::command]
pub fn library_peaks(id: i64, app: tauri::AppHandle) -> Result<Response, String> {
    library(&app)?.peaks(id).map(Response::new)
}

/// The user corrected a beatgrid. `bpm` 0 or less removes the grid.
#[tauri::command]
pub fn library_set_grid(
    id: i64,
    bpm: f64,
    first_beat: f64,
    audio: State<'_, AudioService>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let grid = (bpm.is_finite() && bpm > 0.0 && first_beat.is_finite()).then_some(BeatGrid {
        bpm: bpm.clamp(20.0, 400.0),
        first_beat,
    });
    library(&app)?.set_grid(id, grid)?;
    audio.update_grid(id, grid);
    Ok(())
}

#[tauri::command]
pub fn library_tracks(app: tauri::AppHandle) -> Result<Vec<LibraryTrack>, String> {
    library(&app)?.tracks().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn library_progress(app: tauri::AppHandle) -> Result<LibraryProgress, String> {
    Ok(library(&app)?.progress())
}

/// Import dropped folders or files.
#[tauri::command]
pub fn library_import(paths: Vec<PathBuf>, app: tauri::AppHandle) -> Result<(), String> {
    library(&app)?.import(paths);
    Ok(())
}

/// Ask for a folder (native dialog) and import it. The dialog lives on the Rust side, so the
/// webview needs no dialog permission.
#[tauri::command]
pub fn library_choose_folder(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt;
    let service = library(&app)?.inner().clone();
    app.dialog()
        .file()
        .set_title("Musikordner importieren")
        .pick_folder(move |folder| {
            if let Some(path) = folder.and_then(|f| f.into_path().ok()) {
                service.import(vec![path]);
            }
        });
    Ok(())
}

#[tauri::command]
pub fn deck_command(
    deck: Deck,
    action: DeckAction,
    audio: State<'_, AudioService>,
    app: tauri::AppHandle,
) {
    match action {
        // Unloading also cancels a load that is still decoding.
        DeckAction::Unload => audio.unload(deck.into()),
        _ => audio.send(Command::Deck(deck.into(), action.into())),
    }
    if let (Some(controller), Some((key, value))) =
        (app.try_state::<ControllerService>(), action.takeover(deck))
    {
        controller.software_changed(key, value);
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

/// Pad mode of a deck (the DDJ-200 has no mode buttons; the app chooses).
#[tauri::command]
pub fn controller_pad_mode(deck: Deck, mode: PadMode, app: tauri::AppHandle) {
    if let Some(controller) = app.try_state::<ControllerService>() {
        controller.set_pad_mode(deck.into(), mode);
    }
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
