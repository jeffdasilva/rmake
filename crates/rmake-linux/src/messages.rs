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
fn lookup_key(key: &CStr) -> Result<Vec<u8>, FormatError> {
    // SAFETY: the msgid is a static C string and gettext returns immutable
    // library-owned text. Copy immediately; no pointer enters core or survives
    // later domain/locale operations. gettext leaves errno unchanged.
    let value = unsafe { gettext(key.as_ptr()) };
    if value.is_null() {
        return Err(FormatError::CatalogLookup);
    }
    Ok(unsafe { CStr::from_ptr(value) }.to_bytes().to_vec())
}

pub fn render_ref(message: MessageRef<'_>) -> Result<Vec<u8>, FormatError> {
    message.render(&lookup_key(message.id().msgid())?)
}

/// Preserve the live catalog and perror formatting for the C stdout owner.
pub(crate) fn render_stdout_error(detail: Option<&[u8]>) -> Vec<u8> {
    let key = c"write error: stdout";
    let label = lookup_key(key).unwrap_or_else(|_| key.to_bytes().to_vec());
    let Some(detail) = detail else { return label };
    let key = c"%s%s: %s";
    let args = [
        Argument::Bytes(&label),
        Argument::Bytes(b""),
        Argument::Bytes(detail),
    ];
    lookup_key(key)
        .and_then(|template| checked_format(&template, &args))
        .unwrap_or_else(|_| [&label, b": ".as_slice(), detail].concat())
}
