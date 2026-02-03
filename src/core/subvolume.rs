//! BTRFS Subvolume and snapshot support
//!
//! Subvolumes are independent filesystem trees that can be mounted separately.

use super::{item_type, objectid, tree::BtrfsKey, BtrfsError, BtrfsFilesystem, Result};
use byteorder::{ByteOrder, LittleEndian};

/// A BTRFS subvolume
#[derive(Debug, Clone)]
pub struct Subvolume {
    /// Subvolume ID (object ID)
    pub id: u64,
    /// Parent subvolume ID (0 for top-level)
    pub parent_id: u64,
    /// Generation when created
    pub generation: u64,
    /// Generation of parent (for snapshots)
    pub parent_generation: u64,
    /// Flags
    pub flags: u64,
    /// UUID
    pub uuid: [u8; 16],
    /// Parent UUID (for snapshots)
    pub parent_uuid: [u8; 16],
    /// Received UUID (for send/receive)
    pub received_uuid: [u8; 16],
    /// Creation time
    pub otime: TimeSpec,
    /// Send time
    pub stime: TimeSpec,
    /// Receive time
    pub rtime: TimeSpec,
    /// Name
    pub name: String,
    /// Path relative to top-level
    pub path: String,
    /// Root tree logical address
    pub root_bytenr: u64,
    /// Root level
    pub root_level: u8,
}

/// Time specification
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeSpec {
    pub sec: i64,
    pub nsec: u32,
}

/// Subvolume flags
pub mod subvol_flags {
    /// Subvolume is read-only
    pub const RDONLY: u64 = 1 << 0;
}

/// Root item structure from the root tree
#[derive(Debug, Clone)]
pub struct RootItem {
    /// Inode item
    pub inode: RootInode,
    /// Generation
    pub generation: u64,
    /// Root directory ID
    pub root_dirid: u64,
    /// Byte number of root node
    pub bytenr: u64,
    /// Byte limit
    pub byte_limit: u64,
    /// Bytes used
    pub bytes_used: u64,
    /// Last snapshot generation
    pub last_snapshot: u64,
    /// Flags
    pub flags: u64,
    /// Number of references
    pub refs: u32,
    /// Drop progress key
    pub drop_progress: BtrfsKey,
    /// Drop level
    pub drop_level: u8,
    /// Root level
    pub level: u8,
    /// Generation v2
    pub generation_v2: u64,
    /// UUID
    pub uuid: [u8; 16],
    /// Parent UUID
    pub parent_uuid: [u8; 16],
    /// Received UUID
    pub received_uuid: [u8; 16],
    /// Transaction ID for creation
    pub ctransid: u64,
    /// Transaction ID for last modification
    pub otransid: u64,
    /// Transaction ID for send
    pub stransid: u64,
    /// Transaction ID for receive
    pub rtransid: u64,
    /// Creation time
    pub ctime: TimeSpec,
    /// Last modification time
    pub otime: TimeSpec,
    /// Send time
    pub stime: TimeSpec,
    /// Receive time
    pub rtime: TimeSpec,
}

/// Root inode embedded in root item
#[derive(Debug, Clone, Default)]
pub struct RootInode {
    pub generation: u64,
    pub transid: u64,
    pub size: u64,
    pub nbytes: u64,
    pub block_group: u64,
    pub nlink: u32,
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
    pub rdev: u64,
    pub flags: u64,
    pub sequence: u64,
}

