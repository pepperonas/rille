import type { Backend } from './types';
import { tauriBackend } from './tauri';
import { createMockBackend } from './mock';

export * from './types';

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** The one backend instance. Inside Tauri it talks to Rust, in a plain browser it simulates. */
export const backend: Backend = inTauri ? tauriBackend : createMockBackend();
