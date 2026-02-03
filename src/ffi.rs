//! FFI (Foreign Function Interface) for the BTRFS library
//!
//! This module provides C-compatible functions for dynamic library loading.
//! The library can be loaded at runtime and updated independently of the GUI.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

use crate::core::{BtrfsFilesystem, Subvolume};
use crate::blockdev;

/// Library version
pub const LIB_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Error codes
pub const BTRFS_OK: c_int = 0;
pub const BTRFS_ERR_INVALID_ARG: c_int = -1;
pub const BTRFS_ERR_NOT_FOUND: c_int = -2;
pub const BTRFS_ERR_IO: c_int = -3;
pub const BTRFS_ERR_CORRUPT: c_int = -4;
pub const BTRFS_ERR_UNSUPPORTED: c_int = -5;
pub const BTRFS_ERR_PERMISSION: c_int = -6;
pub const BTRFS_ERR_UNKNOWN: c_int = -99;

/// Opaque handle to a BTRFS filesystem
pub struct BtrfsHandle {
    fs: BtrfsFilesystem,
}

/// Get the library version
/// 
/// # Safety
/// Returns a pointer to a static string. Do not free.
#[unsafe(no_mangle)]
pub extern "C" fn btrfs_lib_version() -> *const c_char {
    static VERSION: std::sync::OnceLock<CString> = std::sync::OnceLock::new();
    VERSION
        .get_or_init(|| CString::new(LIB_VERSION).unwrap())
        .as_ptr()
}

/// Get the library version as components
#[unsafe(no_mangle)]
pub extern "C" fn btrfs_lib_version_parts(major: *mut c_int, minor: *mut c_int, patch: *mut c_int) {
    let parts: Vec<&str> = LIB_VERSION.split('.').collect();
    unsafe {
        if !major.is_null() {
            *major = parts.get(0).and_then(|s| s.parse().ok()).unwrap_or(0);
        }
        if !minor.is_null() {
            *minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        }
        if !patch.is_null() {
            *patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        }
    }
}

/// Open a BTRFS filesystem from a path (device or image file)
/// 
/// # Safety
/// - `path` must be a valid null-terminated UTF-8 string
/// - `handle_out` must be a valid pointer
/// - The returned handle must be freed with `btrfs_close`
#[unsafe(no_mangle)]
pub unsafe extern "C" fn btrfs_open(
    path: *const c_char,
    read_only: c_int,
    handle_out: *mut *mut BtrfsHandle,
) -> c_int {
    if path.is_null() || handle_out.is_null() {
        return BTRFS_ERR_INVALID_ARG;
    }

    let path_str = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => return BTRFS_ERR_INVALID_ARG,
    };

    let device = match blockdev::open(path_str, read_only != 0) {
        Ok(d) => d,
        Err(_) => return BTRFS_ERR_NOT_FOUND,
    };

    let fs = match BtrfsFilesystem::open(std::sync::Arc::from(device), read_only != 0) {
        Ok(fs) => fs,
        Err(e) => {
            return match e {
                crate::core::BtrfsError::Io(_) => BTRFS_ERR_IO,
                crate::core::BtrfsError::Corrupt(_) => BTRFS_ERR_CORRUPT,
                crate::core::BtrfsError::InvalidMagic => BTRFS_ERR_CORRUPT,
                crate::core::BtrfsError::UnsupportedFeature(_) => BTRFS_ERR_UNSUPPORTED,
                _ => BTRFS_ERR_UNKNOWN,
            };
        }
    };

    let handle = Box::new(BtrfsHandle { fs });
    *handle_out = Box::into_raw(handle);

    BTRFS_OK
}

/// Close a BTRFS filesystem handle
/// 
/// # Safety
/// - `handle` must be a valid handle returned by `btrfs_open`
/// - The handle must not be used after this call
#[unsafe(no_mangle)]
pub unsafe extern "C" fn btrfs_close(handle: *mut BtrfsHandle) -> c_int {
    if handle.is_null() {
        return BTRFS_ERR_INVALID_ARG;
    }

    drop(Box::from_raw(handle));
    BTRFS_OK
}

/// Get filesystem UUID as a string
/// 
/// # Safety
/// - `handle` must be a valid handle
/// - `uuid_out` must point to a buffer of at least 37 bytes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn btrfs_get_uuid(
    handle: *const BtrfsHandle,
    uuid_out: *mut c_char,
    uuid_len: usize,
) -> c_int {
    if handle.is_null() || uuid_out.is_null() || uuid_len < 37 {
        return BTRFS_ERR_INVALID_ARG;
    }

    let handle = &*handle;
    let uuid = handle.fs.uuid().to_string();
    
    let bytes = uuid.as_bytes();
    let copy_len = std::cmp::min(bytes.len(), uuid_len - 1);
    
    ptr::copy_nonoverlapping(bytes.as_ptr(), uuid_out as *mut u8, copy_len);
    *uuid_out.add(copy_len) = 0;

    BTRFS_OK
}

