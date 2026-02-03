//! BTRFS comprehensive benchmarks using Criterion
//!
//! Run with: cargo bench
//! Run specific: cargo bench -- checksum
//! Generate flamegraph: cargo bench -- --profile-time=5

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};

// ============================================================================
// Superblock Benchmarks
// ============================================================================

/// Benchmark superblock parsing operations
fn superblock_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("superblock");

    // Create a mock superblock with valid magic and structure
    let mut superblock_data = vec![0u8; 4096];
    // Set magic at offset 0x40
    superblock_data[0x40..0x48].copy_from_slice(b"_BHRfS_M");
    // Set csum_type (CRC32c = 0)
    superblock_data[0x60..0x62].copy_from_slice(&0u16.to_le_bytes());
    // Set generation
    superblock_data[0x48..0x50].copy_from_slice(&100u64.to_le_bytes());
    // Set node_size
    superblock_data[0x94..0x98].copy_from_slice(&16384u32.to_le_bytes());

    group.throughput(Throughput::Bytes(superblock_data.len() as u64));

    group.bench_function("magic_check", |b| {
        b.iter(|| {
            let magic = &superblock_data[0x40..0x48];
            black_box(magic == b"_BHRfS_M")
        })
    });

    group.bench_function("parse_generation", |b| {
        use byteorder::{ByteOrder, LittleEndian};
        b.iter(|| {
            black_box(LittleEndian::read_u64(&superblock_data[0x48..0x50]))
        })
    });

    group.bench_function("parse_multiple_fields", |b| {
        use byteorder::{ByteOrder, LittleEndian};
        b.iter(|| {
            let generation = LittleEndian::read_u64(&superblock_data[0x48..0x50]);
            let node_size = LittleEndian::read_u32(&superblock_data[0x94..0x98]);
            let csum_type = LittleEndian::read_u16(&superblock_data[0x60..0x62]);
            black_box((generation, node_size, csum_type))
        })
    });

    group.finish();
}

// ============================================================================
// Checksum Benchmarks
// ============================================================================

/// Benchmark CRC32c checksum calculation at various sizes
fn checksum_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("checksum");

    // Test various block sizes relevant to BTRFS
    for size in [512, 4096, 16384, 65536, 131072, 1048576].iter() {
        let data = vec![0x42u8; *size];

        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("crc32c", size), &data, |b, data| {
            b.iter(|| crc32c::crc32c(black_box(data)))
        });
    }

    // Benchmark incremental checksum (for streaming)
    group.bench_function("crc32c_incremental_4k_chunks", |b| {
        let data = vec![0x42u8; 65536];
        b.iter(|| {
            let mut csum = 0u32;
            for chunk in data.chunks(4096) {
                csum = crc32c::crc32c_append(csum, black_box(chunk));
            }
            csum
        })
    });

    group.finish();
}

// ============================================================================
// Compression Benchmarks
// ============================================================================

