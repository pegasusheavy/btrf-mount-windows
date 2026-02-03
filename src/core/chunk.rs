//! BTRFS Chunk tree implementation
//!
//! The chunk tree maps logical addresses to physical addresses on disk.

use super::{item_type, tree::BtrfsKey, BtrfsError, Result, Superblock};
use crate::blockdev::BlockDevice;
use byteorder::{ByteOrder, LittleEndian};
use std::collections::BTreeMap;
use std::sync::Arc;

/// A chunk mapping entry
#[derive(Debug, Clone)]
pub struct ChunkMapping {
    /// Logical address start
    pub logical: u64,
    /// Size of the chunk
    pub size: u64,
    /// Stripe length
    pub stripe_len: u64,
    /// Type flags
    pub type_flags: u64,
    /// Number of stripes
    pub num_stripes: u16,
    /// Sub stripes
    pub sub_stripes: u16,
    /// Stripe information
    pub stripes: Vec<Stripe>,
}

/// A stripe within a chunk
#[derive(Debug, Clone)]
pub struct Stripe {
    /// Device ID
    pub devid: u64,
    /// Offset on device
    pub offset: u64,
    /// Device UUID
    pub dev_uuid: [u8; 16],
}

/// Chunk type flags
pub mod chunk_type {
    pub const DATA: u64 = 1 << 0;
    pub const SYSTEM: u64 = 1 << 1;
    pub const METADATA: u64 = 1 << 2;
    pub const RAID0: u64 = 1 << 3;
    pub const RAID1: u64 = 1 << 4;
    pub const DUP: u64 = 1 << 5;
    pub const RAID10: u64 = 1 << 6;
    pub const RAID5: u64 = 1 << 7;
    pub const RAID6: u64 = 1 << 8;
    pub const RAID1C3: u64 = 1 << 9;
    pub const RAID1C4: u64 = 1 << 10;
}

/// The chunk tree manages logical to physical address mappings
pub struct ChunkTree {
    /// Ordered map of logical address -> chunk mapping
    chunks: BTreeMap<u64, ChunkMapping>,
    /// Block device for reading tree nodes
    device: Arc<dyn BlockDevice>,
}

impl ChunkTree {
    /// Creates a chunk tree from the superblock's bootstrap chunks
    pub fn from_superblock(superblock: &Superblock, device: Arc<dyn BlockDevice>) -> Result<Self> {
        let mut chunks = BTreeMap::new();

        // Parse system chunks from superblock
        let sys_chunk_array = superblock.sys_chunk_array();
        let mut offset = 0;

        while offset < sys_chunk_array.len() {
            // Parse key
            if offset + 17 > sys_chunk_array.len() {
                break;
            }

            let key = BtrfsKey::from_bytes(&sys_chunk_array[offset..])?;
            offset += 17;

            if key.item_type != item_type::CHUNK_ITEM {
                return Err(BtrfsError::Corrupt(format!(
                    "Expected CHUNK_ITEM in sys_chunk_array, got {}",
                    key.item_type
                )));
            }

            // Parse chunk item
            if offset + 0x30 > sys_chunk_array.len() {
                break;
            }

            let chunk = Self::parse_chunk_item(&sys_chunk_array[offset..], key.offset)?;
            let chunk_size = 0x30 + chunk.num_stripes as usize * 0x20;
            offset += chunk_size;

            chunks.insert(chunk.logical, chunk);
        }

        Ok(Self { chunks, device })
    }

