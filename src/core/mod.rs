//! BTRFS filesystem core implementation
//!
//! This module provides a pure Rust implementation of the BTRFS filesystem,
//! supporting reading and writing of BTRFS volumes.

pub mod checksum;
pub mod chunk;
pub mod compress;
pub mod extent;
pub mod inode;
pub mod subvolume;
pub mod superblock;
pub mod tree;

use crate::blockdev::BlockDevice;
use std::sync::Arc;
use thiserror::Error;

pub use checksum::Checksum;
pub use chunk::ChunkTree;
pub use compress::CompressionType;
pub use extent::ExtentTree;
pub use inode::{Inode, InodeType};
pub use subvolume::Subvolume;
pub use superblock::Superblock;
pub use tree::{BtrfsKey, BtrfsTree, TreeType};

/// BTRFS magic number: "_BHRfS_M"
pub const BTRFS_MAGIC: [u8; 8] = *b"_BHRfS_M";

/// Primary superblock offset (64 KiB)
pub const SUPERBLOCK_OFFSET: u64 = 0x10000;

/// First superblock mirror offset (64 MiB)
pub const SUPERBLOCK_MIRROR1_OFFSET: u64 = 0x4000000;

/// Second superblock mirror offset (256 GiB)
pub const SUPERBLOCK_MIRROR2_OFFSET: u64 = 0x4000000000;

/// Default node size
pub const DEFAULT_NODE_SIZE: u32 = 16384;

/// Default sector size
pub const DEFAULT_SECTOR_SIZE: u32 = 4096;

/// Errors that can occur during BTRFS operations
#[derive(Error, Debug)]
pub enum BtrfsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Block device error: {0}")]
    BlockDevice(#[from] crate::blockdev::BlockDeviceError),

    #[error("Invalid magic number")]
    InvalidMagic,

    #[error("Checksum mismatch: expected {expected:08x}, got {actual:08x}")]
    ChecksumMismatch { expected: u32, actual: u32 },

    #[error("Unsupported BTRFS feature: {0}")]
    UnsupportedFeature(String),

    #[error("Corrupt filesystem: {0}")]
    Corrupt(String),

    #[error("Item not found: {0}")]
    NotFound(String),

    #[error("Invalid tree type: {0}")]
    InvalidTreeType(u8),

    #[error("Decompression error: {0}")]
    DecompressionError(String),

    #[error("Compression not supported: {0}")]
    UnsupportedCompression(u8),

    #[error("Invalid inode: {0}")]
    InvalidInode(u64),

    #[error("Not a directory")]
    NotADirectory,

    #[error("Not a file")]
    NotAFile,

    #[error("Subvolume not found: {0}")]
    SubvolumeNotFound(u64),

    #[error("Read-only filesystem")]
    ReadOnly,

    #[error("No space left")]
    NoSpace,
}

pub type Result<T> = std::result::Result<T, BtrfsError>;

/// A BTRFS filesystem instance
pub struct BtrfsFilesystem {
    /// The underlying block device
    device: Arc<dyn BlockDevice>,

    /// The superblock
    superblock: Superblock,

    /// The chunk tree for address translation
    chunk_tree: ChunkTree,

    /// Whether the filesystem is mounted read-only
    read_only: bool,
}

impl BtrfsFilesystem {
    /// Opens a BTRFS filesystem from a block device
    pub fn open(device: Arc<dyn BlockDevice>, read_only: bool) -> Result<Self> {
        // Read and validate superblock
        let superblock = Superblock::read(device.as_ref())?;

        // Initialize chunk tree from superblock's bootstrap chunks
        let chunk_tree = ChunkTree::from_superblock(&superblock, device.clone())?;

        Ok(Self {
            device,
            superblock,
            chunk_tree,
            read_only,
        })
    }

    /// Returns the superblock
    pub fn superblock(&self) -> &Superblock {
        &self.superblock
    }

    /// Returns the chunk tree
    pub fn chunk_tree(&self) -> &ChunkTree {
        &self.chunk_tree
    }

    /// Returns the underlying block device
    pub fn device(&self) -> &Arc<dyn BlockDevice> {
        &self.device
    }

    /// Returns the filesystem UUID
    pub fn uuid(&self) -> uuid::Uuid {
        self.superblock.fsid()
    }

    /// Returns the filesystem label
    pub fn label(&self) -> &str {
        self.superblock.label()
    }

    /// Returns the total size of the filesystem in bytes
    pub fn total_bytes(&self) -> u64 {
        self.superblock.total_bytes()
    }

    /// Returns the used bytes
    pub fn bytes_used(&self) -> u64 {
        self.superblock.bytes_used()
    }

    /// Returns whether the filesystem is mounted read-only
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Returns the node size
    pub fn node_size(&self) -> u32 {
        self.superblock.node_size()
    }

