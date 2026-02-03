//! BTRFS Inode and directory operations
//!
//! This module handles file and directory metadata.
//! Parsing functions are optimized with inline hints for hot paths.

use super::{item_type, tree::BtrfsKey, BtrfsError, BtrfsFilesystem, Result};
use byteorder::{ByteOrder, LittleEndian};

/// Inode types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InodeType {
    Unknown,
    File,
    Directory,
    Symlink,
    BlockDevice,
    CharDevice,
    Fifo,
    Socket,
}

impl InodeType {
    /// Creates an inode type from a directory entry type
    #[inline]
    pub const fn from_dir_type(t: u8) -> Self {
        match t {
            1 => Self::File,
            2 => Self::Directory,
            3 => Self::CharDevice,
            4 => Self::BlockDevice,
            5 => Self::Fifo,
            6 => Self::Socket,
            7 => Self::Symlink,
            _ => Self::Unknown,
        }
    }

    /// Creates an inode type from mode bits - hot path
    #[inline]
    pub const fn from_mode(mode: u32) -> Self {
        match mode & 0o170000 {
            0o100000 => Self::File,
            0o040000 => Self::Directory,
            0o120000 => Self::Symlink,
            0o060000 => Self::BlockDevice,
            0o020000 => Self::CharDevice,
            0o010000 => Self::Fifo,
            0o140000 => Self::Socket,
            _ => Self::Unknown,
        }
    }
    
    /// Returns true if this is a file type
    #[inline]
    pub const fn is_file(&self) -> bool {
        matches!(self, Self::File)
    }
    
    /// Returns true if this is a directory type
    #[inline]
    pub const fn is_dir(&self) -> bool {
        matches!(self, Self::Directory)
    }
}

/// BTRFS inode item
#[derive(Debug, Clone)]
pub struct Inode {
    /// Inode number (object ID)
    pub ino: u64,
    /// Generation
    pub generation: u64,
    /// Transaction ID
    pub transid: u64,
    /// Size in bytes
    pub size: u64,
    /// Size in blocks
    pub nbytes: u64,
    /// Block group
    pub block_group: u64,
    /// Number of hard links
    pub nlink: u32,
    /// User ID
    pub uid: u32,
    /// Group ID
    pub gid: u32,
    /// Mode (permissions + type)
    pub mode: u32,
    /// Device ID (for device files)
    pub rdev: u64,
    /// Flags
    pub flags: u64,
    /// Sequence number
    pub sequence: u64,
    /// Access time
    pub atime: TimeSpec,
    /// Change time
    pub ctime: TimeSpec,
    /// Modification time
    pub mtime: TimeSpec,
    /// Creation time
    pub otime: TimeSpec,
}

/// Time specification
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeSpec {
    /// Seconds since epoch
    pub sec: i64,
    /// Nanoseconds
    pub nsec: u32,
}

impl Inode {
    /// Parses an inode item from bytes
    pub fn from_bytes(ino: u64, data: &[u8]) -> Result<Self> {
        if data.len() < 160 {
            return Err(BtrfsError::Corrupt("Inode item too small".to_string()));
        }

        Ok(Self {
            ino,
            generation: LittleEndian::read_u64(&data[0..8]),
            transid: LittleEndian::read_u64(&data[8..16]),
            size: LittleEndian::read_u64(&data[16..24]),
            nbytes: LittleEndian::read_u64(&data[24..32]),
            block_group: LittleEndian::read_u64(&data[32..40]),
            nlink: LittleEndian::read_u32(&data[40..44]),
            uid: LittleEndian::read_u32(&data[44..48]),
            gid: LittleEndian::read_u32(&data[48..52]),
            mode: LittleEndian::read_u32(&data[52..56]),
            rdev: LittleEndian::read_u64(&data[56..64]),
            flags: LittleEndian::read_u64(&data[64..72]),
            sequence: LittleEndian::read_u64(&data[72..80]),
            atime: TimeSpec {
                sec: LittleEndian::read_i64(&data[120..128]),
                nsec: LittleEndian::read_u32(&data[128..132]),
            },
            ctime: TimeSpec {
                sec: LittleEndian::read_i64(&data[132..140]),
                nsec: LittleEndian::read_u32(&data[140..144]),
            },
            mtime: TimeSpec {
                sec: LittleEndian::read_i64(&data[144..152]),
                nsec: LittleEndian::read_u32(&data[152..156]),
            },
            otime: TimeSpec {
                sec: LittleEndian::read_i64(&data[156..164]),
                nsec: LittleEndian::read_u32(&data[164..168]),
            },
        })
    }

