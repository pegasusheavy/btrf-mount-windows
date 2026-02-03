//! BTRFS Mount Windows GUI Application
//!
//! Tauri backend for the BTRFS volume management GUI.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod commands;

use tracing_subscriber::EnvFilter;

fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    tracing::info!("Starting BTRFS Mount Windows GUI");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_devices,
            commands::detect_btrfs,
            commands::mount_volume,
            commands::unmount_volume,
            commands::list_subvolumes,
            commands::get_volume_info,
            commands::list_mounts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
