//! BTRFS Mount Windows CLI
//!
//! Command-line interface for mounting BTRFS volumes on Windows.

use btrf_mount_windows::{blockdev, BtrfsFilesystem, BtrfsMount, MountOptions};
use std::sync::Arc;

fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        eprintln!("BTRFS Mount Windows v{}", btrf_mount_windows::VERSION);
        eprintln!();
        eprintln!("Usage: {} <source> <drive_letter>", args[0]);
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  source       Path to BTRFS image file or physical drive");
        eprintln!("               (e.g., ./disk.img or \\\\.\\PhysicalDrive1)");
        eprintln!("  drive_letter Drive letter to mount (e.g., Z:)");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  {} ./btrfs.img Z:", args[0]);
        eprintln!("  {} \\\\.\\PhysicalDrive1 Y:", args[0]);
        std::process::exit(1);
    }

    let source = &args[1];
    let drive_letter = &args[2];

    tracing::info!("Mounting {} to {}", source, drive_letter);

    // Open block device
    let device = match blockdev::open(source, false) {
        Ok(d) => Arc::from(d),
        Err(e) => {
            eprintln!("Failed to open device: {}", e);
            std::process::exit(1);
        }
    };

    // Open BTRFS filesystem
    let fs = match BtrfsFilesystem::open(device, false) {
        Ok(fs) => Arc::new(fs),
        Err(e) => {
            eprintln!("Failed to open BTRFS filesystem: {}", e);
            std::process::exit(1);
        }
    };

    tracing::info!("Filesystem label: {}", fs.label());
    tracing::info!("Filesystem UUID: {}", fs.uuid());
    tracing::info!(
        "Total size: {} bytes ({:.2} GB)",
        fs.total_bytes(),
        fs.total_bytes() as f64 / 1_073_741_824.0
    );

    // Mount filesystem
    let options = MountOptions {
        drive_letter: drive_letter.chars().next().unwrap_or('Z'),
        read_only: false,
        debug: std::env::var("BTRFS_DEBUG").is_ok(),
        ..Default::default()
    };

    match BtrfsMount::mount(fs, options) {
        Ok(_) => {
            tracing::info!("Filesystem mounted successfully");
        }
        Err(e) => {
            eprintln!("Failed to mount filesystem: {}", e);
            std::process::exit(1);
        }
    }
}
