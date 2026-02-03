//! BTRFS Mount Windows GUI Application
//!
//! Tauri backend for the BTRFS volume management GUI.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod commands;

use tauri::Manager;
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // Check for updates on startup in release builds
            #[cfg(not(debug_assertions))]
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = check_for_updates(handle).await {
                        tracing::warn!("Failed to check for updates: {}", e);
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_devices,
            commands::detect_btrfs,
            commands::mount_volume,
            commands::unmount_volume,
            commands::list_subvolumes,
            commands::get_volume_info,
            commands::list_mounts,
            commands::get_library_version,
            commands::check_library_update,
            commands::install_library_update,
            check_update,
            install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Check for updates from GitHub releases
#[cfg(not(debug_assertions))]
async fn check_for_updates(app: tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_updater::UpdaterExt;
    
    tracing::info!("Checking for updates...");
    
    let updater = app.updater()?;
    
    match updater.check().await {
        Ok(Some(update)) => {
            tracing::info!(
                "Update available: {} -> {}",
                update.current_version,
                update.version
            );
            
            // Emit event to frontend about available update
            app.emit("update-available", serde_json::json!({
                "version": update.version,
                "current_version": update.current_version,
                "body": update.body,
                "date": update.date,
            }))?;
        }
        Ok(None) => {
            tracing::info!("No updates available");
        }
        Err(e) => {
            tracing::warn!("Failed to check for updates: {}", e);
        }
    }
    
    Ok(())
}

/// Command to manually check for updates
#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
    use tauri_plugin_updater::UpdaterExt;
    
    let updater = app.updater().map_err(|e| e.to_string())?;
    
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(UpdateInfo {
            version: update.version.clone(),
            current_version: update.current_version.clone(),
            body: update.body.clone(),
            date: update.date.map(|d| d.to_string()),
        })),
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Command to install an available update
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    
    let updater = app.updater().map_err(|e| e.to_string())?;
    
    if let Some(update) = updater.check().await.map_err(|e| e.to_string())? {
        tracing::info!("Downloading and installing update {}", update.version);
        
        // Download and install
        let mut downloaded = 0;
        
        update
            .download_and_install(
                |chunk_length, content_length| {
                    downloaded += chunk_length;
                    tracing::debug!("Downloaded {} of {:?}", downloaded, content_length);
                },
                || {
                    tracing::info!("Download complete, installing...");
                },
            )
            .await
            .map_err(|e| e.to_string())?;
        
        tracing::info!("Update installed, restarting...");
        app.restart();
    }
    
    Ok(())
}

#[derive(serde::Serialize)]
struct UpdateInfo {
    version: String,
    current_version: String,
    body: Option<String>,
    date: Option<String>,
}