/// Benchmark compression/decompression at various levels and sizes
fn compression_benchmarks(c: &mut Criterion) {
    // Highly compressible data (zeros)
    let zeros_64k = vec![0u8; 65536];
    
    // Moderately compressible data (repeating pattern)
    let pattern_64k: Vec<u8> = (0..65536)
        .map(|i| ((i * 7 + 13) % 256) as u8)
        .collect();
    
    // Incompressible data (pseudo-random)
    let random_64k: Vec<u8> = (0u64..65536)
        .map(|i| {
            let x = i.wrapping_mul(1103515245).wrapping_add(12345);
            (x >> 16) as u8
        })
        .collect();

    // Zlib benchmarks
    {
        let mut group = c.benchmark_group("zlib");
        group.throughput(Throughput::Bytes(65536));

        for (name, data) in [
            ("zeros", &zeros_64k),
            ("pattern", &pattern_64k),
            ("random", &random_64k),
        ] {
            for level in [1, 6, 9] {
                group.bench_function(format!("compress_{}_L{}", name, level), |b| {
                    use flate2::write::ZlibEncoder;
                    use flate2::Compression;
                    use std::io::Write;

                    b.iter(|| {
                        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(level as u32));
                        encoder.write_all(black_box(data)).unwrap();
                        encoder.finish().unwrap()
                    })
                });
            }

            // Decompress benchmark
            let compressed = {
                use flate2::write::ZlibEncoder;
                use flate2::Compression;
                use std::io::Write;
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(6));
                encoder.write_all(data).unwrap();
                encoder.finish().unwrap()
            };

            group.bench_function(format!("decompress_{}", name), |b| {
                use flate2::read::ZlibDecoder;
                use std::io::Read;

                b.iter(|| {
                    let mut decoder = ZlibDecoder::new(black_box(&compressed[..]));
                    let mut output = Vec::with_capacity(data.len());
                    decoder.read_to_end(&mut output).unwrap();
                    output
                })
            });
        }
        group.finish();
    }

    // Zstd benchmarks
    {
        let mut group = c.benchmark_group("zstd");
        group.throughput(Throughput::Bytes(65536));

        for (name, data) in [
            ("zeros", &zeros_64k),
            ("pattern", &pattern_64k),
            ("random", &random_64k),
        ] {
            for level in [1, 3, 10, 19] {
                group.bench_function(format!("compress_{}_L{}", name, level), |b| {
                    b.iter(|| zstd::encode_all(black_box(&data[..]), level).unwrap())
                });
            }

            let compressed = zstd::encode_all(&data[..], 3).unwrap();

            group.bench_function(format!("decompress_{}", name), |b| {
                b.iter(|| zstd::decode_all(black_box(&compressed[..])).unwrap())
            });
        }
        group.finish();
    }

    // LZ4 benchmarks
    {
        let mut group = c.benchmark_group("lz4");
        group.throughput(Throughput::Bytes(65536));

        for (name, data) in [
            ("zeros", &zeros_64k),
            ("pattern", &pattern_64k),
            ("random", &random_64k),
        ] {
            group.bench_function(format!("compress_{}", name), |b| {
                b.iter(|| lz4::block::compress(black_box(data), None, false).unwrap())
            });

            let compressed = lz4::block::compress(data, None, false).unwrap();

            group.bench_function(format!("decompress_{}", name), |b| {
                b.iter(|| {
                    lz4::block::decompress(black_box(&compressed), Some(data.len() as i32))
                        .unwrap()
                })
            });
        }
        group.finish();
    }

    // Size comparison benchmark
    {
        let mut group = c.benchmark_group("compression_size");
        
        for size in [4096, 16384, 65536, 262144] {
            let data: Vec<u8> = (0..size).map(|i| ((i * 7 + 13) % 256) as u8).collect();
            
            group.throughput(Throughput::Bytes(size as u64));
            
            group.bench_with_input(BenchmarkId::new("zstd_L3", size), &data, |b, data| {
                b.iter(|| zstd::encode_all(black_box(&data[..]), 3).unwrap())
            });
        }
        group.finish();
    }
}

// ============================================================================
// B-tree Benchmarks
// ============================================================================

/// Benchmark B-tree key operations
fn btree_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("btree");

    // BtrfsKey comparison (packed struct simulation)
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    #[repr(C, packed)]
    struct BtrfsKey {
        objectid: u64,
        item_type: u8,
        offset: u64,
    }

    // Create test keys
    let keys: Vec<BtrfsKey> = (0..10000)
        .map(|i| BtrfsKey {
            objectid: i * 256,
            item_type: (i % 256) as u8,
            offset: i * 4096,
        })
        .collect();

    group.bench_function("key_compare", |b| {
        let key1 = BtrfsKey { objectid: 1000, item_type: 0x54, offset: 5000 };
        let key2 = BtrfsKey { objectid: 1000, item_type: 0x54, offset: 5001 };

        b.iter(|| black_box(key1.cmp(&key2)))
    });

    group.bench_function("key_equality", |b| {
        let key1 = BtrfsKey { objectid: 1000, item_type: 0x54, offset: 5000 };
        let key2 = BtrfsKey { objectid: 1000, item_type: 0x54, offset: 5000 };

        b.iter(|| black_box(key1 == key2))
    });

    // Binary search in sorted key array
    group.bench_function("binary_search_10k", |b| {
        let target = BtrfsKey { objectid: 5000 * 256, item_type: (5000 % 256) as u8, offset: 5000 * 4096 };
        b.iter(|| keys.binary_search(&black_box(target)))
    });

    // Key parsing from bytes
    group.bench_function("key_parse", |b| {
        use byteorder::{ByteOrder, LittleEndian};
        let mut data = vec![0u8; 17];
        data[0..8].copy_from_slice(&256u64.to_le_bytes());
        data[8] = 0x01;
        data[9..17].copy_from_slice(&4096u64.to_le_bytes());

        b.iter(|| {
            let objectid = LittleEndian::read_u64(&data[0..8]);
            let item_type = data[8];
            let offset = LittleEndian::read_u64(&data[9..17]);
            black_box((objectid, item_type, offset))
        })
    });

    // Simulate tree traversal decisions
    group.bench_function("tree_bisect_100", |b| {
        let ptrs: Vec<(BtrfsKey, u64)> = (0..100)
            .map(|i| (BtrfsKey { objectid: i * 100, item_type: 0x54, offset: 0 }, i * 0x10000))
            .collect();
        let target = BtrfsKey { objectid: 5000, item_type: 0x54, offset: 0 };

        b.iter(|| {
            let mut result = ptrs[0].1;
            for (key, ptr) in &ptrs {
                if *key > target {
                    break;
                }
                result = *ptr;
            }
            black_box(result)
        })
    });

    group.finish();
}

