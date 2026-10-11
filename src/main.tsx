import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '@fontsource-variable/roboto-flex/full.css';
import './design/tokens.css';
import './global.css';
import { App } from './App';
import { initTheme } from './settings/applyPrefs';

// Inside the Tauri window the body is transparent so macOS vibrancy shows through.
if ('__TAURI_INTERNALS__' in window) document.documentElement.classList.add('native');

// Before the first paint, so a light theme never flashes dark.
initTheme();

const root = document.getElementById('root');
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
