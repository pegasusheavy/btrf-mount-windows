//! Block device abstraction layer
//!
//! This module provides a unified interface for accessing storage backends,
//! including physical disks and image files.

pub mod image;
pub mod physical;

use std::io::{Read, Seek, Write};
use thiserror::Error;

pub use image::ImageFile;
pub use physical::{DriveInfo, PhysicalDisk};

/// Errors that can occur during block device operations
#[derive(Error, Debug)]
pub enum BlockDeviceError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Device not found: {0}")]
    NotFound(String),

    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Invalid offset: {offset} (device size: {size})")]
    InvalidOffset { offset: u64, size: u64 },

    #[error("Read beyond end of device")]
    ReadBeyondEnd,

    #[error("Device is read-only")]
    ReadOnly,

    #[error("Windows API error: {0}")]
    WindowsError(String),
}

pub type Result<T> = std::result::Result<T, BlockDeviceError>;

/// Trait for block device access
pub trait BlockDevice: Send + Sync {
    /// Returns the total size of the device in bytes
    fn size(&self) -> u64;

    /// Returns the sector size of the device
    fn sector_size(&self) -> u32;

    /// Returns true if the device is read-only
    fn is_read_only(&self) -> bool;

    /// Reads data at the specified offset
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize>;

    /// Writes data at the specified offset
    fn write_at(&self, offset: u64, buf: &[u8]) -> Result<usize>;

    /// Flushes any buffered data to the device
    fn flush_device(&self) -> Result<()>;
}

/// Opens a block device from the given path
///
/// Automatically detects whether the path refers to a physical disk
/// or an image file.
pub fn open(path: &str, read_only: bool) -> Result<Box<dyn BlockDevice>> {
    if path.starts_with("\\\\.\\PhysicalDrive") || path.starts_with("//./PhysicalDrive") {
        Ok(Box::new(PhysicalDisk::open(path, read_only)?))
    } else {
        Ok(Box::new(ImageFile::open(path, read_only)?))
    }
}

/// Lists available physical drives on the system
#[cfg(windows)]
pub fn list_physical_drives() -> Result<Vec<DriveInfo>> {
    physical::list_drives()
}

#[cfg(not(windows))]
pub fn list_physical_drives() -> Result<Vec<DriveInfo>> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_device_error_display() {
        let err = BlockDeviceError::NotFound("test".to_string());
        assert_eq!(format!("{}", err), "Device not found: test");

        let err = BlockDeviceError::AccessDenied("test".to_string());
        assert_eq!(format!("{}", err), "Access denied: test");

        let err = BlockDeviceError::InvalidOffset {
            offset: 1000,
            size: 500,
        };
        assert!(format!("{}", err).contains("1000"));
        assert!(format!("{}", err).contains("500"));

        let err = BlockDeviceError::ReadBeyondEnd;
        assert!(format!("{}", err).contains("beyond"));

        let err = BlockDeviceError::ReadOnly;
        assert!(format!("{}", err).contains("read-only"));

        let err = BlockDeviceError::WindowsError("test".to_string());
        assert!(format!("{}", err).contains("test"));
    }

    #[test]
    fn test_open_image_file() {
        use tempfile::NamedTempFile;

        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_str().unwrap();

        // Create a small file
        std::fs::write(path, vec![0u8; 1024]).unwrap();

        let device = open(path, true).unwrap();
        assert_eq!(device.size(), 1024);
        assert!(device.is_read_only());
    }

    #[test]
    fn test_open_nonexistent_file() {
        let result = open("/nonexistent/path/to/file.img", true);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_physical_drives() {
        // On non-Windows, should return empty list
        #[cfg(not(windows))]
        {
            let drives = list_physical_drives().unwrap();
            assert!(drives.is_empty());
        }
    }
}