// ============================================================================
// Inode Benchmarks
// ============================================================================

/// Benchmark inode parsing operations
fn inode_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("inode");

    // Create mock inode data (160 bytes minimum)
    let mut inode_data = vec![0u8; 168];
    // Set some fields
    inode_data[0..8].copy_from_slice(&100u64.to_le_bytes()); // generation
    inode_data[16..24].copy_from_slice(&4096u64.to_le_bytes()); // size
    inode_data[40..44].copy_from_slice(&1u32.to_le_bytes()); // nlink
    inode_data[52..56].copy_from_slice(&0o100644u32.to_le_bytes()); // mode

    group.bench_function("parse_inode", |b| {
        use byteorder::{ByteOrder, LittleEndian};
        b.iter(|| {
            let generation = LittleEndian::read_u64(&inode_data[0..8]);
            let size = LittleEndian::read_u64(&inode_data[16..24]);
            let nlink = LittleEndian::read_u32(&inode_data[40..44]);
            let mode = LittleEndian::read_u32(&inode_data[52..56]);
            black_box((generation, size, nlink, mode))
        })
    });

    group.bench_function("inode_type_from_mode", |b| {
        let mode = 0o100644u32;
        b.iter(|| {
            let file_type = black_box(mode) & 0o170000;
            match file_type {
                0o100000 => 1u8, // File
                0o040000 => 2u8, // Directory
                0o120000 => 7u8, // Symlink
                _ => 0u8,        // Unknown
            }
        })
    });

    // Directory entry parsing
    let mut dir_entry_data = vec![0u8; 50];
    dir_entry_data[0..8].copy_from_slice(&257u64.to_le_bytes()); // ino
    dir_entry_data[27..29].copy_from_slice(&8u16.to_le_bytes()); // name_len
    dir_entry_data[29] = 1; // type (file)
    dir_entry_data[30..38].copy_from_slice(b"test.txt");

    group.bench_function("parse_dir_entry", |b| {
        use byteorder::{ByteOrder, LittleEndian};
        b.iter(|| {
            let ino = LittleEndian::read_u64(&dir_entry_data[0..8]);
            let name_len = LittleEndian::read_u16(&dir_entry_data[27..29]) as usize;
            let entry_type = dir_entry_data[29];
            let name = std::str::from_utf8(&dir_entry_data[30..30 + name_len]).unwrap_or("");
            black_box((ino, entry_type, name))
        })
    });

    group.finish();
}

// ============================================================================
// Extent Benchmarks
// ============================================================================

