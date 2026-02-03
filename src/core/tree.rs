//! BTRFS B-tree implementation
//!
//! BTRFS uses copy-on-write B-trees for all on-disk data structures.
//! All parsing functions are optimized with inline hints for hot paths.

use super::{checksum, BtrfsError, BtrfsFilesystem, Result};
use byteorder::{ByteOrder, LittleEndian};
use zerocopy::{FromBytes, Immutable, KnownLayout};

/// Size of a node header
pub const NODE_HEADER_SIZE: usize = 0x65;

/// Size of a key pointer in internal nodes
pub const KEY_PTR_SIZE: usize = 0x21;

/// Size of an item header in leaf nodes
pub const ITEM_SIZE: usize = 0x19;

/// Size of a key structure
pub const KEY_SIZE: usize = 0x11;

/// Tree types in BTRFS
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeType {
    Root,
    Extent,
    Chunk,
    Dev,
    Fs,
    Csum,
    Quota,
    Uuid,
    FreeSpace,
    BlockGroup,
    Unknown(u64),
}

impl TreeType {
    /// Creates a TreeType from an object ID
    #[inline]
    pub const fn from_objectid(objectid: u64) -> Self {
        match objectid {
            1 => Self::Root,
            2 => Self::Extent,
            3 => Self::Chunk,
            4 => Self::Dev,
            5 => Self::Fs,
            7 => Self::Csum,
            8 => Self::Quota,
            9 => Self::Uuid,
            10 => Self::FreeSpace,
            _ => Self::Unknown(objectid),
        }
    }
}

/// A BTRFS key used for B-tree lookups
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromBytes, KnownLayout, Immutable)]
#[repr(C, packed)]
pub struct BtrfsKey {
    /// Object ID
    pub objectid: u64,
    /// Item type
    pub item_type: u8,
    /// Offset (meaning depends on item type)
    pub offset: u64,
}

impl BtrfsKey {
    /// Creates a new key
    #[inline]
    pub const fn new(objectid: u64, item_type: u8, offset: u64) -> Self {
        Self {
            objectid,
            item_type,
            offset,
        }
    }

    /// Parses a key from bytes - hot path, optimized
    #[inline]
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < KEY_SIZE {
            return Err(BtrfsError::Corrupt("Key data too small".to_string()));
        }

        Ok(Self {
            objectid: LittleEndian::read_u64(&data[0..8]),
            item_type: data[8],
            offset: LittleEndian::read_u64(&data[9..17]),
        })
    }

    /// Parses a key from bytes without bounds checking (caller must ensure data is valid)
    /// 
    /// # Safety
    /// Caller must ensure `data.len() >= KEY_SIZE` (17 bytes)
    #[inline]
    pub unsafe fn from_bytes_unchecked(data: &[u8]) -> Self {
        Self {
            objectid: LittleEndian::read_u64(data.get_unchecked(0..8)),
            item_type: *data.get_unchecked(8),
            offset: LittleEndian::read_u64(data.get_unchecked(9..17)),
        }
    }

    /// Returns the minimum possible key
    #[inline]
    pub const fn min() -> Self {
        Self::new(0, 0, 0)
    }

    /// Returns the maximum possible key
    #[inline]
    pub const fn max() -> Self {
        Self::new(u64::MAX, u8::MAX, u64::MAX)
    }
}

/// Node header structure
#[derive(Debug, Clone, Copy, FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NodeHeader {
    /// Checksum
    pub csum: [u8; 32],
    /// Filesystem UUID
    pub fsid: [u8; 16],
    /// Logical address of this node
    pub bytenr: u64,
    /// Flags
    pub flags: [u8; 7],
    /// Backref revision
    pub backref_rev: u8,
    /// Chunk tree UUID
    pub chunk_tree_uuid: [u8; 16],
    /// Generation
    pub generation: u64,
    /// Owner tree ID
    pub owner: u64,
    /// Number of items
    pub nritems: u32,
    /// Level (0 for leaf)
    pub level: u8,
}

impl NodeHeader {
    /// Parses a node header from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < NODE_HEADER_SIZE {
            return Err(BtrfsError::Corrupt("Node header too small".to_string()));
        }

        Self::read_from_bytes(&data[..NODE_HEADER_SIZE])
            .map(|h| h.clone())
            .map_err(|_| BtrfsError::Corrupt("Failed to parse node header".to_string()))
    }

    /// Returns true if this is a leaf node
    pub fn is_leaf(&self) -> bool {
        self.level == 0
    }
}

/// Key pointer in internal nodes
#[derive(Debug, Clone, Copy)]
pub struct KeyPtr {
    /// Key
    pub key: BtrfsKey,
    /// Block number (logical address)
    pub blockptr: u64,
    /// Generation
    pub generation: u64,
}

