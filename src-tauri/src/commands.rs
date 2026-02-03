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