    /// Returns the inode type
    #[inline]
    pub fn inode_type(&self) -> InodeType {
        InodeType::from_mode(self.mode)
    }

    /// Returns true if this is a directory
    #[inline]
    pub fn is_dir(&self) -> bool {
        (self.mode & 0o170000) == 0o040000
    }

    /// Returns true if this is a regular file
    #[inline]
    pub fn is_file(&self) -> bool {
        (self.mode & 0o170000) == 0o100000
    }

    /// Returns true if this is a symlink
    #[inline]
    pub fn is_symlink(&self) -> bool {
        (self.mode & 0o170000) == 0o120000
    }
    
    /// Returns the file permissions (lower 12 bits of mode)
    #[inline]
    pub const fn permissions(&self) -> u32 {
        self.mode & 0o7777
    }
}

/// Directory entry
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Child inode number
    pub ino: u64,
    /// Child tree ID (for subvolumes)
    pub child_tree: u64,
    /// Entry type
    pub entry_type: InodeType,
    /// Entry name
    pub name: String,
}

impl DirEntry {
    /// Parses a directory item from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 30 {
            return Err(BtrfsError::Corrupt("Dir item too small".to_string()));
        }

        // Parse location key
        let ino = LittleEndian::read_u64(&data[0..8]);
        let child_tree = LittleEndian::read_u64(&data[9..17]);

        let _transid = LittleEndian::read_u64(&data[17..25]);
        let _data_len = LittleEndian::read_u16(&data[25..27]);
        let name_len = LittleEndian::read_u16(&data[27..29]);
        let entry_type = InodeType::from_dir_type(data[29]);

        if data.len() < 30 + name_len as usize {
            return Err(BtrfsError::Corrupt("Dir item name truncated".to_string()));
        }

        let name = String::from_utf8_lossy(&data[30..30 + name_len as usize]).to_string();

        Ok(Self {
            ino,
            child_tree,
            entry_type,
            name,
        })
    }
}

/// Inode reference (hard link)
#[derive(Debug, Clone)]
pub struct InodeRef {
    /// Index in parent directory
    pub index: u64,
    /// Name length
    pub name_len: u16,
    /// Name
    pub name: String,
}

impl InodeRef {
    /// Parses an inode reference from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 10 {
            return Err(BtrfsError::Corrupt("Inode ref too small".to_string()));
        }

        let index = LittleEndian::read_u64(&data[0..8]);
        let name_len = LittleEndian::read_u16(&data[8..10]);

        if data.len() < 10 + name_len as usize {
            return Err(BtrfsError::Corrupt("Inode ref name truncated".to_string()));
        }

        let name = String::from_utf8_lossy(&data[10..10 + name_len as usize]).to_string();

        Ok(Self {
            index,
            name_len,
            name,
        })
    }
}

/// File extent data
#[derive(Debug, Clone)]
pub struct ExtentData {
    /// Generation
    pub generation: u64,
    /// Decoded (uncompressed) size
    pub ram_bytes: u64,
    /// Compression type
    pub compression: u8,
    /// Encryption type
    pub encryption: u8,
    /// Other encoding
    pub other_encoding: u16,
    /// Extent type (0=inline, 1=regular, 2=prealloc)
    pub extent_type: u8,
    /// Inline data (for inline extents)
    pub inline_data: Option<Vec<u8>>,
    /// Disk byte (logical address) for regular extents
    pub disk_bytenr: Option<u64>,
    /// Size on disk for regular extents
    pub disk_num_bytes: Option<u64>,
    /// Offset within extent
    pub offset: Option<u64>,
    /// Number of bytes in file
    pub num_bytes: Option<u64>,
}