impl KeyPtr {
    /// Parses a key pointer from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < KEY_PTR_SIZE {
            return Err(BtrfsError::Corrupt("Key pointer too small".to_string()));
        }

        Ok(Self {
            key: BtrfsKey::from_bytes(&data[0..KEY_SIZE])?,
            blockptr: LittleEndian::read_u64(&data[KEY_SIZE..KEY_SIZE + 8]),
            generation: LittleEndian::read_u64(&data[KEY_SIZE + 8..KEY_SIZE + 16]),
        })
    }
}

/// Item in leaf nodes
#[derive(Debug, Clone, Copy)]
pub struct Item {
    /// Key
    pub key: BtrfsKey,
    /// Data offset relative to end of header
    pub offset: u32,
    /// Data size
    pub size: u32,
}

impl Item {
    /// Parses an item from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < ITEM_SIZE {
            return Err(BtrfsError::Corrupt("Item too small".to_string()));
        }

        Ok(Self {
            key: BtrfsKey::from_bytes(&data[0..KEY_SIZE])?,
            offset: LittleEndian::read_u32(&data[KEY_SIZE..KEY_SIZE + 4]),
            size: LittleEndian::read_u32(&data[KEY_SIZE + 4..KEY_SIZE + 8]),
        })
    }
}

/// A parsed BTRFS tree node
#[derive(Debug)]
pub struct TreeNode {
    /// Node header
    pub header: NodeHeader,
    /// Raw node data
    data: Vec<u8>,
}

impl TreeNode {
    /// Parses a tree node from raw data
    pub fn parse(data: Vec<u8>) -> Result<Self> {
        // Verify checksum
        checksum::verify_node_checksum(&data)?;

        let header = NodeHeader::from_bytes(&data)?;

        Ok(Self { header, data })
    }

    /// Returns true if this is a leaf node
    pub fn is_leaf(&self) -> bool {
        self.header.is_leaf()
    }

    /// Returns the number of items in this node
    pub fn num_items(&self) -> u32 {
        self.header.nritems
    }

    /// Returns key pointers for internal nodes
    pub fn key_ptrs(&self) -> Result<Vec<KeyPtr>> {
        if self.is_leaf() {
            return Err(BtrfsError::Corrupt(
                "Cannot get key pointers from leaf node".to_string(),
            ));
        }

        let mut ptrs = Vec::with_capacity(self.header.nritems as usize);
        let mut offset = NODE_HEADER_SIZE;

        for _ in 0..self.header.nritems {
            let ptr = KeyPtr::from_bytes(&self.data[offset..])?;
            ptrs.push(ptr);
            offset += KEY_PTR_SIZE;
        }

        Ok(ptrs)
    }

    /// Returns items for leaf nodes
    pub fn items(&self) -> Result<Vec<Item>> {
        if !self.is_leaf() {
            return Err(BtrfsError::Corrupt(
                "Cannot get items from internal node".to_string(),
            ));
        }

        let mut items = Vec::with_capacity(self.header.nritems as usize);
        let mut offset = NODE_HEADER_SIZE;

        for _ in 0..self.header.nritems {
            let item = Item::from_bytes(&self.data[offset..])?;
            items.push(item);
            offset += ITEM_SIZE;
        }

        Ok(items)
    }

    /// Gets item data for a leaf node item
    pub fn item_data(&self, item: &Item) -> &[u8] {
        let start = NODE_HEADER_SIZE + item.offset as usize;
        let end = start + item.size as usize;
        &self.data[start..end]
    }

    /// Gets the raw node data
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

/// A BTRFS tree for traversal and lookups
pub struct BtrfsTree<'a> {
    fs: &'a BtrfsFilesystem,
    root_logical: u64,
    root_level: u8,
}

impl<'a> BtrfsTree<'a> {
    /// Creates a new tree from a root address
    pub fn new(fs: &'a BtrfsFilesystem, root_logical: u64, root_level: u8) -> Self {
        Self {
            fs,
            root_logical,
            root_level,
        }
    }

    /// Reads a node at the given logical address
    pub fn read_node(&self, logical: u64) -> Result<TreeNode> {
        let data = self.fs.read_node(logical)?;
        TreeNode::parse(data)
    }

    /// Searches for a key in the tree
    pub fn search(&self, key: &BtrfsKey) -> Result<Option<(Item, Vec<u8>)>> {
        self.search_from(self.root_logical, key)
    }

