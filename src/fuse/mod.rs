//! Dokan FUSE handler for Windows
//!
//! This module provides the Windows filesystem integration using Dokan.

#[cfg(windows)]
pub mod handler;
pub mod mount;
pub mod operations;

#[cfg(windows)]
pub use handler::BtrfsHandler;
pub use mount::{BtrfsMount, MountOptions};
