import { Channel, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type { Backend, StateFrame } from './types';

function fire(command: string, args: Record<string, unknown>) {
  // Fire-and-forget: controls must never wait on IPC. Errors are logged, not thrown.
  invoke(command, args).catch((e: unknown) => console.error(command, e));
}

export const tauriBackend: Backend = {
  kind: 'tauri',

  async subscribeState(onFrame) {
    const channel = new Channel<StateFrame>();
    channel.onmessage = onFrame;
    await invoke('state_subscribe', { channel });
    return () => {
      channel.onmessage = () => {};
    };
  },

  on(event, handler) {
    return listen(event, (e) => handler(e.payload as never));
  },

  onFileDrop(handler) {
    return getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type !== 'drop') return;
      const scale = window.devicePixelRatio || 1;
      handler({
        paths: event.payload.paths,
        x: event.payload.position.x / scale,
        y: event.payload.position.y / scale,
      });
    });
  },

  audioDevices: () => invoke('audio_devices'),
  audioStatus: () => invoke('audio_status'),
  audioOpen: (deviceId, bufferFrames) => invoke('audio_open', { deviceId, bufferFrames }),
  loadFile: (deck, path) => invoke('deck_load_file', { deck, path }),
  controllerStatus: () => invoke('controller_status'),
  setVinylMode: (on) => invoke('controller_set_vinyl', { on }),
  setMidiMonitor: (on) => invoke('midi_monitor', { on }),
  deck: (deck, action) => fire('deck_command', { deck, action }),
  mixer: (action) => fire('mixer_command', { action }),
};
