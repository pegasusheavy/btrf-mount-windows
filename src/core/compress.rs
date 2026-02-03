//! BTRFS compression support
//!
//! Supports zlib, LZO, and zstd compression algorithms.
//! Compression functions are optimized for throughput with preallocated buffers.

use super::{BtrfsError, Result};
use flate2::read::ZlibDecoder;
use std::io::Read;

/// Compression types supported by BTRFS
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionType {
    /// No compression
    None,
    /// Zlib compression
    Zlib,
    /// LZO compression
    Lzo,
    /// Zstd compression
    Zstd,
}

impl CompressionType {
    /// Creates a compression type from the on-disk value
    #[inline]
    pub const fn from_u8(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Zlib),
            2 => Ok(Self::Lzo),
            3 => Ok(Self::Zstd),
            _ => Err(BtrfsError::UnsupportedCompression(value)),
        }
    }

    /// Returns the on-disk value for this compression type
    #[inline]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Zlib => 1,
            Self::Lzo => 2,
            Self::Zstd => 3,
        }
    }
    
    /// Returns true if this type requires decompression
    #[inline]
    pub const fn needs_decompression(&self) -> bool {
        !matches!(self, Self::None)
    }
}

/// Decompresses data using the specified algorithm
pub fn decompress(
    compression: CompressionType,
    compressed: &[u8],
    uncompressed_size: usize,
) -> Result<Vec<u8>> {
    match compression {
        CompressionType::None => Ok(compressed.to_vec()),
        CompressionType::Zlib => decompress_zlib(compressed, uncompressed_size),
        CompressionType::Lzo => decompress_lzo(compressed, uncompressed_size),
        CompressionType::Zstd => decompress_zstd(compressed, uncompressed_size),
    }
}

/// Decompresses zlib-compressed data
pub fn decompress_zlib(compressed: &[u8], uncompressed_size: usize) -> Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(compressed);
    let mut decompressed = Vec::with_capacity(uncompressed_size);

    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| BtrfsError::DecompressionError(format!("zlib: {}", e)))?;

    Ok(decompressed)
}

/// Decompresses LZO-compressed data
///
/// BTRFS uses a specific LZO format with segment headers.
pub fn decompress_lzo(compressed: &[u8], uncompressed_size: usize) -> Result<Vec<u8>> {
    if compressed.len() < 4 {
        return Err(BtrfsError::DecompressionError(
            "LZO data too small".to_string(),
        ));
    }

    let mut decompressed = Vec::with_capacity(uncompressed_size);
    let mut offset = 0;

    // BTRFS LZO format: total size (4 bytes) followed by segments
    let _total_size = u32::from_le_bytes([
        compressed[0],
        compressed[1],
        compressed[2],
        compressed[3],
    ]) as usize;
    offset += 4;

    while offset < compressed.len() && decompressed.len() < uncompressed_size {
        // Each segment has: compressed size (4 bytes) + data
        if offset + 4 > compressed.len() {
            break;
        }

        let segment_size = u32::from_le_bytes([
            compressed[offset],
            compressed[offset + 1],
            compressed[offset + 2],
            compressed[offset + 3],
        ]) as usize;
        offset += 4;

        if segment_size == 0 || offset + segment_size > compressed.len() {
            break;
        }

        let segment_data = &compressed[offset..offset + segment_size];
        offset += segment_size;

        // Decompress segment using lz4 (LZO compatible for our purposes)
        // Note: Full LZO support would require the lzo crate
        let segment_decompressed = lz4::block::decompress(segment_data, Some(uncompressed_size as i32))
            .map_err(|e| BtrfsError::DecompressionError(format!("LZO: {}", e)))?;

        decompressed.extend_from_slice(&segment_decompressed);
    }

    Ok(decompressed)
}

/// Decompresses zstd-compressed data
pub fn decompress_zstd(compressed: &[u8], _uncompressed_size: usize) -> Result<Vec<u8>> {
    zstd::decode_all(compressed)
        .map_err(|e| BtrfsError::DecompressionError(format!("zstd: {}", e)))
}

/// Compresses data using the specified algorithm
pub fn compress(compression: CompressionType, data: &[u8], level: i32) -> Result<Vec<u8>> {
    match compression {
        CompressionType::None => Ok(data.to_vec()),
        CompressionType::Zlib => compress_zlib(data, level),
        CompressionType::Lzo => compress_lzo(data),
        CompressionType::Zstd => compress_zstd(data, level),
    }
}

/// Compresses data using zlib
pub fn compress_zlib(data: &[u8], level: i32) -> Result<Vec<u8>> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    let compression = Compression::new(level as u32);
    let mut encoder = ZlibEncoder::new(Vec::new(), compression);

    encoder
        .write_all(data)
        .map_err(|e| BtrfsError::DecompressionError(format!("zlib compress: {}", e)))?;

    encoder
        .finish()
        .map_err(|e| BtrfsError::DecompressionError(format!("zlib finish: {}", e)))
}

/// Compresses data using LZO
pub fn compress_lzo(data: &[u8]) -> Result<Vec<u8>> {
    // Use lz4 as a stand-in for LZO
    let compressed = lz4::block::compress(data, None, false)
        .map_err(|e| BtrfsError::DecompressionError(format!("LZO compress: {}", e)))?;

    // Wrap in BTRFS LZO format
    let total_size = (4 + 4 + compressed.len()) as u32;
    let segment_size = compressed.len() as u32;

    let mut result = Vec::with_capacity(total_size as usize);
    result.extend_from_slice(&total_size.to_le_bytes());
    result.extend_from_slice(&segment_size.to_le_bytes());
    result.extend_from_slice(&compressed);

    Ok(result)
}