    /// Parses a CHUNK_ITEM from bytes
    fn parse_chunk_item(data: &[u8], logical: u64) -> Result<ChunkMapping> {
        if data.len() < 0x30 {
            return Err(BtrfsError::Corrupt("CHUNK_ITEM too small".to_string()));
        }

        let size = LittleEndian::read_u64(&data[0..8]);
        let _owner = LittleEndian::read_u64(&data[8..16]);
        let stripe_len = LittleEndian::read_u64(&data[16..24]);
        let type_flags = LittleEndian::read_u64(&data[24..32]);
        let _io_align = LittleEndian::read_u32(&data[32..36]);
        let _io_width = LittleEndian::read_u32(&data[36..40]);
        let _sector_size = LittleEndian::read_u32(&data[40..44]);
        let num_stripes = LittleEndian::read_u16(&data[44..46]);
        let sub_stripes = LittleEndian::read_u16(&data[46..48]);

        // Parse stripes
        let mut stripes = Vec::with_capacity(num_stripes as usize);
        let mut offset = 0x30;

        for _ in 0..num_stripes {
            if offset + 0x20 > data.len() {
                return Err(BtrfsError::Corrupt("CHUNK_ITEM stripe data truncated".to_string()));
            }

            let devid = LittleEndian::read_u64(&data[offset..offset + 8]);
            let stripe_offset = LittleEndian::read_u64(&data[offset + 8..offset + 16]);
            let mut dev_uuid = [0u8; 16];
            dev_uuid.copy_from_slice(&data[offset + 16..offset + 32]);

            stripes.push(Stripe {
                devid,
                offset: stripe_offset,
                dev_uuid,
            });

            offset += 0x20;
        }

        Ok(ChunkMapping {
            logical,
            size,
            stripe_len,
            type_flags,
            num_stripes,
            sub_stripes,
            stripes,
        })
    }

    /// Translates a logical address to physical address(es)
    pub fn logical_to_physical(&self, logical: u64) -> Result<Vec<u64>> {
        // Find the chunk containing this logical address
        let chunk = self
            .chunks
            .range(..=logical)
            .next_back()
            .map(|(_, v)| v)
            .ok_or_else(|| {
                BtrfsError::NotFound(format!("No chunk mapping for logical address {}", logical))
            })?;

        // Check if address is within chunk
        if logical >= chunk.logical + chunk.size {
            return Err(BtrfsError::NotFound(format!(
                "Logical address {} not in any chunk",
                logical
            )));
        }

        let offset_in_chunk = logical - chunk.logical;

        // Calculate physical addresses based on RAID type
        let mut physical_addrs = Vec::new();

        if chunk.type_flags & chunk_type::RAID0 != 0 {
            // RAID0: stripe across devices
            let stripe_nr = offset_in_chunk / chunk.stripe_len;
            let stripe_offset = offset_in_chunk % chunk.stripe_len;
            let stripe_index = (stripe_nr % chunk.num_stripes as u64) as usize;

            if stripe_index < chunk.stripes.len() {
                let stripe = &chunk.stripes[stripe_index];
                let physical =
                    stripe.offset + (stripe_nr / chunk.num_stripes as u64) * chunk.stripe_len + stripe_offset;
                physical_addrs.push(physical);
            }
        } else if chunk.type_flags & (chunk_type::RAID1 | chunk_type::DUP) != 0 {
            // RAID1/DUP: mirrored
            for stripe in &chunk.stripes {
                physical_addrs.push(stripe.offset + offset_in_chunk);
            }
        } else {
            // Single device
            if let Some(stripe) = chunk.stripes.first() {
                physical_addrs.push(stripe.offset + offset_in_chunk);
            }
        }

        if physical_addrs.is_empty() {
            return Err(BtrfsError::NotFound(format!(
                "Could not translate logical address {}",
                logical
            )));
        }

        Ok(physical_addrs)
    }

    /// Returns all chunks
    pub fn chunks(&self) -> &BTreeMap<u64, ChunkMapping> {
        &self.chunks
    }

