//! BTRFS Extent tree implementation
//!
//! The extent tree tracks space allocation on disk.

use super::{BtrfsError, BtrfsFilesystem, Result};
use byteorder::{ByteOrder, LittleEndian};

/// An extent item describing allocated space
#[derive(Debug, Clone)]
pub struct ExtentItem {
    /// Reference count
    pub refs: u64,
    /// Generation
    pub generation: u64,
    /// Flags
    pub flags: u64,
}

/// Extent flags
pub mod extent_flags {
    pub const DATA: u64 = 1 << 0;
    pub const TREE_BLOCK: u64 = 1 << 1;
    pub const FULL_BACKREF: u64 = 1 << 8;
}

/// Block group item
#[derive(Debug, Clone)]
pub struct BlockGroupItem {
    /// Used bytes in this block group
    pub used: u64,
    /// Chunk object ID
    pub chunk_objectid: u64,
    /// Flags
    pub flags: u64,
}

impl BlockGroupItem {
    /// Parses a block group item from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 24 {
            return Err(BtrfsError::Corrupt("BlockGroupItem too small".to_string()));
        }

        Ok(Self {
            used: LittleEndian::read_u64(&data[0..8]),
            chunk_objectid: LittleEndian::read_u64(&data[8..16]),
            flags: LittleEndian::read_u64(&data[16..24]),
        })
    }
}

/// The extent tree for space allocation tracking
pub struct ExtentTree<'a> {
    fs: &'a BtrfsFilesystem,
}

impl<'a> ExtentTree<'a> {
    /// Creates a new extent tree accessor
    pub fn new(fs: &'a BtrfsFilesystem) -> Self {
        Self { fs }
    }

    /// Gets the total allocated space
    pub fn total_allocated(&self) -> Result<u64> {
        // TODO: Implement by iterating extent tree
        Ok(self.fs.superblock().bytes_used())
    }

    /// Gets the total free space
    pub fn total_free(&self) -> Result<u64> {
        let total = self.fs.superblock().total_bytes();
        let used = self.total_allocated()?;
        Ok(total.saturating_sub(used))
    }

    /// Checks if an extent is allocated
    pub fn is_allocated(&self, _logical: u64, _size: u64) -> Result<bool> {
        // TODO: Implement extent lookup
        Ok(false)
    }
}

/// Device extent information
#[derive(Debug, Clone)]
pub struct DevExtent {
    /// Chunk tree (always 3)
    pub chunk_tree: u64,
    /// Chunk object ID
    pub chunk_objectid: u64,
    /// Chunk offset (logical address)
    pub chunk_offset: u64,
    /// Length in bytes
    pub length: u64,
    /// Chunk tree UUID
    pub chunk_tree_uuid: [u8; 16],
}

impl DevExtent {
    /// Parses a device extent from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 48 {
            return Err(BtrfsError::Corrupt("DevExtent too small".to_string()));
        }

        let mut chunk_tree_uuid = [0u8; 16];
        chunk_tree_uuid.copy_from_slice(&data[32..48]);

        Ok(Self {
            chunk_tree: LittleEndian::read_u64(&data[0..8]),
            chunk_objectid: LittleEndian::read_u64(&data[8..16]),
            chunk_offset: LittleEndian::read_u64(&data[16..24]),
            length: LittleEndian::read_u64(&data[24..32]),
            chunk_tree_uuid,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extent_flags() {
        assert_eq!(extent_flags::DATA, 1);
        assert_eq!(extent_flags::TREE_BLOCK, 2);
        assert_eq!(extent_flags::FULL_BACKREF, 256);
    }

    fn create_mock_block_group_item_data() -> Vec<u8> {
        let mut data = vec![0u8; 24];
        // used
        data[0..8].copy_from_slice(&1048576u64.to_le_bytes()); // 1MB used
        // chunk_objectid
        data[8..16].copy_from_slice(&256u64.to_le_bytes());
        // flags (DATA | SINGLE)
        data[16..24].copy_from_slice(&1u64.to_le_bytes());
        data
    }

    #[test]
    fn test_block_group_item_from_bytes() {
        let data = create_mock_block_group_item_data();
        let item = BlockGroupItem::from_bytes(&data).unwrap();

        assert_eq!(item.used, 1048576);
        assert_eq!(item.chunk_objectid, 256);
        assert_eq!(item.flags, 1);
    }

    #[test]
    fn test_block_group_item_from_bytes_too_small() {
        let data = vec![0u8; 20]; // Too small
        let result = BlockGroupItem::from_bytes(&data);
        assert!(result.is_err());
    }

    fn create_mock_dev_extent_data() -> Vec<u8> {
        let mut data = vec![0u8; 48];
        // chunk_tree (always 3)
        data[0..8].copy_from_slice(&3u64.to_le_bytes());
        // chunk_objectid
        data[8..16].copy_from_slice(&256u64.to_le_bytes());
        // chunk_offset
        data[16..24].copy_from_slice(&0x100000u64.to_le_bytes());
        // length
        data[24..32].copy_from_slice(&0x10000000u64.to_le_bytes()); // 256MB
        // chunk_tree_uuid
        for i in 0..16 {
            data[32 + i] = i as u8;
        }
        data
    }

    #[test]
    fn test_dev_extent_from_bytes() {
        let data = create_mock_dev_extent_data();
        let extent = DevExtent::from_bytes(&data).unwrap();

        assert_eq!(extent.chunk_tree, 3);
        assert_eq!(extent.chunk_objectid, 256);
        assert_eq!(extent.chunk_offset, 0x100000);
        assert_eq!(extent.length, 0x10000000);
        
        // Check UUID
        for i in 0..16 {
            assert_eq!(extent.chunk_tree_uuid[i], i as u8);
        }
    }

    #[test]
    fn test_dev_extent_from_bytes_too_small() {
        let data = vec![0u8; 40]; // Too small
        let result = DevExtent::from_bytes(&data);
        assert!(result.is_err());
    }
}