    /// Searches for a key starting from a given node
    fn search_from(&self, logical: u64, key: &BtrfsKey) -> Result<Option<(Item, Vec<u8>)>> {
        let node = self.read_node(logical)?;

        if node.is_leaf() {
            // Search in leaf node
            let items = node.items()?;
            for item in items {
                if item.key == *key {
                    let data = node.item_data(&item).to_vec();
                    return Ok(Some((item, data)));
                }
            }
            Ok(None)
        } else {
            // Search in internal node
            let ptrs = node.key_ptrs()?;

            // Find the child to descend into
            let mut child_ptr = ptrs[0].blockptr;
            for ptr in &ptrs {
                if ptr.key > *key {
                    break;
                }
                child_ptr = ptr.blockptr;
            }

            self.search_from(child_ptr, key)
        }
    }

    /// Searches for items in a range
    pub fn search_range(
        &self,
        min_key: &BtrfsKey,
        max_key: &BtrfsKey,
    ) -> Result<Vec<(Item, Vec<u8>)>> {
        let mut results = Vec::new();
        self.search_range_from(self.root_logical, min_key, max_key, &mut results)?;
        Ok(results)
    }

    /// Searches for items in a range starting from a given node
    fn search_range_from(
        &self,
        logical: u64,
        min_key: &BtrfsKey,
        max_key: &BtrfsKey,
        results: &mut Vec<(Item, Vec<u8>)>,
    ) -> Result<()> {
        let node = self.read_node(logical)?;

        if node.is_leaf() {
            let items = node.items()?;
            for item in items {
                if item.key >= *min_key && item.key <= *max_key {
                    let data = node.item_data(&item).to_vec();
                    results.push((item, data));
                }
            }
        } else {
            let ptrs = node.key_ptrs()?;
            for ptr in ptrs {
                // Check if this subtree might contain items in range
                if ptr.key <= *max_key {
                    self.search_range_from(ptr.blockptr, min_key, max_key, results)?;
                }
            }
        }

        Ok(())
    }

    /// Iterates over all items in the tree
    pub fn iter(&'a self) -> TreeIterator<'a> {
        TreeIterator::new(self)
    }
}

/// Iterator over tree items
pub struct TreeIterator<'a> {
    tree: &'a BtrfsTree<'a>,
    stack: Vec<(TreeNode, usize)>,
    initialized: bool,
}

impl<'a> TreeIterator<'a> {
    fn new(tree: &'a BtrfsTree<'a>) -> Self {
        Self {
            tree,
            stack: Vec::new(),
            initialized: false,
        }
    }

    fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        // Descend to leftmost leaf
        let mut logical = self.tree.root_logical;
        loop {
            let node = self.tree.read_node(logical)?;
            if node.is_leaf() {
                self.stack.push((node, 0));
                break;
            } else {
                let ptrs = node.key_ptrs()?;
                if ptrs.is_empty() {
                    break;
                }
                logical = ptrs[0].blockptr;
                self.stack.push((node, 0));
            }
        }

        self.initialized = true;
        Ok(())
    }
}

impl<'a> Iterator for TreeIterator<'a> {
    type Item = Result<(Item, Vec<u8>)>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Err(e) = self.initialize() {
            return Some(Err(e));
        }