    /// Translates a logical address to physical address(es)
    pub fn logical_to_physical(&self, logical: u64) -> Result<Vec<u64>> {
        self.chunk_tree.logical_to_physical(logical)
    }

    /// Reads data from a logical address
    pub fn read_logical(&self, logical: u64, buf: &mut [u8]) -> Result<usize> {
        let physical_addrs = self.logical_to_physical(logical)?;

        if physical_addrs.is_empty() {
            return Err(BtrfsError::NotFound(format!(
                "No physical mapping for logical address {}",
                logical
            )));
        }

        // Read from the first physical address (TODO: handle RAID)
        let physical = physical_addrs[0];
        Ok(self.device.read_at(physical, buf)?)
    }

    /// Reads a tree node from a logical address
    pub fn read_node(&self, logical: u64) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; self.node_size() as usize];
        self.read_logical(logical, &mut buf)?;
        Ok(buf)
    }

    /// Lists all subvolumes in the filesystem
    pub fn list_subvolumes(&self) -> Result<Vec<Subvolume>> {
        subvolume::list_subvolumes(self)
    }

    /// Gets a subvolume by ID
    pub fn get_subvolume(&self, id: u64) -> Result<Subvolume> {
        subvolume::get_subvolume(self, id)
    }

    /// Gets the default subvolume
    pub fn default_subvolume(&self) -> Result<Subvolume> {
        let default_id = self.superblock.root_dir_objectid();
        self.get_subvolume(default_id)
    }
}

/// Object IDs for well-known trees
pub mod objectid {
    /// Root tree object ID
    pub const ROOT_TREE: u64 = 1;
    /// Extent tree object ID
    pub const EXTENT_TREE: u64 = 2;
    /// Chunk tree object ID
    pub const CHUNK_TREE: u64 = 3;
    /// Dev tree object ID
    pub const DEV_TREE: u64 = 4;
    /// FS tree object ID
    pub const FS_TREE: u64 = 5;
    /// Root tree directory object ID
    pub const ROOT_TREE_DIR: u64 = 6;
    /// Checksum tree object ID
    pub const CSUM_TREE: u64 = 7;
    /// Quota tree object ID
    pub const QUOTA_TREE: u64 = 8;
    /// UUID tree object ID
    pub const UUID_TREE: u64 = 9;
    /// Free space tree object ID
    pub const FREE_SPACE_TREE: u64 = 10;
    /// First free object ID for subvolumes
    pub const FIRST_FREE: u64 = 256;
    /// Last free object ID
    pub const LAST_FREE: u64 = u64::MAX - 256;
}

/// Item types in BTRFS trees
pub mod item_type {
    pub const INODE_ITEM: u8 = 0x01;
    pub const INODE_REF: u8 = 0x0C;
    pub const INODE_EXTREF: u8 = 0x0D;
    pub const XATTR_ITEM: u8 = 0x18;
    pub const ORPHAN_ITEM: u8 = 0x30;
    pub const DIR_LOG_ITEM: u8 = 0x3C;
    pub const DIR_LOG_INDEX: u8 = 0x48;
    pub const DIR_ITEM: u8 = 0x54;
    pub const DIR_INDEX: u8 = 0x60;
    pub const EXTENT_DATA: u8 = 0x6C;
    pub const EXTENT_CSUM: u8 = 0x80;
    pub const ROOT_ITEM: u8 = 0x84;
    pub const ROOT_BACKREF: u8 = 0x90;
    pub const ROOT_REF: u8 = 0x9C;
    pub const EXTENT_ITEM: u8 = 0xA8;
    pub const METADATA_ITEM: u8 = 0xA9;
    pub const TREE_BLOCK_REF: u8 = 0xB0;
    pub const EXTENT_DATA_REF: u8 = 0xB2;
    pub const SHARED_BLOCK_REF: u8 = 0xB6;
    pub const SHARED_DATA_REF: u8 = 0xB8;
    pub const BLOCK_GROUP_ITEM: u8 = 0xC0;
    pub const DEV_EXTENT: u8 = 0xCC;
    pub const DEV_ITEM: u8 = 0xD8;
    pub const CHUNK_ITEM: u8 = 0xE4;
    pub const STRING_ITEM: u8 = 0xFD;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_btrfs_constants() {
        assert_eq!(BTRFS_MAGIC, *b"_BHRfS_M");
        assert_eq!(SUPERBLOCK_OFFSET, 0x10000);
        assert_eq!(SUPERBLOCK_MIRROR1_OFFSET, 0x4000000);
        assert_eq!(SUPERBLOCK_MIRROR2_OFFSET, 0x4000000000);
        assert_eq!(DEFAULT_NODE_SIZE, 16384);
        assert_eq!(DEFAULT_SECTOR_SIZE, 4096);
    }