    /// Adds a chunk mapping
    pub fn add_chunk(&mut self, chunk: ChunkMapping) {
        self.chunks.insert(chunk.logical, chunk);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_type_flags() {
        assert_eq!(chunk_type::DATA, 1);
        assert_eq!(chunk_type::SYSTEM, 2);
        assert_eq!(chunk_type::METADATA, 4);
        assert_eq!(chunk_type::RAID0, 8);
        assert_eq!(chunk_type::RAID1, 16);
        assert_eq!(chunk_type::DUP, 32);
        assert_eq!(chunk_type::RAID10, 64);
        assert_eq!(chunk_type::RAID5, 128);
        assert_eq!(chunk_type::RAID6, 256);
        assert_eq!(chunk_type::RAID1C3, 512);
        assert_eq!(chunk_type::RAID1C4, 1024);
    }

    fn create_mock_chunk_item_data(num_stripes: u16, type_flags: u64) -> Vec<u8> {
        let mut data = vec![0u8; 0x30 + num_stripes as usize * 0x20];
        // size
        data[0..8].copy_from_slice(&0x10000000u64.to_le_bytes()); // 256MB
        // owner
        data[8..16].copy_from_slice(&2u64.to_le_bytes()); // EXTENT_TREE
        // stripe_len
        data[16..24].copy_from_slice(&0x10000u64.to_le_bytes()); // 64KB
        // type_flags
        data[24..32].copy_from_slice(&type_flags.to_le_bytes());
        // io_align
        data[32..36].copy_from_slice(&4096u32.to_le_bytes());
        // io_width
        data[36..40].copy_from_slice(&4096u32.to_le_bytes());
        // sector_size
        data[40..44].copy_from_slice(&4096u32.to_le_bytes());
        // num_stripes
        data[44..46].copy_from_slice(&num_stripes.to_le_bytes());
        // sub_stripes
        data[46..48].copy_from_slice(&0u16.to_le_bytes());

        // Add stripes
        for i in 0..num_stripes {
            let offset = 0x30 + i as usize * 0x20;
            // devid
            data[offset..offset + 8].copy_from_slice(&(i as u64 + 1).to_le_bytes());
            // stripe offset
            data[offset + 8..offset + 16].copy_from_slice(&(0x100000u64 + i as u64 * 0x10000000).to_le_bytes());
            // dev_uuid
            for j in 0..16 {
                data[offset + 16 + j] = (i as u8).wrapping_add(j as u8);
            }
        }

        data
    }

    #[test]
    fn test_parse_chunk_item_single() {
        let data = create_mock_chunk_item_data(1, chunk_type::DATA);
        let chunk = ChunkTree::parse_chunk_item(&data, 0x1000000).unwrap();

        assert_eq!(chunk.logical, 0x1000000);
        assert_eq!(chunk.size, 0x10000000);
        assert_eq!(chunk.stripe_len, 0x10000);
        assert_eq!(chunk.type_flags, chunk_type::DATA);
        assert_eq!(chunk.num_stripes, 1);
        assert_eq!(chunk.stripes.len(), 1);
        assert_eq!(chunk.stripes[0].devid, 1);
        assert_eq!(chunk.stripes[0].offset, 0x100000);
    }

    #[test]
    fn test_parse_chunk_item_raid1() {
        let data = create_mock_chunk_item_data(2, chunk_type::DATA | chunk_type::RAID1);
        let chunk = ChunkTree::parse_chunk_item(&data, 0x1000000).unwrap();

        assert_eq!(chunk.num_stripes, 2);
        assert_eq!(chunk.stripes.len(), 2);
        assert_eq!(chunk.stripes[0].devid, 1);
        assert_eq!(chunk.stripes[1].devid, 2);
    }

    #[test]
    fn test_parse_chunk_item_too_small() {
        let data = vec![0u8; 0x20]; // Too small
        let result = ChunkTree::parse_chunk_item(&data, 0x1000000);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_chunk_item_stripe_truncated() {
        let mut data = create_mock_chunk_item_data(1, chunk_type::DATA);
        data[44..46].copy_from_slice(&5u16.to_le_bytes()); // Claim 5 stripes but only have 1
        let result = ChunkTree::parse_chunk_item(&data, 0x1000000);
        assert!(result.is_err());
    }

    #[test]
    fn test_stripe_debug() {
        let stripe = Stripe {
            devid: 1,
            offset: 0x100000,
            dev_uuid: [0; 16],
        };
        let debug_str = format!("{:?}", stripe);
        assert!(debug_str.contains("devid: 1"));
    }

    #[test]
    fn test_chunk_mapping_debug() {
        let chunk = ChunkMapping {
            logical: 0x1000000,
            size: 0x10000000,
            stripe_len: 0x10000,
            type_flags: chunk_type::DATA,
            num_stripes: 1,
            sub_stripes: 0,
            stripes: vec![Stripe {
                devid: 1,
                offset: 0x100000,
                dev_uuid: [0; 16],
            }],
        };
        let debug_str = format!("{:?}", chunk);
        assert!(debug_str.contains("logical: 16777216"));
    }
}