impl ExtentData {
    /// Parses extent data from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 21 {
            return Err(BtrfsError::Corrupt("Extent data too small".to_string()));
        }

        let generation = LittleEndian::read_u64(&data[0..8]);
        let ram_bytes = LittleEndian::read_u64(&data[8..16]);
        let compression = data[16];
        let encryption = data[17];
        let other_encoding = LittleEndian::read_u16(&data[18..20]);
        let extent_type = data[20];

        if extent_type == 0 {
            // Inline extent
            let inline_data = data[21..].to_vec();
            Ok(Self {
                generation,
                ram_bytes,
                compression,
                encryption,
                other_encoding,
                extent_type,
                inline_data: Some(inline_data),
                disk_bytenr: None,
                disk_num_bytes: None,
                offset: None,
                num_bytes: None,
            })
        } else {
            // Regular or prealloc extent
            if data.len() < 53 {
                return Err(BtrfsError::Corrupt(
                    "Regular extent data too small".to_string(),
                ));
            }

            Ok(Self {
                generation,
                ram_bytes,
                compression,
                encryption,
                other_encoding,
                extent_type,
                inline_data: None,
                disk_bytenr: Some(LittleEndian::read_u64(&data[21..29])),
                disk_num_bytes: Some(LittleEndian::read_u64(&data[29..37])),
                offset: Some(LittleEndian::read_u64(&data[37..45])),
                num_bytes: Some(LittleEndian::read_u64(&data[45..53])),
            })
        }
    }

    /// Returns true if this is an inline extent
    #[inline]
    pub const fn is_inline(&self) -> bool {
        self.extent_type == 0
    }

    /// Returns true if this is a sparse (hole) extent
    #[inline]
    pub fn is_sparse(&self) -> bool {
        self.extent_type == 1 && self.disk_bytenr == Some(0)
    }
    
    /// Returns true if this is a regular (non-inline, non-prealloc) extent
    #[inline]
    pub const fn is_regular(&self) -> bool {
        self.extent_type == 1
    }
    
    /// Returns true if this is a preallocated extent
    #[inline]
    pub const fn is_prealloc(&self) -> bool {
        self.extent_type == 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inode_type_from_dir_type() {
        assert_eq!(InodeType::from_dir_type(1), InodeType::File);
        assert_eq!(InodeType::from_dir_type(2), InodeType::Directory);
        assert_eq!(InodeType::from_dir_type(3), InodeType::CharDevice);
        assert_eq!(InodeType::from_dir_type(4), InodeType::BlockDevice);
        assert_eq!(InodeType::from_dir_type(5), InodeType::Fifo);
        assert_eq!(InodeType::from_dir_type(6), InodeType::Socket);
        assert_eq!(InodeType::from_dir_type(7), InodeType::Symlink);
        assert_eq!(InodeType::from_dir_type(0), InodeType::Unknown);
        assert_eq!(InodeType::from_dir_type(255), InodeType::Unknown);
    }

    #[test]
    fn test_inode_type_from_mode() {
        assert_eq!(InodeType::from_mode(0o100644), InodeType::File);
        assert_eq!(InodeType::from_mode(0o040755), InodeType::Directory);
        assert_eq!(InodeType::from_mode(0o120777), InodeType::Symlink);
        assert_eq!(InodeType::from_mode(0o060660), InodeType::BlockDevice);
        assert_eq!(InodeType::from_mode(0o020660), InodeType::CharDevice);
        assert_eq!(InodeType::from_mode(0o010644), InodeType::Fifo);
        assert_eq!(InodeType::from_mode(0o140755), InodeType::Socket);
        assert_eq!(InodeType::from_mode(0o000000), InodeType::Unknown);
    }

    fn create_mock_inode_data() -> Vec<u8> {
        let mut data = vec![0u8; 168];
        // generation
        data[0..8].copy_from_slice(&100u64.to_le_bytes());
        // transid
        data[8..16].copy_from_slice(&101u64.to_le_bytes());
        // size
        data[16..24].copy_from_slice(&4096u64.to_le_bytes());
        // nbytes
        data[24..32].copy_from_slice(&4096u64.to_le_bytes());
        // block_group
        data[32..40].copy_from_slice(&0u64.to_le_bytes());
        // nlink
        data[40..44].copy_from_slice(&1u32.to_le_bytes());
        // uid
        data[44..48].copy_from_slice(&1000u32.to_le_bytes());
        // gid
        data[48..52].copy_from_slice(&1000u32.to_le_bytes());
        // mode (regular file with 0644)
        data[52..56].copy_from_slice(&0o100644u32.to_le_bytes());
        // rdev
        data[56..64].copy_from_slice(&0u64.to_le_bytes());
        // flags
        data[64..72].copy_from_slice(&0u64.to_le_bytes());
        // sequence
        data[72..80].copy_from_slice(&1u64.to_le_bytes());
        // reserved (80-120)
        // atime
        data[120..128].copy_from_slice(&1700000000i64.to_le_bytes());
        data[128..132].copy_from_slice(&123456u32.to_le_bytes());
        // ctime
        data[132..140].copy_from_slice(&1700000001i64.to_le_bytes());
        data[140..144].copy_from_slice(&234567u32.to_le_bytes());
        // mtime
        data[144..152].copy_from_slice(&1700000002i64.to_le_bytes());
        data[152..156].copy_from_slice(&345678u32.to_le_bytes());
        // otime
        data[156..164].copy_from_slice(&1700000003i64.to_le_bytes());
        data[164..168].copy_from_slice(&456789u32.to_le_bytes());
        data
    }

    #[test]
    fn test_inode_from_bytes() {
        let data = create_mock_inode_data();
        let inode = Inode::from_bytes(256, &data).unwrap();

        assert_eq!(inode.ino, 256);
        assert_eq!(inode.generation, 100);
        assert_eq!(inode.transid, 101);
        assert_eq!(inode.size, 4096);
        assert_eq!(inode.nbytes, 4096);
        assert_eq!(inode.nlink, 1);
        assert_eq!(inode.uid, 1000);
        assert_eq!(inode.gid, 1000);
        assert_eq!(inode.mode, 0o100644);
        assert_eq!(inode.atime.sec, 1700000000);
        assert_eq!(inode.atime.nsec, 123456);
    }

    #[test]
    fn test_inode_from_bytes_too_small() {
        let data = vec![0u8; 100]; // Too small
        let result = Inode::from_bytes(256, &data);
        assert!(result.is_err());
    }

    #[test]
    fn test_inode_type_methods() {
        let mut data = create_mock_inode_data();

        // Test file
        data[52..56].copy_from_slice(&0o100644u32.to_le_bytes());
        let inode = Inode::from_bytes(256, &data).unwrap();
        assert!(inode.is_file());
        assert!(!inode.is_dir());
        assert!(!inode.is_symlink());
        assert_eq!(inode.inode_type(), InodeType::File);

        // Test directory
        data[52..56].copy_from_slice(&0o040755u32.to_le_bytes());
        let inode = Inode::from_bytes(256, &data).unwrap();
        assert!(!inode.is_file());
        assert!(inode.is_dir());
        assert!(!inode.is_symlink());
        assert_eq!(inode.inode_type(), InodeType::Directory);

        // Test symlink
        data[52..56].copy_from_slice(&0o120777u32.to_le_bytes());
        let inode = Inode::from_bytes(256, &data).unwrap();
        assert!(!inode.is_file());
        assert!(!inode.is_dir());
        assert!(inode.is_symlink());
        assert_eq!(inode.inode_type(), InodeType::Symlink);
    }

    fn create_mock_dir_entry_data(name: &str) -> Vec<u8> {
        let name_bytes = name.as_bytes();
        let mut data = vec![0u8; 30 + name_bytes.len()];
        // ino (location key objectid)
        data[0..8].copy_from_slice(&257u64.to_le_bytes());
        // type in location key
        data[8] = 0x01;
        // child_tree (location key offset)
        data[9..17].copy_from_slice(&5u64.to_le_bytes());
        // transid
        data[17..25].copy_from_slice(&100u64.to_le_bytes());
        // data_len
        data[25..27].copy_from_slice(&0u16.to_le_bytes());
        // name_len
        data[27..29].copy_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        // type (1 = file)
        data[29] = 1;
        // name
        data[30..30 + name_bytes.len()].copy_from_slice(name_bytes);
        data
    }

    #[test]
    fn test_dir_entry_from_bytes() {
        let data = create_mock_dir_entry_data("test.txt");
        let entry = DirEntry::from_bytes(&data).unwrap();

        assert_eq!(entry.ino, 257);
        assert_eq!(entry.child_tree, 5);
        assert_eq!(entry.entry_type, InodeType::File);
        assert_eq!(entry.name, "test.txt");
    }

    #[test]
    fn test_dir_entry_from_bytes_too_small() {
        let data = vec![0u8; 20]; // Too small
        let result = DirEntry::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_dir_entry_name_truncated() {
        let mut data = vec![0u8; 30];
        data[27..29].copy_from_slice(&100u16.to_le_bytes()); // name_len = 100, but no name data
        let result = DirEntry::from_bytes(&data);
        assert!(result.is_err());
    }

    fn create_mock_inode_ref_data(name: &str) -> Vec<u8> {
        let name_bytes = name.as_bytes();
        let mut data = vec![0u8; 10 + name_bytes.len()];
        // index
        data[0..8].copy_from_slice(&42u64.to_le_bytes());
        // name_len
        data[8..10].copy_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        // name
        data[10..10 + name_bytes.len()].copy_from_slice(name_bytes);
        data
    }

    #[test]
    fn test_inode_ref_from_bytes() {
        let data = create_mock_inode_ref_data("myfile.txt");
        let iref = InodeRef::from_bytes(&data).unwrap();

        assert_eq!(iref.index, 42);
        assert_eq!(iref.name_len, 10);
        assert_eq!(iref.name, "myfile.txt");
    }

    #[test]
    fn test_inode_ref_from_bytes_too_small() {
        let data = vec![0u8; 5]; // Too small
        let result = InodeRef::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_inode_ref_name_truncated() {
        let mut data = vec![0u8; 10];
        data[8..10].copy_from_slice(&100u16.to_le_bytes()); // name_len = 100, but no name data
        let result = InodeRef::from_bytes(&data);
        assert!(result.is_err());
    }

    fn create_mock_inline_extent_data(content: &[u8]) -> Vec<u8> {
        let mut data = vec![0u8; 21 + content.len()];
        // generation
        data[0..8].copy_from_slice(&100u64.to_le_bytes());
        // ram_bytes
        data[8..16].copy_from_slice(&(content.len() as u64).to_le_bytes());
        // compression
        data[16] = 0;
        // encryption
        data[17] = 0;
        // other_encoding
        data[18..20].copy_from_slice(&0u16.to_le_bytes());
        // extent_type (0 = inline)
        data[20] = 0;
        // inline data
        data[21..21 + content.len()].copy_from_slice(content);
        data
    }

    fn create_mock_regular_extent_data() -> Vec<u8> {
        let mut data = vec![0u8; 53];
        // generation
        data[0..8].copy_from_slice(&100u64.to_le_bytes());
        // ram_bytes
        data[8..16].copy_from_slice(&4096u64.to_le_bytes());
        // compression
        data[16] = 0;
        // encryption
        data[17] = 0;
        // other_encoding
        data[18..20].copy_from_slice(&0u16.to_le_bytes());
        // extent_type (1 = regular)
        data[20] = 1;
        // disk_bytenr
        data[21..29].copy_from_slice(&0x100000u64.to_le_bytes());
        // disk_num_bytes
        data[29..37].copy_from_slice(&4096u64.to_le_bytes());
        // offset
        data[37..45].copy_from_slice(&0u64.to_le_bytes());
        // num_bytes
        data[45..53].copy_from_slice(&4096u64.to_le_bytes());
        data
    }

    #[test]
    fn test_extent_data_inline() {
        let content = b"Hello, BTRFS!";
        let data = create_mock_inline_extent_data(content);
        let extent = ExtentData::from_bytes(&data).unwrap();

        assert_eq!(extent.generation, 100);
        assert_eq!(extent.ram_bytes, content.len() as u64);
        assert_eq!(extent.compression, 0);
        assert_eq!(extent.extent_type, 0);
        assert!(extent.is_inline());
        assert!(!extent.is_sparse());
        assert_eq!(extent.inline_data, Some(content.to_vec()));
        assert!(extent.disk_bytenr.is_none());
    }

    #[test]
    fn test_extent_data_regular() {
        let data = create_mock_regular_extent_data();
        let extent = ExtentData::from_bytes(&data).unwrap();

        assert_eq!(extent.generation, 100);
        assert_eq!(extent.ram_bytes, 4096);
        assert_eq!(extent.extent_type, 1);
        assert!(!extent.is_inline());
        assert!(!extent.is_sparse());
        assert!(extent.inline_data.is_none());
        assert_eq!(extent.disk_bytenr, Some(0x100000));
        assert_eq!(extent.disk_num_bytes, Some(4096));
        assert_eq!(extent.offset, Some(0));
        assert_eq!(extent.num_bytes, Some(4096));
    }

    #[test]
    fn test_extent_data_sparse() {
        let mut data = create_mock_regular_extent_data();
        // Set disk_bytenr to 0 for sparse
        data[21..29].copy_from_slice(&0u64.to_le_bytes());
        let extent = ExtentData::from_bytes(&data).unwrap();

        assert!(extent.is_sparse());
    }

    #[test]
    fn test_extent_data_too_small() {
        let data = vec![0u8; 15]; // Too small
        let result = ExtentData::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_extent_data_regular_too_small() {
        let mut data = vec![0u8; 30];
        data[20] = 1; // Regular extent type
        let result = ExtentData::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_timespec_default() {
        let ts = TimeSpec::default();
        assert_eq!(ts.sec, 0);
        assert_eq!(ts.nsec, 0);
    }
}
