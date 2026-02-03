//! Dokan FileSystemHandler implementation for BTRFS

use crate::core::{BtrfsFilesystem, Inode, InodeType};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[cfg(windows)]
use dokan::{
    CreateFileInfo, DiskSpaceInfo, FileInfo, FileSystemHandler, FileTimeInfo, FindData,
    MountFlags, OperationError, OperationInfo, VolumeInfo,
};

#[cfg(windows)]
use windows::Win32::Foundation::NTSTATUS;

/// Context for an open file
#[derive(Debug)]
pub struct FileContext {
    /// Inode number
    pub ino: u64,
    /// Tree ID (subvolume)
    pub tree_id: u64,
    /// Is directory
    pub is_dir: bool,
    /// Current read position
    pub position: AtomicU64,
}

/// BTRFS Dokan handler
pub struct BtrfsHandler {
    /// The filesystem
    fs: Arc<BtrfsFilesystem>,
    /// Read-only mode
    read_only: bool,
    /// Open file handles
    handles: RwLock<HashMap<u64, Arc<FileContext>>>,
    /// Next handle ID
    next_handle: AtomicU64,
}

impl BtrfsHandler {
    /// Creates a new handler
    pub fn new(fs: Arc<BtrfsFilesystem>, read_only: bool) -> Self {
        Self {
            fs,
            read_only,
            handles: RwLock::new(HashMap::new()),
            next_handle: AtomicU64::new(1),
        }
    }

    /// Allocates a new handle
    fn alloc_handle(&self, ctx: FileContext) -> u64 {
        let handle = self.next_handle.fetch_add(1, Ordering::SeqCst);
        self.handles.write().insert(handle, Arc::new(ctx));
        handle
    }

    /// Gets a handle context
    fn get_handle(&self, handle: u64) -> Option<Arc<FileContext>> {
        self.handles.read().get(&handle).cloned()
    }

    /// Releases a handle
    fn release_handle(&self, handle: u64) {
        self.handles.write().remove(&handle);
    }

    /// Converts a path to an inode
    fn path_to_inode(&self, _path: &str) -> Option<(u64, u64)> {
        // TODO: Implement path resolution
        // Returns (tree_id, ino)
        None
    }
}

#[cfg(windows)]
impl FileSystemHandler for BtrfsHandler {
    type Context = u64;

    fn create_file(
        &self,
        file_name: &dokan::OperationInfo<'_, '_, Self>,
        _security_context: dokan::PDOKAN_IO_SECURITY_CONTEXT,
        _desired_access: u32,
        _file_attributes: u32,
        _share_access: u32,
        create_disposition: u32,
        create_options: u32,
        _info: &mut dokan::OperationInfo<'_, '_, Self>,
    ) -> std::result::Result<CreateFileInfo<Self::Context>, OperationError> {
        let path = file_name.path().to_string_lossy();

        // Check if read-only and write access requested
        if self.read_only {
            // Allow read-only access
        }

        // Root directory
        if path == "\\" {
            let ctx = FileContext {
                ino: 256, // Root inode
                tree_id: 5, // FS_TREE
                is_dir: true,
                position: AtomicU64::new(0),
            };
            let handle = self.alloc_handle(ctx);
            return Ok(CreateFileInfo {
                context: handle,
                is_dir: true,
                new_file_created: false,
            });
        }

        // TODO: Implement full path resolution
        Err(OperationError::NtStatus(NTSTATUS(0xC0000034u32 as i32))) // STATUS_OBJECT_NAME_NOT_FOUND
    }

    fn close_file(
        &self,
        _file_name: &dokan::OperationInfo<'_, '_, Self>,
        _info: &dokan::OperationInfo<'_, '_, Self>,
        context: &Self::Context,
    ) {
        self.release_handle(*context);
    }

