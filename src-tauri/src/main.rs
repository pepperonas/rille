// Prevents an extra console window on Windows release builds; harmless on macOS.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// In debug builds every audio callback runs inside `assert_no_alloc`; this allocator makes an
// allocation there abort loudly instead of causing a silent glitch.
#[cfg(debug_assertions)]
#[global_allocator]
static ALLOC: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

fn main() {
    rille_app::run();
}
