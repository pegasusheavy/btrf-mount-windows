//! Image file access
//!
//! Provides access to BTRFS filesystem images stored in regular files.

use super::{BlockDevice, BlockDeviceError, Result};
use memmap2::{MmapMut, MmapOptions};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::RwLock;

/// Default sector size for image files
const DEFAULT_SECTOR_SIZE: u32 = 512;

/// An image file backed block device
pub struct ImageFile {
    file: RwLock<File>,
    mmap: Option<MmapMut>,
    size: u64,
    read_only: bool,
    use_mmap: bool,
}

impl ImageFile {
    /// Opens an image file
    pub fn open<P: AsRef<Path>>(path: P, read_only: bool) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(!read_only)
            .open(path.as_ref())?;

        let metadata = file.metadata()?;
        let size = metadata.len();

        // Try to memory map for better performance on large files
        let (mmap, use_mmap) = if size > 0 && !read_only {
            match unsafe { MmapOptions::new().map_mut(&file) } {
                Ok(m) => (Some(m), true),
                Err(_) => (None, false),
            }
        } else {
            (None, false)
        };

        Ok(Self {
            file: RwLock::new(file),
            mmap,
            size,
            read_only,
            use_mmap,
        })
    }

    /// Creates a new image file with the specified size
    pub fn create<P: AsRef<Path>>(path: P, size: u64) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path.as_ref())?;

        // Set file size
        file.set_len(size)?;

        let (mmap, use_mmap) = if size > 0 {
            match unsafe { MmapOptions::new().map_mut(&file) } {
                Ok(m) => (Some(m), true),
                Err(_) => (None, false),
            }
        } else {
            (None, false)
        };

        Ok(Self {
            file: RwLock::new(file),
            mmap,
            size,
            read_only: false,
            use_mmap,
        })
    }
}

impl BlockDevice for ImageFile {
    fn size(&self) -> u64 {
        self.size
    }

    fn sector_size(&self) -> u32 {
        DEFAULT_SECTOR_SIZE
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        if offset >= self.size {
            return Err(BlockDeviceError::InvalidOffset {
                offset,
                size: self.size,
            });
        }

        let bytes_to_read = std::cmp::min(buf.len() as u64, self.size - offset) as usize;

        if self.use_mmap {
            if let Some(ref mmap) = self.mmap {
                let src = &mmap[offset as usize..offset as usize + bytes_to_read];
                buf[..bytes_to_read].copy_from_slice(src);
                return Ok(bytes_to_read);
            }
        }

        let mut file = self.file.write().unwrap();
        file.seek(SeekFrom::Start(offset))?;
        let n = file.read(&mut buf[..bytes_to_read])?;
        Ok(n)
    }

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

        let bytes_to_write = std::cmp::min(buf.len() as u64, self.size - offset) as usize;

        // For mmap writes, we need mutable access - but mmap is not behind RwLock
        // So we fall back to file I/O for writes when using shared references
        let mut file = self.file.write().unwrap();
        file.seek(SeekFrom::Start(offset))?;
        let n = file.write(&buf[..bytes_to_write])?;
        Ok(n)
    }

    fn flush_device(&self) -> Result<()> {
        self.file.write().unwrap().flush()?;
        Ok(())
    }
}

// ImageFile is Send + Sync because all mutable state is behind RwLock
unsafe impl Send for ImageFile {}
unsafe impl Sync for ImageFile {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_create_and_read() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        // Create a 1MB image
        let size = 1024 * 1024;
        let img = ImageFile::create(path, size).unwrap();

        assert_eq!(img.size(), size);
        assert!(!img.is_read_only());

        // Write some data
        let data = b"Hello, BTRFS!";
        img.write_at(0, data).unwrap();

        // Read it back
        let mut buf = vec![0u8; data.len()];
        img.read_at(0, &mut buf).unwrap();

        assert_eq!(&buf, data);
    }

    #[test]
    fn test_read_only() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        // Create image first
        let _img = ImageFile::create(path, 1024).unwrap();

        // Open read-only
        let img = ImageFile::open(path, true).unwrap();
        assert!(img.is_read_only());

        // Write should fail
        let result = img.write_at(0, b"test");
        assert!(result.is_err());
    }
}