impl RootItem {
    /// Parses a root item from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 439 {
            return Err(BtrfsError::Corrupt(format!(
                "Root item too small: {} bytes",
                data.len()
            )));
        }

        // Parse embedded inode
        let inode = RootInode {
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
        };

        let generation = LittleEndian::read_u64(&data[160..168]);
        let root_dirid = LittleEndian::read_u64(&data[168..176]);
        let bytenr = LittleEndian::read_u64(&data[176..184]);
        let byte_limit = LittleEndian::read_u64(&data[184..192]);
        let bytes_used = LittleEndian::read_u64(&data[192..200]);
        let last_snapshot = LittleEndian::read_u64(&data[200..208]);
        let flags = LittleEndian::read_u64(&data[208..216]);
        let refs = LittleEndian::read_u32(&data[216..220]);

        let drop_progress = BtrfsKey::from_bytes(&data[220..237])?;
        let drop_level = data[237];
        let level = data[238];

        // Generation v2 and UUIDs
        let generation_v2 = LittleEndian::read_u64(&data[239..247]);

        let mut uuid = [0u8; 16];
        uuid.copy_from_slice(&data[247..263]);

        let mut parent_uuid = [0u8; 16];
        parent_uuid.copy_from_slice(&data[263..279]);

        let mut received_uuid = [0u8; 16];
        received_uuid.copy_from_slice(&data[279..295]);

        let ctransid = LittleEndian::read_u64(&data[295..303]);
        let otransid = LittleEndian::read_u64(&data[303..311]);
        let stransid = LittleEndian::read_u64(&data[311..319]);
        let rtransid = LittleEndian::read_u64(&data[319..327]);

        let ctime = TimeSpec {
            sec: LittleEndian::read_i64(&data[327..335]),
            nsec: LittleEndian::read_u32(&data[335..339]),
        };
        let otime = TimeSpec {
            sec: LittleEndian::read_i64(&data[339..347]),
            nsec: LittleEndian::read_u32(&data[347..351]),
        };
        let stime = TimeSpec {
            sec: LittleEndian::read_i64(&data[351..359]),
            nsec: LittleEndian::read_u32(&data[359..363]),
        };
        let rtime = TimeSpec {
            sec: LittleEndian::read_i64(&data[363..371]),
            nsec: LittleEndian::read_u32(&data[371..375]),
        };

        Ok(Self {
            inode,
            generation,
            root_dirid,
            bytenr,
            byte_limit,
            bytes_used,
            last_snapshot,
            flags,
            refs,
            drop_progress,
            drop_level,
            level,
            generation_v2,
            uuid,
            parent_uuid,
            received_uuid,
            ctransid,
            otransid,
            stransid,
            rtransid,
            ctime,
            otime,
            stime,
            rtime,
        })
    }

    /// Returns true if this is a read-only subvolume
    pub fn is_readonly(&self) -> bool {
        self.flags & subvol_flags::RDONLY != 0
    }
}

/// Lists all subvolumes in the filesystem
pub fn list_subvolumes(fs: &BtrfsFilesystem) -> Result<Vec<Subvolume>> {
    let mut subvolumes = Vec::new();

    // The root tree contains ROOT_ITEMs for all subvolumes
    // For now, return the default subvolume
    let default_subvol = get_subvolume(fs, objectid::FS_TREE)?;
    subvolumes.push(default_subvol);

    // TODO: Iterate root tree for all ROOT_ITEMs with objectid >= FIRST_FREE

    Ok(subvolumes)
}

/// Gets a subvolume by ID
pub fn get_subvolume(fs: &BtrfsFilesystem, id: u64) -> Result<Subvolume> {
    // For the default FS tree, use superblock info
    if id == objectid::FS_TREE {
        return Ok(Subvolume {
            id: objectid::FS_TREE,
            parent_id: 0,
            generation: fs.superblock().generation(),
            parent_generation: 0,
            flags: 0,
            uuid: [0; 16],
            parent_uuid: [0; 16],
            received_uuid: [0; 16],
            otime: TimeSpec::default(),
            stime: TimeSpec::default(),
            rtime: TimeSpec::default(),
            name: String::from("(FS_TREE)"),
            path: String::from("/"),
            root_bytenr: fs.superblock().root(),
            root_level: fs.superblock().root_level(),
        });
    }

    // TODO: Look up ROOT_ITEM in root tree
    Err(BtrfsError::SubvolumeNotFound(id))
}

