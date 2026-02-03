//! Tauri IPC commands for BTRFS operations

use btrf_mount_windows::{blockdev, BtrfsFilesystem, BtrfsMount, MountOptions};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::State;

/// Application state
pub struct AppState {
    /// Active mounts
    pub mounts: Mutex<HashMap<String, BtrfsMount>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            mounts: Mutex::new(HashMap::new()),
        }
    }
}

/// Device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub path: String,
    pub size: u64,
    pub sector_size: u32,
    pub model: Option<String>,
    pub is_btrfs: bool,
}

/// Volume information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub uuid: String,
    pub label: String,
    pub total_bytes: u64,
    pub bytes_used: u64,
    pub num_devices: u64,
    pub generation: u64,
}

/// Subvolume information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubvolumeInfo {
    pub id: u64,
    pub parent_id: u64,
    pub name: String,
    pub path: String,
    pub generation: u64,
    pub flags: u64,
}

/// Mount information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountInfo {
    pub source: String,
    pub mount_point: String,
    pub read_only: bool,
}

/// Mount request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountRequest {
    pub source: String,
    pub drive_letter: char,
    pub read_only: bool,
    pub subvolume_id: Option<u64>,
}

/// Lists available devices (physical drives and common image locations)
#[tauri::command]
pub async fn list_devices() -> Result<Vec<DeviceInfo>, String> {
    let mut devices = Vec::new();

    // List physical drives
    #[cfg(windows)]
    {
        match blockdev::list_physical_drives() {
            Ok(drives) => {
                for drive in drives {
                    devices.push(DeviceInfo {
                        path: drive.path,
                        size: drive.size,
                        sector_size: drive.sector_size,
                        model: drive.model,
                        is_btrfs: false, // Will be detected separately
                    });
                }
            }
            Err(e) => {
                tracing::warn!("Failed to list physical drives: {}", e);
            }
        }
    }

    Ok(devices)
}

/// Detects if a device/image contains a BTRFS filesystem
#[tauri::command]
pub async fn detect_btrfs(path: String) -> Result<bool, String> {
    let device = blockdev::open(&path, true).map_err(|e| e.to_string())?;

    // Try to read superblock
    let mut buf = [0u8; 8];
    let offset = 0x10000 + 0x40; // Superblock offset + magic offset

    match device.read_at(offset, &mut buf) {
        Ok(_) => Ok(&buf == b"_BHRfS_M"),
        Err(_) => Ok(false),
    }
}

/// Mounts a BTRFS volume
#[tauri::command]
pub async fn mount_volume(
    state: State<'_, AppState>,
    request: MountRequest,
) -> Result<MountInfo, String> {
    // Open device
    let device = blockdev::open(&request.source, request.read_only).map_err(|e| e.to_string())?;

    // Open filesystem
    let fs =
        BtrfsFilesystem::open(Arc::from(device), request.read_only).map_err(|e| e.to_string())?;

    // Mount
    let options = MountOptions {
        drive_letter: request.drive_letter,
        read_only: request.read_only,
        subvolume_id: request.subvolume_id,
        ..Default::default()
    };

    let mount = BtrfsMount::mount(Arc::new(fs), options).map_err(|e| e.to_string())?;

    let mount_point = mount.mount_point().to_string();

    // Store mount
    let mut mounts = state.mounts.lock().unwrap();
    mounts.insert(mount_point.clone(), mount);

    Ok(MountInfo {
        source: request.source,
        mount_point,
        read_only: request.read_only,
    })
}

/// Unmounts a volume
#[tauri::command]
pub async fn unmount_volume(
    state: State<'_, AppState>,
    mount_point: String,
) -> Result<(), String> {
    let mut mounts = state.mounts.lock().unwrap();

    if let Some(mut mount) = mounts.remove(&mount_point) {
        mount.unmount().map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Lists subvolumes in a mounted volume
#[tauri::command]
pub async fn list_subvolumes(source: String) -> Result<Vec<SubvolumeInfo>, String> {
    let device = blockdev::open(&source, true).map_err(|e| e.to_string())?;

    let fs = BtrfsFilesystem::open(Arc::from(device), true).map_err(|e| e.to_string())?;

    let subvolumes = fs.list_subvolumes().map_err(|e| e.to_string())?;

    Ok(subvolumes
        .into_iter()
        .map(|s| SubvolumeInfo {
            id: s.id,
            parent_id: s.parent_id,
            name: s.name,
            path: s.path,
            generation: s.generation,
            flags: s.flags,
        })
        .collect())
}

/// Gets volume information
#[tauri::command]
pub async fn get_volume_info(source: String) -> Result<VolumeInfo, String> {
    let device = blockdev::open(&source, true).map_err(|e| e.to_string())?;

    let fs = BtrfsFilesystem::open(Arc::from(device), true).map_err(|e| e.to_string())?;

    Ok(VolumeInfo {
        uuid: fs.uuid().to_string(),
        label: fs.label().to_string(),
        total_bytes: fs.total_bytes(),
        bytes_used: fs.bytes_used(),
        num_devices: fs.superblock().num_devices(),
        generation: fs.superblock().generation(),
    })
}

/// Lists active mounts
#[tauri::command]
pub async fn list_mounts(state: State<'_, AppState>) -> Result<Vec<MountInfo>, String> {
    let mounts = state.mounts.lock().unwrap();

    Ok(mounts
        .values()
        .map(|m| MountInfo {
            source: String::new(), // TODO: Store source in mount
            mount_point: m.mount_point().to_string(),
            read_only: m.filesystem().is_read_only(),
        })
        .collect())
}

// ============================================================================
// Library Update Commands
// ============================================================================

/// Library version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryVersionInfo {
    pub version: String,
    pub installed_version: Option<String>,
    pub update_available: bool,
}

/// Library update information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryUpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
    pub download_size: u64,
}