    fn read_file(
        &self,
        _file_name: &dokan::OperationInfo<'_, '_, Self>,
        offset: i64,
        buffer: &mut [u8],
        _info: &dokan::OperationInfo<'_, '_, Self>,
        context: &Self::Context,
    ) -> std::result::Result<u32, OperationError> {
        let _ctx = self
            .get_handle(*context)
            .ok_or(OperationError::NtStatus(NTSTATUS(0xC0000008u32 as i32)))?; // STATUS_INVALID_HANDLE

        // TODO: Implement file reading
        Ok(0)
    }

    fn write_file(
        &self,
        _file_name: &dokan::OperationInfo<'_, '_, Self>,
        _offset: i64,
        _buffer: &[u8],
        _info: &dokan::OperationInfo<'_, '_, Self>,
        _context: &Self::Context,
    ) -> std::result::Result<u32, OperationError> {
        if self.read_only {
            return Err(OperationError::NtStatus(NTSTATUS(0xC00000A2u32 as i32))); // STATUS_MEDIA_WRITE_PROTECTED
        }

        // TODO: Implement file writing
        Err(OperationError::NtStatus(NTSTATUS(0xC0000022u32 as i32))) // STATUS_ACCESS_DENIED
    }

    fn flush_file_buffers(
        &self,
        _file_name: &dokan::OperationInfo<'_, '_, Self>,
        _info: &dokan::OperationInfo<'_, '_, Self>,
        _context: &Self::Context,
    ) -> std::result::Result<(), OperationError> {
        Ok(())
    }

    fn get_file_information(
        &self,
        _file_name: &dokan::OperationInfo<'_, '_, Self>,
        _info: &dokan::OperationInfo<'_, '_, Self>,
        context: &Self::Context,
    ) -> std::result::Result<FileInfo, OperationError> {
        let ctx = self
            .get_handle(*context)
            .ok_or(OperationError::NtStatus(NTSTATUS(0xC0000008u32 as i32)))?;

        // TODO: Get actual file info from inode
        Ok(FileInfo {
            attributes: if ctx.is_dir {
                0x10 // FILE_ATTRIBUTE_DIRECTORY
            } else {
                0x80 // FILE_ATTRIBUTE_NORMAL
            },
            creation_time: std::time::UNIX_EPOCH,
            last_access_time: std::time::UNIX_EPOCH,
            last_write_time: std::time::UNIX_EPOCH,
            file_size: 0,
            number_of_links: 1,
            file_index: ctx.ino,
        })
    }

    fn find_files(
        &self,
        _file_name: &dokan::OperationInfo<'_, '_, Self>,
        mut fill_find_data: impl FnMut(&FindData) -> std::result::Result<(), dokan::FillDataError>,
        _info: &dokan::OperationInfo<'_, '_, Self>,
        _context: &Self::Context,
    ) -> std::result::Result<(), OperationError> {
        // TODO: List directory contents
        Ok(())
    }

    fn set_file_attributes(
        &self,
        _file_name: &dokan::OperationInfo<'_, '_, Self>,
        _file_attributes: u32,
        _info: &dokan::OperationInfo<'_, '_, Self>,
        _context: &Self::Context,
    ) -> std::result::Result<(), OperationError> {
        if self.read_only {
            return Err(OperationError::NtStatus(NTSTATUS(0xC00000A2u32 as i32)));
        }
        Ok(())
    }

    fn set_file_time(
        &self,
        _file_name: &dokan::OperationInfo<'_, '_, Self>,
        _creation_time: FileTimeInfo,
        _last_access_time: FileTimeInfo,
        _last_write_time: FileTimeInfo,
        _info: &dokan::OperationInfo<'_, '_, Self>,
        _context: &Self::Context,
    ) -> std::result::Result<(), OperationError> {
        if self.read_only {
            return Err(OperationError::NtStatus(NTSTATUS(0xC00000A2u32 as i32)));
        }
        Ok(())
    }

