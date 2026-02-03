//! Checksum utilities for BTRFS
//!
//! BTRFS uses CRC32c checksums for data integrity verification.
//! All hot-path functions are marked inline for performance.

use super::{BtrfsError, Result};

/// Checksum algorithms supported by BTRFS
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checksum {
    /// CRC32c (Castagnoli)
    Crc32c,
    /// XXHash64 (not yet implemented)
    XxHash64,
    /// SHA256 (not yet implemented)
    Sha256,
    /// Blake2b (not yet implemented)
    Blake2b,
}

impl Checksum {
    /// Returns the checksum type from a numeric value
    #[inline]
    pub fn from_type(csum_type: u16) -> Result<Self> {
        match csum_type {
            0 => Ok(Self::Crc32c),
            1 => Ok(Self::XxHash64),
            2 => Ok(Self::Sha256),
            3 => Ok(Self::Blake2b),
            _ => Err(BtrfsError::UnsupportedFeature(format!(
                "Unknown checksum type: {}",
                csum_type
            ))),
        }
    }

    /// Returns the size of the checksum in bytes
    #[inline]
    pub const fn size(&self) -> usize {
        match self {
            Self::Crc32c => 4,
            Self::XxHash64 => 8,
            Self::Sha256 => 32,
            Self::Blake2b => 32,
        }
    }
}

/// Computes a CRC32c checksum
/// 
/// This is a hot path - marked inline for performance
#[inline]
pub fn crc32c(data: &[u8]) -> u32 {
    crc32c::crc32c(data)
}

/// Computes a CRC32c checksum incrementally (for streaming)
#[inline]
pub fn crc32c_append(crc: u32, data: &[u8]) -> u32 {
    crc32c::crc32c_append(crc, data)
}

/// Verifies a CRC32c checksum
#[inline]
pub fn verify_crc32c(data: &[u8], expected: u32) -> Result<()> {
    let actual = crc32c(data);
    if actual != expected {
        return Err(BtrfsError::ChecksumMismatch { expected, actual });
    }
    Ok(())
}

/// Computes a checksum for a tree node
/// 
/// Node checksum covers everything after the checksum field (offset 0x20)
#[inline]
pub fn compute_node_checksum(data: &[u8]) -> u32 {
    if data.len() > 0x20 {
        crc32c(&data[0x20..])
    } else {
        0
    }
}

/// Verifies a tree node checksum
pub fn verify_node_checksum(data: &[u8]) -> Result<()> {
    if data.len() < 0x24 {
        return Err(BtrfsError::Corrupt(
            "Node too small for checksum".to_string(),
        ));
    }

    let expected = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let actual = compute_node_checksum(data);

    if expected != actual {
        return Err(BtrfsError::ChecksumMismatch { expected, actual });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32c_empty() {
        assert_eq!(crc32c(&[]), 0);
    }

    #[test]
    fn test_crc32c_hello() {
        // Known CRC32c value for "hello"
        let data = b"hello";
        let csum = crc32c(data);
        assert_ne!(csum, 0);
    }

    #[test]
    fn test_checksum_size() {
        assert_eq!(Checksum::Crc32c.size(), 4);
        assert_eq!(Checksum::XxHash64.size(), 8);
        assert_eq!(Checksum::Sha256.size(), 32);
        assert_eq!(Checksum::Blake2b.size(), 32);
    }

    #[test]
    fn test_checksum_from_type() {
        assert_eq!(Checksum::from_type(0).unwrap(), Checksum::Crc32c);
        assert_eq!(Checksum::from_type(1).unwrap(), Checksum::XxHash64);
        assert_eq!(Checksum::from_type(2).unwrap(), Checksum::Sha256);
        assert_eq!(Checksum::from_type(3).unwrap(), Checksum::Blake2b);
        assert!(Checksum::from_type(4).is_err());
        assert!(Checksum::from_type(255).is_err());
    }

    #[test]
    fn test_verify_crc32c_success() {
        let data = b"test data";
        let expected = crc32c(data);
        assert!(verify_crc32c(data, expected).is_ok());
    }

    #[test]
    fn test_verify_crc32c_failure() {
        let data = b"test data";
        let result = verify_crc32c(data, 0xDEADBEEF);
        assert!(result.is_err());
        match result {
            Err(BtrfsError::ChecksumMismatch { expected, actual }) => {
                assert_eq!(expected, 0xDEADBEEF);
                assert_ne!(actual, 0xDEADBEEF);
            }
            _ => panic!("Expected ChecksumMismatch error"),
        }
    }

    #[test]
    fn test_compute_node_checksum() {
        // Create a mock node with data after offset 0x20
        let mut data = vec![0u8; 100];
        let test_content = b"test data!!!"; // 12 bytes
        data[0x20..0x20 + test_content.len()].copy_from_slice(test_content);
        
        let csum = compute_node_checksum(&data);
        assert_ne!(csum, 0);
    }

    #[test]
    fn test_compute_node_checksum_small() {
        let data = vec![0u8; 10]; // Smaller than 0x20
        let csum = compute_node_checksum(&data);
        assert_eq!(csum, 0);
    }

    #[test]
    fn test_verify_node_checksum_too_small() {
        let data = vec![0u8; 30]; // Smaller than 0x24
        let result = verify_node_checksum(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_node_checksum_valid() {
        // Create a mock node
        let mut data = vec![0u8; 100];
        let test_content = b"test data!!!"; // 12 bytes
        data[0x20..0x20 + test_content.len()].copy_from_slice(test_content);
        
        // Compute and set the checksum
        let csum = compute_node_checksum(&data);
        data[0..4].copy_from_slice(&csum.to_le_bytes());
        
        assert!(verify_node_checksum(&data).is_ok());
    }

    #[test]
    fn test_verify_node_checksum_invalid() {
        // Create a mock node with wrong checksum
        let mut data = vec![0u8; 100];
        let test_content = b"test data!!!"; // 12 bytes
        data[0x20..0x20 + test_content.len()].copy_from_slice(test_content);
        data[0..4].copy_from_slice(&0xDEADBEEFu32.to_le_bytes());
        
        let result = verify_node_checksum(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_crc32c_deterministic() {
        let data = b"consistent input";
        let csum1 = crc32c(data);
        let csum2 = crc32c(data);
        assert_eq!(csum1, csum2);
    }

    #[test]
    fn test_crc32c_different_input() {
        let data1 = b"input one";
        let data2 = b"input two";
        let csum1 = crc32c(data1);
        let csum2 = crc32c(data2);
        assert_ne!(csum1, csum2);
    }

    #[test]
    fn test_checksum_debug() {
        let csum = Checksum::Crc32c;
        let debug_str = format!("{:?}", csum);
        assert!(debug_str.contains("Crc32c"));
    }

    #[test]
    fn test_checksum_clone() {
        let csum1 = Checksum::Sha256;
        let csum2 = csum1.clone();
        assert_eq!(csum1, csum2);
    }

    #[test]
    fn test_checksum_copy() {
        let csum1 = Checksum::Blake2b;
        let csum2 = csum1;
        assert_eq!(csum1, csum2);
    }
}
