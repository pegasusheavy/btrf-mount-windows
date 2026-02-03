//! File operations mapping for BTRFS
//!
//! This module provides helper functions for mapping filesystem operations
//! to BTRFS tree operations.

use crate::core::{
    inode::{DirEntry, ExtentData, Inode, InodeRef},
    item_type,
    tree::{BtrfsKey, BtrfsTree},
    BtrfsError, BtrfsFilesystem, Result,
};

/// Reads an inode from the filesystem
pub fn read_inode(fs: &BtrfsFilesystem, tree_id: u64, ino: u64) -> Result<Inode> {
    let root_addr = fs.superblock().root();
    let root_level = fs.superblock().root_level();

    // TODO: Get tree root from root tree
    let tree = BtrfsTree::new(fs, root_addr, root_level);

    let key = BtrfsKey::new(ino, item_type::INODE_ITEM, 0);

    match tree.search(&key)? {
        Some((_, data)) => Inode::from_bytes(ino, &data),
        None => Err(BtrfsError::InvalidInode(ino)),
    }
}

/// Reads directory entries
pub fn read_dir(fs: &BtrfsFilesystem, tree_id: u64, ino: u64) -> Result<Vec<DirEntry>> {
    let root_addr = fs.superblock().root();
    let root_level = fs.superblock().root_level();

    let tree = BtrfsTree::new(fs, root_addr, root_level);

    let min_key = BtrfsKey::new(ino, item_type::DIR_INDEX, 0);
    let max_key = BtrfsKey::new(ino, item_type::DIR_INDEX, u64::MAX);

    let items = tree.search_range(&min_key, &max_key)?;

    let mut entries = Vec::new();
    for (_, data) in items {
        if let Ok(entry) = DirEntry::from_bytes(&data) {
            entries.push(entry);
        }
    }

    Ok(entries)
}

/// Looks up a name in a directory
pub fn lookup(fs: &BtrfsFilesystem, tree_id: u64, dir_ino: u64, name: &str) -> Result<DirEntry> {
    // Hash the name for DIR_ITEM lookup
    let name_hash = btrfs_name_hash(name);

    let root_addr = fs.superblock().root();
    let root_level = fs.superblock().root_level();

    let tree = BtrfsTree::new(fs, root_addr, root_level);

    let key = BtrfsKey::new(dir_ino, item_type::DIR_ITEM, name_hash);

    match tree.search(&key)? {
        Some((_, data)) => {
            // DIR_ITEM may contain multiple entries with same hash
            // Parse and find the one with matching name
            let entry = DirEntry::from_bytes(&data)?;
            if entry.name == name {
                Ok(entry)
            } else {
                Err(BtrfsError::NotFound(name.to_string()))
            }
        }
        None => Err(BtrfsError::NotFound(name.to_string())),
    }
}

/// Reads file extent data
pub fn read_file_extents(fs: &BtrfsFilesystem, tree_id: u64, ino: u64) -> Result<Vec<ExtentData>> {
    let root_addr = fs.superblock().root();
    let root_level = fs.superblock().root_level();

    let tree = BtrfsTree::new(fs, root_addr, root_level);

    let min_key = BtrfsKey::new(ino, item_type::EXTENT_DATA, 0);
    let max_key = BtrfsKey::new(ino, item_type::EXTENT_DATA, u64::MAX);

    let items = tree.search_range(&min_key, &max_key)?;

    let mut extents = Vec::new();
    for (_, data) in items {
        if let Ok(extent) = ExtentData::from_bytes(&data) {
            extents.push(extent);
        }
    }

    Ok(extents)
}

/// Reads file data at an offset
pub fn read_file_data(
    fs: &BtrfsFilesystem,
    tree_id: u64,
    ino: u64,
    offset: u64,
    size: usize,
) -> Result<Vec<u8>> {
    let extents = read_file_extents(fs, tree_id, ino)?;

    let mut result = vec![0u8; size];
    let mut bytes_read = 0;

    for extent in extents {
        // TODO: Handle extent reading with decompression
        if extent.is_inline() {
            if let Some(data) = &extent.inline_data {
                let copy_size = std::cmp::min(data.len(), size - bytes_read);
                result[bytes_read..bytes_read + copy_size].copy_from_slice(&data[..copy_size]);
                bytes_read += copy_size;
            }
        }
    }

    result.truncate(bytes_read);
    Ok(result)
}

