//! Library updater for independent BTRFS library updates
//!
//! This module provides functionality to check for and install updates
//! to the BTRFS library independently of the GUI application.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Current library version
pub const LIB_VERSION: &str = env!("CARGO_PKG_VERSION");

/// GitHub repository for updates
pub const GITHUB_REPO: &str = "pegasusheavy/btrf-mount-windows";

/// Update manifest URL
pub const UPDATE_MANIFEST_URL: &str = 
    "https://github.com/pegasusheavy/btrf-mount-windows/releases/latest/download/lib-latest.json";

/// Library update information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryUpdate {
    /// New version available
    pub version: String,
    /// Current installed version
    pub current_version: String,
    /// Release notes
    pub notes: Option<String>,
    /// Publication date
    pub pub_date: Option<String>,
    /// Platform-specific download info
    pub platforms: LibraryPlatforms,
}

/// Platform-specific library downloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryPlatforms {
    #[serde(rename = "windows-x86_64")]
    pub windows_x64: Option<LibraryDownload>,
    #[serde(rename = "linux-x86_64")]
    pub linux_x64: Option<LibraryDownload>,
}

/// Library download information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryDownload {
    /// Download URL for the library
    pub url: String,
    /// SHA256 hash of the file
    pub sha256: String,
    /// File size in bytes
    pub size: u64,
}

/// Library manifest for tracking installed version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryManifest {
    /// Installed version
    pub version: String,
    /// Installation date
    pub installed_date: String,
    /// Library file path
    pub library_path: PathBuf,
    /// SHA256 hash of installed library
    pub sha256: String,
}

/// Error type for updater operations
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Checksum mismatch")]
    ChecksumMismatch,
    #[error("Platform not supported")]
    PlatformNotSupported,
    #[error("No update available")]
    NoUpdate,
}

/// Result type for updater operations
pub type UpdateResult<T> = Result<T, UpdateError>;

/// Library updater
pub struct LibraryUpdater {
    /// Base directory for library storage
    lib_dir: PathBuf,
    /// HTTP client for downloads
    #[cfg(feature = "updater-network")]
    client: reqwest::Client,
}

impl LibraryUpdater {
    /// Create a new library updater
    pub fn new(lib_dir: PathBuf) -> Self {
        Self {
            lib_dir,
            #[cfg(feature = "updater-network")]
            client: reqwest::Client::new(),
        }
    }

    /// Get the default library directory
    pub fn default_lib_dir() -> PathBuf {
        #[cfg(windows)]
        {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("BtrfsMountWindows")
                .join("lib")
        }
        #[cfg(not(windows))]
        {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("btrfs-mount-windows")
                .join("lib")
        }
    }