    #[test]
    fn test_objectid_constants() {
        assert_eq!(objectid::ROOT_TREE, 1);
        assert_eq!(objectid::EXTENT_TREE, 2);
        assert_eq!(objectid::CHUNK_TREE, 3);
        assert_eq!(objectid::DEV_TREE, 4);
        assert_eq!(objectid::FS_TREE, 5);
        assert_eq!(objectid::ROOT_TREE_DIR, 6);
        assert_eq!(objectid::CSUM_TREE, 7);
        assert_eq!(objectid::QUOTA_TREE, 8);
        assert_eq!(objectid::UUID_TREE, 9);
        assert_eq!(objectid::FREE_SPACE_TREE, 10);
        assert_eq!(objectid::FIRST_FREE, 256);
        assert_eq!(objectid::LAST_FREE, u64::MAX - 256);
    }

    #[test]
    fn test_item_type_constants() {
        assert_eq!(item_type::INODE_ITEM, 0x01);
        assert_eq!(item_type::INODE_REF, 0x0C);
        assert_eq!(item_type::INODE_EXTREF, 0x0D);
        assert_eq!(item_type::XATTR_ITEM, 0x18);
        assert_eq!(item_type::ORPHAN_ITEM, 0x30);
        assert_eq!(item_type::DIR_LOG_ITEM, 0x3C);
        assert_eq!(item_type::DIR_LOG_INDEX, 0x48);
        assert_eq!(item_type::DIR_ITEM, 0x54);
        assert_eq!(item_type::DIR_INDEX, 0x60);
        assert_eq!(item_type::EXTENT_DATA, 0x6C);
        assert_eq!(item_type::EXTENT_CSUM, 0x80);
        assert_eq!(item_type::ROOT_ITEM, 0x84);
        assert_eq!(item_type::ROOT_BACKREF, 0x90);
        assert_eq!(item_type::ROOT_REF, 0x9C);
        assert_eq!(item_type::EXTENT_ITEM, 0xA8);
        assert_eq!(item_type::METADATA_ITEM, 0xA9);
        assert_eq!(item_type::TREE_BLOCK_REF, 0xB0);
        assert_eq!(item_type::EXTENT_DATA_REF, 0xB2);
        assert_eq!(item_type::SHARED_BLOCK_REF, 0xB6);
        assert_eq!(item_type::SHARED_DATA_REF, 0xB8);
        assert_eq!(item_type::BLOCK_GROUP_ITEM, 0xC0);
        assert_eq!(item_type::DEV_EXTENT, 0xCC);
        assert_eq!(item_type::DEV_ITEM, 0xD8);
        assert_eq!(item_type::CHUNK_ITEM, 0xE4);
        assert_eq!(item_type::STRING_ITEM, 0xFD);
    }

    #[test]
    fn test_btrfs_error_display() {
        let err = BtrfsError::InvalidMagic;
        assert!(format!("{}", err).contains("magic"));

        let err = BtrfsError::ChecksumMismatch {
            expected: 0x12345678,
            actual: 0x87654321,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("12345678"));
        assert!(msg.contains("87654321"));

        let err = BtrfsError::UnsupportedFeature("test feature".to_string());
        assert!(format!("{}", err).contains("test feature"));

        let err = BtrfsError::Corrupt("corruption details".to_string());
        assert!(format!("{}", err).contains("corruption details"));

        let err = BtrfsError::NotFound("missing item".to_string());
        assert!(format!("{}", err).contains("missing item"));

        let err = BtrfsError::InvalidTreeType(42);
        assert!(format!("{}", err).contains("42"));

        let err = BtrfsError::DecompressionError("zstd failed".to_string());
        assert!(format!("{}", err).contains("zstd failed"));

        let err = BtrfsError::UnsupportedCompression(99);
        assert!(format!("{}", err).contains("99"));

        let err = BtrfsError::InvalidInode(256);
        assert!(format!("{}", err).contains("256"));

        let err = BtrfsError::NotADirectory;
        assert!(format!("{}", err).contains("directory"));

        let err = BtrfsError::NotAFile;
        assert!(format!("{}", err).contains("file"));

        let err = BtrfsError::SubvolumeNotFound(1000);
        assert!(format!("{}", err).contains("1000"));

        let err = BtrfsError::ReadOnly;
        assert!(format!("{}", err).contains("read-only") || format!("{}", err).contains("Read-only"));

        let err = BtrfsError::NoSpace;
        assert!(format!("{}", err).contains("space"));
    }

    #[test]
    fn test_btrfs_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let btrfs_err: BtrfsError = io_err.into();
        match btrfs_err {
            BtrfsError::Io(_) => (),
            _ => panic!("Expected Io error variant"),
        }
    }

    #[test]
    fn test_btrfs_error_from_block_device() {
        let bd_err = crate::blockdev::BlockDeviceError::NotFound("test".to_string());
        let btrfs_err: BtrfsError = bd_err.into();
        match btrfs_err {
            BtrfsError::BlockDevice(_) => (),
            _ => panic!("Expected BlockDevice error variant"),
        }
    }
}