    fn delete_file(
        &self,
        _file_name: &dokan::OperationInfo<'_, '_, Self>,
        _info: &dokan::OperationInfo<'_, '_, Self>,
        _context: &Self::Context,
    ) -> std::result::Result<(), OperationError> {
        if self.read_only {
            return Err(OperationError::NtStatus(NTSTATUS(0xC00000A2u32 as i32)));
        }
        Err(OperationError::NtStatus(NTSTATUS(0xC0000022u32 as i32)))
    }

    fn delete_directory(
        &self,
        _file_name: &dokan::OperationInfo<'_, '_, Self>,
        _info: &dokan::OperationInfo<'_, '_, Self>,
        _context: &Self::Context,
    ) -> std::result::Result<(), OperationError> {
        if self.read_only {
            return Err(OperationError::NtStatus(NTSTATUS(0xC00000A2u32 as i32)));
        }
        Err(OperationError::NtStatus(NTSTATUS(0xC0000022u32 as i32)))
    }

    fn move_file(
        &self,
        _file_name: &dokan::OperationInfo<'_, '_, Self>,
        _new_file_name: &dokan::OperationInfo<'_, '_, Self>,
        _replace_if_existing: bool,
        _info: &dokan::OperationInfo<'_, '_, Self>,
        _context: &Self::Context,
    ) -> std::result::Result<(), OperationError> {
        if self.read_only {
            return Err(OperationError::NtStatus(NTSTATUS(0xC00000A2u32 as i32)));
        }
        Err(OperationError::NtStatus(NTSTATUS(0xC0000022u32 as i32)))
    }

    fn set_end_of_file(
        &self,
        _file_name: &dokan::OperationInfo<'_, '_, Self>,
        _offset: i64,
        _info: &dokan::OperationInfo<'_, '_, Self>,
        _context: &Self::Context,
    ) -> std::result::Result<(), OperationError> {
        if self.read_only {
            return Err(OperationError::NtStatus(NTSTATUS(0xC00000A2u32 as i32)));
        }
        Err(OperationError::NtStatus(NTSTATUS(0xC0000022u32 as i32)))
    }

    fn set_allocation_size(
        &self,
        _file_name: &dokan::OperationInfo<'_, '_, Self>,
        _alloc_size: i64,
        _info: &dokan::OperationInfo<'_, '_, Self>,
        _context: &Self::Context,
    ) -> std::result::Result<(), OperationError> {
        if self.read_only {
            return Err(OperationError::NtStatus(NTSTATUS(0xC00000A2u32 as i32)));
        }
        Ok(())
    }

    fn get_disk_free_space(
        &self,
        _info: &dokan::OperationInfo<'_, '_, Self>,
    ) -> std::result::Result<DiskSpaceInfo, OperationError> {
        let total = self.fs.total_bytes();
        let used = self.fs.bytes_used();
        let free = total.saturating_sub(used);

        Ok(DiskSpaceInfo {
            byte_count: total,
            free_byte_count: free,
            available_byte_count: free,
        })
    }

    fn get_volume_information(
        &self,
        _info: &dokan::OperationInfo<'_, '_, Self>,
    ) -> std::result::Result<VolumeInfo, OperationError> {
        Ok(VolumeInfo {
            name: self.fs.label().to_string(),
            serial_number: 0x42545246, // "BTRF"
            max_component_length: 255,
            fs_flags: 0x0000001F, // Case sensitive, unicode, etc.
            fs_name: String::from("BTRFS"),
        })
    }

    fn mounted(
        &self,
        _mount_point: &dokan::OperationInfo<'_, '_, Self>,
        _info: &dokan::OperationInfo<'_, '_, Self>,
    ) -> std::result::Result<(), OperationError> {
        tracing::info!("BTRFS volume mounted");
        Ok(())
    }

    fn unmounted(&self, _info: &dokan::OperationInfo<'_, '_, Self>) -> std::result::Result<(), OperationError> {
        tracing::info!("BTRFS volume unmounted");
        Ok(())
    }
}

// Non-windows stub
#[cfg(not(windows))]
impl BtrfsHandler {
    // Stub methods for non-Windows platforms
}