/// Get filesystem label
/// 
/// # Safety
/// - `handle` must be a valid handle
/// - `label_out` must point to a buffer of at least 256 bytes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn btrfs_get_label(
    handle: *const BtrfsHandle,
    label_out: *mut c_char,
    label_len: usize,
) -> c_int {
    if handle.is_null() || label_out.is_null() || label_len < 1 {
        return BTRFS_ERR_INVALID_ARG;
    }

    let handle = &*handle;
    let label = handle.fs.label();
    
    let bytes = label.as_bytes();
    let copy_len = std::cmp::min(bytes.len(), label_len - 1);
    
    ptr::copy_nonoverlapping(bytes.as_ptr(), label_out as *mut u8, copy_len);
    *label_out.add(copy_len) = 0;

    BTRFS_OK
}

/// Get total filesystem size in bytes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn btrfs_get_total_bytes(handle: *const BtrfsHandle) -> u64 {
    if handle.is_null() {
        return 0;
    }
    (*handle).fs.total_bytes()
}

/// Get used bytes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn btrfs_get_used_bytes(handle: *const BtrfsHandle) -> u64 {
    if handle.is_null() {
        return 0;
    }
    (*handle).fs.bytes_used()
}

/// Get number of subvolumes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn btrfs_subvolume_count(handle: *const BtrfsHandle) -> c_int {
    if handle.is_null() {
        return BTRFS_ERR_INVALID_ARG;
    }

    match (*handle).fs.list_subvolumes() {
        Ok(subvols) => subvols.len() as c_int,
        Err(_) => 0,
    }
}

/// Subvolume info structure for FFI
#[repr(C)]
pub struct BtrfsSubvolumeInfo {
    pub id: u64,
    pub parent_id: u64,
    pub generation: u64,
    pub flags: u64,
    pub name: [c_char; 256],
    pub path: [c_char; 4096],
}

/// Get subvolume info by index
/// 
/// # Safety
/// - `handle` must be a valid handle
/// - `info_out` must be a valid pointer
#[unsafe(no_mangle)]
pub unsafe extern "C" fn btrfs_get_subvolume(
    handle: *const BtrfsHandle,
    index: usize,
    info_out: *mut BtrfsSubvolumeInfo,
) -> c_int {
    if handle.is_null() || info_out.is_null() {
        return BTRFS_ERR_INVALID_ARG;
    }

    let subvols = match (*handle).fs.list_subvolumes() {
        Ok(s) => s,
        Err(_) => return BTRFS_ERR_IO,
    };

    if index >= subvols.len() {
        return BTRFS_ERR_NOT_FOUND;
    }

    let subvol = &subvols[index];
    let info = &mut *info_out;
    
    info.id = subvol.id;
    info.parent_id = subvol.parent_id;
    info.generation = subvol.generation;
    info.flags = subvol.flags;
    
    // Copy name
    let name_bytes = subvol.name.as_bytes();
    let name_len = std::cmp::min(name_bytes.len(), 255);
    ptr::copy_nonoverlapping(
        name_bytes.as_ptr(),
        info.name.as_mut_ptr() as *mut u8,
        name_len,
    );
    info.name[name_len] = 0;
    
    // Copy path
    let path_bytes = subvol.path.as_bytes();
    let path_len = std::cmp::min(path_bytes.len(), 4095);
    ptr::copy_nonoverlapping(
        path_bytes.as_ptr(),
        info.path.as_mut_ptr() as *mut u8,
        path_len,
    );
    info.path[path_len] = 0;

    BTRFS_OK
}

/// Get the last error message
/// 
/// # Safety
/// Thread-local storage, returns pointer to static buffer
#[unsafe(no_mangle)]
pub extern "C" fn btrfs_last_error() -> *const c_char {
    thread_local! {
        static LAST_ERROR: std::cell::RefCell<CString> = std::cell::RefCell::new(CString::new("").unwrap());
    }
    
    LAST_ERROR.with(|e| e.borrow().as_ptr())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let version = unsafe { CStr::from_ptr(btrfs_lib_version()) };
        assert!(!version.to_str().unwrap().is_empty());
    }

    #[test]
    fn test_version_parts() {
        let mut major = 0;
        let mut minor = 0;
        let mut patch = 0;
        btrfs_lib_version_parts(&mut major, &mut minor, &mut patch);
        assert!(major >= 0);
    }

    #[test]
    fn test_null_handle() {
        unsafe {
            assert_eq!(btrfs_close(ptr::null_mut()), BTRFS_ERR_INVALID_ARG);
            assert_eq!(btrfs_get_total_bytes(ptr::null()), 0);
        }
    }
}