/// Get the current library version
#[tauri::command]
pub async fn get_library_version() -> Result<LibraryVersionInfo, String> {
    use btrf_mount_windows::{LibraryUpdater, VERSION};
    
    let updater = LibraryUpdater::new(LibraryUpdater::default_lib_dir());
    let installed = updater.installed_version().map_err(|e| e.to_string())?;
    
    Ok(LibraryVersionInfo {
        version: VERSION.to_string(),
        installed_version: installed.clone(),
        update_available: false, // Will be updated by check_library_update
    })
}

/// Check for library updates
#[tauri::command]
pub async fn check_library_update() -> Result<Option<LibraryUpdateInfo>, String> {
    use btrf_mount_windows::VERSION;
    
    // Fetch the update manifest
    let manifest_url = "https://github.com/pegasusheavy/btrf-mount-windows/releases/latest/download/lib-latest.json";
    
    let response = reqwest::get(manifest_url)
        .await
        .map_err(|e| format!("Failed to fetch update manifest: {}", e))?;
    
    if !response.status().is_success() {
        return Ok(None);
    }
    
    let manifest: btrf_mount_windows::LibraryUpdate = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse update manifest: {}", e))?;
    
    // Check if update is needed
    if manifest.version == VERSION {
        return Ok(None);
    }
    
    // Get platform-specific info
    #[cfg(windows)]
    let platform_info = &manifest.platforms.windows_x64;
    #[cfg(target_os = "linux")]
    let platform_info = &manifest.platforms.linux_x64;
    #[cfg(not(any(windows, target_os = "linux")))]
    let platform_info: &Option<btrf_mount_windows::updater::LibraryDownload> = &None;
    
    let download_size = platform_info.as_ref().map(|p| p.size).unwrap_or(0);
    
    Ok(Some(LibraryUpdateInfo {
        version: manifest.version,
        current_version: VERSION.to_string(),
        notes: manifest.notes,
        pub_date: manifest.pub_date,
        download_size,
    }))
}

/// Install a library update
#[tauri::command]
pub async fn install_library_update(app: tauri::AppHandle) -> Result<(), String> {
    use btrf_mount_windows::LibraryUpdater;
    
    // Fetch the update manifest
    let manifest_url = "https://github.com/pegasusheavy/btrf-mount-windows/releases/latest/download/lib-latest.json";
    
    let response = reqwest::get(manifest_url)
        .await
        .map_err(|e| format!("Failed to fetch update manifest: {}", e))?;
    
    let manifest: btrf_mount_windows::LibraryUpdate = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse update manifest: {}", e))?;
    
    // Get platform-specific info
    #[cfg(windows)]
    let platform_info = manifest.platforms.windows_x64.ok_or("Windows platform not available")?;
    #[cfg(target_os = "linux")]
    let platform_info = manifest.platforms.linux_x64.ok_or("Linux platform not available")?;
    #[cfg(not(any(windows, target_os = "linux")))]
    return Err("Platform not supported".to_string());
    
    // Download the library
    let _ = app.emit("library-update-progress", serde_json::json!({
        "stage": "downloading",
        "progress": 0
    }));
    
    let lib_response = reqwest::get(&platform_info.url)
        .await
        .map_err(|e| format!("Failed to download library: {}", e))?;
    
    let lib_data = lib_response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read library data: {}", e))?;
    
    let _ = app.emit("library-update-progress", serde_json::json!({
        "stage": "installing",
        "progress": 50
    }));
    
    // Install the library
    let updater = LibraryUpdater::new(LibraryUpdater::default_lib_dir());
    updater
        .install_library(&lib_data, &manifest.version, &platform_info.sha256)
        .map_err(|e| format!("Failed to install library: {}", e))?;
    
    let _ = app.emit("library-update-progress", serde_json::json!({
        "stage": "complete",
        "progress": 100
    }));
    
    Ok(())
}
