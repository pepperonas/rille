//! App shell: wires the engine, the controller layer and the library to the frontend.

use tracing_subscriber::EnvFilter;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "rille=info".into()))
        .init();

    let result = tauri::Builder::default().run(tauri::generate_context!());
    if let Err(err) = result {
        tracing::error!(%err, "rille konnte nicht starten");
        std::process::exit(1);
    }
}