/// Benchmark extent data parsing and handling
fn extent_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("extent");

    // Regular extent data (53 bytes)
    let mut extent_data = vec![0u8; 53];
    extent_data[0..8].copy_from_slice(&100u64.to_le_bytes()); // generation
    extent_data[8..16].copy_from_slice(&4096u64.to_le_bytes()); // ram_bytes
    extent_data[16] = 0; // compression
    extent_data[20] = 1; // extent_type (regular)
    extent_data[21..29].copy_from_slice(&0x100000u64.to_le_bytes()); // disk_bytenr
    extent_data[29..37].copy_from_slice(&4096u64.to_le_bytes()); // disk_num_bytes

    group.bench_function("parse_extent", |b| {
        use byteorder::{ByteOrder, LittleEndian};
        b.iter(|| {
            let generation = LittleEndian::read_u64(&extent_data[0..8]);
            let ram_bytes = LittleEndian::read_u64(&extent_data[8..16]);
            let compression = extent_data[16];
            let extent_type = extent_data[20];
            let disk_bytenr = if extent_type != 0 {
                LittleEndian::read_u64(&extent_data[21..29])
            } else {
                0
            };
            black_box((generation, ram_bytes, compression, disk_bytenr))
        })
    });

    // Inline extent with data
    let mut inline_extent = vec![0u8; 21 + 100];
    inline_extent[20] = 0; // inline type
    inline_extent[8..16].copy_from_slice(&100u64.to_le_bytes()); // ram_bytes

    group.bench_function("parse_inline_extent", |b| {
        use byteorder::{ByteOrder, LittleEndian};
        b.iter(|| {
            let extent_type = inline_extent[20];
            let ram_bytes = LittleEndian::read_u64(&inline_extent[8..16]) as usize;
            let data = if extent_type == 0 {
                &inline_extent[21..21 + ram_bytes]
            } else {
                &[]
            };
            black_box(data)
        })
    });

    group.finish();
}

// ============================================================================
// Chunk Tree Benchmarks
// ============================================================================

/// Benchmark chunk mapping operations
fn chunk_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("chunk");

    // Create mock chunk mappings
    use std::collections::BTreeMap;
    
    struct ChunkMapping {
        logical: u64,
        size: u64,
        stripe_offset: u64,
    }

    let mut chunks: BTreeMap<u64, ChunkMapping> = BTreeMap::new();
    for i in 0..100 {
        let logical = i * 0x10000000; // 256MB chunks
        chunks.insert(logical, ChunkMapping {
            logical,
            size: 0x10000000,
            stripe_offset: i * 0x10000000 + 0x100000,
        });
    }

    group.bench_function("logical_to_physical", |b| {
        let test_logical = 50 * 0x10000000 + 0x1000; // Middle of chunk 50
        
        b.iter(|| {
            let chunk = chunks.range(..=black_box(test_logical))
                .next_back()
                .map(|(_, v)| v)
                .unwrap();
            
            let offset_in_chunk = test_logical - chunk.logical;
            black_box(chunk.stripe_offset + offset_in_chunk)
        })
    });

    group.bench_function("chunk_lookup", |b| {
        b.iter(|| {
            let logical = black_box(75 * 0x10000000);
            chunks.get(&logical)
        })
    });

    group.bench_function("chunk_range_search", |b| {
        b.iter(|| {
            let logical = black_box(25 * 0x10000000 + 0x5000);
            chunks.range(..=logical).next_back()
        })
    });

    group.finish();
}

// ============================================================================
// Memory Operations Benchmarks
// ============================================================================

/// Benchmark memory operations relevant to BTRFS I/O
fn memory_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory");

    // Block copy operations
    for size in [512, 4096, 16384, 65536, 131072].iter() {
        let src = vec![0x42u8; *size];

        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("copy", size), &src, |b, src| {
            b.iter(|| {
                let mut dst = vec![0u8; src.len()];
                dst.copy_from_slice(black_box(src));
                dst
            })
        });

        group.bench_with_input(BenchmarkId::new("copy_preallocated", size), &src, |b, src| {
            let mut dst = vec![0u8; src.len()];
            b.iter(|| {
                dst.copy_from_slice(black_box(src));
                black_box(dst.len())
            })
        });
    }

    // Vec allocation
    for size in [4096, 16384, 65536].iter() {
        group.bench_with_input(BenchmarkId::new("vec_alloc", size), size, |b, &size| {
            b.iter(|| {
                let v: Vec<u8> = vec![0; black_box(size)];
                black_box(v)
            })
        });

        group.bench_with_input(BenchmarkId::new("vec_with_capacity", size), size, |b, &size| {
            b.iter(|| {
                let mut v: Vec<u8> = Vec::with_capacity(black_box(size));
                v.resize(size, 0);
                black_box(v)
            })
        });
    }

    group.finish();
}

