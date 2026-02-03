//! Mount/unmount operations for BTRFS volumes

#[cfg(windows)]
use super::handler::BtrfsHandler;
use crate::core::{BtrfsError, BtrfsFilesystem, Result};
use std::sync::Arc;

#[cfg(windows)]
use dokan::{Drive, MountFlags};

/// Options for mounting a BTRFS volume
#[derive(Debug, Clone)]
pub struct MountOptions {
    /// Drive letter to mount to
    pub drive_letter: char,
    /// Mount read-only
    pub read_only: bool,
    /// Subvolume ID to mount (None for default)
    pub subvolume_id: Option<u64>,
    /// Enable debug output
    pub debug: bool,
    /// Thread count for Dokan (0 for auto)
    pub thread_count: u16,
    /// Volume name
    pub volume_name: String,
    /// Filesystem name
    pub filesystem_name: String,
}

impl Default for MountOptions {
    fn default() -> Self {
        Self {
            drive_letter: 'Z',
            read_only: false,
            subvolume_id: None,
            debug: false,
            thread_count: 0,
            volume_name: String::from("BTRFS Volume"),
            filesystem_name: String::from("BTRFS"),
        }
    }
}

/// A mounted BTRFS filesystem
pub struct BtrfsMount {
    /// The filesystem
    fs: Arc<BtrfsFilesystem>,
    /// Mount point (drive letter)
    mount_point: String,
    /// Whether mounted
    mounted: bool,
}

impl BtrfsMount {
    /// Mounts a BTRFS filesystem
    #[cfg(windows)]
    pub fn mount(fs: Arc<BtrfsFilesystem>, options: MountOptions) -> Result<Self> {
        let mount_point = format!("{}:", options.drive_letter);
        let handler = BtrfsHandler::new(fs.clone(), options.read_only);

        let mut flags = MountFlags::empty();
        if options.debug {
            flags |= MountFlags::DEBUG;
        }
        if options.read_only {
            flags |= MountFlags::WRITE_PROTECT;
        }

        // Convert mount point to wide string
        let mount_point_wide: Vec<u16> = mount_point
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let mut drive = Drive::new();
        drive
            .mount_point(&mount_point)
            .flags(flags)
            .thread_count(options.thread_count);

        // Start mount in a separate thread
        let handler_arc = Arc::new(handler);
        let handle = std::thread::spawn(move || {
            if let Err(e) = drive.mount(&*handler_arc) {
                tracing::error!("Mount error: {:?}", e);
            }
        });

        Ok(Self {
            fs,
            mount_point,
            mounted: true,
        })
    }

    #[cfg(not(windows))]
    pub fn mount(fs: Arc<BtrfsFilesystem>, options: MountOptions) -> Result<Self> {
        let mount_point = format!("{}:", options.drive_letter);
        tracing::warn!("Dokan mount not available on this platform");

        Ok(Self {
            fs,
            mount_point,
            mounted: false,
        })
    }

    /// Unmounts the filesystem
    #[cfg(windows)]
    pub fn unmount(&mut self) -> Result<()> {
        if self.mounted {
            dokan::unmount(&self.mount_point);
            self.mounted = false;
        }
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn unmount(&mut self) -> Result<()> {
        self.mounted = false;
        Ok(())
    }

    /// Returns the mount point
    pub fn mount_point(&self) -> &str {
        &self.mount_point
    }

    /// Returns true if mounted
    pub fn is_mounted(&self) -> bool {
        self.mounted
    }

    /// Returns the filesystem
    pub fn filesystem(&self) -> &Arc<BtrfsFilesystem> {
        &self.fs
    }
}

impl Drop for BtrfsMount {
    fn drop(&mut self) {
        let _ = self.unmount();
    }
}

/// Lists active Dokan mount points
#[cfg(windows)]
pub fn list_mount_points() -> Vec<String> {
    dokan::get_mount_point_list()
        .into_iter()
        .map(|info| info.mount_point)
        .collect()
}

#[cfg(not(windows))]
pub fn list_mount_points() -> Vec<String> {
    Vec::new()
}