/// Creates a snapshot of a subvolume
pub fn create_snapshot(
    _fs: &BtrfsFilesystem,
    _source_id: u64,
    _name: &str,
    _readonly: bool,
) -> Result<Subvolume> {
    // TODO: Implement snapshot creation
    Err(BtrfsError::ReadOnly)
}

/// Deletes a subvolume
pub fn delete_subvolume(_fs: &BtrfsFilesystem, _id: u64) -> Result<()> {
    // TODO: Implement subvolume deletion
    Err(BtrfsError::ReadOnly)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subvol_flags() {
        assert_eq!(subvol_flags::RDONLY, 1);
    }

    #[test]
    fn test_timespec_default() {
        let ts = TimeSpec::default();
        assert_eq!(ts.sec, 0);
        assert_eq!(ts.nsec, 0);
    }

    #[test]
    fn test_root_inode_default() {
        let inode = RootInode::default();
        assert_eq!(inode.generation, 0);
        assert_eq!(inode.nlink, 0);
        assert_eq!(inode.mode, 0);
    }

    fn create_mock_root_item_data() -> Vec<u8> {
        let mut data = vec![0u8; 440];
        
        // Embedded inode (0-80)
        data[0..8].copy_from_slice(&100u64.to_le_bytes()); // generation
        data[8..16].copy_from_slice(&101u64.to_le_bytes()); // transid
        data[16..24].copy_from_slice(&0u64.to_le_bytes()); // size
        data[24..32].copy_from_slice(&0u64.to_le_bytes()); // nbytes
        data[32..40].copy_from_slice(&0u64.to_le_bytes()); // block_group
        data[40..44].copy_from_slice(&1u32.to_le_bytes()); // nlink
        data[44..48].copy_from_slice(&0u32.to_le_bytes()); // uid
        data[48..52].copy_from_slice(&0u32.to_le_bytes()); // gid
        data[52..56].copy_from_slice(&0o40755u32.to_le_bytes()); // mode
        data[56..64].copy_from_slice(&0u64.to_le_bytes()); // rdev
        data[64..72].copy_from_slice(&0u64.to_le_bytes()); // flags
        data[72..80].copy_from_slice(&1u64.to_le_bytes()); // sequence
        
        // Root item fields (160+)
        data[160..168].copy_from_slice(&200u64.to_le_bytes()); // generation
        data[168..176].copy_from_slice(&256u64.to_le_bytes()); // root_dirid
        data[176..184].copy_from_slice(&0x1000000u64.to_le_bytes()); // bytenr
        data[184..192].copy_from_slice(&0u64.to_le_bytes()); // byte_limit
        data[192..200].copy_from_slice(&4096u64.to_le_bytes()); // bytes_used
        data[200..208].copy_from_slice(&150u64.to_le_bytes()); // last_snapshot
        data[208..216].copy_from_slice(&0u64.to_le_bytes()); // flags
        data[216..220].copy_from_slice(&1u32.to_le_bytes()); // refs
        
        // drop_progress key (220-237)
        data[220..228].copy_from_slice(&0u64.to_le_bytes()); // objectid
        data[228] = 0; // type
        data[229..237].copy_from_slice(&0u64.to_le_bytes()); // offset
        
        data[237] = 0; // drop_level
        data[238] = 0; // level
        
        // generation_v2
        data[239..247].copy_from_slice(&200u64.to_le_bytes());
        
        // UUID (247-263)
        for i in 0..16 {
            data[247 + i] = i as u8;
        }
        
        // parent_uuid (263-279)
        // received_uuid (279-295)
        
        // Transaction IDs
        data[295..303].copy_from_slice(&100u64.to_le_bytes()); // ctransid
        data[303..311].copy_from_slice(&101u64.to_le_bytes()); // otransid
        data[311..319].copy_from_slice(&0u64.to_le_bytes()); // stransid
        data[319..327].copy_from_slice(&0u64.to_le_bytes()); // rtransid
        
        // Timestamps
        data[327..335].copy_from_slice(&1700000000i64.to_le_bytes()); // ctime.sec
        data[335..339].copy_from_slice(&123456u32.to_le_bytes()); // ctime.nsec
        data[339..347].copy_from_slice(&1700000001i64.to_le_bytes()); // otime.sec
        data[347..351].copy_from_slice(&234567u32.to_le_bytes()); // otime.nsec
        data[351..359].copy_from_slice(&0i64.to_le_bytes()); // stime.sec
        data[359..363].copy_from_slice(&0u32.to_le_bytes()); // stime.nsec
        data[363..371].copy_from_slice(&0i64.to_le_bytes()); // rtime.sec
        data[371..375].copy_from_slice(&0u32.to_le_bytes()); // rtime.nsec
        
        data
    }

    #[test]
    fn test_root_item_from_bytes() {
        let data = create_mock_root_item_data();
        let item = RootItem::from_bytes(&data).unwrap();

        assert_eq!(item.inode.generation, 100);
        assert_eq!(item.inode.nlink, 1);
        assert_eq!(item.generation, 200);
        assert_eq!(item.root_dirid, 256);
        assert_eq!(item.bytenr, 0x1000000);
        assert_eq!(item.bytes_used, 4096);
        assert_eq!(item.last_snapshot, 150);
        assert_eq!(item.flags, 0);
        assert_eq!(item.refs, 1);
        assert_eq!(item.level, 0);
        assert_eq!(item.ctransid, 100);
        assert_eq!(item.otransid, 101);
        assert_eq!(item.ctime.sec, 1700000000);
        assert_eq!(item.otime.sec, 1700000001);
        
        // Check UUID
        for i in 0..16 {
            assert_eq!(item.uuid[i], i as u8);
        }
    }

    #[test]
    fn test_root_item_from_bytes_too_small() {
        let data = vec![0u8; 300]; // Too small
        let result = RootItem::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_root_item_is_readonly() {
        let mut data = create_mock_root_item_data();
        
        // Not read-only
        let item = RootItem::from_bytes(&data).unwrap();
        assert!(!item.is_readonly());
        
        // Set read-only flag
        data[208..216].copy_from_slice(&subvol_flags::RDONLY.to_le_bytes());
        let item = RootItem::from_bytes(&data).unwrap();
        assert!(item.is_readonly());
    }

    #[test]
    fn test_subvolume_debug() {
        let subvol = Subvolume {
            id: 256,
            parent_id: 5,
            generation: 100,
            parent_generation: 50,
            flags: 0,
            uuid: [0; 16],
            parent_uuid: [0; 16],
            received_uuid: [0; 16],
            otime: TimeSpec::default(),
            stime: TimeSpec::default(),
            rtime: TimeSpec::default(),
            name: String::from("test_subvol"),
            path: String::from("/test_subvol"),
            root_bytenr: 0x1000000,
            root_level: 0,
        };
        
        let debug_str = format!("{:?}", subvol);
        assert!(debug_str.contains("test_subvol"));
        assert!(debug_str.contains("256"));
    }

    #[test]
    fn test_subvolume_clone() {
        let subvol = Subvolume {
            id: 256,
            parent_id: 5,
            generation: 100,
            parent_generation: 50,
            flags: 0,
            uuid: [1; 16],
            parent_uuid: [2; 16],
            received_uuid: [3; 16],
            otime: TimeSpec { sec: 1000, nsec: 500 },
            stime: TimeSpec::default(),
            rtime: TimeSpec::default(),
            name: String::from("test"),
            path: String::from("/test"),
            root_bytenr: 0x1000000,
            root_level: 1,
        };
        
        let cloned = subvol.clone();
        assert_eq!(cloned.id, subvol.id);
        assert_eq!(cloned.name, subvol.name);
        assert_eq!(cloned.uuid, subvol.uuid);
    }
}