    /// Get the library filename for the current platform
    pub fn library_filename() -> &'static str {
        #[cfg(windows)]
        {
            "btrf_mount_windows.dll"
        }
        #[cfg(target_os = "linux")]
        {
            "libbtrf_mount_windows.so"
        }
        #[cfg(target_os = "macos")]
        {
            "libbtrf_mount_windows.dylib"
        }
    }

    /// Get the path to the installed library
    pub fn library_path(&self) -> PathBuf {
        self.lib_dir.join(Self::library_filename())
    }

    /// Get the path to the manifest file
    pub fn manifest_path(&self) -> PathBuf {
        self.lib_dir.join("manifest.json")
    }

    /// Read the installed library manifest
    pub fn read_manifest(&self) -> UpdateResult<Option<LibraryManifest>> {
        let path = self.manifest_path();
        if !path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&path)?;
        let manifest: LibraryManifest =
            serde_json::from_str(&content).map_err(|e| UpdateError::Parse(e.to_string()))?;
        Ok(Some(manifest))
    }

    /// Write the library manifest
    pub fn write_manifest(&self, manifest: &LibraryManifest) -> UpdateResult<()> {
        std::fs::create_dir_all(&self.lib_dir)?;
        let content =
            serde_json::to_string_pretty(manifest).map_err(|e| UpdateError::Parse(e.to_string()))?;
        std::fs::write(self.manifest_path(), content)?;
        Ok(())
    }

    /// Get the currently installed version
    pub fn installed_version(&self) -> UpdateResult<Option<String>> {
        Ok(self.read_manifest()?.map(|m| m.version))
    }

    /// Check if an update is available (without network, just compare versions)
    pub fn needs_update(&self, latest_version: &str) -> UpdateResult<bool> {
        let installed = self.installed_version()?;
        match installed {
            Some(v) => Ok(compare_versions(&v, latest_version) < 0),
            None => Ok(true), // Not installed, needs update
        }
    }

    /// Parse a library update manifest from JSON
    pub fn parse_update_manifest(json: &str) -> UpdateResult<LibraryUpdate> {
        serde_json::from_str(json).map_err(|e| UpdateError::Parse(e.to_string()))
    }

    /// Verify a downloaded file's checksum
    pub fn verify_checksum(data: &[u8], expected_sha256: &str) -> bool {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        let actual = hex::encode(result);
        actual.eq_ignore_ascii_case(expected_sha256)
    }

    /// Install a library from downloaded data
    pub fn install_library(&self, data: &[u8], version: &str, sha256: &str) -> UpdateResult<()> {
        // Verify checksum
        if !Self::verify_checksum(data, sha256) {
            return Err(UpdateError::ChecksumMismatch);
        }

        // Create lib directory
        std::fs::create_dir_all(&self.lib_dir)?;

        // Write the library file
        let lib_path = self.library_path();
        
        // On Windows, we may need to rename the old file first
        #[cfg(windows)]
        if lib_path.exists() {
            let backup_path = lib_path.with_extension("dll.old");
            let _ = std::fs::remove_file(&backup_path);
            std::fs::rename(&lib_path, &backup_path)?;
        }

        std::fs::write(&lib_path, data)?;

        // Write manifest
        let manifest = LibraryManifest {
            version: version.to_string(),
            installed_date: chrono::Utc::now().to_rfc3339(),
            library_path: lib_path,
            sha256: sha256.to_string(),
        };
        self.write_manifest(&manifest)?;

        Ok(())
    }
}

/// Compare two semantic version strings
/// Returns -1 if a < b, 0 if a == b, 1 if a > b
fn compare_versions(a: &str, b: &str) -> i32 {
    let parse = |s: &str| -> Vec<u32> {
        s.split('.')
            .filter_map(|p| p.trim_start_matches('v').parse().ok())
            .collect()
    };

    let va = parse(a);
    let vb = parse(b);

    for i in 0..std::cmp::max(va.len(), vb.len()) {
        let pa = va.get(i).copied().unwrap_or(0);
        let pb = vb.get(i).copied().unwrap_or(0);
        if pa < pb {
            return -1;
        }
        if pa > pb {
            return 1;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions() {
        assert_eq!(compare_versions("0.1.0", "0.1.0"), 0);
        assert_eq!(compare_versions("0.1.0", "0.1.1"), -1);
        assert_eq!(compare_versions("0.2.0", "0.1.1"), 1);
        assert_eq!(compare_versions("1.0.0", "0.9.9"), 1);
        assert_eq!(compare_versions("v0.1.0", "0.1.0"), 0);
    }

    #[test]
    fn test_library_filename() {
        let filename = LibraryUpdater::library_filename();
        assert!(!filename.is_empty());
        #[cfg(windows)]
        assert!(filename.ends_with(".dll"));
        #[cfg(target_os = "linux")]
        assert!(filename.ends_with(".so"));
    }

    #[test]
    fn test_parse_update_manifest() {
        let json = r#"{
            "version": "0.2.0",
            "current_version": "0.1.0",
            "notes": "Bug fixes",
            "pub_date": "2024-01-01T00:00:00Z",
            "platforms": {
                "windows-x86_64": {
                    "url": "https://example.com/lib.dll",
                    "sha256": "abc123",
                    "size": 1024
                },
                "linux-x86_64": null
            }
        }"#;

        let update = LibraryUpdater::parse_update_manifest(json).unwrap();
        assert_eq!(update.version, "0.2.0");
        assert!(update.platforms.windows_x64.is_some());
    }
}
