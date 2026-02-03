//! Physical disk access for Windows
//!
//! Provides raw access to physical drives using Windows APIs.

use super::{BlockDevice, BlockDeviceError, Result};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(windows)]
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{
            CreateFileW, FlushFileBuffers, ReadFile, SetFilePointerEx, WriteFile,
            FILE_ATTRIBUTE_NORMAL, FILE_BEGIN, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
        System::Ioctl::{DISK_GEOMETRY, IOCTL_DISK_GET_DRIVE_GEOMETRY, IOCTL_DISK_GET_LENGTH_INFO},
        System::IO::DeviceIoControl,
    },
};

/// Information about a physical drive
#[derive(Debug, Clone)]
pub struct DriveInfo {
    /// Drive path (e.g., \\.\PhysicalDrive0)
    pub path: String,
    /// Drive number
    pub number: u32,
    /// Total size in bytes
    pub size: u64,
    /// Sector size in bytes
    pub sector_size: u32,
    /// Model name if available
    pub model: Option<String>,
}

/// A physical disk device
pub struct PhysicalDisk {
    #[cfg(windows)]
    handle: HANDLE,
    #[cfg(not(windows))]
    _marker: std::marker::PhantomData<()>,
    path: String,
    size: u64,
    sector_size: u32,
    read_only: bool,
    position: AtomicU64,
}

impl PhysicalDisk {
    /// Opens a physical disk by path
    #[cfg(windows)]
    pub fn open(path: &str, read_only: bool) -> Result<Self> {
        use windows::Win32::Storage::FileSystem::{FILE_GENERIC_READ, FILE_GENERIC_WRITE};

        let wide_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

        let access = if read_only {
            FILE_GENERIC_READ
        } else {
            FILE_GENERIC_READ | FILE_GENERIC_WRITE
        };

        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide_path.as_ptr()),
                access.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .map_err(|e| BlockDeviceError::WindowsError(e.to_string()))?;

        if handle == INVALID_HANDLE_VALUE {
            return Err(BlockDeviceError::NotFound(path.to_string()));
        }

        let (size, sector_size) = Self::get_disk_geometry(handle)?;

        Ok(Self {
            handle,
            path: path.to_string(),
            size,
            sector_size,
            read_only,
            position: AtomicU64::new(0),
        })
    }

    #[cfg(not(windows))]
    pub fn open(path: &str, _read_only: bool) -> Result<Self> {
        Err(BlockDeviceError::NotFound(format!(
            "Physical disk access not supported on this platform: {}",
            path
        )))
    }

    #[cfg(windows)]
    fn get_disk_geometry(handle: HANDLE) -> Result<(u64, u32)> {
        // Get disk length
        let mut length_info: i64 = 0;
        let mut bytes_returned: u32 = 0;

        unsafe {
            DeviceIoControl(
                handle,
                IOCTL_DISK_GET_LENGTH_INFO,
                None,
                0,
                Some(&mut length_info as *mut _ as *mut _),
                std::mem::size_of::<i64>() as u32,
                Some(&mut bytes_returned),
                None,
            )
        }
        .map_err(|e| BlockDeviceError::WindowsError(e.to_string()))?;

        // Get geometry for sector size
        let mut geometry: DISK_GEOMETRY = unsafe { std::mem::zeroed() };

        unsafe {
            DeviceIoControl(
                handle,
                IOCTL_DISK_GET_DRIVE_GEOMETRY,
                None,
                0,
                Some(&mut geometry as *mut _ as *mut _),
                std::mem::size_of::<DISK_GEOMETRY>() as u32,
                Some(&mut bytes_returned),
                None,
            )
        }
        .map_err(|e| BlockDeviceError::WindowsError(e.to_string()))?;

        Ok((length_info as u64, geometry.BytesPerSector))
    }

    /// Returns the path of the disk
    pub fn path(&self) -> &str {
        &self.path
    }
}

#[cfg(windows)]
impl Drop for PhysicalDisk {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

impl BlockDevice for PhysicalDisk {
    fn size(&self) -> u64 {
        self.size
    }

    fn sector_size(&self) -> u32 {
        self.sector_size
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }

    #[cfg(windows)]
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        if offset >= self.size {
            return Err(BlockDeviceError::InvalidOffset {
                offset,
                size: self.size,
            });
        }

        let mut new_pos: i64 = 0;
        unsafe {
            SetFilePointerEx(self.handle, offset as i64, Some(&mut new_pos), FILE_BEGIN)
                .map_err(|e| BlockDeviceError::WindowsError(e.to_string()))?;
        }

        let mut bytes_read: u32 = 0;
        unsafe {
            ReadFile(self.handle, Some(buf), Some(&mut bytes_read), None)
                .map_err(|e| BlockDeviceError::WindowsError(e.to_string()))?;
        }

        self.position.store(offset + bytes_read as u64, Ordering::SeqCst);
        Ok(bytes_read as usize)
    }

    #[cfg(not(windows))]
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let _ = (offset, buf);
        Err(BlockDeviceError::WindowsError(
            "Not supported on this platform".to_string(),
        ))
    }

    #[cfg(windows)]
    fn write_at(&self, offset: u64, buf: &[u8]) -> Result<usize> {
        if self.read_only {
            return Err(BlockDeviceError::ReadOnly);
        }

        if offset >= self.size {
            return Err(BlockDeviceError::InvalidOffset {
                offset,
                size: self.size,
            });
        }

        let mut new_pos: i64 = 0;
        unsafe {
            SetFilePointerEx(self.handle, offset as i64, Some(&mut new_pos), FILE_BEGIN)
                .map_err(|e| BlockDeviceError::WindowsError(e.to_string()))?;
        }

        let mut bytes_written: u32 = 0;
        unsafe {
            WriteFile(self.handle, Some(buf), Some(&mut bytes_written), None)
                .map_err(|e| BlockDeviceError::WindowsError(e.to_string()))?;
        }

        self.position
            .store(offset + bytes_written as u64, Ordering::SeqCst);
        Ok(bytes_written as usize)
    }

    #[cfg(not(windows))]
    fn write_at(&self, offset: u64, buf: &[u8]) -> Result<usize> {
        let _ = (offset, buf);
        Err(BlockDeviceError::WindowsError(
            "Not supported on this platform".to_string(),
        ))
    }

    #[cfg(windows)]
    fn flush_device(&self) -> Result<()> {
        unsafe {
            FlushFileBuffers(self.handle)
                .map_err(|e| BlockDeviceError::WindowsError(e.to_string()))?;
        }
        Ok(())
    }

    #[cfg(not(windows))]
    fn flush_device(&self) -> Result<()> {
        Ok(())
    }
}

/// Lists all physical drives on the system
#[cfg(windows)]
pub fn list_drives() -> Result<Vec<DriveInfo>> {
    let mut drives = Vec::new();

    for i in 0..32 {
        let path = format!("\\\\.\\PhysicalDrive{}", i);
        if let Ok(disk) = PhysicalDisk::open(&path, true) {
            drives.push(DriveInfo {
                path,
                number: i,
                size: disk.size,
                sector_size: disk.sector_size,
                model: None, // Would need additional WMI queries
            });
        }
    }

    Ok(drives)
}

#[cfg(not(windows))]
pub fn list_drives() -> Result<Vec<DriveInfo>> {
    Ok(Vec::new())
}

// Safety: PhysicalDisk handle operations are thread-safe on Windows
unsafe impl Send for PhysicalDisk {}
unsafe impl Sync for PhysicalDisk {}