        loop {
            let (node, idx) = self.stack.last_mut()?;

            if node.is_leaf() {
                let items = match node.items() {
                    Ok(items) => items,
                    Err(e) => return Some(Err(e)),
                };

                if *idx < items.len() {
                    let item = items[*idx];
                    let data = node.item_data(&item).to_vec();
                    *idx += 1;
                    return Some(Ok((item, data)));
                } else {
                    // Move to next node
                    self.stack.pop();
                    // TODO: Implement sibling traversal
                    return None;
                }
            } else {
                // Should not happen after initialization
                self.stack.pop();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_type_from_objectid() {
        assert_eq!(TreeType::from_objectid(1), TreeType::Root);
        assert_eq!(TreeType::from_objectid(2), TreeType::Extent);
        assert_eq!(TreeType::from_objectid(3), TreeType::Chunk);
        assert_eq!(TreeType::from_objectid(4), TreeType::Dev);
        assert_eq!(TreeType::from_objectid(5), TreeType::Fs);
        assert_eq!(TreeType::from_objectid(7), TreeType::Csum);
        assert_eq!(TreeType::from_objectid(8), TreeType::Quota);
        assert_eq!(TreeType::from_objectid(9), TreeType::Uuid);
        assert_eq!(TreeType::from_objectid(10), TreeType::FreeSpace);
        assert_eq!(TreeType::from_objectid(100), TreeType::Unknown(100));
        assert_eq!(TreeType::from_objectid(256), TreeType::Unknown(256));
    }

    #[test]
    fn test_btrfs_key_new() {
        let key = BtrfsKey::new(256, 0x01, 0);
        // Copy packed fields to avoid alignment issues
        let objectid = { key.objectid };
        let item_type = { key.item_type };
        let offset = { key.offset };
        assert_eq!(objectid, 256);
        assert_eq!(item_type, 0x01);
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_btrfs_key_min_max() {
        let min = BtrfsKey::min();
        let max = BtrfsKey::max();

        // Copy packed fields to avoid alignment issues
        let min_objectid = { min.objectid };
        let min_item_type = { min.item_type };
        let min_offset = { min.offset };
        let max_objectid = { max.objectid };
        let max_item_type = { max.item_type };
        let max_offset = { max.offset };

        assert_eq!(min_objectid, 0);
        assert_eq!(min_item_type, 0);
        assert_eq!(min_offset, 0);

        assert_eq!(max_objectid, u64::MAX);
        assert_eq!(max_item_type, u8::MAX);
        assert_eq!(max_offset, u64::MAX);

        assert!(min < max);
    }

    #[test]
    fn test_btrfs_key_ordering() {
        let key1 = BtrfsKey::new(100, 0x01, 0);
        let key2 = BtrfsKey::new(100, 0x01, 1);
        let key3 = BtrfsKey::new(100, 0x02, 0);
        let key4 = BtrfsKey::new(200, 0x01, 0);

        assert!(key1 < key2);
        assert!(key2 < key3);
        assert!(key3 < key4);
        assert!(key1 < key4);
    }

    #[test]
    fn test_btrfs_key_from_bytes() {
        // Create a key: objectid=256, type=0x01, offset=4096
        let mut data = vec![0u8; KEY_SIZE];
        data[0..8].copy_from_slice(&256u64.to_le_bytes());
        data[8] = 0x01;
        data[9..17].copy_from_slice(&4096u64.to_le_bytes());

        let key = BtrfsKey::from_bytes(&data).unwrap();
        // Copy packed fields to avoid alignment issues
        let objectid = { key.objectid };
        let item_type = { key.item_type };
        let offset = { key.offset };
        assert_eq!(objectid, 256);
        assert_eq!(item_type, 0x01);
        assert_eq!(offset, 4096);
    }

    #[test]
    fn test_btrfs_key_from_bytes_too_small() {
        let data = vec![0u8; 10]; // Too small
        let result = BtrfsKey::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_key_ptr_from_bytes() {
        let mut data = vec![0u8; KEY_PTR_SIZE];
        // Key
        data[0..8].copy_from_slice(&256u64.to_le_bytes());
        data[8] = 0x84; // ROOT_ITEM
        data[9..17].copy_from_slice(&0u64.to_le_bytes());
        // Block pointer
        data[17..25].copy_from_slice(&0x100000u64.to_le_bytes());
        // Generation
        data[25..33].copy_from_slice(&42u64.to_le_bytes());

        let ptr = KeyPtr::from_bytes(&data).unwrap();
        // Copy packed fields to avoid alignment issues
        let key_objectid = { ptr.key.objectid };
        let key_item_type = { ptr.key.item_type };
        assert_eq!(key_objectid, 256);
        assert_eq!(key_item_type, 0x84);
        assert_eq!(ptr.blockptr, 0x100000);
        assert_eq!(ptr.generation, 42);
    }

    #[test]
    fn test_key_ptr_from_bytes_too_small() {
        let data = vec![0u8; 20]; // Too small
        let result = KeyPtr::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_item_from_bytes() {
        let mut data = vec![0u8; ITEM_SIZE];
        // Key
        data[0..8].copy_from_slice(&256u64.to_le_bytes());
        data[8] = 0x01; // INODE_ITEM
        data[9..17].copy_from_slice(&0u64.to_le_bytes());
        // Offset
        data[17..21].copy_from_slice(&100u32.to_le_bytes());
        // Size
        data[21..25].copy_from_slice(&160u32.to_le_bytes());

        let item = Item::from_bytes(&data).unwrap();
        // Copy packed fields to avoid alignment issues
        let key_objectid = { item.key.objectid };
        let key_item_type = { item.key.item_type };
        assert_eq!(key_objectid, 256);
        assert_eq!(key_item_type, 0x01);
        assert_eq!(item.offset, 100);
        assert_eq!(item.size, 160);
    }

    #[test]
    fn test_item_from_bytes_too_small() {
        let data = vec![0u8; 15]; // Too small
        let result = Item::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_constants() {
        assert_eq!(NODE_HEADER_SIZE, 0x65);
        assert_eq!(KEY_PTR_SIZE, 0x21);
        assert_eq!(ITEM_SIZE, 0x19);
        assert_eq!(KEY_SIZE, 0x11);
    }
}