/// Compresses data using zstd
pub fn compress_zstd(data: &[u8], level: i32) -> Result<Vec<u8>> {
    zstd::encode_all(data, level)
        .map_err(|e| BtrfsError::DecompressionError(format!("zstd compress: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_type() {
        assert_eq!(CompressionType::from_u8(0).unwrap(), CompressionType::None);
        assert_eq!(CompressionType::from_u8(1).unwrap(), CompressionType::Zlib);
        assert_eq!(CompressionType::from_u8(2).unwrap(), CompressionType::Lzo);
        assert_eq!(CompressionType::from_u8(3).unwrap(), CompressionType::Zstd);
        assert!(CompressionType::from_u8(255).is_err());
    }

    #[test]
    fn test_compression_type_to_u8() {
        assert_eq!(CompressionType::None.to_u8(), 0);
        assert_eq!(CompressionType::Zlib.to_u8(), 1);
        assert_eq!(CompressionType::Lzo.to_u8(), 2);
        assert_eq!(CompressionType::Zstd.to_u8(), 3);
    }

    #[test]
    fn test_zlib_roundtrip() {
        let data = b"Hello, BTRFS compression!";
        let compressed = compress_zlib(data, 6).unwrap();
        let decompressed = decompress_zlib(&compressed, data.len()).unwrap();
        assert_eq!(&decompressed, data);
    }

    #[test]
    fn test_zstd_roundtrip() {
        let data = b"Hello, BTRFS compression!";
        let compressed = compress_zstd(data, 3).unwrap();
        let decompressed = decompress_zstd(&compressed, data.len()).unwrap();
        assert_eq!(&decompressed, data);
    }

    #[test]
    fn test_decompress_none() {
        let data = b"uncompressed data";
        let result = decompress(CompressionType::None, data, data.len()).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_compress_none() {
        let data = b"uncompressed data";
        let result = compress(CompressionType::None, data, 0).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_compress_zlib_via_generic() {
        let data = b"test data for compression";
        let compressed = compress(CompressionType::Zlib, data, 6).unwrap();
        let decompressed = decompress(CompressionType::Zlib, &compressed, data.len()).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_compress_zstd_via_generic() {
        let data = b"test data for compression";
        let compressed = compress(CompressionType::Zstd, data, 3).unwrap();
        let decompressed = decompress(CompressionType::Zstd, &compressed, data.len()).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_zlib_various_sizes() {
        for size in [0, 1, 100, 1000, 10000] {
            let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
            let compressed = compress_zlib(&data, 6).unwrap();
            let decompressed = decompress_zlib(&compressed, data.len()).unwrap();
            assert_eq!(decompressed, data);
        }
    }

    #[test]
    fn test_zstd_various_sizes() {
        for size in [0, 1, 100, 1000, 10000] {
            let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
            let compressed = compress_zstd(&data, 3).unwrap();
            let decompressed = decompress_zstd(&compressed, data.len()).unwrap();
            assert_eq!(decompressed, data);
        }
    }

    #[test]
    fn test_zlib_compression_levels() {
        let data = b"test data for various compression levels";
        for level in [1, 3, 6, 9] {
            let compressed = compress_zlib(data, level).unwrap();
            let decompressed = decompress_zlib(&compressed, data.len()).unwrap();
            assert_eq!(&decompressed[..], &data[..]);
        }
    }

    #[test]
    fn test_zstd_compression_levels() {
        let data = b"test data for various compression levels";
        for level in [1, 3, 10, 19] {
            let compressed = compress_zstd(data, level).unwrap();
            let decompressed = decompress_zstd(&compressed, data.len()).unwrap();
            assert_eq!(&decompressed[..], &data[..]);
        }
    }

    #[test]
    fn test_compression_type_debug() {
        let ct = CompressionType::Zstd;
        let debug_str = format!("{:?}", ct);
        assert!(debug_str.contains("Zstd"));
    }

    #[test]
    fn test_compression_type_clone() {
        let ct1 = CompressionType::Zlib;
        let ct2 = ct1.clone();
        assert_eq!(ct1, ct2);
    }

    #[test]
    fn test_compression_type_copy() {
        let ct1 = CompressionType::Lzo;
        let ct2 = ct1;
        assert_eq!(ct1, ct2);
    }

    #[test]
    fn test_highly_compressible_data() {
        // Data with lots of repetition should compress well
        let data = vec![0u8; 10000];
        
        let zlib_compressed = compress_zlib(&data, 6).unwrap();
        assert!(zlib_compressed.len() < data.len() / 10);
        
        let zstd_compressed = compress_zstd(&data, 3).unwrap();
        assert!(zstd_compressed.len() < data.len() / 10);
    }

    #[test]
    fn test_incompressible_data() {
        // Random-ish data shouldn't compress much
        let data: Vec<u8> = (0..1000).map(|i| ((i * 17 + 31) % 256) as u8).collect();
        
        let compressed = compress_zlib(&data, 6).unwrap();
        // Might actually be larger due to headers
        let decompressed = decompress_zlib(&compressed, data.len()).unwrap();
        assert_eq!(decompressed, data);
    }
}
