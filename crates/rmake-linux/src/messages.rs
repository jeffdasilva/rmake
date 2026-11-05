// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva
//! Public gettext adapter. The initial domain owns its assets; explicit native
//! textdomain/bindtextdomain changes retain their public libc behavior.
use rmake_core::messages::{Argument, FormatError, MessageRef, checked_format};
use std::ffi::{CStr, CString, c_char};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
unsafe extern "C" {
    fn gettext(message: *const c_char) -> *mut c_char;
    fn dgettext(domain: *const c_char, message: *const c_char) -> *mut c_char;
    fn bindtextdomain(domain: *const c_char, directory: *const c_char) -> *mut c_char;
    fn textdomain(domain: *const c_char) -> *mut c_char;
}
pub fn initialize(directory: &Path) -> Result<(), &'static str> {
    let directory =
        CString::new(directory.as_os_str().as_bytes()).map_err(|_| "invalid catalog directory")?;
    // SAFETY: public gettext receives live NUL-terminated strings. It retains
    // its own binding copies; no private locale/catalog state is inspected.
    if unsafe { bindtextdomain(c"make".as_ptr(), directory.as_ptr()) }.is_null()
        || unsafe { textdomain(c"make".as_ptr()) }.is_null()
    {
        return Err("unable to initialize message catalogs");
    }
    Ok(())
}
pub fn lookup_static(key: &'static CStr) -> Result<Vec<u8>, FormatError> {
    lookup(key, None)
}

/// getopt uses libc's domain independently of the application domain.
pub fn lookup_libc(key: &'static CStr) -> Result<Vec<u8>, FormatError> {
    lookup(key, Some(c"libc"))
}

fn lookup(key: &'static CStr, domain: Option<&'static CStr>) -> Result<Vec<u8>, FormatError> {
    // SAFETY: the key and domain are static C strings. Copy the immutable
    // library-owned result before any later domain or locale operation.
    let value = unsafe {
        match domain {
            Some(domain) => dgettext(domain.as_ptr(), key.as_ptr()),
            None => gettext(key.as_ptr()),
        }
    };
    if value.is_null() {
        return Err(FormatError::CatalogLookup);
    }
    Ok(unsafe { CStr::from_ptr(value) }.to_bytes().to_vec())
}

pub fn render_ref(message: MessageRef<'_>) -> Result<Vec<u8>, FormatError> {
    message.render(&lookup_static(message.id().msgid())?)
}

/// Render a static protocol key with checked byte and numeric arguments.
pub fn render_format(key: &'static CStr, args: &[Argument<'_>]) -> Result<Vec<u8>, FormatError> {
    checked_format(&lookup_static(key)?, args)
}