// ============================================================================
// Name Hashing Benchmarks
// ============================================================================

/// Benchmark BTRFS name hashing (used for directory lookups)
fn name_hash_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("name_hash");

    let short_name = "a.txt";
    let medium_name = "my_important_document.pdf";
    let long_name = "this_is_a_very_long_filename_that_someone_might_actually_use_in_practice.tar.gz";

    group.bench_function("short_name", |b| {
        b.iter(|| crc32c::crc32c(black_box(short_name.as_bytes())) as u64)
    });

    group.bench_function("medium_name", |b| {
        b.iter(|| crc32c::crc32c(black_box(medium_name.as_bytes())) as u64)
    });

    group.bench_function("long_name", |b| {
        b.iter(|| crc32c::crc32c(black_box(long_name.as_bytes())) as u64)
    });

    // Unicode name
    let unicode_name = "файл_документ.txt";
    group.bench_function("unicode_name", |b| {
        b.iter(|| crc32c::crc32c(black_box(unicode_name.as_bytes())) as u64)
    });

    group.finish();
}

// ============================================================================
// Path Parsing Benchmarks
// ============================================================================

/// Benchmark path parsing operations
fn path_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("path");

    let short_path = "/home/user";
    let medium_path = "/home/user/documents/projects/rust";
    let long_path = "/home/user/documents/projects/rust/btrfs-mount-windows/src/core/tree.rs";

    group.bench_function("parse_short", |b| {
        b.iter(|| {
            black_box(short_path)
                .split(['/', '\\'])
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
    });

    group.bench_function("parse_medium", |b| {
        b.iter(|| {
            black_box(medium_path)
                .split(['/', '\\'])
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
    });

    group.bench_function("parse_long", |b| {
        b.iter(|| {
            black_box(long_path)
                .split(['/', '\\'])
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
    });

    // Windows path
    let windows_path = r"C:\Users\user\Documents\Projects\Rust";
    group.bench_function("parse_windows", |b| {
        b.iter(|| {
            black_box(windows_path)
                .split(['/', '\\'])
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
    });

    group.finish();
}

// ============================================================================
// UUID Benchmarks
// ============================================================================

/// Benchmark UUID operations
fn uuid_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("uuid");

    group.bench_function("parse_from_bytes", |b| {
        let bytes = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        b.iter(|| uuid::Uuid::from_bytes(black_box(bytes)))
    });

    group.bench_function("generate_v4", |b| {
        b.iter(|| uuid::Uuid::new_v4())
    });

    group.bench_function("to_string", |b| {
        let uuid = uuid::Uuid::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        b.iter(|| black_box(&uuid).to_string())
    });

    group.bench_function("compare", |b| {
        let uuid1 = uuid::Uuid::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let uuid2 = uuid::Uuid::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 17]);
        b.iter(|| black_box(&uuid1) == black_box(&uuid2))
    });

    group.finish();
}

// ============================================================================
// Byteorder Parsing Benchmarks
// ============================================================================

/// Benchmark byte order parsing operations
fn byteorder_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("byteorder");

    let data = vec![0x42u8; 1024];

    group.bench_function("read_u64_le", |b| {
        use byteorder::{ByteOrder, LittleEndian};
        b.iter(|| LittleEndian::read_u64(black_box(&data[0..8])))
    });

    group.bench_function("read_u32_le", |b| {
        use byteorder::{ByteOrder, LittleEndian};
        b.iter(|| LittleEndian::read_u32(black_box(&data[0..4])))
    });

    group.bench_function("read_u16_le", |b| {
        use byteorder::{ByteOrder, LittleEndian};
        b.iter(|| LittleEndian::read_u16(black_box(&data[0..2])))
    });

    // Compare with native parsing
    group.bench_function("read_u64_native", |b| {
        b.iter(|| {
            let bytes: [u8; 8] = black_box(&data[0..8]).try_into().unwrap();
            u64::from_le_bytes(bytes)
        })
    });

    // Batch parsing (simulating header parsing)
    group.bench_function("read_header_batch", |b| {
        use byteorder::{ByteOrder, LittleEndian};
        b.iter(|| {
            let a = LittleEndian::read_u64(&data[0..8]);
            let b = LittleEndian::read_u64(&data[8..16]);
            let c = LittleEndian::read_u64(&data[16..24]);
            let d = LittleEndian::read_u32(&data[24..28]);
            let e = LittleEndian::read_u16(&data[28..30]);
            black_box((a, b, c, d, e))
        })
    });

    group.finish();
}

// ============================================================================
// Concurrent Access Benchmarks
// ============================================================================

/// Benchmark concurrent access patterns
fn concurrent_benchmarks(c: &mut Criterion) {
    use parking_lot::RwLock;
    use std::sync::Arc;

    let mut group = c.benchmark_group("concurrent");

    // RwLock read performance
    let data = Arc::new(RwLock::new(vec![0u8; 4096]));

    group.bench_function("rwlock_read", |b| {
        let data = data.clone();
        b.iter(|| {
            let guard = data.read();
            let len = guard.len();
            drop(guard);
            black_box(len)
        })
    });

    group.bench_function("rwlock_write", |b| {
        let data = data.clone();
        b.iter(|| {
            let mut guard = data.write();
            guard[0] = 0x42;
            let len = guard.len();
            drop(guard);
            black_box(len)
        })
    });

    // Arc clone
    group.bench_function("arc_clone", |b| {
        let data = data.clone();
        b.iter(|| {
            let cloned = data.clone();
            black_box(cloned)
        })
    });

    group.finish();
}

// ============================================================================
// I/O Simulation Benchmarks
// ============================================================================

/// Benchmark simulated I/O patterns
fn io_simulation_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("io_simulation");

    // Simulate reading a 16KB node
    let node_data = vec![0x42u8; 16384];

    group.throughput(Throughput::Bytes(16384));

    group.bench_function("parse_node_header", |b| {
        use byteorder::{ByteOrder, LittleEndian};
        b.iter(|| {
            // Parse node header (first 101 bytes)
            let _csum = &node_data[0..32];
            let _fsid = &node_data[32..48];
            let bytenr = LittleEndian::read_u64(&node_data[48..56]);
            let generation = LittleEndian::read_u64(&node_data[64..72]);
            let owner = LittleEndian::read_u64(&node_data[72..80]);
            let nritems = LittleEndian::read_u32(&node_data[80..84]);
            let level = node_data[84];
            black_box((bytenr, generation, owner, nritems, level))
        })
    });

    // Simulate parsing multiple items from a leaf node
    group.bench_function("parse_leaf_items", |b| {
        use byteorder::{ByteOrder, LittleEndian};
        let nritems = 50;
        let item_size = 25; // key (17) + offset (4) + size (4)
        let header_size = 101;

        b.iter(|| {
            let mut items = Vec::with_capacity(nritems);
            for i in 0..nritems {
                let offset = header_size + i * item_size;
                if offset + item_size > node_data.len() {
                    break;
                }
                let objectid = LittleEndian::read_u64(&node_data[offset..offset + 8]);
                let item_type = node_data[offset + 8];
                let key_offset = LittleEndian::read_u64(&node_data[offset + 9..offset + 17]);
                let data_offset = LittleEndian::read_u32(&node_data[offset + 17..offset + 21]);
                let data_size = LittleEndian::read_u32(&node_data[offset + 21..offset + 25]);
                items.push((objectid, item_type, key_offset, data_offset, data_size));
            }
            black_box(items)
        })
    });

    group.finish();
}

// ============================================================================
// Main Criterion Groups
// ============================================================================

criterion_group!(
    name = core_benches;
    config = Criterion::default();
    targets = 
        superblock_benchmarks,
        checksum_benchmarks,
        btree_benchmarks,
        inode_benchmarks,
        extent_benchmarks,
        chunk_benchmarks,
);

criterion_group!(
    name = compression_benches;
    config = Criterion::default().sample_size(50);
    targets = compression_benchmarks,
);

criterion_group!(
    name = utility_benches;
    config = Criterion::default();
    targets = 
        memory_benchmarks,
        name_hash_benchmarks,
        path_benchmarks,
        uuid_benchmarks,
        byteorder_benchmarks,
);

criterion_group!(
    name = advanced_benches;
    config = Criterion::default();
    targets = 
        concurrent_benchmarks,
        io_simulation_benchmarks,
);

criterion_main!(core_benches, compression_benches, utility_benches, advanced_benches);