/// Gets inode references (hard links)
pub fn get_inode_refs(
    fs: &BtrfsFilesystem,
    tree_id: u64,
    ino: u64,
) -> Result<Vec<(u64, InodeRef)>> {
    let root_addr = fs.superblock().root();
    let root_level = fs.superblock().root_level();

    let tree = BtrfsTree::new(fs, root_addr, root_level);

    let min_key = BtrfsKey::new(ino, item_type::INODE_REF, 0);
    let max_key = BtrfsKey::new(ino, item_type::INODE_REF, u64::MAX);

    let items = tree.search_range(&min_key, &max_key)?;

    let mut refs = Vec::new();
    for (item, data) in items {
        if let Ok(iref) = InodeRef::from_bytes(&data) {
            refs.push((item.key.offset, iref)); // offset is parent dir ino
        }
    }

    Ok(refs)
}

/// BTRFS name hash function (CRC32c based)
pub fn btrfs_name_hash(name: &str) -> u64 {
    let crc = crc32c::crc32c(name.as_bytes());
    crc as u64
}

/// Resolves a path to an inode
pub fn resolve_path(fs: &BtrfsFilesystem, tree_id: u64, path: &str) -> Result<(u64, Inode)> {
    let components: Vec<&str> = path
        .split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .collect();

    // Start from root inode (256)
    let mut current_ino = 256u64;

    for component in components {
        // Look up component in current directory
        let entry = lookup(fs, tree_id, current_ino, component)?;
        current_ino = entry.ino;
    }

    let inode = read_inode(fs, tree_id, current_ino)?;
    Ok((current_ino, inode))
}

/// Parses path components from a path string
pub fn parse_path_components(path: &str) -> Vec<&str> {
    path.split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_btrfs_name_hash() {
        // Hash should be deterministic
        let hash1 = btrfs_name_hash("test.txt");
        let hash2 = btrfs_name_hash("test.txt");
        assert_eq!(hash1, hash2);

        // Different names should have different hashes (usually)
        let hash_a = btrfs_name_hash("file_a.txt");
        let hash_b = btrfs_name_hash("file_b.txt");
        assert_ne!(hash_a, hash_b);
    }

    #[test]
    fn test_btrfs_name_hash_empty() {
        let hash = btrfs_name_hash("");
        assert_eq!(hash, 0); // CRC32c of empty data is 0
    }

    #[test]
    fn test_btrfs_name_hash_special_chars() {
        let hash1 = btrfs_name_hash("file with spaces.txt");
        let hash2 = btrfs_name_hash("file-with-dashes.txt");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_btrfs_name_hash_unicode() {
        let hash = btrfs_name_hash("файл.txt"); // Russian word for "file"
        assert_ne!(hash, 0);
    }

    #[test]
    fn test_parse_path_components_unix() {
        let components = parse_path_components("/home/user/file.txt");
        assert_eq!(components, vec!["home", "user", "file.txt"]);
    }

    #[test]
    fn test_parse_path_components_windows() {
        let components = parse_path_components("C:\\Users\\user\\file.txt");
        assert_eq!(components, vec!["C:", "Users", "user", "file.txt"]);
    }

    #[test]
    fn test_parse_path_components_mixed() {
        let components = parse_path_components("/path/to\\mixed/separators");
        assert_eq!(components, vec!["path", "to", "mixed", "separators"]);
    }

    #[test]
    fn test_parse_path_components_empty() {
        let components = parse_path_components("");
        assert!(components.is_empty());
    }

    #[test]
    fn test_parse_path_components_root() {
        let components = parse_path_components("/");
        assert!(components.is_empty());
    }

    #[test]
    fn test_parse_path_components_trailing_slash() {
        let components = parse_path_components("/path/to/dir/");
        assert_eq!(components, vec!["path", "to", "dir"]);
    }

    #[test]
    fn test_parse_path_components_double_slash() {
        let components = parse_path_components("/path//to///dir");
        assert_eq!(components, vec!["path", "to", "dir"]);
    }

    #[test]
    fn test_parse_path_components_single_name() {
        let components = parse_path_components("filename.txt");
        assert_eq!(components, vec!["filename.txt"]);
    }
}
