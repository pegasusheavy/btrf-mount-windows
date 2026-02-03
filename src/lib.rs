//! BTRFS Mount Windows
//!
//! A userspace BTRFS filesystem driver for Windows using Dokan (Windows FUSE equivalent).
//!
//! # Features
//!
//! - Read/write support for BTRFS filesystems
//! - Subvolume and snapshot management
//! - Compression support (zlib, LZO, zstd)
//! - Physical disk and image file mounting
//! - GUI for volume management (via Tauri)
//!
//! # Architecture
//!
//! The crate is organized into several modules:
//!
//! - [`blockdev`]: Block device abstraction layer for physical disks and image files
//! - [`core`]: BTRFS filesystem implementation (parsing, trees, compression)
//! - [`fuse`]: Dokan filesystem handler for Windows integration

pub mod blockdev;
pub mod core;
pub mod fuse;

pub use blockdev::{BlockDevice, BlockDeviceError};
pub use core::{
    BtrfsError, BtrfsFilesystem, BtrfsKey, CompressionType, Inode, InodeType, Subvolume,
    Superblock, TreeType,
};
pub use fuse::{BtrfsMount, MountOptions};

#[cfg(windows)]
pub use fuse::BtrfsHandler;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
